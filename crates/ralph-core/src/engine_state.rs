//! Ralph-owned engine state layout.
//!
//! Ralph launches the autoloop engine with its runtime state rooted beneath
//! `.ralph/autoloop` instead of autoloop's standalone `.autoloop` default.
//! This module is the single seam defining that layout: launch code exports
//! the root to the engine (via `AUTOLOOP_STATE_DIR`), and observation code
//! (TUI, headless, diagnostics) derives run-scoped paths from it instead of
//! reconstructing a top-level `.autoloop`.

use std::path::{Path, PathBuf};

/// The canonical engine state root for a Ralph workspace.
pub fn engine_state_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".ralph").join("autoloop")
}

/// The run-scoped state directory for one engine run.
pub fn engine_run_dir(workspace_root: &Path, run_id: &str) -> PathBuf {
    engine_state_root(workspace_root).join("runs").join(run_id)
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
    fn engine_run_dir_is_run_scoped_beneath_the_root() {
        let run_dir = engine_run_dir(Path::new("/work"), "run-1");
        assert_eq!(run_dir, PathBuf::from("/work/.ralph/autoloop/runs/run-1"));
        assert!(run_dir.starts_with(engine_state_root(Path::new("/work"))));
    }

    #[test]
    fn relative_workspace_keeps_a_relative_root() {
        assert_eq!(
            engine_state_root(Path::new(".")),
            PathBuf::from("./.ralph/autoloop")
        );
    }
}
