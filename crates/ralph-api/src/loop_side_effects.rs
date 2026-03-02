use std::path::{Path, PathBuf};
use std::process::Command;

use ralph_core::{LoopRegistry, MergeQueue, list_ralph_worktrees};

use crate::errors::ApiError;
use crate::loop_support::{loop_not_found_error, map_merge_error};

pub struct ResolvedLoop {
    pub id: String,
    pub worktree_path: Option<PathBuf>,
}

pub fn spawn_retry_merge_flow(
    workspace_root: &Path,
    ralph_command: &str,
    loop_id: &str,
) -> Result<(), ApiError> {
    let status = Command::new(ralph_command)
        .args(["loops", "retry", loop_id])
        .current_dir(workspace_root)
        .status()
        .map_err(|error| {
            ApiError::internal(format!(
                "failed invoking '{ralph_command}' for loop retry '{loop_id}': {error}"
            ))
        })?;

    if !status.success() {
        return Err(ApiError::internal(format!(
            "loop retry command exited with status {status} for loop '{loop_id}'"
        )));
    }

    Ok(())
}

pub fn resolve_discard_target(
    workspace_root: &Path,
    loop_id: &str,
) -> Result<ResolvedLoop, ApiError> {
    let registry = LoopRegistry::new(workspace_root);
    match registry.get(loop_id) {
        Ok(Some(entry)) => {
            return Ok(ResolvedLoop {
                id: entry.id,
                worktree_path: entry.worktree_path.map(PathBuf::from),
            });
        }
        Ok(None) => {}
        Err(error) => {
            return Err(ApiError::internal(format!(
                "failed resolving loop '{loop_id}' from registry: {error}"
            )));
        }
    }

    let queue = MergeQueue::new(workspace_root);
    if let Some(entry) = queue.get_entry(loop_id).map_err(map_merge_error)? {
        return Ok(ResolvedLoop {
            id: entry.loop_id.clone(),
            worktree_path: find_worktree_path(workspace_root, &entry.loop_id)?,
        });
    }

    if let Some(worktree_path) = find_worktree_path(workspace_root, loop_id)? {
        return Ok(ResolvedLoop {
            id: loop_id.to_string(),
            worktree_path: Some(worktree_path),
        });
    }

    Err(loop_not_found_error(loop_id))
}

pub fn resolve_loop_root(workspace_root: &Path, loop_id: &str) -> Result<PathBuf, ApiError> {
    if loop_id == "(primary)" || loop_id == "primary" {
        return Ok(workspace_root.to_path_buf());
    }

    let registry = LoopRegistry::new(workspace_root);
    match registry.get(loop_id) {
        Ok(Some(entry)) => Ok(entry
            .worktree_path
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_root.to_path_buf())),
        Ok(None) => Err(loop_not_found_error(loop_id)),
        Err(error) => Err(ApiError::internal(format!(
            "failed resolving loop '{loop_id}': {error}"
        ))),
    }
}

fn find_worktree_path(workspace_root: &Path, loop_id: &str) -> Result<Option<PathBuf>, ApiError> {
    let worktrees = list_ralph_worktrees(workspace_root)
        .map_err(|error| ApiError::internal(format!("failed listing worktrees: {error}")))?;

    Ok(worktrees.into_iter().find_map(|worktree| {
        worktree
            .branch
            .strip_prefix("ralph/")
            .is_some_and(|branch_loop_id| branch_loop_id == loop_id)
            .then_some(worktree.path)
    }))
}
