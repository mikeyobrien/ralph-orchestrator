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
