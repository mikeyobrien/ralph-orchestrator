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
}

#[derive(Debug, Clone)]
pub struct TaskUpdateInput {
    pub id: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<u8>,
    pub blocked_by: Option<Option<String>>,
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
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
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
        let unblocking_ids = self.unblocking_ids();
        let mut tasks: Vec<_> = self
            .tasks
            .values()
            .filter(|task| task.status == "ready" && task.archived_at.is_none())
            .filter(|task| {
                task.blocked_by
                    .as_ref()
                    .is_none_or(|blocker_id| unblocking_ids.contains(blocker_id))
            })
            .cloned()
            .collect();

        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        tasks
    }

    pub fn create(&mut self, params: TaskCreateParams) -> Result<TaskRecord, ApiError> {
        if self.tasks.contains_key(&params.id) {
            return Err(
                ApiError::conflict(format!("Task with id '{}' already exists", params.id))
                    .with_details(serde_json::json!({ "taskId": params.id })),
            );
        }

        let requested_status = params.status.unwrap_or_else(|| "ready".to_string());
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
            created_at: now.clone(),
            updated_at: now,
            completed_at,
            error_message: None,
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

            if task.status != "cancelled" {
                return Err(
                    ApiError::precondition_failed("Only cancelled tasks can be retried")
                        .with_details(serde_json::json!({
                            "taskId": id,
                            "status": task.status,
                        })),
                );
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

    pub fn cancel(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| task_not_found_error(id))?;

        if task.status != "in_progress" {
            return Err(
                ApiError::precondition_failed("Only in_progress tasks can be cancelled")
                    .with_details(serde_json::json!({
                        "taskId": id,
                        "status": task.status,
                    })),
            );
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

    fn unblocking_ids(&self) -> HashSet<String> {
        self.tasks
            .values()
            .filter(|task| task.status == "done" || task.archived_at.is_some())
            .map(|task| task.id.clone())
            .collect()
    }

    fn sorted_tasks(&self) -> Vec<TaskRecord> {
        let mut tasks: Vec<_> = self.tasks.values().cloned().collect();
        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        tasks
    }
}

fn task_not_found_error(task_id: &str) -> ApiError {
    ApiError::task_not_found(format!("Task with id '{task_id}' not found"))
        .with_details(serde_json::json!({ "taskId": task_id }))
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "done" | "cancelled")
}
