//! Autoloop dependency discovery and version health checks.

use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use serde_json::Value;

use crate::utils::find_executable;

/// The oldest autoloop release that provides Ralph's required protocol.
pub const MIN_AUTOLOOP_VERSION: &str = "0.10.0";

/// The standalone autoloop release installed by Ralph.
pub const VENDORED_AUTOLOOP_VERSION: &str = "0.10.1";

/// Command users can run to install or update autoloop.
pub const AUTOLOOP_INSTALL_HINT: &str = "npm install -g @mobrienv/autoloop";

/// How Ralph located the autoloop executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoloopSource {
    /// Ralph's managed engine directory.
    Vendored,
    /// The user's `PATH`.
    PathLookup,
}

/// The result of locating autoloop and inspecting its version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoloopHealth {
    /// No `autoloop` executable was found in the engine directory or on `PATH`.
    Missing,
    /// The executable exists, but its version could not be determined.
    VersionUnknown {
        path: PathBuf,
        source: AutoloopSource,
    },
    /// The installed package is older than [`MIN_AUTOLOOP_VERSION`].
    TooOld {
        path: PathBuf,
        version: String,
        source: AutoloopSource,
    },
    /// The installed package meets the minimum version requirement.
    Ok {
        path: PathBuf,
        version: String,
        source: AutoloopSource,
    },
}

/// Return the directory containing Ralph's managed autoloop engine.
///
/// `RALPH_ENGINE_DIR` is an explicit override. Otherwise Ralph uses
/// `$HOME/.ralph/engine`; if neither variable is available there is no managed
/// engine directory.
pub fn engine_dir() -> Option<PathBuf> {
    std::env::var_os("RALPH_ENGINE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".ralph/engine"))
        })
}

/// Locate Ralph's managed autoloop first, then fall back to `PATH`.
pub fn check_autoloop() -> AutoloopHealth {
    if let Some(path) = engine_dir().map(|directory| directory.join("autoloop"))
        && is_executable(&path)
    {
        return check_autoloop_at(&path, AutoloopSource::Vendored);
    }

    let Some(path) = find_executable("autoloop") else {
        return AutoloopHealth::Missing;
    };

    check_autoloop_at(&path, AutoloopSource::PathLookup)
}

fn check_autoloop_at(bin_path: &Path, source: AutoloopSource) -> AutoloopHealth {
    let path = bin_path
        .canonicalize()
        .unwrap_or_else(|_| bin_path.to_path_buf());
    let Some(version) = probe_version(&path) else {
        return AutoloopHealth::VersionUnknown { path, source };
    };

    let Some(installed) = parse_version(&version) else {
        return AutoloopHealth::VersionUnknown { path, source };
    };
    let minimum = parse_version(MIN_AUTOLOOP_VERSION)
        .expect("MIN_AUTOLOOP_VERSION must be a major.minor.patch version");

    if installed < minimum {
        AutoloopHealth::TooOld {
            path,
            version,
            source,
        }
    } else {
        AutoloopHealth::Ok {
            path,
            version,
            source,
        }
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Read the version from the nearest enclosing autoloop npm package, then ask
/// the executable itself. The latter supports standalone release binaries.
///
/// Walking upward from the resolved binary supports npm's global symlink
/// layout as well as package-manager stores and direct package checkouts.
fn probe_version(bin_path: &Path) -> Option<String> {
    let start = if bin_path.is_dir() {
        bin_path
    } else {
        bin_path.parent()?
    };

    for directory in start.ancestors() {
        let package_json = directory.join("package.json");
        let Ok(contents) = std::fs::read_to_string(package_json) else {
            continue;
        };
        let Ok(package) = serde_json::from_str::<Value>(&contents) else {
            continue;
        };
        if package.get("name").and_then(Value::as_str) != Some("@mobrienv/autoloop") {
            continue;
        }

        return package
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }

    let output = Command::new(bin_path).arg("--version").output().ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    extract_version(&stdout)
}

fn extract_version(output: &str) -> Option<String> {
    let pattern =
        Regex::new(r"(?P<version>\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)")
            .expect("version regex must compile");

    pattern
        .captures(output)
        .and_then(|captures| captures.name("version"))
        .map(|version| version.as_str().to_owned())
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.split(['-', '+']).next()?;
    let mut components = core.split('.');
    let parsed = (
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
    );

    components.next().is_none().then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_package(directory: &Path, version: &str) {
        fs::create_dir_all(directory).unwrap();
        fs::write(
            directory.join("package.json"),
            format!(r#"{{"name":"@mobrienv/autoloop","version":"{version}"}}"#),
        )
        .unwrap();
    }

    fn write_binary(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn autoloop_health_resolves_version_through_bin_shim() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("lib/node_modules/@mobrienv/autoloop");
        let real_binary = package.join("bin/autoloop.js");
        write_package(&package, "0.11.0");
        write_binary(&real_binary, "");

        let shim = temp.path().join("bin/autoloop");
        fs::create_dir_all(shim.parent().unwrap()).unwrap();
        symlink(&real_binary, &shim).unwrap();

        assert_eq!(
            check_autoloop_at(&shim, AutoloopSource::PathLookup),
            AutoloopHealth::Ok {
                path: real_binary.canonicalize().unwrap(),
                version: "0.11.0".to_string(),
                source: AutoloopSource::PathLookup,
            }
        );
    }

    #[test]
    fn autoloop_health_reads_package_json_adjacent_to_binary() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("autoloop");
        write_package(temp.path(), "0.10.0");
        write_binary(&binary, "");

        assert!(matches!(
            check_autoloop_at(&binary, AutoloopSource::PathLookup),
            AutoloopHealth::Ok { version, .. } if version == "0.10.0"
        ));
    }

    #[test]
    fn autoloop_health_accepts_unversionable_binary() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("autoloop");
        write_binary(&binary, "");

        assert!(matches!(
            check_autoloop_at(&binary, AutoloopSource::PathLookup),
            AutoloopHealth::VersionUnknown { .. }
        ));
    }

    #[test]
    fn autoloop_health_rejects_old_version() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("bin/autoloop");
        write_package(temp.path(), "0.9.2");
        write_binary(&binary, "");

        assert!(matches!(
            check_autoloop_at(&binary, AutoloopSource::PathLookup),
            AutoloopHealth::TooOld { version, .. } if version == "0.9.2"
        ));
    }

    #[test]
    fn autoloop_health_accepts_minimum_and_newer_versions() {
        for version in ["0.10.0", "0.11.1"] {
            let temp = tempfile::tempdir().unwrap();
            let binary = temp.path().join("bin/autoloop");
            write_package(temp.path(), version);
            write_binary(&binary, "");

            assert!(matches!(
                check_autoloop_at(&binary, AutoloopSource::PathLookup),
                AutoloopHealth::Ok { version: found, .. } if found == version
            ));
        }
    }

    #[test]
    fn autoloop_health_ignores_prerelease_suffix_for_comparison() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("bin/autoloop");
        write_package(temp.path(), "0.10.0-beta.1");
        write_binary(&binary, "");

        assert!(matches!(
            check_autoloop_at(&binary, AutoloopSource::PathLookup),
            AutoloopHealth::Ok { version, .. } if version == "0.10.0-beta.1"
        ));
    }

    #[test]
    fn autoloop_health_treats_unparseable_package_version_as_unknown() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("bin/autoloop");
        write_package(temp.path(), "latest");
        write_binary(&binary, "");

        assert!(matches!(
            check_autoloop_at(&binary, AutoloopSource::PathLookup),
            AutoloopHealth::VersionUnknown { .. }
        ));
    }

    #[test]
    fn standalone_version_output_is_parsed() {
        for output in ["autoloop 0.10.1\n", "0.10.1\n"] {
            assert_eq!(extract_version(output).as_deref(), Some("0.10.1"));
        }
    }

    #[test]
    fn autoloop_health_uses_executable_version_without_package_json() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("autoloop");
        write_binary(&binary, "printf 'autoloop 0.10.1\\n'");

        assert_eq!(
            check_autoloop_at(&binary, AutoloopSource::Vendored),
            AutoloopHealth::Ok {
                path: binary.canonicalize().unwrap(),
                version: "0.10.1".to_string(),
                source: AutoloopSource::Vendored,
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn autoloop_health_env_resolution_child() {
        let Some(expected_source) = std::env::var_os("RALPH_TEST_EXPECTED_SOURCE") else {
            return;
        };
        let expected_path = PathBuf::from(std::env::var_os("RALPH_TEST_EXPECTED_PATH").unwrap());
        let health = check_autoloop();

        let expected_source = match expected_source.to_str().unwrap() {
            "vendored" => AutoloopSource::Vendored,
            "path" => AutoloopSource::PathLookup,
            other => panic!("unexpected source {other}"),
        };

        assert!(matches!(
            health,
            AutoloopHealth::Ok { path, source, .. }
                if path == expected_path.canonicalize().unwrap() && source == expected_source
        ));
    }

    #[cfg(unix)]
    #[test]
    fn vendored_engine_precedes_path_and_path_is_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let engine_dir = temp.path().join("engine");
        let path_dir = temp.path().join("path-bin");
        let vendored = engine_dir.join("autoloop");
        let path_binary = path_dir.join("autoloop");
        write_binary(&vendored, "printf '0.10.1\\n'");
        write_binary(&path_binary, "printf '0.11.0\\n'");

        run_resolution_child(&engine_dir, &path_dir, &vendored, "vendored");
        fs::remove_file(&vendored).unwrap();
        run_resolution_child(&engine_dir, &path_dir, &path_binary, "path");
    }

    #[cfg(unix)]
    fn run_resolution_child(
        engine_dir: &Path,
        path_dir: &Path,
        expected_path: &Path,
        expected_source: &str,
    ) {
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "autoloop_health::tests::autoloop_health_env_resolution_child",
                "--nocapture",
            ])
            .env("RALPH_ENGINE_DIR", engine_dir)
            .env("PATH", path_dir)
            .env("RALPH_TEST_EXPECTED_PATH", expected_path)
            .env("RALPH_TEST_EXPECTED_SOURCE", expected_source)
            .status()
            .unwrap();

        assert!(status.success(), "resolution child failed: {status}");
    }
}
