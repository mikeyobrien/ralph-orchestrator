use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use ralph_core::{
    LoopLock, LoopRegistry, MergeButtonState, MergeQueue, MergeState, RegistryError,
    merge_button_state, remove_worktree,
};
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;
use crate::loop_side_effects::{resolve_discard_target, resolve_loop_root, spawn_retry_merge_flow};
use crate::loop_support::{
    current_commit, is_pid_alive, loop_not_found_error, map_merge_error, map_worktree_error, now_ts,
};
use crate::task_domain::{TaskCreateParams, TaskDomain};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopListParams {
    pub include_terminal: Option<bool>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopRetryParams {
    pub id: String,
    pub steering_input: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopStopMergeParams {
    pub id: String,
    pub force: Option<bool>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopTriggerMergeTaskParams {
    pub loop_id: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopRecord {
    pub id: String,
    pub status: String,
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopStatusResult {
    pub running: bool,
    pub interval_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_processed_at: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeButtonStateResult {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerMergeTaskResult {
    pub success: bool,
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queued_task_id: Option<String>,
}

pub struct LoopDomain {
    workspace_root: PathBuf,
    process_interval_ms: u64,
    ralph_command: String,
    last_processed_at: Option<String>,
}

impl LoopDomain {
    pub fn new(
        workspace_root: impl AsRef<Path>,
        process_interval_ms: u64,
        ralph_command: impl Into<String>,
    ) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            process_interval_ms,
            ralph_command: ralph_command.into(),
            last_processed_at: None,
        }
    }
    pub fn list(&self, params: LoopListParams) -> Result<Vec<LoopRecord>, ApiError> {
        let include_terminal = params.include_terminal.unwrap_or(false);
        let registry = LoopRegistry::new(&self.workspace_root);
        let merge_queue = MergeQueue::new(&self.workspace_root);

        let mut loops = Vec::new();
        let mut listed_ids = HashSet::new();

        if let Ok(Some(metadata)) = LoopLock::read_existing(&self.workspace_root)
            && is_pid_alive(metadata.pid)
        {
            loops.push(LoopRecord {
                id: "(primary)".to_string(),
                status: "running".to_string(),
                location: "(in-place)".to_string(),
                prompt: Some(metadata.prompt),
                merge_commit: None,
            });
            listed_ids.insert("(primary)".to_string());
        }

        let registry_entries = registry
            .list()
            .map_err(|error| ApiError::internal(format!("failed listing loops: {error}")))?;

        for entry in registry_entries {
            let status = if entry.is_alive() {
                "running"
            } else {
                "crashed"
            };

            let location = entry
                .worktree_path
                .clone()
                .unwrap_or_else(|| "(in-place)".to_string());

            listed_ids.insert(entry.id.clone());
            loops.push(LoopRecord {
                id: entry.id,
                status: status.to_string(),
                location,
                prompt: Some(entry.prompt),
                merge_commit: None,
            });
        }

        for entry in merge_queue
            .list()
            .map_err(|error| ApiError::internal(format!("failed reading merge queue: {error}")))?
        {
            if listed_ids.contains(&entry.loop_id) {
                continue;
            }

            let status = match entry.state {
                MergeState::Queued => "queued",
                MergeState::Merging => "merging",
                MergeState::Merged => "merged",
                MergeState::NeedsReview => "needs-review",
                MergeState::Discarded => "discarded",
            };

            loops.push(LoopRecord {
                id: entry.loop_id,
                status: status.to_string(),
                location: entry
                    .merge_commit
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                prompt: Some(entry.prompt),
                merge_commit: entry.merge_commit,
            });
        }

        if !include_terminal {
            loops.retain(|loop_info| !matches!(loop_info.status.as_str(), "merged" | "discarded"));
        }

        Ok(loops)
    }
    pub fn status(&self) -> LoopStatusResult {
        let running = LoopLock::is_locked(&self.workspace_root).unwrap_or(false);
        LoopStatusResult {
            running,
            interval_ms: self.process_interval_ms,
            last_processed_at: self.last_processed_at.clone(),
        }
    }
    pub fn process(&mut self) -> Result<(), ApiError> {
        let queue = MergeQueue::new(&self.workspace_root);
        let pending_entries = queue
            .list_by_state(MergeState::Queued)
            .map_err(map_merge_error)?;

        if pending_entries.is_empty() {
            self.last_processed_at = Some(now_ts());
            return Ok(());
        }

        let status = Command::new(&self.ralph_command)
            .args(["loops", "process"])
            .current_dir(&self.workspace_root)
            .status()
            .map_err(|error| {
                ApiError::internal(format!(
                    "failed invoking '{}' for loop.process: {error}",
                    self.ralph_command
                ))
            })?;

        if !status.success() {
            return Err(ApiError::internal(format!(
                "loop.process command '{}' exited with status {status}",
                self.ralph_command
            )));
        }

        self.last_processed_at = Some(now_ts());
        Ok(())
    }
    pub fn prune(&self) -> Result<(), ApiError> {
        let registry = LoopRegistry::new(&self.workspace_root);
        registry
            .clean_stale()
            .map_err(|error| ApiError::internal(format!("failed pruning stale loops: {error}")))?;
        Ok(())
    }
    pub fn retry(&self, params: LoopRetryParams) -> Result<(), ApiError> {
        if let Some(steering_input) = params.steering_input
            && !steering_input.trim().is_empty()
        {
            let steering_path = self.workspace_root.join(".ralph/merge-steering.txt");
            if let Some(parent) = steering_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    ApiError::internal(format!(
                        "failed creating merge steering directory '{}': {error}",
                        parent.display()
                    ))
                })?;
            }
            fs::write(&steering_path, steering_input.trim()).map_err(|error| {
                ApiError::internal(format!(
                    "failed writing merge steering file '{}': {error}",
                    steering_path.display()
                ))
            })?;
        }

        let queue = MergeQueue::new(&self.workspace_root);
        let entry = queue
            .get_entry(&params.id)
            .map_err(map_merge_error)?
            .ok_or_else(|| loop_not_found_error(&params.id))?;

        if entry.state != MergeState::NeedsReview {
            return Err(ApiError::precondition_failed(format!(
                "Loop '{}' is in state {:?}, can only retry 'needs-review' loops",
                params.id, entry.state
            )));
        }

        spawn_retry_merge_flow(&self.workspace_root, &self.ralph_command, &params.id)
    }

    pub fn discard(&self, id: &str) -> Result<(), ApiError> {
        let resolved = resolve_discard_target(&self.workspace_root, id)?;
        let queue = MergeQueue::new(&self.workspace_root);
        let registry = LoopRegistry::new(&self.workspace_root);

        if queue
            .get_entry(&resolved.id)
            .map_err(map_merge_error)?
            .is_some()
        {
            queue
                .discard(&resolved.id, Some("User requested discard"))
                .map_err(map_merge_error)?;
        }

        match registry.deregister(&resolved.id) {
            Ok(()) | Err(RegistryError::NotFound(_)) => {}
            Err(error) => {
                return Err(ApiError::internal(format!(
                    "failed deregistering loop '{}': {error}",
                    resolved.id
                )));
            }
        }

        if let Some(worktree_path) = resolved.worktree_path {
            remove_worktree(&self.workspace_root, &worktree_path)
                .map_err(|error| map_worktree_error(&resolved.id, error))?;
        }

        Ok(())
    }
    pub fn stop(&self, params: LoopStopMergeParams) -> Result<(), ApiError> {
        let target_root = resolve_loop_root(&self.workspace_root, &params.id)?;
        let lock_metadata = LoopLock::read_existing(&target_root)
            .map_err(|error| ApiError::internal(format!("failed reading loop lock: {error}")))?
            .ok_or_else(|| loop_not_found_error(&params.id))?;

        if params.force.unwrap_or(false) {
            if !is_pid_alive(lock_metadata.pid) {
                return Err(ApiError::precondition_failed(format!(
                    "Loop '{}' is not running (process {} not found)",
                    params.id, lock_metadata.pid
                )));
            }

            let status = Command::new("kill")
                .args(["-9", &lock_metadata.pid.to_string()])
                .status()
                .map_err(|error| {
                    ApiError::internal(format!(
                        "failed sending force stop signal to process {}: {error}",
                        lock_metadata.pid
                    ))
                })?;

            if !status.success() {
                return Err(ApiError::internal(format!(
                    "failed force-stopping process {}",
                    lock_metadata.pid
                )));
            }

            return Ok(());
        }

        let stop_path = target_root.join(".ralph/stop-requested");
        if let Some(parent) = stop_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed creating stop marker directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        fs::write(&stop_path, "").map_err(|error| {
            ApiError::internal(format!(
                "failed writing stop marker '{}': {error}",
                stop_path.display()
            ))
        })?;

        Ok(())
    }
    pub fn merge(&self, params: LoopStopMergeParams) -> Result<(), ApiError> {
        let queue = MergeQueue::new(&self.workspace_root);
        let entry = queue
            .get_entry(&params.id)
            .map_err(map_merge_error)?
            .ok_or_else(|| loop_not_found_error(&params.id))?;

        match entry.state {
            MergeState::Merged => {
                return Err(ApiError::precondition_failed(format!(
                    "Loop '{}' is already merged",
                    params.id
                )));
            }
            MergeState::Discarded => {
                return Err(ApiError::precondition_failed(format!(
                    "Loop '{}' is discarded",
                    params.id
                )));
            }
            MergeState::Merging if !params.force.unwrap_or(false) => {
                return Err(ApiError::precondition_failed(format!(
                    "Loop '{}' is currently merging. Use force=true to override.",
                    params.id
                )));
            }
            _ => {}
        }

        if entry.state != MergeState::Merging {
            queue
                .mark_merging(&params.id, std::process::id())
                .map_err(map_merge_error)?;
        }

        queue
            .mark_merged(&params.id, &current_commit(&self.workspace_root))
            .map_err(map_merge_error)
    }
    pub fn merge_button_state(&self, id: &str) -> Result<MergeButtonStateResult, ApiError> {
        match merge_button_state(&self.workspace_root, id).map_err(map_merge_error)? {
            MergeButtonState::Active => Ok(MergeButtonStateResult {
                enabled: true,
                reason: None,
                action: Some("merge".to_string()),
            }),
            MergeButtonState::Blocked { reason } => Ok(MergeButtonStateResult {
                enabled: false,
                reason: Some(reason),
                action: Some("wait".to_string()),
            }),
        }
    }
    pub fn trigger_merge_task(
        &self,
        params: LoopTriggerMergeTaskParams,
        tasks: &mut TaskDomain,
    ) -> Result<TriggerMergeTaskResult, ApiError> {
        let loop_info = self
            .list(LoopListParams {
                include_terminal: Some(true),
            })?
            .into_iter()
            .find(|loop_info| loop_info.id == params.loop_id)
            .ok_or_else(|| loop_not_found_error(&params.loop_id))?;

        if loop_info.location == "(in-place)" {
            return Err(ApiError::invalid_params(
                "Cannot trigger merge for in-place loop (primary)",
            ));
        }

        let loop_prompt = loop_info
            .prompt
            .clone()
            .unwrap_or_else(|| "(no prompt recorded)".to_string());

        let merge_prompt = format!(
            "Merge worktree loop '{}' into main branch.\n\nThe worktree is located at: {}\nOriginal task: {}\n\nInstructions:\n1. Review the commits in the worktree branch\n2. Merge the changes into main branch\n3. Resolve any conflicts if present\n4. Delete the worktree after successful merge",
            params.loop_id, loop_info.location, loop_prompt
        );

        let task_id = format!("merge-{}-{}", params.loop_id, Utc::now().timestamp_millis());
        let task = tasks.create(TaskCreateParams {
            id: task_id,
            title: format!(
                "Merge: {}",
                loop_info
                    .prompt
                    .clone()
                    .unwrap_or_else(|| params.loop_id.clone())
                    .chars()
                    .take(50)
                    .collect::<String>()
            ),
            status: Some("open".to_string()),
            priority: Some(1),
            blocked_by: None,
            auto_execute: Some(true),
            merge_loop_prompt: Some(merge_prompt),
        })?;

        Ok(TriggerMergeTaskResult {
            success: true,
            task_id: task.id,
            queued_task_id: task.queued_task_id,
        })
    }
}
