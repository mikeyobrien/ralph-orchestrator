use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::ApiError;
use crate::loop_support::now_ts;

mod storage;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListParams {
    pub status: Option<String>,
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
    pub merge_loop_prompt: Option<String>,
    pub assignee_worker_id: Option<String>,
    pub claimed_at: Option<String>,
    pub lease_expires_at: Option<String>,
    pub scope_files: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct TaskUpdateInput {
    pub id: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<u8>,
    pub blocked_by: Option<Option<String>>,
    pub assignee_worker_id: Option<Option<String>>,
    pub claimed_at: Option<Option<String>>,
    pub lease_expires_at: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_loop_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_worker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_files: Vec<String>,
}

pub struct TaskDomain {
    store_path: PathBuf,
    tasks: BTreeMap<String, TaskRecord>,
}

impl TaskDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let store_path = workspace_root.as_ref().join(".ralph/api/tasks-v1.json");
        let mut domain = Self {
            store_path,
            tasks: BTreeMap::new(),
        };
        domain.load();
        domain
    }

    pub fn list(&self, params: TaskListParams) -> Vec<TaskRecord> {
        let include_archived = params.include_archived.unwrap_or(false);
        let mut tasks = self.sorted_tasks();

        if let Some(status) = params.status {
            tasks.retain(|task| task.status == status);
        }

        if !include_archived {
            tasks.retain(|task| task.archived_at.is_none());
        }

        tasks
    }

    pub fn get(&self, id: &str) -> Result<TaskRecord, ApiError> {
        self.tasks
            .get(id)
            .cloned()
            .ok_or_else(|| task_not_found_error(id))
    }

    pub fn ready(&self) -> Vec<TaskRecord> {
        Self::ready_from_tasks(&self.tasks)
    }

    pub fn create(&mut self, params: TaskCreateParams) -> Result<TaskRecord, ApiError> {
        if self.tasks.contains_key(&params.id) {
            return Err(
                ApiError::conflict(format!("Task with id '{}' already exists", params.id))
                    .with_details(serde_json::json!({ "taskId": params.id })),
            );
        }

        let requested_status = params.status.unwrap_or_else(|| "ready".to_string());

        if !VALID_CREATION_STATUSES.contains(&requested_status.as_str()) {
            return Err(ApiError::invalid_params(format!(
                "Invalid initial status '{}'. Tasks can only be created with status: {}",
                requested_status,
                VALID_CREATION_STATUSES.join(", ")
            ))
            .with_details(serde_json::json!({
                "requestedStatus": requested_status,
                "allowedStatuses": VALID_CREATION_STATUSES,
            })));
        }

        let now = now_ts();
        let completed_at = is_terminal_status(&requested_status).then_some(now.clone());

        let task = TaskRecord {
            id: params.id.clone(),
            title: params.title,
            status: requested_status,
            priority: params.priority.unwrap_or(2).clamp(1, 5),
            blocked_by: params.blocked_by,
            archived_at: None,
            merge_loop_prompt: params.merge_loop_prompt,
            assignee_worker_id: params.assignee_worker_id,
            claimed_at: params.claimed_at,
            lease_expires_at: params.lease_expires_at,
            created_at: now.clone(),
            updated_at: now,
            completed_at,
            error_message: None,
            scope_files: params.scope_files.unwrap_or_default(),
        };

        let task_id = task.id.clone();
        self.tasks.insert(task_id.clone(), task);
        self.persist()?;

        self.get(&task_id)
    }

    pub fn update(&mut self, input: TaskUpdateInput) -> Result<TaskRecord, ApiError> {
        let now = now_ts();
        let task = self
            .tasks
            .get_mut(&input.id)
            .ok_or_else(|| task_not_found_error(&input.id))?;

        if let Some(title) = input.title {
            task.title = title;
        }
        if let Some(status) = input.status {
            if !is_valid_transition(&task.status, &status) {
                let from = task.status.clone();
                let allowed = allowed_targets(&from);
                return Err(ApiError::precondition_failed(format!(
                    "Invalid transition from '{}' to '{}'",
                    from, status
                ))
                .with_details(serde_json::json!({
                    "taskId": input.id,
                    "from": from,
                    "to": status,
                    "allowedTargets": allowed,
                })));
            }
            task.status = status;

            if is_terminal_status(&task.status) {
                task.completed_at = Some(now.clone());
            } else {
                task.completed_at = None;
            }

            if task.status != "cancelled" {
                task.error_message = None;
            }
        }
        if let Some(priority) = input.priority {
            task.priority = priority.clamp(1, 5);
        }
        if let Some(blocked_by) = input.blocked_by {
            task.blocked_by = blocked_by;
        }
        if let Some(assignee_worker_id) = input.assignee_worker_id {
            task.assignee_worker_id = assignee_worker_id;
        }
        if let Some(claimed_at) = input.claimed_at {
            task.claimed_at = claimed_at;
        }
        if let Some(lease_expires_at) = input.lease_expires_at {
            task.lease_expires_at = lease_expires_at;
        }

        task.updated_at = now;
        self.persist()?;
        self.get(&input.id)
    }

    pub fn close(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        self.transition_task(id, "done")
    }

    pub fn archive(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| task_not_found_error(id))?;

        task.archived_at = Some(now_ts());
        task.updated_at = now_ts();
        self.persist()?;
        self.get(id)
    }

    pub fn unarchive(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| task_not_found_error(id))?;

        task.archived_at = None;
        task.updated_at = now_ts();
        self.persist()?;
        self.get(id)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), ApiError> {
        let task = self.tasks.get(id).ok_or_else(|| task_not_found_error(id))?;

        if !matches!(task.status.as_str(), "done" | "cancelled") {
            return Err(ApiError::precondition_failed(format!(
                "Cannot delete task in '{}' state. Only done or cancelled tasks can be deleted.",
                task.status
            ))
            .with_details(serde_json::json!({
                "taskId": id,
                "status": task.status,
                "allowedStatuses": ["done", "cancelled"]
            })));
        }

        self.tasks.remove(id);
        self.persist()?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), ApiError> {
        self.tasks.clear();
        self.persist()?;
        Ok(())
    }

    pub fn retry(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        {
            let task = self
                .tasks
                .get_mut(id)
                .ok_or_else(|| task_not_found_error(id))?;

            if !is_valid_transition(&task.status, "ready") {
                let from = task.status.clone();
                let allowed = allowed_targets(&from);
                return Err(ApiError::precondition_failed(format!(
                    "Invalid transition from '{}' to 'ready'",
                    from
                ))
                .with_details(serde_json::json!({
                    "taskId": id,
                    "from": from,
                    "to": "ready",
                    "allowedTargets": allowed,
                })));
            }

            let now = now_ts();
            task.status = "ready".to_string();
            task.completed_at = None;
            task.error_message = None;
            task.updated_at = now;
        }

        self.persist()?;
        self.get(id)
    }

    pub fn promote(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        self.transition_task(id, "ready")
    }

    pub fn submit_for_review(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        self.transition_task(id, "in_review")
    }

    pub fn request_changes(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        self.transition_task(id, "in_progress")
    }

    pub fn in_review(&self) -> Vec<TaskRecord> {
        let mut tasks: Vec<_> = self
            .tasks
            .values()
            .filter(|t| t.status == "in_review" && t.archived_at.is_none())
            .cloned()
            .collect();
        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        tasks
    }

    pub fn cancel(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| task_not_found_error(id))?;

        if !is_valid_transition(&task.status, "cancelled") {
            let from = task.status.clone();
            let allowed = allowed_targets(&from);
            return Err(ApiError::precondition_failed(format!(
                "Invalid transition from '{}' to 'cancelled'",
                from
            ))
            .with_details(serde_json::json!({
                "taskId": id,
                "from": from,
                "to": "cancelled",
                "allowedTargets": allowed,
            })));
        }

        let now = now_ts();
        task.status = "cancelled".to_string();
        task.completed_at = Some(now.clone());
        task.updated_at = now;
        task.error_message = Some("Task cancelled by user".to_string());

        self.persist()?;
        self.get(id)
    }

    fn transition_task(&mut self, id: &str, status: &str) -> Result<TaskRecord, ApiError> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| task_not_found_error(id))?;

        if !is_valid_transition(&task.status, status) {
            let from = task.status.clone();
            let allowed = allowed_targets(&from);
            return Err(ApiError::precondition_failed(format!(
                "Invalid transition from '{}' to '{}'",
                from, status
            ))
            .with_details(serde_json::json!({
                "taskId": id,
                "from": from,
                "to": status,
                "allowedTargets": allowed,
            })));
        }

        let now = now_ts();
        task.status = status.to_string();
        task.updated_at = now.clone();

        if is_terminal_status(status) {
            task.completed_at = Some(now);
        } else {
            task.completed_at = None;
        }

        if status != "cancelled" {
            task.error_message = None;
        }

        self.persist()?;
        self.get(id)
    }

    fn sorted_tasks(&self) -> Vec<TaskRecord> {
        Self::sorted_tasks_from(&self.tasks)
    }

    pub(crate) fn ready_from_tasks(tasks: &BTreeMap<String, TaskRecord>) -> Vec<TaskRecord> {
        let unblocking_ids = Self::unblocking_ids_from_tasks(tasks);
        let mut ready_tasks: Vec<_> = tasks
            .values()
            .filter(|task| task.status == "ready" && task.archived_at.is_none())
            .filter(|task| {
                task.blocked_by
                    .as_ref()
                    .is_none_or(|blocker_id| unblocking_ids.contains(blocker_id))
            })
            .cloned()
            .collect();

        ready_tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        ready_tasks
    }

    fn unblocking_ids_from_tasks(tasks: &BTreeMap<String, TaskRecord>) -> HashSet<String> {
        tasks
            .values()
            .filter(|task| task.status == "done" || task.archived_at.is_some())
            .map(|task| task.id.clone())
            .collect()
    }

    fn sorted_tasks_from(tasks: &BTreeMap<String, TaskRecord>) -> Vec<TaskRecord> {
        let mut sorted_tasks: Vec<_> = tasks.values().cloned().collect();
        sorted_tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        sorted_tasks
    }
}

fn task_not_found_error(task_id: &str) -> ApiError {
    ApiError::task_not_found(format!("Task with id '{task_id}' not found"))
        .with_details(serde_json::json!({ "taskId": task_id }))
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "done" | "cancelled")
}

/// Returns the list of statuses a task may transition to from `from`.
pub fn allowed_targets(from: &str) -> &'static [&'static str] {
    match from {
        "backlog" => &["ready"],
        "ready" => &["in_progress", "cancelled"],
        "in_progress" => &["in_review", "blocked", "done", "cancelled"],
        "in_review" => &["in_progress", "done", "blocked"],
        "blocked" => &["ready", "cancelled"],
        "cancelled" => &["ready"],
        _ => &[],
    }
}

/// Returns true when transitioning from `from` to `to` is allowed per the
/// canonical board-state machine defined in the spec.
pub fn is_valid_transition(from: &str, to: &str) -> bool {
    allowed_targets(from).contains(&to)
}

/// Statuses allowed when creating a new task.
pub const VALID_CREATION_STATUSES: &[&str] = &["backlog", "ready"];

#[cfg(test)]
mod tests {
    use super::*;

    // ── allowed transitions ──────────────────────────────────────────

    #[test]
    fn backlog_to_ready() {
        assert!(is_valid_transition("backlog", "ready"));
    }

    #[test]
    fn ready_to_in_progress() {
        assert!(is_valid_transition("ready", "in_progress"));
    }

    #[test]
    fn ready_to_cancelled() {
        assert!(is_valid_transition("ready", "cancelled"));
    }

    #[test]
    fn in_progress_to_in_review() {
        assert!(is_valid_transition("in_progress", "in_review"));
    }

    #[test]
    fn in_progress_to_blocked() {
        assert!(is_valid_transition("in_progress", "blocked"));
    }

    #[test]
    fn in_progress_to_done() {
        assert!(is_valid_transition("in_progress", "done"));
    }

    #[test]
    fn in_progress_to_cancelled() {
        assert!(is_valid_transition("in_progress", "cancelled"));
    }

    #[test]
    fn in_review_to_in_progress() {
        assert!(is_valid_transition("in_review", "in_progress"));
    }

    #[test]
    fn in_review_to_done() {
        assert!(is_valid_transition("in_review", "done"));
    }

    #[test]
    fn in_review_to_blocked() {
        assert!(is_valid_transition("in_review", "blocked"));
    }

    #[test]
    fn blocked_to_ready() {
        assert!(is_valid_transition("blocked", "ready"));
    }

    #[test]
    fn blocked_to_cancelled() {
        assert!(is_valid_transition("blocked", "cancelled"));
    }

    #[test]
    fn cancelled_to_ready() {
        assert!(is_valid_transition("cancelled", "ready"));
    }

    // ── forbidden transitions ────────────────────────────────────────

    #[test]
    fn backlog_to_done_rejected() {
        assert!(!is_valid_transition("backlog", "done"));
    }

    #[test]
    fn backlog_to_in_progress_rejected() {
        assert!(!is_valid_transition("backlog", "in_progress"));
    }

    #[test]
    fn ready_to_done_rejected() {
        assert!(!is_valid_transition("ready", "done"));
    }

    #[test]
    fn ready_to_blocked_rejected() {
        assert!(!is_valid_transition("ready", "blocked"));
    }

    #[test]
    fn done_to_anything_rejected() {
        assert!(!is_valid_transition("done", "ready"));
        assert!(!is_valid_transition("done", "in_progress"));
        assert!(!is_valid_transition("done", "cancelled"));
    }

    #[test]
    fn cancelled_to_done_rejected() {
        assert!(!is_valid_transition("cancelled", "done"));
    }

    #[test]
    fn cancelled_to_in_progress_rejected() {
        assert!(!is_valid_transition("cancelled", "in_progress"));
    }

    #[test]
    fn unknown_status_rejected() {
        assert!(!is_valid_transition("unknown", "ready"));
        assert!(!is_valid_transition("ready", "unknown"));
    }

    // ── allowed_targets exhaustive ───────────────────────────────────

    #[test]
    fn allowed_targets_backlog() {
        assert_eq!(allowed_targets("backlog"), &["ready"]);
    }

    #[test]
    fn allowed_targets_ready() {
        assert_eq!(allowed_targets("ready"), &["in_progress", "cancelled"]);
    }

    #[test]
    fn allowed_targets_in_progress() {
        assert_eq!(
            allowed_targets("in_progress"),
            &["in_review", "blocked", "done", "cancelled"]
        );
    }

    #[test]
    fn allowed_targets_in_review() {
        assert_eq!(
            allowed_targets("in_review"),
            &["in_progress", "done", "blocked"]
        );
    }

    #[test]
    fn allowed_targets_blocked() {
        assert_eq!(allowed_targets("blocked"), &["ready", "cancelled"]);
    }

    #[test]
    fn allowed_targets_cancelled() {
        assert_eq!(allowed_targets("cancelled"), &["ready"]);
    }

    #[test]
    fn allowed_targets_done_is_empty() {
        assert!(allowed_targets("done").is_empty());
    }

    #[test]
    fn allowed_targets_unknown_is_empty() {
        assert!(allowed_targets("garbage").is_empty());
    }

    // ── VALID_CREATION_STATUSES ──────────────────────────────────────

    #[test]
    fn creation_statuses_are_backlog_and_ready() {
        assert_eq!(VALID_CREATION_STATUSES, &["backlog", "ready"]);
    }

    #[test]
    fn creation_rejects_in_progress() {
        assert!(!VALID_CREATION_STATUSES.contains(&"in_progress"));
    }

    #[test]
    fn creation_rejects_done() {
        assert!(!VALID_CREATION_STATUSES.contains(&"done"));
    }
}
