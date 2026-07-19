//! Standalone autoloop engine installer used by `ralph doctor --install-engine`.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use ralph_core::autoloop_health::{
    AutoloopHealth, AutoloopSource, MIN_AUTOLOOP_VERSION, VENDORED_AUTOLOOP_VERSION,
    check_autoloop, engine_dir,
};
use regex::Regex;
use sha2::{Digest, Sha256};

const RELEASE_BASE_URL_ENV: &str = "RALPH_ENGINE_RELEASE_BASE_URL";

/// Download, verify, install, and confirm Ralph's standalone autoloop engine.
pub fn install() -> Result<()> {
    let artifact = artifact_name()?;
    let engine_dir = engine_dir().ok_or_else(|| {
        anyhow!(
            "cannot determine the engine directory: set RALPH_ENGINE_DIR or HOME, or install with `npm install -g @mobrienv/autoloop`"
        )
    })?;
    fs::create_dir_all(&engine_dir)
        .with_context(|| format!("failed to create engine directory {}", engine_dir.display()))?;

    let temp_dir = tempfile::Builder::new()
        .prefix(".autoloop-install-")
        .tempdir_in(&engine_dir)
        .with_context(|| {
            format!(
                "failed to create a temporary download directory in {}",
                engine_dir.display()
            )
        })?;
    let artifact_path = temp_dir.path().join(artifact);
    let checksums_path = temp_dir.path().join("SHA256SUMS");
    let base_url = release_base_url();

    download(
        &format!("{base_url}/{artifact}"),
        &artifact_path,
        "engine artifact",
    )?;
    download(
        &format!("{base_url}/SHA256SUMS"),
        &checksums_path,
        "checksum manifest",
    )?;
    verify_checksum(&artifact_path, &checksums_path, artifact)?;
    make_executable(&artifact_path)?;

    let installed_path = engine_dir.join("autoloop");
    fs::rename(&artifact_path, &installed_path).with_context(|| {
        format!(
            "failed to atomically install engine at {}",
            installed_path.display()
        )
    })?;

    if let Err(error) = confirm_version(&installed_path) {
        let _ = fs::remove_file(&installed_path);
        return Err(error);
    }

    match check_autoloop() {
        AutoloopHealth::Ok {
            path,
            version,
            source: AutoloopSource::Vendored,
        } => {
            println!(
                "Installed autoloop {version} at {} (checksum verified)",
                path.display()
            );
            Ok(())
        }
        health => {
            let _ = fs::remove_file(&installed_path);
            bail!(
                "installed engine failed Ralph's normal resolution check: {health:?}; try `npm install -g @mobrienv/autoloop`"
            )
        }
    }
}

fn artifact_name() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("autoloop-darwin-arm64"),
        ("macos", "x86_64") => Ok("autoloop-darwin-x64"),
        ("linux", "x86_64") => Ok("autoloop-linux-x64"),
        ("linux", "aarch64") => Ok("autoloop-linux-arm64"),
        (os, arch) => bail!(
            "no standalone autoloop engine is available for platform {os}/{arch}; install the Node package instead with `npm install -g @mobrienv/autoloop`"
        ),
    }
}

fn release_base_url() -> String {
    std::env::var(RELEASE_BASE_URL_ENV).unwrap_or_else(|_| {
        format!(
            "https://github.com/mikeyobrien/autoloop/releases/download/v{VENDORED_AUTOLOOP_VERSION}"
        )
    })
    .trim_end_matches('/')
    .to_owned()
}

fn download(url: &str, destination: &Path, label: &str) -> Result<()> {
    let status = Command::new("curl")
        .args(["-fsSL", "--proto-default", "https", "-o"])
        .arg(destination)
        .arg(url)
        .status();

    match status {
        Err(error) if error.kind() == ErrorKind::NotFound => bail!(
            "curl is required to download the autoloop {label} but was not found; install curl and retry, or use `npm install -g @mobrienv/autoloop`"
        ),
        Err(error) => Err(error).with_context(|| format!("failed to launch curl for {url}")),
        Ok(status) if !status.success() => bail!(
            "curl failed to download the autoloop {label} from {url} (exit {status}; the release artifact may be missing or returned HTTP 404); check network access and retry, or use `npm install -g @mobrienv/autoloop`"
        ),
        Ok(_) => Ok(()),
    }
}

fn verify_checksum(artifact_path: &Path, checksums_path: &Path, artifact: &str) -> Result<()> {
    let manifest = fs::read_to_string(checksums_path).with_context(|| {
        format!(
            "failed to read checksum manifest {}",
            checksums_path.display()
        )
    })?;
    let expected = manifest
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let checksum = fields.next()?;
            let name = fields.next()?.trim_start_matches('*');
            (name == artifact).then_some(checksum)
        })
        .next()
        .ok_or_else(|| anyhow!("checksum manifest does not contain an entry for {artifact}"))?;

    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("checksum manifest contains an invalid SHA256 checksum for {artifact}");
    }

    let bytes = fs::read(artifact_path).with_context(|| {
        format!(
            "failed to read downloaded artifact {}",
            artifact_path.display()
        )
    })?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!(
            "checksum mismatch for {artifact}: expected {expected}, got {actual}; refusing to install the downloaded engine"
        );
    }

    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).with_context(|| {
        format!(
            "failed to make downloaded engine executable: {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn confirm_version(installed_path: &Path) -> Result<()> {
    let output = Command::new(installed_path)
        .arg("--version")
        .output()
        .with_context(|| {
            format!(
                "failed to run installed engine {} --version",
                installed_path.display()
            )
        })?;
    if !output.status.success() {
        bail!(
            "installed engine failed its --version check (exit {}); the incomplete install was removed",
            output.status
        );
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let version = extract_version(&text).ok_or_else(|| {
        anyhow!(
            "installed engine returned no semantic version from --version; the incomplete install was removed"
        )
    })?;
    if parse_version(&version) < parse_version(MIN_AUTOLOOP_VERSION) {
        bail!(
            "installed engine version {version} is too old; Ralph requires >= {MIN_AUTOLOOP_VERSION}; the incomplete install was removed"
        );
    }

    Ok(())
}

fn extract_version(output: &str) -> Option<String> {
    Regex::new(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?")
        .expect("version regex must compile")
        .find(output)
        .map(|matched| matched.as_str().to_owned())
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut components = version.split(['-', '+']).next()?.split('.');
    let parsed = (
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
    );
    components.next().is_none().then_some(parsed)
}
