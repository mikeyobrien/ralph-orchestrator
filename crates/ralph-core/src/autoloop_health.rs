//! Autoloop dependency discovery and version health checks.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::utils::find_executable;

/// The oldest autoloop release that provides Ralph's required protocol.
pub const MIN_AUTOLOOP_VERSION: &str = "0.10.0";

/// Command users can run to install or update autoloop.
pub const AUTOLOOP_INSTALL_HINT: &str = "npm install -g @mobrienv/autoloop";

/// The result of locating autoloop and inspecting its package metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoloopHealth {
    /// No `autoloop` executable was found on `PATH`.
    Missing,
    /// The executable exists, but its package version could not be determined.
    VersionUnknown { path: PathBuf },
    /// The installed package is older than [`MIN_AUTOLOOP_VERSION`].
    TooOld { path: PathBuf, version: String },
    /// The installed package meets the minimum version requirement.
    Ok { path: PathBuf, version: String },
}

/// Locate `autoloop` on `PATH` and inspect its adjacent npm package metadata.
pub fn check_autoloop() -> AutoloopHealth {
    let Some(path) = find_executable("autoloop") else {
        return AutoloopHealth::Missing;
    };

    check_autoloop_at(&path)
}

fn check_autoloop_at(bin_path: &Path) -> AutoloopHealth {
    let path = bin_path
        .canonicalize()
        .unwrap_or_else(|_| bin_path.to_path_buf());
    let Some(version) = probe_version(&path) else {
        return AutoloopHealth::VersionUnknown { path };
    };

    let Some(installed) = parse_version(&version) else {
        return AutoloopHealth::VersionUnknown { path };
    };
    let minimum = parse_version(MIN_AUTOLOOP_VERSION)
        .expect("MIN_AUTOLOOP_VERSION must be a major.minor.patch version");

    if installed < minimum {
        AutoloopHealth::TooOld { path, version }
    } else {
        AutoloopHealth::Ok { path, version }
    }
}

/// Read the version from the nearest enclosing autoloop npm package.
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

    None
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

    fn write_binary(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "#!/bin/sh\n").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn autoloop_health_resolves_version_through_bin_shim() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("lib/node_modules/@mobrienv/autoloop");
        let real_binary = package.join("bin/autoloop.js");
        write_package(&package, "0.11.0");
        write_binary(&real_binary);

        let shim = temp.path().join("bin/autoloop");
        fs::create_dir_all(shim.parent().unwrap()).unwrap();
        symlink(&real_binary, &shim).unwrap();

        assert_eq!(
            check_autoloop_at(&shim),
            AutoloopHealth::Ok {
                path: real_binary.canonicalize().unwrap(),
                version: "0.11.0".to_string(),
            }
        );
    }

    #[test]
    fn autoloop_health_reads_package_json_adjacent_to_binary() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("autoloop");
        write_package(temp.path(), "0.10.0");
        write_binary(&binary);

        assert!(matches!(
            check_autoloop_at(&binary),
            AutoloopHealth::Ok { version, .. } if version == "0.10.0"
        ));
    }

    #[test]
    fn autoloop_health_accepts_unversionable_binary() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("autoloop");
        write_binary(&binary);

        assert!(matches!(
            check_autoloop_at(&binary),
            AutoloopHealth::VersionUnknown { .. }
        ));
    }

    #[test]
    fn autoloop_health_rejects_old_version() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("bin/autoloop");
        write_package(temp.path(), "0.9.2");
        write_binary(&binary);

        assert!(matches!(
            check_autoloop_at(&binary),
            AutoloopHealth::TooOld { version, .. } if version == "0.9.2"
        ));
    }

    #[test]
    fn autoloop_health_accepts_minimum_and_newer_versions() {
        for version in ["0.10.0", "0.11.1"] {
            let temp = tempfile::tempdir().unwrap();
            let binary = temp.path().join("bin/autoloop");
            write_package(temp.path(), version);
            write_binary(&binary);

            assert!(matches!(
                check_autoloop_at(&binary),
                AutoloopHealth::Ok { version: found, .. } if found == version
            ));
        }
    }

    #[test]
    fn autoloop_health_ignores_prerelease_suffix_for_comparison() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("bin/autoloop");
        write_package(temp.path(), "0.10.0-beta.1");
        write_binary(&binary);

        assert!(matches!(
            check_autoloop_at(&binary),
            AutoloopHealth::Ok { version, .. } if version == "0.10.0-beta.1"
        ));
    }

    #[test]
    fn autoloop_health_treats_unparseable_package_version_as_unknown() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("bin/autoloop");
        write_package(temp.path(), "latest");
        write_binary(&binary);

        assert!(matches!(
            check_autoloop_at(&binary),
            AutoloopHealth::VersionUnknown { .. }
        ));
    }
}
