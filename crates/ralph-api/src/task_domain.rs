use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use ralph_core::task::{Task, TaskStatus};
use ralph_core::task_store::TaskStore;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::errors::ApiError;
use crate::loop_support::now_ts;

// ---------------------------------------------------------------------------
// Request / filter types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListParams {
    pub status: Option<String>,
    pub loop_id: Option<String>,
    pub hat: Option<String>,
    pub priority: Option<u8>,
    pub tag: Option<String>,
    pub include_archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreateParams {
    pub id: String,
    pub title: String,
    pub status: Option<String>,
    pub priority: Option<u8>,
    pub blocked_by: Option<String>,
    pub auto_execute: Option<bool>,
    pub merge_loop_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskUpdateInput {
    pub id: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<u8>,
    pub blocked_by: Option<Option<String>>,
}

// ---------------------------------------------------------------------------
// API-only sidecar metadata (not stored in core TaskStore)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiTaskMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_loop_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// API-layer execution status override (e.g. "pending", "running")
    /// When set, overrides the core task status in API responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_status: Option<String>,
}

// ---------------------------------------------------------------------------
// Response types (camelCase for JSON API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub status: String,
    pub priority: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_hat: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<StatusTransitionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_context: Option<serde_json::Value>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    // API-only fields from sidecar
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queued_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_loop_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusTransitionResponse {
    pub from: String,
    pub to: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hat: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunResult {
    pub success: bool,
    pub queued_task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunAllResult {
    pub enqueued: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusResult {
    pub is_queued: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner_pid: Option<u32>,
}

// ---------------------------------------------------------------------------
// TaskDomain — wraps core TaskStore + API sidecar metadata
// ---------------------------------------------------------------------------

pub struct TaskDomain {
    workspace_root: PathBuf,
    queue_counter: u64,
    api_meta: BTreeMap<String, ApiTaskMeta>,
    meta_path: PathBuf,
}

impl TaskDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        let meta_path = root.join(".ralph/api/task-meta.json");
        let mut domain = Self {
            workspace_root: root,
            queue_counter: 0,
            api_meta: BTreeMap::new(),
            meta_path,
        };
        domain.load_meta();
        domain
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn store_path(&self) -> PathBuf {
        self.workspace_root.join(".ralph/agent/tasks.jsonl")
    }

    fn load_store(&self) -> Result<TaskStore, ApiError> {
        TaskStore::load(&self.store_path())
            .map_err(|e| ApiError::internal(format!("failed to load task store: {e}")))
    }

    fn task_to_response(&self, task: &Task) -> TaskResponse {
        let meta = self.api_meta.get(&task.id);
        let transitions = task
            .transitions
            .iter()
            .map(|t| StatusTransitionResponse {
                from: status_to_string(t.from),
                to: status_to_string(t.to),
                timestamp: t.timestamp.clone(),
                hat: t.hat.clone(),
            })
            .collect();

        // Use execution_status override if present (for pending/running)
        let status = meta
            .and_then(|m| m.execution_status.clone())
            .unwrap_or_else(|| status_to_string(task.status));

        TaskResponse {
            id: task.id.clone(),
            title: task.title.clone(),
            description: task.description.clone(),
            key: task.key.clone(),
            status,
            priority: task.priority,
            blocked_by: task.blocked_by.clone(),
            loop_id: task.loop_id.clone(),
            last_hat: task.last_hat.clone(),
            tags: task.tags.clone(),
            transitions,
            loop_context: None, // Step 7
            created_at: task.created.clone(),
            updated_at: Some(task.closed.clone().unwrap_or_else(|| task.created.clone())),
            started_at: task.started.clone(),
            completed_at: task.closed.clone(),
            archived_at: meta.and_then(|m| m.archived_at.clone()),
            queued_task_id: meta.and_then(|m| m.queued_task_id.clone()),
            merge_loop_prompt: meta.and_then(|m| m.merge_loop_prompt.clone()),
            error_message: meta.and_then(|m| m.error_message.clone()),
        }
    }

    fn meta_mut(&mut self, id: &str) -> &mut ApiTaskMeta {
        self.api_meta.entry(id.to_string()).or_default()
    }

    fn load_meta(&mut self) {
        if !self.meta_path.exists() {
            return;
        }
        match std::fs::read_to_string(&self.meta_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(parsed) => {
                    let snapshot: MetaSnapshot = parsed;
                    self.api_meta = snapshot.meta;
                    self.queue_counter = snapshot.queue_counter;
                }
                Err(e) => warn!(path = %self.meta_path.display(), %e, "failed parsing task meta"),
            },
            Err(e) => warn!(path = %self.meta_path.display(), %e, "failed reading task meta"),
        }
    }

    fn persist_meta(&self) -> Result<(), ApiError> {
        if let Some(parent) = self.meta_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ApiError::internal(format!(
                    "failed to create meta directory '{}': {e}",
                    parent.display()
                ))
            })?;
        }
        let snapshot = MetaSnapshot {
            meta: self.api_meta.clone(),
            queue_counter: self.queue_counter,
        };
        let payload = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| ApiError::internal(format!("failed to serialize task meta: {e}")))?;
        std::fs::write(&self.meta_path, payload).map_err(|e| {
            ApiError::internal(format!(
                "failed to write task meta '{}': {e}",
                self.meta_path.display()
            ))
        })
    }

    // -----------------------------------------------------------------------
    // Read methods (sub-task 6.1)
    // -----------------------------------------------------------------------

    pub fn list(&self, params: TaskListParams) -> Result<Vec<TaskResponse>, ApiError> {
        let store = self.load_store()?;
        let status = params.status.as_deref().and_then(parse_task_status);

        let filtered = store.filter(
            status,
            params.loop_id.as_deref(),
            params.hat.as_deref(),
            params.priority,
            params.tag.as_deref(),
        );

        let include_archived = params.include_archived.unwrap_or(false);
        let tasks = filtered
            .into_iter()
            .map(|t| self.task_to_response(t))
            .filter(|r| include_archived || r.archived_at.is_none())
            .collect();

        Ok(tasks)
    }

    pub fn get(&self, id: &str) -> Result<TaskResponse, ApiError> {
        let store = self.load_store()?;
        let task = store.get(id).ok_or_else(|| task_not_found_error(id))?;
        Ok(self.task_to_response(task))
    }

    pub fn ready(&self) -> Result<Vec<TaskResponse>, ApiError> {
        let store = self.load_store()?;
        let tasks = store
            .ready()
            .into_iter()
            .filter(|t| {
                self.api_meta
                    .get(&t.id)
                    .and_then(|m| m.archived_at.as_ref())
                    .is_none()
            })
            .map(|t| self.task_to_response(t))
            .collect();
        Ok(tasks)
    }

    // -----------------------------------------------------------------------
    // Mutation methods (kept compiling; full rewrite in sub-task 6.2)
    // -----------------------------------------------------------------------

    pub fn create(&mut self, params: TaskCreateParams) -> Result<TaskResponse, ApiError> {
        let requested_status = params.status.as_deref().unwrap_or("open");
        let auto_execute = params.auto_execute.unwrap_or(true);

        if auto_execute && requested_status != "open" {
            return Err(ApiError::invalid_params(
                "task.create autoExecute=true is only valid when status is 'open'",
            ));
        }

        let target_status = parse_task_status(requested_status).ok_or_else(|| {
            ApiError::invalid_params(format!("invalid task status: '{requested_status}'"))
        })?;

        let mut task = Task::new(params.title, params.priority.unwrap_or(2));
        task.id = params.id.clone();
        if let Some(ref blocker) = params.blocked_by {
            task.blocked_by = vec![blocker.clone()];
        }

        // Store API-only metadata in sidecar
        if params.merge_loop_prompt.is_some() {
            self.meta_mut(&params.id).merge_loop_prompt = params.merge_loop_prompt;
        }

        let should_auto_execute =
            auto_execute && task.blocked_by.is_empty() && target_status == TaskStatus::Open;

        let mut store = self.load_store()?;
        store
            .with_exclusive_lock(|s| -> Result<(), ApiError> {
                if s.get(&params.id).is_some() {
                    return Err(ApiError::conflict(format!(
                        "Task with id '{}' already exists",
                        params.id
                    ))
                    .with_details(serde_json::json!({ "taskId": params.id })));
                }
                s.add(task);
                // Use transition() for non-Open initial status to record the change
                if target_status != TaskStatus::Open {
                    s.transition(&params.id, target_status, None);
                }
                Ok(())
            })
            .map_err(|e| ApiError::internal(format!("failed to save task: {e}")))??;

        if should_auto_execute {
            let result = self.run(&params.id)?;
            return Ok(result
                .task
                .unwrap_or_else(|| self.get(&params.id).expect("just created")));
        }

        self.persist_meta()?;
        self.get(&params.id)
    }

    pub fn update(&mut self, input: TaskUpdateInput) -> Result<TaskResponse, ApiError> {
        let new_status = input
            .status
            .as_deref()
            .map(|s| {
                parse_task_status(s)
                    .ok_or_else(|| ApiError::invalid_params(format!("invalid task status: '{s}'")))
            })
            .transpose()?;

        let mut store = self.load_store()?;
        store
            .with_exclusive_lock(|s| -> Result<(), ApiError> {
                // Verify task exists before any mutations
                if s.get(&input.id).is_none() {
                    return Err(task_not_found_error(&input.id));
                }

                // Use transition() for status changes to record transitions properly
                if let Some(status) = new_status {
                    s.transition(&input.id, status, None);
                }

                let task = s
                    .get_mut(&input.id)
                    .ok_or_else(|| task_not_found_error(&input.id))?;

                if let Some(ref title) = input.title {
                    task.title.clone_from(title);
                }
                if let Some(priority) = input.priority {
                    task.priority = priority.clamp(1, 5);
                }
                if let Some(ref blocked_by) = input.blocked_by {
                    task.blocked_by = blocked_by.iter().cloned().collect();
                }
                Ok(())
            })
            .map_err(|e| ApiError::internal(format!("failed to update task: {e}")))??;

        // Clear sidecar fields based on status transition
        if let Some(ref status_str) = input.status {
            if status_str != "failed" {
                if let Some(meta) = self.api_meta.get_mut(&input.id) {
                    meta.error_message = None;
                }
            }
            if !matches!(status_str.as_str(), "pending" | "running") {
                if let Some(meta) = self.api_meta.get_mut(&input.id) {
                    meta.queued_task_id = None;
                    meta.execution_status = None;
                }
            }
            self.persist_meta()?;
        }

        self.get(&input.id)
    }

    pub fn close(&mut self, id: &str) -> Result<TaskResponse, ApiError> {
        let mut store = self.load_store()?;
        store
            .with_exclusive_lock(|s| {
                s.close(id)
                    .ok_or_else(|| task_not_found_error(id))
                    .map(|_| ())
            })
            .map_err(|e| ApiError::internal(format!("failed to close task: {e}")))??;

        if let Some(meta) = self.api_meta.get_mut(id) {
            meta.queued_task_id = None;
            meta.error_message = None;
            meta.execution_status = None;
        }
        self.persist_meta()?;
        self.get(id)
    }

    pub fn archive(&mut self, id: &str) -> Result<TaskResponse, ApiError> {
        let _ = self.get(id)?;
        self.meta_mut(id).archived_at = Some(now_ts());
        self.persist_meta()?;
        self.get(id)
    }

    pub fn unarchive(&mut self, id: &str) -> Result<TaskResponse, ApiError> {
        let _ = self.get(id)?;
        if let Some(meta) = self.api_meta.get_mut(id) {
            meta.archived_at = None;
        }
        self.persist_meta()?;
        self.get(id)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), ApiError> {
        let resp = self.get(id)?;
        if !is_terminal_status(&resp.status) {
            return Err(ApiError::precondition_failed(format!(
                "Cannot delete task in '{}' state. Only failed or closed tasks can be deleted.",
                resp.status
            )));
        }

        let mut store = self.load_store()?;
        let id_owned = id.to_string();
        store
            .with_exclusive_lock(|s| {
                s.remove(&id_owned);
            })
            .map_err(|e| ApiError::internal(format!("failed to delete task: {e}")))?;

        self.api_meta.remove(id);
        self.persist_meta()
    }

    pub fn clear(&mut self) -> Result<(), ApiError> {
        let mut store = self.load_store()?;
        store
            .with_exclusive_lock(|s| {
                s.clear();
            })
            .map_err(|e| ApiError::internal(format!("failed to clear tasks: {e}")))?;

        self.api_meta.clear();
        self.persist_meta()
    }

    // -----------------------------------------------------------------------
    // Queue / execution methods (kept compiling; full rewrite in sub-task 6.3)
    // -----------------------------------------------------------------------

    pub fn run(&mut self, id: &str) -> Result<TaskRunResult, ApiError> {
        let queued_task_id = self.queue_task(id)?;
        let task = self.get(id).ok();
        Ok(TaskRunResult {
            success: true,
            queued_task_id,
            task,
        })
    }

    pub fn run_all(&mut self) -> Result<TaskRunAllResult, ApiError> {
        let ready_ids: Vec<String> = self.ready()?.into_iter().map(|t| t.id).collect();
        let mut enqueued = 0_u64;
        let mut errors = Vec::new();

        for task_id in ready_ids {
            match self.queue_task(&task_id) {
                Ok(_) => enqueued = enqueued.saturating_add(1),
                Err(e) => errors.push(format!("{task_id}: {}", e.message)),
            }
        }

        Ok(TaskRunAllResult { enqueued, errors })
    }

    pub fn retry(&mut self, id: &str) -> Result<TaskRunResult, ApiError> {
        let resp = self.get(id)?;
        if resp.status != "failed" {
            return Err(ApiError::precondition_failed(
                "Only failed tasks can be retried",
            ));
        }

        let mut store = self.load_store()?;
        store
            .with_exclusive_lock(|s| {
                if let Some(task) = s.get_mut(id) {
                    task.status = TaskStatus::Open;
                    task.closed = None;
                }
            })
            .map_err(|e| ApiError::internal(format!("failed to retry task: {e}")))?;

        if let Some(meta) = self.api_meta.get_mut(id) {
            meta.queued_task_id = None;
            meta.error_message = None;
            meta.execution_status = None;
        }

        self.run(id)
    }

    pub fn cancel(&mut self, id: &str) -> Result<TaskResponse, ApiError> {
        let resp = self.get(id)?;
        if !matches!(resp.status.as_str(), "pending" | "running") {
            return Err(ApiError::precondition_failed(
                "Only running or pending tasks can be cancelled",
            ));
        }

        let mut store = self.load_store()?;
        store
            .with_exclusive_lock(|s| {
                s.fail(id);
            })
            .map_err(|e| ApiError::internal(format!("failed to cancel task: {e}")))?;

        let meta = self.meta_mut(id);
        meta.error_message = Some("Task cancelled by user".to_string());
        meta.queued_task_id = None;
        meta.execution_status = None;
        self.persist_meta()?;
        self.get(id)
    }

    pub fn status(&self, id: &str) -> Result<TaskStatusResult, ApiError> {
        let meta = self.api_meta.get(id);
        let has_queue_id = meta.and_then(|m| m.queued_task_id.as_ref()).is_some();

        let task_resp = self.get(id).ok();
        let task_status = task_resp.as_ref().map(|r| r.status.as_str());

        // A task is queued if it has a queue ID and is not in a terminal state
        let is_queued =
            has_queue_id && !task_status.is_some_and(|s| matches!(s, "closed" | "failed"));

        let runner_pid = if is_queued {
            Some(std::process::id())
        } else {
            None
        };

        Ok(TaskStatusResult {
            is_queued,
            queue_position: None, // Simplified; full impl in 6.3
            runner_pid,
        })
    }

    fn queue_task(&mut self, id: &str) -> Result<String, ApiError> {
        let resp = self.get(id)?;

        if resp.archived_at.is_some() {
            return Err(ApiError::precondition_failed("Cannot run archived task"));
        }
        if matches!(resp.status.as_str(), "pending" | "running") {
            return Err(ApiError::precondition_failed(
                "Task is already queued or running",
            ));
        }

        let queued_task_id = self.next_queued_task_id();

        let meta = self.meta_mut(id);
        meta.queued_task_id = Some(queued_task_id.clone());
        meta.execution_status = Some("pending".to_string());
        meta.error_message = None;
        self.persist_meta()?;

        Ok(queued_task_id)
    }

    fn next_queued_task_id(&mut self) -> String {
        self.queue_counter = self.queue_counter.saturating_add(1);
        format!(
            "queued-{}-{:04x}",
            Utc::now().timestamp_millis(),
            self.queue_counter
        )
    }
}

// ---------------------------------------------------------------------------
// Sidecar persistence format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MetaSnapshot {
    #[serde(default)]
    meta: BTreeMap<String, ApiTaskMeta>,
    #[serde(default)]
    queue_counter: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn task_not_found_error(task_id: &str) -> ApiError {
    ApiError::task_not_found(format!("Task with id '{task_id}' not found"))
        .with_details(serde_json::json!({ "taskId": task_id }))
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "closed" | "failed")
}

fn parse_task_status(s: &str) -> Option<TaskStatus> {
    match s {
        "open" => Some(TaskStatus::Open),
        "in_progress" => Some(TaskStatus::InProgress),
        "closed" => Some(TaskStatus::Closed),
        "failed" => Some(TaskStatus::Failed),
        "blocked" => Some(TaskStatus::Blocked),
        "in_review" => Some(TaskStatus::InReview),
        _ => None,
    }
}

fn status_to_string(status: TaskStatus) -> String {
    match status {
        TaskStatus::Open => "open".to_string(),
        TaskStatus::InProgress => "in_progress".to_string(),
        TaskStatus::Closed => "closed".to_string(),
        TaskStatus::Failed => "failed".to_string(),
        TaskStatus::Blocked => "blocked".to_string(),
        TaskStatus::InReview => "in_review".to_string(),
    }
}
