use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should be resolvable")
}

fn files_below(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("failed to enumerate {}: {error}", directory.display()));
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            files.extend(files_below(&path));
        } else if path.is_file() {
            files.push(path);
        }
    }

    files
}

#[test]
fn shipped_artifacts_do_not_reference_deleted_wave_cli() {
    let root = repo_root();
    let artifact_directories = [
        root.join("crates/ralph-core/data"),
        root.join("presets"),
        root.join("examples"),
        root.join("crates/ralph-cli/presets"),
    ];
    let forbidden = concat!("ralph", " wave");
    let mut violations = Vec::new();

    for directory in artifact_directories {
        for path in files_below(&directory) {
            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            for (line_index, line) in contents.lines().enumerate() {
                if line.contains(forbidden) {
                    let relative_path = path.strip_prefix(&root).unwrap_or(&path);
                    violations.push(format!(
                        "{}:{}: {}",
                        relative_path.display(),
                        line_index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "shipped artifacts reference the deleted wave CLI:\n{}",
        violations.join("\n")
    );
}

/// R26 tripwire: no Rust source may hardcode a sibling-checkout path to the
/// autoloop repo. Tests that need a real engine must resolve it through
/// `AUTOLOOP_ROOT` / ancestor discovery and skip when absent — a literal
/// dot-dot-slash reference to the engine repo reintroduces the
/// CI-green-but-asserting-on-an-external-repo failure mode removed in the
/// v3 GA cleanup.
#[test]
fn no_sibling_repo_paths_in_rust_sources() {
    let root = repo_root();
    let forbidden = concat!("../", "autoloop");
    let mut violations = Vec::new();

    for path in files_below(&root.join("crates")) {
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        // target/ build output is not source.
        if path
            .components()
            .any(|component| component.as_os_str() == "target")
        {
            continue;
        }
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for (line_index, line) in contents.lines().enumerate() {
            if line.contains(forbidden) {
                let relative_path = path.strip_prefix(&root).unwrap_or(&path);
                violations.push(format!(
                    "{}:{}: {}",
                    relative_path.display(),
                    line_index + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Rust sources reference a sibling autoloop checkout by relative path:\n{}",
        violations.join("\n")
    );
}
