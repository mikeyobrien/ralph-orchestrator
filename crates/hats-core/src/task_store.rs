//! Persistent task storage with JSONL format.
//!
//! TaskStore provides load/save operations for the .hats/agent/tasks.jsonl file,
//! with convenience methods for querying and updating tasks.
//!
//! # Multi-loop Safety
//!
//! When multiple Hats loops run concurrently (in worktrees), this store uses
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

    /// Gets a task by ID (mutable reference).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Closes a task by ID and returns a reference to it.
    pub fn close(&mut self, id: &str) -> Option<&Task> {
        if let Some(task) = self.get_mut(id) {
            task.status = TaskStatus::Closed;
            task.closed = Some(chrono::Utc::now().to_rfc3339());
            return self.get(id);
        }
        None
    }

    /// Fails a task by ID and returns a reference to it.
    pub fn fail(&mut self, id: &str) -> Option<&Task> {
        if let Some(task) = self.get_mut(id) {
            task.status = TaskStatus::Failed;
            task.closed = Some(chrono::Utc::now().to_rfc3339());
            return self.get(id);
        }
        None
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
}
