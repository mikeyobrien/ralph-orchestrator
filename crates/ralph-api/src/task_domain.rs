use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;

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
    pub queued_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_loop_prompt: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunResult {
    pub success: bool,
    pub queued_task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskRecord>,
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

pub struct TaskDomain {
    store_path: PathBuf,
    tasks: BTreeMap<String, TaskRecord>,
    queue_counter: u64,
}

impl TaskDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let store_path = workspace_root.as_ref().join(".ralph/api/tasks-v1.json");
        let mut domain = Self {
            store_path,
            tasks: BTreeMap::new(),
            queue_counter: 0,
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
            .filter(|task| task.status == "open" && task.archived_at.is_none())
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

        let requested_status = params.status.unwrap_or_else(|| "open".to_string());
        let auto_execute = params.auto_execute.unwrap_or(true);

        if auto_execute && requested_status != "open" {
            return Err(ApiError::invalid_params(
                "task.create autoExecute=true is only valid when status is 'open'",
            )
            .with_details(serde_json::json!({
                "taskId": params.id,
                "status": requested_status.clone(),
                "autoExecute": auto_execute,
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
            queued_task_id: None,
            merge_loop_prompt: params.merge_loop_prompt,
            created_at: now.clone(),
            updated_at: now,
            completed_at,
            error_message: None,
        };

        let task_id = task.id.clone();
        self.tasks.insert(task_id.clone(), task);

        let should_auto_execute = auto_execute
            && self
                .tasks
                .get(&task_id)
                .is_some_and(|task| task.blocked_by.is_none() && task.status == "open");

        if should_auto_execute {
            let _ = self.run(&task_id)?;
        } else {
            self.persist()?;
        }

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
                task.queued_task_id = None;
            } else {
                task.completed_at = None;
                if !matches!(task.status.as_str(), "pending" | "running") {
                    task.queued_task_id = None;
                }
            }

            if task.status != "failed" {
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
        self.transition_task(id, "closed")
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

        if !matches!(task.status.as_str(), "failed" | "closed") {
            return Err(ApiError::precondition_failed(format!(
                "Cannot delete task in '{}' state. Only failed or closed tasks can be deleted.",
                task.status
            ))
            .with_details(serde_json::json!({
                "taskId": id,
                "status": task.status,
                "allowedStatuses": ["failed", "closed"]
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

    pub fn run(&mut self, id: &str) -> Result<TaskRunResult, ApiError> {
        let queued_task_id = self.queue_task(id)?;
        Ok(TaskRunResult {
            success: true,
            queued_task_id,
            task: Some(self.get(id)?),
        })
    }

    pub fn run_all(&mut self) -> TaskRunAllResult {
        let ready_task_ids: Vec<String> = self.ready().into_iter().map(|task| task.id).collect();
        let mut enqueued = 0_u64;
        let mut errors = Vec::new();

        for task_id in ready_task_ids {
            match self.queue_task(&task_id) {
                Ok(_) => {
                    enqueued = enqueued.saturating_add(1);
                }
                Err(error) => {
                    errors.push(format!("{task_id}: {}", error.message));
                }
            }
        }

        TaskRunAllResult { enqueued, errors }
    }

    pub fn retry(&mut self, id: &str) -> Result<TaskRunResult, ApiError> {
        {
            let task = self
                .tasks
                .get_mut(id)
                .ok_or_else(|| task_not_found_error(id))?;

            if task.status != "failed" {
                return Err(
                    ApiError::precondition_failed("Only failed tasks can be retried").with_details(
                        serde_json::json!({
                            "taskId": id,
                            "status": task.status,
                        }),
                    ),
                );
            }

            let now = now_ts();
            task.status = "open".to_string();
            task.queued_task_id = None;
            task.completed_at = None;
            task.error_message = None;
            task.updated_at = now;
        }

        self.run(id)
    }

    pub fn cancel(&mut self, id: &str) -> Result<TaskRecord, ApiError> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| task_not_found_error(id))?;

        if !matches!(task.status.as_str(), "pending" | "running") {
            return Err(ApiError::precondition_failed(
                "Only running or pending tasks can be cancelled",
            )
            .with_details(serde_json::json!({
                "taskId": id,
                "status": task.status,
            })));
        }

        let now = now_ts();
        task.status = "failed".to_string();
        task.completed_at = Some(now.clone());
        task.updated_at = now;
        task.error_message = Some("Task cancelled by user".to_string());
        task.queued_task_id = None;

        self.persist()?;
        self.get(id)
    }

    pub fn status(&self, id: &str) -> TaskStatusResult {
        let Some(task) = self.tasks.get(id) else {
            return TaskStatusResult {
                is_queued: false,
                queue_position: None,
                runner_pid: None,
            };
        };

        let is_queued =
            task.queued_task_id.is_some() && matches!(task.status.as_str(), "pending" | "running");

        let queue_position = if is_queued {
            self.queue_position(id)
        } else {
            None
        };

        let runner_pid = if task.status == "running" {
            Some(std::process::id())
        } else {
            None
        };

        TaskStatusResult {
            is_queued,
            queue_position,
            runner_pid,
        }
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
            task.queued_task_id = None;
        } else {
            task.completed_at = None;
            if !matches!(status, "pending" | "running") {
                task.queued_task_id = None;
            }
        }

        if status != "failed" {
            task.error_message = None;
        }

        self.persist()?;
        self.get(id)
    }

    fn queue_task(&mut self, id: &str) -> Result<String, ApiError> {
        let queued_task_id = self.next_queued_task_id();
        let now = now_ts();

        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| task_not_found_error(id))?;

        if task.archived_at.is_some() {
            return Err(
                ApiError::precondition_failed("Cannot run archived task").with_details(
                    serde_json::json!({
                        "taskId": id,
                    }),
                ),
            );
        }

        if matches!(task.status.as_str(), "pending" | "running") {
            return Err(
                ApiError::precondition_failed("Task is already queued or running").with_details(
                    serde_json::json!({
                        "taskId": id,
                        "status": task.status
                    }),
                ),
            );
        }

        task.status = "pending".to_string();
        task.queued_task_id = Some(queued_task_id.clone());
        task.completed_at = None;
        task.error_message = None;
        task.updated_at = now;
        self.persist()?;

        Ok(queued_task_id)
    }

    fn queue_position(&self, id: &str) -> Option<u64> {
        let mut queued: Vec<&TaskRecord> = self
            .tasks
            .values()
            .filter(|task| {
                task.queued_task_id.is_some()
                    && matches!(task.status.as_str(), "pending" | "running")
            })
            .collect();
        queued.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));

        queued
            .iter()
            .position(|task| task.id == id)
            .map(|index| index as u64)
    }

    fn unblocking_ids(&self) -> HashSet<String> {
        self.tasks
            .values()
            .filter(|task| task.status == "closed" || task.archived_at.is_some())
            .map(|task| task.id.clone())
            .collect()
    }

    fn next_queued_task_id(&mut self) -> String {
        self.queue_counter = self.queue_counter.saturating_add(1);
        format!(
            "queued-{}-{:04x}",
            Utc::now().timestamp_millis(),
            self.queue_counter
        )
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
    matches!(status, "closed" | "failed")
}

fn now_ts() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}
