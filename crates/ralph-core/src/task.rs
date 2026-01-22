//! Task tracking for Ralph.
//!
//! Lightweight task tracking system inspired by Steve Yegge's Beads.
//! Provides structured task data with JSONL persistence and dependency tracking.

use serde::{Deserialize, Serialize};

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Not started
    Open,
    /// Being worked on
    InProgress,
    /// Complete
    Closed,
}

/// A task in the task tracking system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique ID: task-{unix_timestamp}-{4_hex_chars}
    pub id: String,

    /// Short description
    pub title: String,

    /// Optional detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Current state
    pub status: TaskStatus,

    /// Priority 1-5 (1 = highest)
    pub priority: u8,

    /// Tasks that must complete before this one
    #[serde(default)]
    pub blocked_by: Vec<String>,

    /// Creation timestamp (ISO 8601)
    pub created: String,

    /// Completion timestamp (ISO 8601), if closed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
}

impl Task {
    /// Creates a new task with the given title and priority.
    pub fn new(title: String, priority: u8) -> Self {
        Self {
            id: Self::generate_id(),
            title,
            description: None,
            status: TaskStatus::Open,
            priority: priority.clamp(1, 5),
            blocked_by: Vec::new(),
            created: chrono::Utc::now().to_rfc3339(),
            closed: None,
        }
    }

    /// Generates a unique task ID: task-{timestamp}-{hex_suffix}
    pub fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let timestamp = duration.as_secs();
        let hex_suffix = format!("{:04x}", duration.subsec_micros() % 0x10000);
        format!("task-{}-{}", timestamp, hex_suffix)
    }

    /// Returns true if this task is ready to work on (open + no blockers pending).
    pub fn is_ready(&self, all_tasks: &[Task]) -> bool {
        if self.status != TaskStatus::Open {
            return false;
        }
        self.blocked_by.iter().all(|blocker_id| {
            all_tasks
                .iter()
                .find(|t| &t.id == blocker_id)
                .is_some_and(|t| t.status == TaskStatus::Closed)
        })
    }

    /// Sets the description of the task.
    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    /// Adds a blocker task ID.
    pub fn with_blocker(mut self, task_id: String) -> Self {
        self.blocked_by.push(task_id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("Test task".to_string(), 2);
        assert_eq!(task.title, "Test task");
        assert_eq!(task.priority, 2);
        assert_eq!(task.status, TaskStatus::Open);
        assert!(task.blocked_by.is_empty());
    }

    #[test]
    fn test_priority_clamping() {
        let task_low = Task::new("Low".to_string(), 0);
        assert_eq!(task_low.priority, 1);

        let task_high = Task::new("High".to_string(), 10);
        assert_eq!(task_high.priority, 5);
    }

    #[test]
    fn test_task_id_format() {
        let task = Task::new("Test".to_string(), 1);
        assert!(task.id.starts_with("task-"));
        let parts: Vec<&str> = task.id.split('-').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn test_is_ready_open_no_blockers() {
        let task = Task::new("Test".to_string(), 1);
        assert!(task.is_ready(&[]));
    }

    #[test]
    fn test_is_ready_with_open_blocker() {
        let blocker = Task::new("Blocker".to_string(), 1);
        let mut task = Task::new("Test".to_string(), 1);
        task.blocked_by.push(blocker.id.clone());

        assert!(!task.is_ready(std::slice::from_ref(&blocker)));
    }

    #[test]
    fn test_is_ready_with_closed_blocker() {
        let mut blocker = Task::new("Blocker".to_string(), 1);
        blocker.status = TaskStatus::Closed;

        let mut task = Task::new("Test".to_string(), 1);
        task.blocked_by.push(blocker.id.clone());

        assert!(task.is_ready(std::slice::from_ref(&blocker)));
    }

    #[test]
    fn test_is_not_ready_when_not_open() {
        let mut task = Task::new("Test".to_string(), 1);
        task.status = TaskStatus::Closed;
        assert!(!task.is_ready(&[]));

        task.status = TaskStatus::InProgress;
        assert!(!task.is_ready(&[]));
    }
}
