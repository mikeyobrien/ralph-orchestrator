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
    /// Failed/abandoned
    Failed,
    /// Waiting on external dependency or human input
    Blocked,
    /// Submitted for review
    InReview,
}

impl TaskStatus {
    /// Returns true if this status is terminal (Closed or Failed).
    ///
    /// Terminal statuses indicate the task is done and no longer needs attention.
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Closed | TaskStatus::Failed)
    }
}

/// A recorded status transition for audit/kanban tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusTransition {
    pub from: TaskStatus,
    pub to: TaskStatus,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hat: Option<String>,
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

    /// Stable key for idempotent orchestrator-managed tasks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Current state
    pub status: TaskStatus,

    /// Priority 1-5 (1 = highest)
    pub priority: u8,

    /// Tasks that must complete before this one
    #[serde(default)]
    pub blocked_by: Vec<String>,

    /// Loop ID that created this task (from RALPH_LOOP_ID env var).
    /// Used to filter tasks by ownership when multiple loops share a task list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_id: Option<String>,

    /// Creation timestamp (ISO 8601)
    pub created: String,

    /// Start timestamp (ISO 8601), if the task entered in_progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,

    /// Completion timestamp (ISO 8601), if closed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,

    /// Last hat that touched this task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_hat: Option<String>,

    /// Status transition history for kanban tracking
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<StatusTransition>,

    /// Freeform tags for filtering and grouping
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Task {
    /// Creates a new task with the given title and priority.
    pub fn new(title: String, priority: u8) -> Self {
        Self {
            id: Self::generate_id(),
            title,
            description: None,
            key: None,
            status: TaskStatus::Open,
            priority: priority.clamp(1, 5),
            blocked_by: Vec::new(),
            loop_id: None,
            created: chrono::Utc::now().to_rfc3339(),
            started: None,
            closed: None,
            last_hat: None,
            transitions: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Sets the loop ID for this task.
    pub fn with_loop_id(mut self, loop_id: Option<String>) -> Self {
        self.loop_id = loop_id;
        self
    }

    /// Sets freeform tags for filtering and grouping.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Sets the last hat that touched this task.
    pub fn with_last_hat(mut self, hat: Option<String>) -> Self {
        self.last_hat = hat;
        self
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

    /// Sets the stable orchestration key for the task.
    pub fn with_key(mut self, key: Option<String>) -> Self {
        self.key = key;
        self
    }

    /// Adds a blocker task ID.
    pub fn with_blocker(mut self, task_id: String) -> Self {
        self.blocked_by.push(task_id);
        self
    }

    /// Marks the task as in progress and records a start timestamp if absent.
    pub fn start(&mut self) {
        self.status = TaskStatus::InProgress;
        if self.started.is_none() {
            self.started = Some(chrono::Utc::now().to_rfc3339());
        }
        self.closed = None;
    }

    /// Reopens a terminal task for further work.
    pub fn reopen(&mut self) {
        self.status = TaskStatus::Open;
        self.closed = None;
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
        assert!(task.key.is_none());
        assert!(task.started.is_none());
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

        task.status = TaskStatus::Failed;
        assert!(!task.is_ready(&[]));
    }

    #[test]
    fn test_is_terminal() {
        assert!(!TaskStatus::Open.is_terminal());
        assert!(!TaskStatus::InProgress.is_terminal());
        assert!(TaskStatus::Closed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
    }

    #[test]
    fn test_with_key_sets_stable_key() {
        let task = Task::new("Test".to_string(), 1).with_key(Some("spec:build".to_string()));
        assert_eq!(task.key.as_deref(), Some("spec:build"));
    }

    #[test]
    fn test_start_marks_task_in_progress() {
        let mut task = Task::new("Test".to_string(), 1);
        task.start();
        assert_eq!(task.status, TaskStatus::InProgress);
        assert!(task.started.is_some());
        assert!(task.closed.is_none());
    }

    #[test]
    fn test_reopen_resets_terminal_state() {
        let mut task = Task::new("Test".to_string(), 1);
        task.status = TaskStatus::Closed;
        task.closed = Some(chrono::Utc::now().to_rfc3339());
        task.reopen();
        assert_eq!(task.status, TaskStatus::Open);
        assert!(task.closed.is_none());
    }

    #[test]
    fn test_serde_roundtrip_blocked_and_in_review() {
        for (status, expected_str) in [
            (TaskStatus::Blocked, "\"blocked\""),
            (TaskStatus::InReview, "\"in_review\""),
        ] {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected_str);
            let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, status);
        }
    }

    #[test]
    fn test_is_terminal_new_statuses() {
        assert!(!TaskStatus::Blocked.is_terminal());
        assert!(!TaskStatus::InReview.is_terminal());
    }

    #[test]
    fn test_backward_compat_deserialize_task_missing_new_fields() {
        let json = serde_json::json!({
            "id": "task-123-abcd",
            "title": "Legacy task",
            "status": "open",
            "priority": 2,
            "blocked_by": [],
            "created": "2026-01-01T00:00:00Z"
        });
        let task: Task = serde_json::from_value(json).unwrap();
        assert!(task.last_hat.is_none());
        assert!(task.transitions.is_empty());
        assert!(task.tags.is_empty());
    }

    #[test]
    fn test_status_transition_serde_roundtrip() {
        let transition = StatusTransition {
            from: TaskStatus::Open,
            to: TaskStatus::InProgress,
            timestamp: "2026-03-10T12:00:00Z".to_string(),
            hat: Some("builder".to_string()),
        };
        let json = serde_json::to_string(&transition).unwrap();
        let deserialized: StatusTransition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from, TaskStatus::Open);
        assert_eq!(deserialized.to, TaskStatus::InProgress);
        assert_eq!(deserialized.timestamp, "2026-03-10T12:00:00Z");
        assert_eq!(deserialized.hat.as_deref(), Some("builder"));
    }

    #[test]
    fn test_is_ready_returns_false_for_blocked() {
        let mut task = Task::new("Test".to_string(), 1);
        task.status = TaskStatus::Blocked;
        assert!(!task.is_ready(&[]));
    }

    #[test]
    fn test_builder_with_tags_and_with_last_hat() {
        let task = Task::new("Test".to_string(), 1)
            .with_tags(vec!["api".to_string(), "auth".to_string()])
            .with_last_hat(Some("planner".to_string()));
        assert_eq!(task.tags, vec!["api", "auth"]);
        assert_eq!(task.last_hat.as_deref(), Some("planner"));
    }
}
