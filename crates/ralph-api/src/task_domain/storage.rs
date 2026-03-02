use std::fs;

use serde::{Deserialize, Serialize};
use tracing::warn;

use super::{TaskDomain, TaskRecord};
use crate::errors::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskSnapshot {
    tasks: Vec<TaskRecord>,
    queue_counter: u64,
}

impl TaskDomain {
    pub(super) fn load(&mut self) {
        if !self.store_path.exists() {
            return;
        }

        let content = match fs::read_to_string(&self.store_path) {
            Ok(content) => content,
            Err(error) => {
                warn!(
                    path = %self.store_path.display(),
                    %error,
                    "failed reading task domain snapshot"
                );
                return;
            }
        };

        let snapshot: TaskSnapshot = match serde_json::from_str(&content) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                warn!(
                    path = %self.store_path.display(),
                    %error,
                    "failed parsing task domain snapshot"
                );
                return;
            }
        };

        self.tasks = snapshot
            .tasks
            .into_iter()
            .map(|task| (task.id.clone(), task))
            .collect();
        self.queue_counter = snapshot.queue_counter;
    }

    pub(super) fn persist(&self) -> Result<(), ApiError> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed to create task snapshot directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let snapshot = TaskSnapshot {
            tasks: self.sorted_tasks(),
            queue_counter: self.queue_counter,
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
}
