use std::collections::BTreeMap;
use std::fs;

use ralph_core::FileLock;
use serde::{Deserialize, Serialize};
use tracing::warn;

use super::{TaskDomain, TaskRecord};
use crate::errors::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskSnapshot {
    tasks: Vec<TaskRecord>,
}

impl TaskDomain {
    pub(super) fn load(&mut self) {
        self.tasks = match self.read_tasks_from_disk() {
            Ok(tasks) => tasks,
            Err(error) => {
                warn!(
                    path = %self.store_path.display(),
                    error = %error.message,
                    "failed loading task domain snapshot"
                );
                BTreeMap::new()
            }
        };
    }

    pub(super) fn persist(&self) -> Result<(), ApiError> {
        self.persist_tasks_to_disk(&self.tasks)
    }

    #[allow(dead_code)]
    pub(crate) fn with_exclusive_snapshot<F, T>(&mut self, f: F) -> Result<T, ApiError>
    where
        F: FnOnce(&mut BTreeMap<String, TaskRecord>) -> Result<T, ApiError>,
    {
        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking task snapshot '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut tasks = self.read_tasks_from_disk()?;
        let result = f(&mut tasks)?;
        self.persist_tasks_to_disk(&tasks)?;
        self.tasks = tasks;
        Ok(result)
    }

    fn read_tasks_from_disk(&self) -> Result<BTreeMap<String, TaskRecord>, ApiError> {
        if !self.store_path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = fs::read_to_string(&self.store_path).map_err(|error| {
            ApiError::internal(format!(
                "failed reading task snapshot '{}': {error}",
                self.store_path.display()
            ))
        })?;

        if content.trim().is_empty() {
            return Err(ApiError::internal(format!(
                "failed parsing task snapshot '{}': file is empty",
                self.store_path.display()
            )));
        }

        let snapshot: TaskSnapshot = serde_json::from_str(&content).map_err(|error| {
            ApiError::internal(format!(
                "failed parsing task snapshot '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut tasks = BTreeMap::new();
        for task in snapshot.tasks {
            let task_id = task.id.clone();
            if tasks.insert(task_id.clone(), task).is_some() {
                return Err(ApiError::internal(format!(
                    "failed parsing task snapshot '{}': duplicate task id '{}'",
                    self.store_path.display(),
                    task_id
                )));
            }
        }

        Ok(tasks)
    }

    fn persist_tasks_to_disk(&self, tasks: &BTreeMap<String, TaskRecord>) -> Result<(), ApiError> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed to create task snapshot directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let snapshot = TaskSnapshot {
            tasks: Self::sorted_tasks_from(tasks),
        };

        let payload = serde_json::to_string_pretty(&snapshot)
            .map_err(|error| ApiError::internal(format!("failed to serialize tasks: {error}")))?;

        fs::write(&self.store_path, payload).map_err(|error| {
            ApiError::internal(format!(
                "failed to write task snapshot '{}': {error}",
                self.store_path.display()
            ))
        })
    }

    #[allow(dead_code)]
    fn file_lock(&self) -> Result<FileLock, ApiError> {
        FileLock::new(&self.store_path).map_err(|error| {
            ApiError::internal(format!(
                "failed preparing task snapshot lock '{}': {error}",
                self.store_path.display()
            ))
        })
    }
}
