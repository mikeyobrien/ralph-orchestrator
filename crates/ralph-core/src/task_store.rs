//! Persistent task storage with JSONL format.
//!
//! TaskStore provides load/save operations for the .agent/tasks.jsonl file,
//! with convenience methods for querying and updating tasks.

use crate::task::{Task, TaskStatus};
use std::path::Path;

/// A store for managing tasks with JSONL persistence.
pub struct TaskStore {
    path: std::path::PathBuf,
    tasks: Vec<Task>,
}

impl TaskStore {
    /// Loads tasks from the JSONL file at the given path.
    ///
    /// If the file doesn't exist, returns an empty store.
    /// Silently skips malformed JSON lines.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let tasks = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        } else {
            Vec::new()
        };
        Ok(Self {
            path: path.to_path_buf(),
            tasks,
        })
    }

    /// Saves all tasks to the JSONL file.
    ///
    /// Creates parent directories if they don't exist.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content: String = self
            .tasks
            .iter()
            .map(|t| serde_json::to_string(t).unwrap())
            .collect::<Vec<_>>()
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
    pub fn has_open_tasks(&self) -> bool {
        self.tasks.iter().any(|t| t.status != TaskStatus::Closed)
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
        let mut store = TaskStore::load(&std::path::PathBuf::from("/dev/null")).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        assert!(store.get(&id).is_some());
        assert_eq!(store.get(&id).unwrap().title, "Test");
    }

    #[test]
    fn test_close_task() {
        let mut store = TaskStore::load(&std::path::PathBuf::from("/dev/null")).unwrap();
        let task = Task::new("Test".to_string(), 1);
        let id = task.id.clone();
        store.add(task);

        let closed = store.close(&id).unwrap();
        assert_eq!(closed.status, TaskStatus::Closed);
        assert!(closed.closed.is_some());
    }

    #[test]
    fn test_open_tasks() {
        let mut store = TaskStore::load(&std::path::PathBuf::from("/dev/null")).unwrap();

        let task1 = Task::new("Open 1".to_string(), 1);
        store.add(task1);

        let mut task2 = Task::new("Closed".to_string(), 1);
        task2.status = TaskStatus::Closed;
        store.add(task2);

        assert_eq!(store.open().len(), 1);
    }

    #[test]
    fn test_ready_tasks() {
        let mut store = TaskStore::load(&std::path::PathBuf::from("/dev/null")).unwrap();

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
        let mut store = TaskStore::load(&std::path::PathBuf::from("/dev/null")).unwrap();

        assert!(!store.has_open_tasks());

        let task = Task::new("Test".to_string(), 1);
        store.add(task);

        assert!(store.has_open_tasks());
    }
}
