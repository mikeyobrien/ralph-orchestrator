//! Ralph-owned engine state layout.
//!
//! Ralph launches the autoloop engine with its runtime state rooted beneath
//! `.ralph/autoloop` instead of autoloop's standalone `.autoloop` default.
//! This module is the single seam defining that layout: launch code supplies
//! both config overrides and environment exports, and observation code (TUI,
//! headless, diagnostics) derives run-scoped paths from it instead of
//! reconstructing a top-level `.autoloop`.

use std::path::{Path, PathBuf};

/// The canonical engine state root for a Ralph workspace.
pub fn engine_state_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".ralph").join("autoloop")
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

/// Environment exports that keep nested engine tools beneath `engine_root`.
///
/// Pass an absolute root so child working-directory handling cannot re-anchor
/// paths. Top-level native run context also requires
/// [`engine_config_overrides`].
pub fn engine_env(engine_root: &Path) -> [(&'static str, String); 4] {
    let file = |name: &str| engine_root.join(name).to_string_lossy().into_owned();
    [
        (
            "AUTOLOOP_STATE_DIR",
            engine_root.to_string_lossy().into_owned(),
        ),
        ("AUTOLOOP_JOURNAL_FILE", file("journal.jsonl")),
        ("AUTOLOOP_MEMORY_FILE", file("memory.jsonl")),
        ("AUTOLOOP_TASKS_FILE", file("tasks.jsonl")),
    ]
}

/// CLI config overrides that pin the top-level native run context to Ralph's
/// engine root.
///
/// Autoloop's nested tools consume the environment exports above, while its
/// top-level `buildLoopContext` currently resolves stores from layered config.
/// Supplying both surfaces keeps all runtime state under the same owned root.
pub fn engine_config_overrides(engine_root: &Path) -> [(&'static str, String); 4] {
    let file = |name: &str| engine_root.join(name).to_string_lossy().into_owned();
    [
        ("core.state_dir", engine_root.to_string_lossy().into_owned()),
        ("core.journal_file", file("journal.jsonl")),
        ("core.memory_file", file("memory.jsonl")),
        ("core.tasks_file", file("tasks.jsonl")),
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
}
