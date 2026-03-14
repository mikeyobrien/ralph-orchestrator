use std::path::Path;
use std::process::Command;

use chrono::{SecondsFormat, Utc};
use ralph_core::{MergeQueueError, WorktreeError};

use crate::errors::ApiError;

pub fn loop_not_found_error(loop_id: &str) -> ApiError {
    ApiError::loop_not_found(format!("Loop '{loop_id}' not found"))
        .with_details(serde_json::json!({ "loopId": loop_id }))
}

pub fn map_merge_error(error: MergeQueueError) -> ApiError {
    match error {
        MergeQueueError::NotFound(loop_id) => loop_not_found_error(&loop_id),
        MergeQueueError::InvalidTransition(loop_id, from, to) => ApiError::precondition_failed(
            format!("Invalid loop transition for '{loop_id}': {from:?} -> {to:?}"),
        )
        .with_details(serde_json::json!({
            "loopId": loop_id,
            "from": format!("{from:?}"),
            "to": format!("{to:?}")
        })),
        other => ApiError::internal(format!("merge queue operation failed: {other}")),
    }
}

pub fn map_worktree_error(loop_id: &str, error: WorktreeError) -> ApiError {
    ApiError::internal(format!(
        "worktree cleanup failed for loop '{loop_id}': {error}"
    ))
    .with_details(serde_json::json!({ "loopId": loop_id }))
}

pub fn current_commit(workspace_root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(workspace_root)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if sha.is_empty() {
                "manual".to_string()
            } else {
                sha
            }
        }
        _ => "manual".to_string(),
    }
}

pub fn is_pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
}

pub(crate) fn now_ts() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}
