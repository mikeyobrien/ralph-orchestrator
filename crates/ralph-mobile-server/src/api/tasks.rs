//! Tasks API endpoints.
//!
//! GET /api/tasks - List tasks from `.ralph/agent/tasks.jsonl`
//! POST /api/tasks - Create a new task
//! PUT /api/tasks/{id} - Update task status

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::sessions::ErrorResponse;

/// Response format for task list.
#[derive(Debug, Serialize, Deserialize)]
pub struct TasksResponse {
    pub tasks: Vec<TaskItem>,
    pub total: usize,
}

/// Task item in the response.
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    pub priority: u8,
    pub blocked_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_id: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Request body for POST /api/tasks.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub blocked_by: Vec<String>,
}

fn default_priority() -> u8 {
    3
}

/// Request body for PUT /api/tasks/{id}.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub status: String,
}

/// GET /api/tasks - List tasks from `.ralph/agent/tasks.jsonl`.
///
/// Query parameters:
/// - status: Filter by status (open, in_progress, closed, failed)
pub async fn list_tasks(query: web::Query<std::collections::HashMap<String, String>>) -> impl Responder {
    let tasks_path = get_tasks_path();

    // Load tasks using ralph-core's TaskStore
    let store = match ralph_core::task_store::TaskStore::load(&tasks_path) {
        Ok(store) => store,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to load tasks: {}", e),
            });
        }
    };

    let status_filter = query.get("status").map(|s| s.as_str());

    // Convert tasks to response format
    let mut tasks: Vec<TaskItem> = store
        .all()
        .iter()
        .filter(|task| {
            if let Some(filter) = status_filter {
                task_status_to_string(&task.status).to_lowercase() == filter.to_lowercase()
            } else {
                true
            }
        })
        .map(|task| TaskItem {
            id: task.id.clone(),
            title: task.title.clone(),
            description: task.description.clone(),
            status: task_status_to_string(&task.status),
            priority: task.priority,
            blocked_by: task.blocked_by.clone(),
            loop_id: task.loop_id.clone(),
            created_at: task.created.clone(),
            updated_at: task.closed.clone(),
        })
        .collect();

    let total = tasks.len();

    // Sort by priority (1 = highest) then by created date
    tasks.sort_by(|a, b| {
        a.priority.cmp(&b.priority).then_with(|| b.created_at.cmp(&a.created_at))
    });

    HttpResponse::Ok().json(TasksResponse { tasks, total })
}

/// POST /api/tasks - Create a new task.
pub async fn create_task(body: web::Json<CreateTaskRequest>) -> impl Responder {
    let tasks_path = get_tasks_path();

    // Validate title
    if body.title.trim().is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "title cannot be empty".to_string(),
        });
    }

    // Validate priority (1-5)
    if !(1..=5).contains(&body.priority) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "priority must be between 1 and 5".to_string(),
        });
    }

    // Load store and create task atomically
    let mut store = match ralph_core::task_store::TaskStore::load(&tasks_path) {
        Ok(store) => store,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to load tasks: {}", e),
            });
        }
    };

    let result = store.with_exclusive_lock(|store| {
        let task = ralph_core::task::Task::new(body.title.clone(), body.priority)
            .with_description(body.description.clone());

        let task = if !body.blocked_by.is_empty() {
            body.blocked_by.iter().fold(task, |t, blocker| t.with_blocker(blocker.clone()))
        } else {
            task
        };

        let added_task = store.add(task);

        TaskItem {
            id: added_task.id.clone(),
            title: added_task.title.clone(),
            description: added_task.description.clone(),
            status: task_status_to_string(&added_task.status),
            priority: added_task.priority,
            blocked_by: added_task.blocked_by.clone(),
            loop_id: added_task.loop_id.clone(),
            created_at: added_task.created.clone(),
            updated_at: added_task.closed.clone(),
        }
    });

    match result {
        Ok(task) => HttpResponse::Created().json(task),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to create task: {}", e),
        }),
    }
}

/// PUT /api/tasks/{id} - Update task status.
pub async fn update_task(
    path: web::Path<String>,
    body: web::Json<UpdateTaskRequest>,
) -> impl Responder {
    let task_id = path.into_inner();
    let tasks_path = get_tasks_path();

    // Validate status
    let valid_statuses = ["open", "in_progress", "closed", "failed"];
    if !valid_statuses.contains(&body.status.to_lowercase().as_str()) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!(
                "invalid status '{}', must be one of: {}",
                body.status,
                valid_statuses.join(", ")
            ),
        });
    }

    // Load store and update task atomically
    let mut store = match ralph_core::task_store::TaskStore::load(&tasks_path) {
        Ok(store) => store,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to load tasks: {}", e),
            });
        }
    };

    let result = store.with_exclusive_lock(|store| {
        // Check if task exists
        let task = store.get_mut(&task_id);
        if task.is_none() {
            return Err("task_not_found");
        }

        // Update status
        let new_status = match body.status.to_lowercase().as_str() {
            "open" => ralph_core::task::TaskStatus::Open,
            "in_progress" => ralph_core::task::TaskStatus::InProgress,
            "closed" => ralph_core::task::TaskStatus::Closed,
            "failed" => ralph_core::task::TaskStatus::Failed,
            _ => return Err("invalid_status"),
        };

        let task = task.unwrap();
        task.status = new_status;

        // Set closed timestamp if closing or failing
        if matches!(new_status, ralph_core::task::TaskStatus::Closed | ralph_core::task::TaskStatus::Failed) {
            task.closed = Some(chrono::Utc::now().to_rfc3339());
        }

        Ok(TaskItem {
            id: task.id.clone(),
            title: task.title.clone(),
            description: task.description.clone(),
            status: task_status_to_string(&task.status),
            priority: task.priority,
            blocked_by: task.blocked_by.clone(),
            loop_id: task.loop_id.clone(),
            created_at: task.created.clone(),
            updated_at: task.closed.clone(),
        })
    });

    match result {
        Ok(Ok(task)) => HttpResponse::Ok().json(task),
        Ok(Err("task_not_found")) => HttpResponse::NotFound().json(ErrorResponse {
            error: "task_not_found".to_string(),
        }),
        Ok(Err(e)) => HttpResponse::BadRequest().json(ErrorResponse {
            error: e.to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to update task: {}", e),
        }),
    }
}

/// Get the path to the tasks.jsonl file.
///
/// Looks for `.ralph/agent/tasks.jsonl` in the current directory first,
/// then falls back to home directory.
fn get_tasks_path() -> PathBuf {
    get_tasks_path_with_base(None)
}

/// Get the path to the tasks.jsonl file with optional base directory override.
///
/// Used for testing to override the HOME directory lookup.
fn get_tasks_path_with_base(base_dir: Option<PathBuf>) -> PathBuf {
    let cwd_path = PathBuf::from(".ralph/agent/tasks.jsonl");
    if cwd_path.parent().map_or(false, |p| p.exists()) {
        return cwd_path;
    }

    // Use base_dir if provided (for testing), otherwise fall back to home directory
    if let Some(base) = base_dir {
        base.join(".ralph/agent/tasks.jsonl")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".ralph/agent/tasks.jsonl")
    } else {
        cwd_path
    }
}

/// Helper function to convert TaskStatus to string.
fn task_status_to_string(status: &ralph_core::task::TaskStatus) -> String {
    match status {
        ralph_core::task::TaskStatus::Open => "open".to_string(),
        ralph_core::task::TaskStatus::InProgress => "in_progress".to_string(),
        ralph_core::task::TaskStatus::Closed => "closed".to_string(),
        ralph_core::task::TaskStatus::Failed => "failed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit tests for task status conversion.
    #[test]
    fn test_task_status_to_string() {
        assert_eq!(task_status_to_string(&ralph_core::task::TaskStatus::Open), "open");
        assert_eq!(task_status_to_string(&ralph_core::task::TaskStatus::InProgress), "in_progress");
        assert_eq!(task_status_to_string(&ralph_core::task::TaskStatus::Closed), "closed");
        assert_eq!(task_status_to_string(&ralph_core::task::TaskStatus::Failed), "failed");
    }

    /// Test path resolution with base directory.
    #[test]
    fn test_get_tasks_path_with_base() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let path = get_tasks_path_with_base(Some(tmp.path().to_path_buf()));
        assert_eq!(path, tmp.path().join(".ralph/agent/tasks.jsonl"));
    }

    /// Test TaskItem serialization.
    #[test]
    fn test_task_item_serialization() {
        let task = TaskItem {
            id: "test-id".to_string(),
            title: "Test task".to_string(),
            description: Some("Test description".to_string()),
            status: "open".to_string(),
            priority: 2,
            blocked_by: vec!["blocker-1".to_string()],
            loop_id: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: None,
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"title\":\"Test task\""));
        assert!(json.contains("\"priority\":2"));
        // Ensure optional None fields are skipped
        assert!(!json.contains("loop_id"));
        assert!(!json.contains("updated_at"));
    }

    /// Test CreateTaskRequest defaults.
    #[test]
    fn test_create_task_request_defaults() {
        let json = r#"{"title": "Test"}"#;
        let req: CreateTaskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Test");
        assert_eq!(req.priority, 3); // default priority
        assert!(req.blocked_by.is_empty());
        assert!(req.description.is_none());
    }

    /// Test TasksResponse structure.
    #[test]
    fn test_tasks_response_serialization() {
        let response = TasksResponse {
            tasks: vec![],
            total: 0,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"tasks\":[]"));
        assert!(json.contains("\"total\":0"));
    }

    /// Test UpdateTaskRequest parsing.
    #[test]
    fn test_update_task_request_parsing() {
        let json = r#"{"status": "in_progress"}"#;
        let req: UpdateTaskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.status, "in_progress");
    }

    /// Test TaskStore integration - create and retrieve tasks.
    #[test]
    fn test_task_store_integration() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let tasks_path = tmp.path().join(".ralph/agent/tasks.jsonl");
        std::fs::create_dir_all(tasks_path.parent().unwrap()).unwrap();

        // Create store and add tasks
        let mut store = ralph_core::task_store::TaskStore::load(&tasks_path).unwrap();

        let task1 = ralph_core::task::Task::new("Task 1".to_string(), 1);
        let task1_id = task1.id.clone();
        store.add(task1);

        let mut task2 = ralph_core::task::Task::new("Task 2".to_string(), 2);
        task2.status = ralph_core::task::TaskStatus::Closed;
        store.add(task2);

        store.save().unwrap();

        // Reload and verify
        let store2 = ralph_core::task_store::TaskStore::load(&tasks_path).unwrap();
        assert_eq!(store2.all().len(), 2);

        // Verify first task
        let loaded_task = store2.all().iter().find(|t| t.id == task1_id).unwrap();
        assert_eq!(loaded_task.title, "Task 1");
        assert_eq!(loaded_task.priority, 1);
    }
}
