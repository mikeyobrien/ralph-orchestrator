#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

struct Fixture {
    root: TempDir,
    release_dir: PathBuf,
    engine_dir: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("fixture temp dir");
        let release_dir = root.path().join("release");
        let engine_dir = root.path().join("engine");
        let marker = root.path().join("version-checked");
        fs::create_dir(&release_dir).expect("create fixture release directory");

        Self {
            root,
            release_dir,
            engine_dir,
            marker,
        }
    }

    fn write_valid_release(&self) {
        let artifact = artifact_name();
        let body = format!(
            "#!/bin/sh\n[ \"$1\" = \"--version\" ] || exit 64\nprintf 'autoloop 0.10.1\\n'\n: > {}\n",
            shell_quote(&self.marker)
        );
        fs::write(self.release_dir.join(artifact), body.as_bytes())
            .expect("write fixture artifact");
        let digest = format!("{:x}", Sha256::digest(body.as_bytes()));
        fs::write(
            self.release_dir.join("SHA256SUMS"),
            format!("{digest}  {artifact}\n"),
        )
        .expect("write fixture checksums");
    }

    fn corrupt_artifact(&self) {
        fs::write(
            self.release_dir.join(artifact_name()),
            b"corrupted artifact",
        )
        .expect("corrupt fixture artifact");
    }

    fn install(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args(["--color", "never", "doctor", "--install-engine"])
            .env("RALPH_ENGINE_DIR", &self.engine_dir)
            .env(
                "RALPH_ENGINE_RELEASE_BASE_URL",
                format!("file://{}", self.release_dir.display()),
            )
            .output()
            .expect("run engine installer")
    }

    fn installed_engine(&self) -> PathBuf {
        self.engine_dir.join("autoloop")
    }
}

fn artifact_name() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "autoloop-darwin-arm64",
        ("macos", "x86_64") => "autoloop-darwin-x64",
        ("linux", "x86_64") => "autoloop-linux-x64",
        ("linux", "aarch64") => "autoloop-linux-arm64",
        platform => panic!("installer integration test unsupported on {platform:?}"),
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

fn rendered(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn install_engine_downloads_verifies_installs_and_checks_version() {
    let fixture = Fixture::new();
    fixture.write_valid_release();

    let output = fixture.install();
    let text = rendered(&output);
    assert!(output.status.success(), "installer output: {text}");

    let installed = fixture.installed_engine();
    assert!(installed.is_file(), "installer output: {text}");
    assert_ne!(
        fs::metadata(&installed).unwrap().permissions().mode() & 0o111,
        0,
        "installed engine is not executable"
    );
    assert!(fixture.marker.is_file(), "--version was not run: {text}");
    assert!(text.contains("0.10.1"), "installer output: {text}");
    assert!(
        text.contains(&installed.display().to_string()),
        "installer output: {text}"
    );

    // Keep the temporary fixture alive until every assertion has completed.
    assert!(fixture.root.path().exists());
}

#[test]
fn install_engine_rejects_checksum_mismatch_without_installing() {
    let fixture = Fixture::new();
    fixture.write_valid_release();
    fixture.corrupt_artifact();

    let output = fixture.install();
    let text = rendered(&output);
    assert!(
        !output.status.success(),
        "installer unexpectedly passed: {text}"
    );
    assert!(
        text.to_lowercase().contains("checksum"),
        "installer output: {text}"
    );
    assert!(
        !fixture.installed_engine().exists(),
        "corrupted engine was installed: {text}"
    );
}

#[test]
fn install_engine_reports_missing_artifact_without_installing() {
    let fixture = Fixture::new();

    let output = fixture.install();
    let text = rendered(&output);
    assert!(
        !output.status.success(),
        "installer unexpectedly passed: {text}"
    );
    assert!(text.contains("curl failed"), "installer output: {text}");
    assert!(
        text.contains("missing") || text.contains("404"),
        "installer output is not actionable: {text}"
    );
    assert!(
        !fixture.installed_engine().exists(),
        "missing artifact produced an install: {text}"
    );
}
