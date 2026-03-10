//! Persistent task storage with JSONL format.
//!
//! TaskStore provides load/save operations for the .ralph/agent/tasks.jsonl file,
//! with convenience methods for querying and updating tasks.
//!
//! # Multi-loop Safety
//!
//! When multiple Ralph loops run concurrently (in worktrees), this store uses
//! file locking to ensure safe concurrent access:
//!
//! - **Shared locks** for reading: Multiple loops can read simultaneously
//! - **Exclusive locks** for writing: Only one loop can write at a time
//!
//! Use `load()` and `save()` for simple single-operation access, or use
//! `with_exclusive_lock()` for read-modify-write operations that need atomicity.

use crate::file_lock::FileLock;
use crate::task::{Task, TaskStatus};
use std::io;
use std::path::Path;
use tracing::warn;

/// A store for managing tasks with JSONL persistence and file locking.
pub struct TaskStore {
    path: std::path::PathBuf,
    tasks: Vec<Task>,
    lock: FileLock,
}

/// Parses a JSONL line into a Task, logging a warning on failure.
fn parse_task_line(line: &str) -> Option<Task> {
    match serde_json::from_str(line) {
        Ok(task) => Some(task),
        Err(e) => {
            warn!(
                error = %e,
                line = line.chars().take(200).collect::<String>(),
                "Skipping malformed task line in JSONL"
            );
            None
        }
    }
}

impl TaskStore {
    /// Loads tasks from the JSONL file at the given path.
    ///
    /// If the file doesn't exist, returns an empty store.
    /// Logs warnings for malformed JSON lines and skips them.
    ///
    /// Uses a shared lock to allow concurrent reads from multiple loops.
    pub fn load(path: &Path) -> io::Result<Self> {
        let lock = FileLock::new(path)?;
        let _guard = lock.shared()?;

        let tasks = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .filter_map(|line| parse_task_line(line))
                .collect()
        } else {
            Vec::new()
        };

        Ok(Self {
            path: path.to_path_buf(),
            tasks,
            lock,
        })
    }

    /// Saves all tasks to the JSONL file.
    ///
    /// Creates parent directories if they don't exist.
    /// Uses an exclusive lock to prevent concurrent writes.
    pub fn save(&self) -> io::Result<()> {
        let _guard = self.lock.exclusive()?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content: String = self
            .tasks
            .iter()
            .map(|t| {
                serde_json::to_string(t).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("task serialization failed: {e}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        std::fs::write(
            &self.path,
            if content.is_empty() {
                String::new()
            } else {
                content + "\n"
            },
        )
    }

    /// Reloads tasks from disk, useful after external modifications.
    ///
    /// Logs warnings for malformed JSON lines and skips them.
    /// Uses a shared lock to allow concurrent reads.
    pub fn reload(&mut self) -> io::Result<()> {
        let _guard = self.lock.shared()?;

        self.tasks = if self.path.exists() {
            let content = std::fs::read_to_string(&self.path)?;
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .filter_map(|line| parse_task_line(line))
                .collect()
        } else {
            Vec::new()
        };

        Ok(())
    }

    /// Executes a read-modify-write operation atomically.
    ///
    /// Acquires an exclusive lock, reloads from disk, executes the
    /// provided function, and saves back to disk. This ensures that
    /// concurrent modifications from other loops are not lost.
    ///
    /// # Example
    ///
    /// ```ignore
    /// store.with_exclusive_lock(|store| {
    ///     let task = Task::new("New task".to_string(), 1);
    ///     store.add(task);
    /// })?;
    /// ```
    pub fn with_exclusive_lock<F, T>(&mut self, f: F) -> io::Result<T>
    where
        F: FnOnce(&mut Self) -> T,
    {
        let _guard = self.lock.exclusive()?;

        // Reload to get latest changes from other loops
        self.tasks = if self.path.exists() {
            let content = std::fs::read_to_string(&self.path)?;
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .filter_map(|line| parse_task_line(line))
                .collect()
        } else {
            Vec::new()
        };

        // Execute the user function
        let result = f(self);

        // Save changes
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content: String = self
            .tasks
            .iter()
            .map(|t| {
                serde_json::to_string(t).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("task serialization failed: {e}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        std::fs::write(
            &self.path,
            if content.is_empty() {
                String::new()
            } else {
                content + "\n"
            },
        )?;

        Ok(result)
    }

    /// Adds a new task to the store and returns a reference to it.
    pub fn add(&mut self, task: Task) -> &Task {
        self.tasks.push(task);
        self.tasks.last().unwrap()
    }

    /// Gets a task by ID (immutable reference).
    pub fn get(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Gets a task by stable key (immutable reference).
    pub fn get_by_key(&self, key: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.key.as_deref() == Some(key))
    }

    /// Gets a task by ID (mutable reference).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Gets a task by stable key (mutable reference).
    pub fn get_by_key_mut(&mut self, key: &str) -> Option<&mut Task> {
        self.tasks
            .iter_mut()
            .find(|t| t.key.as_deref() == Some(key))
    }

    /// Transitions a task to a new status, recording the transition.
    ///
    /// Records a `StatusTransition` with the old and new status, timestamp, and
    /// optional hat. Also manages lifecycle timestamps: sets `started` on first
    /// `InProgress`, sets `closed` on `Closed`/`Failed`, clears `closed` on `Open`.
    pub fn transition(
        &mut self,
        id: &str,
        new_status: TaskStatus,
        hat: Option<&str>,
    ) -> Option<&Task> {
        if let Some(task) = self.get_mut(id) {
            let from = task.status;
            let now = chrono::Utc::now().to_rfc3339();

            task.transitions.push(crate::task::StatusTransition {
                from,
                to: new_status,
                timestamp: now.clone(),
                hat: hat.map(String::from),
            });

            task.status = new_status;
            if let Some(h) = hat {
                task.last_hat = Some(h.to_string());
            }

            match new_status {
                TaskStatus::InProgress => {
                    if task.started.is_none() {
                        task.started = Some(now);
                    }
                    task.closed = None;
                }
                TaskStatus::Closed | TaskStatus::Failed => {
                    task.closed = Some(now);
                }
                TaskStatus::Open => {
                    task.closed = None;
                }
                _ => {}
            }

            return self.get(id);
        }
        None
    }

    /// Closes a task by ID and returns a reference to it.
    pub fn close(&mut self, id: &str) -> Option<&Task> {
        self.transition(id, TaskStatus::Closed, None)
    }

    /// Starts a task by ID and returns a reference to it.
    pub fn start(&mut self, id: &str) -> Option<&Task> {
        self.transition(id, TaskStatus::InProgress, None)
    }

    /// Fails a task by ID and returns a reference to it.
    pub fn fail(&mut self, id: &str) -> Option<&Task> {
        self.transition(id, TaskStatus::Failed, None)
    }

    /// Reopens a task by ID and returns a reference to it.
    pub fn reopen(&mut self, id: &str) -> Option<&Task> {
        self.transition(id, TaskStatus::Open, None)
    }

    /// Moves a task to review status.
    pub fn review(&mut self, id: &str) -> Option<&Task> {
        self.transition(id, TaskStatus::InReview, None)
    }

    /// Moves a task to blocked status.
    pub fn block(&mut self, id: &str) -> Option<&Task> {
        self.transition(id, TaskStatus::Blocked, None)
    }

    /// Returns tasks matching the given status.
    pub fn filter_by_status(&self, status: TaskStatus) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == status).collect()
    }

    /// Returns tasks matching the given loop ID.
    pub fn filter_by_loop_id(&self, loop_id: &str) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.loop_id.as_deref() == Some(loop_id))
            .collect()
    }

    /// Returns tasks whose `last_hat` matches the given hat.
    pub fn filter_by_hat(&self, hat: &str) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.last_hat.as_deref() == Some(hat))
            .collect()
    }

    /// Returns tasks matching all provided criteria (AND intersection).
    pub fn filter(
        &self,
        status: Option<TaskStatus>,
        loop_id: Option<&str>,
        hat: Option<&str>,
        priority: Option<u8>,
        tag: Option<&str>,
    ) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| {
                status.is_none_or(|s| t.status == s)
                    && loop_id.is_none_or(|l| t.loop_id.as_deref() == Some(l))
                    && hat.is_none_or(|h| t.last_hat.as_deref() == Some(h))
                    && priority.is_none_or(|p| t.priority == p)
                    && tag.is_none_or(|tg| t.tags.iter().any(|x| x == tg))
            })
            .collect()
    }

    /// Returns a count of tasks grouped by status.
    pub fn counts_by_status(&self) -> std::collections::HashMap<TaskStatus, usize> {
        let mut counts = std::collections::HashMap::new();
        for task in &self.tasks {
            *counts.entry(task.status).or_insert(0) += 1;
        }
        counts
    }

    /// Returns a count of tasks grouped by status, scoped to a specific loop.
    pub fn counts_by_status_for_loop(
        &self,
        loop_id: &str,
    ) -> std::collections::HashMap<TaskStatus, usize> {
        let mut counts = std::collections::HashMap::new();
        for task in &self.tasks {
            if task.loop_id.as_deref() == Some(loop_id) {
                *counts.entry(task.status).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Ensures a task exists for a stable key, returning the existing or created task.
    ///
    /// If a task with the same key already exists, its non-lifecycle metadata is refreshed and
    /// the existing task is returned.
    pub fn ensure(&mut self, task: Task) -> &Task {
        if let Some(key) = task.key.as_deref()
            && let Some(existing_idx) = self
                .tasks
                .iter()
                .position(|existing| existing.key.as_deref() == Some(key))
        {
            let existing = &mut self.tasks[existing_idx];
            existing.title = task.title;
            existing.priority = task.priority;
            if task.description.is_some() {
                existing.description = task.description;
            }
            if !task.blocked_by.is_empty() {
                existing.blocked_by = task.blocked_by;
            }
            return &self.tasks[existing_idx];
        }

        self.tasks.push(task);
        self.tasks.last().unwrap()
    }

    /// Returns all tasks as a slice.
    pub fn all(&self) -> &[Task] {
        &self.tasks
    }

    /// Returns all open tasks (not closed).
    pub fn open(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Closed)
            .collect()
    }

    /// Returns all ready tasks (open with no pending blockers).
    pub fn ready(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.is_ready(&self.tasks))
            .collect()
    }

    /// Returns true if there are any open tasks.
    ///
    /// A task is considered open if it is not Closed. This includes Failed tasks.
    pub fn has_open_tasks(&self) -> bool {
        self.tasks.iter().any(|t| t.status != TaskStatus::Closed)
    }

    /// Returns true if there are any pending (non-terminal) tasks.
    ///
    /// A task is pending if its status is not terminal (i.e., not Closed or Failed).
    /// Use this when you need to check if there's active work remaining.
    pub fn has_pending_tasks(&self) -> bool {
        self.tasks.iter().any(|t| !t.status.is_terminal())
    }

    /// Removes a task by ID. Returns true if the task was found and removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.tasks.len();
        self.tasks.retain(|t| t.id != id);
        self.tasks.len() < len
    }

    /// Removes all tasks from the store.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let store = TaskStore::load(&path).unwrap();
        assert_eq!(store.all().len(), 0);
    }

    #[test]
    fn test_add_and_save() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");

        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test task".to_string(), 1);
        store.add(task);
        store.save().unwrap();

        let loaded = TaskStore::load(&path).unwrap();
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.all()[0].title, "Test task");
    }

    #[test]
    fn test_get_task() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        assert!(store.get(&id).is_some());
        assert_eq!(store.get(&id).unwrap().title, "Test");
    }

    #[test]
    fn test_get_task_by_key() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1).with_key(Some("phase:design".to_string()));
        store.add(task);

        assert!(store.get_by_key("phase:design").is_some());
        assert_eq!(store.get_by_key("phase:design").unwrap().title, "Test");
    }

    #[test]
    fn test_close_task() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        let closed = store.close(&id).unwrap();
        assert_eq!(closed.status, TaskStatus::Closed);
        assert!(closed.closed.is_some());
    }

    #[test]
    fn test_start_task() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        let started = store.start(&id).unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);
        assert!(started.started.is_some());
    }

    #[test]
    fn test_reopen_task() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);
        store.close(&id);

        let reopened = store.reopen(&id).unwrap();
        assert_eq!(reopened.status, TaskStatus::Open);
        assert!(reopened.closed.is_none());
    }

    #[test]
    fn test_open_tasks() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();

        let task1 = Task::new("Open 1".to_string(), 1);
        store.add(task1);

        let mut task2 = Task::new("Closed".to_string(), 1);
        task2.status = TaskStatus::Closed;
        store.add(task2);

        assert_eq!(store.open().len(), 1);
    }

    #[test]
    fn test_ready_tasks() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();

        let task1 = Task::new("Ready".to_string(), 1);
        let id1 = task1.id.clone();
        store.add(task1);

        let mut task2 = Task::new("Blocked".to_string(), 1);
        task2.blocked_by.push(id1);
        store.add(task2);

        let ready = store.ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].title, "Ready");
    }

    #[test]
    fn test_ensure_deduplicates_by_key() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();

        let first = Task::new("First".to_string(), 1).with_key(Some("impl:task-01".to_string()));
        let second = Task::new("Second".to_string(), 3).with_key(Some("impl:task-01".to_string()));

        let id = store.ensure(first).id.clone();
        let deduped_id = store.ensure(second).id.clone();
        let deduped = store
            .get_by_key("impl:task-01")
            .expect("deduped task should exist");

        assert_eq!(store.all().len(), 1);
        assert_eq!(deduped_id, id);
        assert_eq!(deduped.title, "Second");
        assert_eq!(deduped.priority, 3);
    }

    #[test]
    fn test_has_open_tasks() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();

        assert!(!store.has_open_tasks());

        let task = Task::new("Test".to_string(), 1);
        store.add(task);

        assert!(store.has_open_tasks());
    }

    #[test]
    fn test_has_pending_tasks_excludes_failed() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();

        // Empty store has no pending tasks
        assert!(!store.has_pending_tasks());

        // Add an open task - should have pending
        let task1 = Task::new("Open task".to_string(), 1);
        store.add(task1);
        assert!(store.has_pending_tasks());

        // Close the task - should have no pending
        let id = store.all()[0].id.clone();
        store.close(&id);
        assert!(!store.has_pending_tasks());
    }

    #[test]
    fn test_has_pending_tasks_failed_is_terminal() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();

        // Add a task and fail it
        let task = Task::new("Failed task".to_string(), 1);
        store.add(task);
        let id = store.all()[0].id.clone();
        store.fail(&id);

        // Failed tasks are terminal, so no pending tasks
        assert!(!store.has_pending_tasks());

        // But has_open_tasks returns true (Failed != Closed)
        assert!(store.has_open_tasks());
    }

    #[test]
    fn test_reload() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");

        // Create and save initial store
        let mut store1 = TaskStore::load(&path).unwrap();
        store1.add(Task::new("Task 1".to_string(), 1));
        store1.save().unwrap();

        // Create second store that reads the same file
        let mut store2 = TaskStore::load(&path).unwrap();
        store2.add(Task::new("Task 2".to_string(), 1));
        store2.save().unwrap();

        // Reload first store to see changes
        store1.reload().unwrap();
        assert_eq!(store1.all().len(), 2);
    }

    #[test]
    fn test_with_exclusive_lock() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");

        let mut store = TaskStore::load(&path).unwrap();

        // Use with_exclusive_lock for atomic operation
        store
            .with_exclusive_lock(|s| {
                s.add(Task::new("Atomic task".to_string(), 1));
            })
            .unwrap();

        // Verify the task was saved
        let loaded = TaskStore::load(&path).unwrap();
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.all()[0].title, "Atomic task");
    }

    #[test]
    fn test_concurrent_writes_with_lock() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let path_clone = path.clone();

        let barrier = Arc::new(Barrier::new(2));
        let barrier_clone = barrier.clone();

        // Thread 1: Add task 1
        let handle1 = thread::spawn(move || {
            let mut store = TaskStore::load(&path).unwrap();
            barrier.wait();

            store
                .with_exclusive_lock(|s| {
                    s.add(Task::new("Task from thread 1".to_string(), 1));
                })
                .unwrap();
        });

        // Thread 2: Add task 2
        let handle2 = thread::spawn(move || {
            let mut store = TaskStore::load(&path_clone).unwrap();
            barrier_clone.wait();

            store
                .with_exclusive_lock(|s| {
                    s.add(Task::new("Task from thread 2".to_string(), 1));
                })
                .unwrap();
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Both tasks should be present
        let final_store = TaskStore::load(tmp.path().join("tasks.jsonl").as_ref()).unwrap();
        assert_eq!(final_store.all().len(), 2);
    }

    #[test]
    fn test_load_skips_malformed_lines() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");

        // Write a file with one valid task line and some malformed lines
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Valid task".to_string(), 1);
        store.add(task);
        store.save().unwrap();

        // Append malformed lines to the file
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("this is not json\n");
        content.push_str("{\"broken\": true}\n");
        std::fs::write(&path, content).unwrap();

        // Load should succeed with only the valid task
        let loaded = TaskStore::load(&path).unwrap();
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.all()[0].title, "Valid task");
    }

    #[test]
    fn test_start_records_transition() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        let started = store.start(&id).unwrap();
        assert_eq!(started.transitions.len(), 1);
        assert_eq!(started.transitions[0].from, TaskStatus::Open);
        assert_eq!(started.transitions[0].to, TaskStatus::InProgress);
        assert!(started.transitions[0].hat.is_none());
    }

    #[test]
    fn test_close_records_transition() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);
        store.start(&id);

        let closed = store.close(&id).unwrap();
        assert_eq!(closed.transitions.len(), 2);
        assert_eq!(closed.transitions[1].from, TaskStatus::InProgress);
        assert_eq!(closed.transitions[1].to, TaskStatus::Closed);
    }

    #[test]
    fn test_review_records_transition() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);
        store.start(&id);

        let reviewed = store.review(&id).unwrap();
        assert_eq!(reviewed.status, TaskStatus::InReview);
        assert_eq!(reviewed.transitions.len(), 2);
        assert_eq!(reviewed.transitions[1].to, TaskStatus::InReview);
    }

    #[test]
    fn test_block_records_transition() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        let blocked = store.block(&id).unwrap();
        assert_eq!(blocked.status, TaskStatus::Blocked);
        assert_eq!(blocked.transitions.len(), 1);
        assert_eq!(blocked.transitions[0].to, TaskStatus::Blocked);
    }

    #[test]
    fn test_reopen_records_transition_and_clears_closed() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);
        store.start(&id);
        store.close(&id);

        let reopened = store.reopen(&id).unwrap();
        assert_eq!(reopened.status, TaskStatus::Open);
        assert!(reopened.closed.is_none());
        assert_eq!(reopened.transitions.len(), 3);
        assert_eq!(reopened.transitions[2].to, TaskStatus::Open);
    }

    #[test]
    fn test_multiple_transitions_accumulate() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        store.start(&id);
        store.review(&id);
        store.block(&id);
        store.reopen(&id);
        store.start(&id);
        let t = store.close(&id).unwrap();
        assert_eq!(t.transitions.len(), 6);
    }

    #[test]
    fn test_transition_records_hat() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        let t = store
            .transition(&id, TaskStatus::InProgress, Some("builder"))
            .unwrap();
        assert_eq!(t.transitions[0].hat.as_deref(), Some("builder"));
        assert_eq!(t.last_hat.as_deref(), Some("builder"));
    }

    #[test]
    fn test_save_reload_preserves_transitions() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);
        store.transition(&id, TaskStatus::InProgress, Some("planner"));
        store.save().unwrap();

        let loaded = TaskStore::load(&path).unwrap();
        let t = loaded.get(&id).unwrap();
        assert_eq!(t.transitions.len(), 1);
        assert_eq!(t.transitions[0].from, TaskStatus::Open);
        assert_eq!(t.transitions[0].to, TaskStatus::InProgress);
        assert_eq!(t.transitions[0].hat.as_deref(), Some("planner"));
        assert_eq!(t.last_hat.as_deref(), Some("planner"));
    }

    #[test]
    fn test_filter_by_status() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        store.add(Task::new("A".into(), 1));
        let b = Task::new("B".into(), 2);
        let b_id = b.id.clone();
        store.add(b);
        store.start(&b_id);

        assert_eq!(store.filter_by_status(TaskStatus::Open).len(), 1);
        assert_eq!(store.filter_by_status(TaskStatus::InProgress).len(), 1);
        assert_eq!(store.filter_by_status(TaskStatus::Closed).len(), 0);
    }

    #[test]
    fn test_filter_by_loop_id() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let mut a = Task::new("A".into(), 1);
        a.loop_id = Some("loop-1".into());
        let mut b = Task::new("B".into(), 1);
        b.loop_id = Some("loop-2".into());
        store.add(a);
        store.add(b);

        assert_eq!(store.filter_by_loop_id("loop-1").len(), 1);
        assert_eq!(store.filter_by_loop_id("loop-1")[0].title, "A");
        assert_eq!(store.filter_by_loop_id("loop-3").len(), 0);
    }

    #[test]
    fn test_filter_by_hat() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let a = Task::new("A".into(), 1);
        let a_id = a.id.clone();
        store.add(a);
        store.transition(&a_id, TaskStatus::InProgress, Some("builder"));

        assert_eq!(store.filter_by_hat("builder").len(), 1);
        assert_eq!(store.filter_by_hat("planner").len(), 0);
    }

    #[test]
    fn test_combined_filter_intersects() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();

        let mut a = Task::new("A".into(), 1).with_tags(vec!["api".into()]);
        a.loop_id = Some("loop-1".into());
        let a_id = a.id.clone();
        store.add(a);
        store.transition(&a_id, TaskStatus::InProgress, Some("builder"));

        let mut b = Task::new("B".into(), 2).with_tags(vec!["api".into()]);
        b.loop_id = Some("loop-1".into());
        store.add(b);

        // status + tag
        let r = store.filter(Some(TaskStatus::InProgress), None, None, None, Some("api"));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "A");

        // loop + priority
        let r = store.filter(None, Some("loop-1"), None, Some(2), None);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "B");

        // no criteria returns all
        assert_eq!(store.filter(None, None, None, None, None).len(), 2);
    }

    #[test]
    fn test_counts_by_status() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let a = Task::new("A".into(), 1);
        let b = Task::new("B".into(), 1);
        let a_id = a.id.clone();
        store.add(a);
        store.add(b);
        store.start(&a_id);

        let counts = store.counts_by_status();
        assert_eq!(counts.get(&TaskStatus::InProgress), Some(&1));
        assert_eq!(counts.get(&TaskStatus::Open), Some(&1));
        assert_eq!(counts.get(&TaskStatus::Closed), None);
    }

    #[test]
    fn test_counts_by_status_for_loop() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let mut store = TaskStore::load(&path).unwrap();
        let mut a = Task::new("A".into(), 1);
        a.loop_id = Some("loop-1".into());
        let mut b = Task::new("B".into(), 1);
        b.loop_id = Some("loop-2".into());
        store.add(a);
        store.add(b);

        let counts = store.counts_by_status_for_loop("loop-1");
        assert_eq!(counts.get(&TaskStatus::Open), Some(&1));
        assert_eq!(counts.len(), 1);

        let empty = store.counts_by_status_for_loop("loop-99");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_empty_store_filters_and_counts() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let store = TaskStore::load(&path).unwrap();

        assert!(store.filter_by_status(TaskStatus::Open).is_empty());
        assert!(store.filter_by_loop_id("x").is_empty());
        assert!(store.filter_by_hat("x").is_empty());
        assert!(store.filter(None, None, None, None, None).is_empty());
        assert!(store.counts_by_status().is_empty());
        assert!(store.counts_by_status_for_loop("x").is_empty());
    }
}
