//! Ralph-owned engine state layout.
//!
//! Ralph launches the autoloop engine with its runtime state rooted beneath
//! `.ralph/autoloop` instead of autoloop's standalone `.autoloop` default.
//! This module is the single seam defining that layout: launch code supplies
//! both config overrides and environment exports, and observation code (TUI,
//! headless, diagnostics) derives run-scoped paths from it instead of
//! reconstructing a top-level `.autoloop`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The canonical engine state root for a Ralph workspace.
pub fn engine_state_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".ralph").join("autoloop")
}

/// Creates and resolves Ralph's engine state root without following symlinks in
/// the owned `.ralph/autoloop` tree.
///
/// The workspace itself is canonicalized first. Each owned root component is
/// inspected before creation and again after creation/canonicalization. Every
/// pre-existing descendant is then inspected with symlink metadata and
/// directories are traversed only after proving that they are not symlinks.
/// This fails closed if a component or descendant is a symlink, if a root
/// component is not a directory, or if the resolved root is not physically
/// beneath the workspace.
///
/// Rust's standard library does not expose portable `openat`-style traversal
/// and subprocess launch, so a hostile process with concurrent filesystem
/// access can still replace an entry after validation and before or during
/// launch. The repeated no-follow checks reject pre-existing attacks but cannot
/// eliminate that unavoidable TOCTOU window.
pub fn prepare_engine_state_root(workspace_root: &Path) -> io::Result<PathBuf> {
    let workspace = fs::canonicalize(workspace_root).map_err(|error| {
        safe_state_error(error.kind(), "could not resolve the workspace directory")
    })?;
    let mut current = workspace.clone();

    for component in [".ralph", "autoloop"] {
        let candidate = current.join(component);
        ensure_owned_directory(&candidate)?;
        let resolved = fs::canonicalize(&candidate).map_err(|error| {
            safe_state_error(
                error.kind(),
                "could not resolve a Ralph-owned state directory",
            )
        })?;
        if !resolved.starts_with(&workspace) {
            return Err(safe_state_error(
                io::ErrorKind::InvalidInput,
                "Ralph-owned engine state resolves outside the workspace",
            ));
        }
        reject_symlink_or_non_directory(&candidate)?;
        current = resolved;
    }

    reject_descendant_symlinks(&current)?;

    Ok(current)
}

fn reject_descendant_symlinks(directory: &Path) -> io::Result<()> {
    let entries = fs::read_dir(directory).map_err(|error| {
        safe_state_error(error.kind(), "could not inspect Ralph-owned engine state")
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            safe_state_error(error.kind(), "could not inspect Ralph-owned engine state")
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
            safe_state_error(error.kind(), "could not inspect Ralph-owned engine state")
        })?;
        if metadata.file_type().is_symlink() {
            return Err(safe_state_error(
                io::ErrorKind::InvalidInput,
                "Ralph-owned engine state contains a symlink",
            ));
        }
        if metadata.is_dir() {
            reject_descendant_symlinks(&entry.path())?;
        }
    }
    Ok(())
}

fn ensure_owned_directory(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => reject_symlink_or_non_directory(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => reject_symlink_or_non_directory(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                reject_symlink_or_non_directory(path)
            }
            Err(error) => Err(safe_state_error(
                error.kind(),
                "could not create a Ralph-owned state directory",
            )),
        },
        Err(error) => Err(safe_state_error(
            error.kind(),
            "could not inspect a Ralph-owned state directory",
        )),
    }
}

fn reject_symlink_or_non_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        safe_state_error(
            error.kind(),
            "could not inspect a Ralph-owned state directory",
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(safe_state_error(
            io::ErrorKind::InvalidInput,
            "Ralph-owned engine state path contains a symlink",
        ));
    }
    if !metadata.is_dir() {
        return Err(safe_state_error(
            io::ErrorKind::InvalidInput,
            "Ralph-owned engine state path contains a non-directory",
        ));
    }
    Ok(())
}

fn safe_state_error(kind: io::ErrorKind, message: &'static str) -> io::Error {
    io::Error::new(kind, message)
}

/// Derives the run-scoped state directory beneath a configured engine root.
///
/// Run IDs come from engine events, so accept only one non-empty normal path
/// component. Invalid IDs are rejected rather than normalized or redirected to
/// another state root.
pub fn engine_run_dir(engine_root: &Path, run_id: &str) -> Option<PathBuf> {
    if run_id.contains(['/', '\\']) {
        return None;
    }

    let mut components = Path::new(run_id).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => {
            Some(engine_root.join("runs").join(run_id))
        }
        _ => None,
    }
}

/// The exact journal file owned by an engine state root.
pub fn engine_journal_path(engine_root: &Path) -> PathBuf {
    engine_root.join("journal.jsonl")
}

/// The exact memory file owned by an engine state root.
pub fn engine_memory_path(engine_root: &Path) -> PathBuf {
    engine_root.join("memory.jsonl")
}

fn engine_tasks_path(engine_root: &Path) -> PathBuf {
    engine_root.join("tasks.jsonl")
}

/// Environment exports that keep nested engine tools beneath `engine_root`.
///
/// Pass an absolute root so child working-directory handling cannot re-anchor
/// paths. Top-level native run context also requires
/// [`engine_config_overrides`].
pub fn engine_env(engine_root: &Path) -> [(&'static str, String); 4] {
    let string = |path: PathBuf| path.to_string_lossy().into_owned();
    [
        (
            "AUTOLOOP_STATE_DIR",
            engine_root.to_string_lossy().into_owned(),
        ),
        (
            "AUTOLOOP_JOURNAL_FILE",
            string(engine_journal_path(engine_root)),
        ),
        (
            "AUTOLOOP_MEMORY_FILE",
            string(engine_memory_path(engine_root)),
        ),
        (
            "AUTOLOOP_TASKS_FILE",
            string(engine_tasks_path(engine_root)),
        ),
    ]
}

/// CLI config overrides that pin the top-level native run context to Ralph's
/// engine root.
///
/// Autoloop's nested tools consume the environment exports above, while its
/// top-level `buildLoopContext` currently resolves stores from layered config.
/// Supplying both surfaces keeps all runtime state under the same owned root.
pub fn engine_config_overrides(engine_root: &Path) -> [(&'static str, String); 4] {
    let string = |path: PathBuf| path.to_string_lossy().into_owned();
    [
        ("core.state_dir", engine_root.to_string_lossy().into_owned()),
        (
            "core.journal_file",
            string(engine_journal_path(engine_root)),
        ),
        ("core.memory_file", string(engine_memory_path(engine_root))),
        ("core.tasks_file", string(engine_tasks_path(engine_root))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_state_root_is_under_ralph() {
        assert_eq!(
            engine_state_root(Path::new("/work")),
            PathBuf::from("/work/.ralph/autoloop")
        );
    }

    #[test]
    fn engine_run_dir_accepts_a_safe_single_component_id() {
        let root = engine_state_root(Path::new("/work"));
        let run_dir = engine_run_dir(&root, "run-1").expect("safe run ID");
        assert_eq!(run_dir, PathBuf::from("/work/.ralph/autoloop/runs/run-1"));
        assert!(run_dir.starts_with(&root));
    }

    #[test]
    fn engine_run_dir_rejects_empty_or_unsafe_ids() {
        let root = engine_state_root(Path::new("/work"));
        for run_id in [
            "",
            ".",
            "..",
            "/absolute",
            "nested/run",
            "nested\\run",
            "../escape",
        ] {
            assert_eq!(
                engine_run_dir(&root, run_id),
                None,
                "run ID should be rejected: {run_id:?}"
            );
        }
    }

    #[test]
    fn engine_env_pins_every_store_beneath_the_root() {
        let env = engine_env(Path::new("/work/.ralph/autoloop"));
        assert_eq!(
            env,
            [
                ("AUTOLOOP_STATE_DIR", "/work/.ralph/autoloop".to_string()),
                (
                    "AUTOLOOP_JOURNAL_FILE",
                    "/work/.ralph/autoloop/journal.jsonl".to_string()
                ),
                (
                    "AUTOLOOP_MEMORY_FILE",
                    "/work/.ralph/autoloop/memory.jsonl".to_string()
                ),
                (
                    "AUTOLOOP_TASKS_FILE",
                    "/work/.ralph/autoloop/tasks.jsonl".to_string()
                ),
            ]
        );
    }

    #[test]
    fn engine_config_overrides_pin_every_store_beneath_the_root() {
        assert_eq!(
            engine_config_overrides(Path::new("/work/.ralph/autoloop")),
            [
                ("core.state_dir", "/work/.ralph/autoloop".to_string()),
                (
                    "core.journal_file",
                    "/work/.ralph/autoloop/journal.jsonl".to_string()
                ),
                (
                    "core.memory_file",
                    "/work/.ralph/autoloop/memory.jsonl".to_string()
                ),
                (
                    "core.tasks_file",
                    "/work/.ralph/autoloop/tasks.jsonl".to_string()
                ),
            ]
        );
    }

    #[test]
    fn engine_stores_never_reference_ralph_agent_stores() {
        let root = engine_state_root(Path::new("/work"));
        for (_, value) in engine_env(&root)
            .into_iter()
            .chain(engine_config_overrides(&root))
        {
            assert!(
                !value.contains(".ralph/agent"),
                "engine stores must stay separate from Ralph's: {value}"
            );
        }
    }

    #[test]
    fn relative_workspace_keeps_a_relative_root() {
        assert_eq!(
            engine_state_root(Path::new(".")),
            PathBuf::from("./.ralph/autoloop")
        );
    }

    #[cfg(unix)]
    #[test]
    fn owned_root_rejects_ralph_symlink_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        symlink(external.path(), workspace.path().join(".ralph")).unwrap();

        let error = prepare_engine_state_root(workspace.path()).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("contains a symlink"));
        assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn owned_root_rejects_autoloop_symlink_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::create_dir(workspace.path().join(".ralph")).unwrap();
        symlink(external.path(), workspace.path().join(".ralph/autoloop")).unwrap();

        let error = prepare_engine_state_root(workspace.path()).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("contains a symlink"));
        assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn owned_root_rejects_direct_and_nested_descendant_symlinks() {
        use std::os::unix::fs::symlink;

        for relative in ["journal.jsonl", "runs", "runs/run-1/stream.jsonl"] {
            let workspace = tempfile::tempdir().unwrap();
            let external = tempfile::tempdir().unwrap();
            let root = workspace.path().join(".ralph/autoloop");
            let leaf = root.join(relative);
            std::fs::create_dir_all(leaf.parent().unwrap()).unwrap();
            symlink(external.path(), &leaf).unwrap();

            let error = prepare_engine_state_root(workspace.path()).unwrap_err();

            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
            assert_eq!(
                error.to_string(),
                "Ralph-owned engine state contains a symlink"
            );
            assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 0);
        }
    }
}
