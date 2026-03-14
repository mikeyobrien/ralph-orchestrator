use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ralph_core::FileLock;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Idle,
    Busy,
    Blocked,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkerRecord {
    pub worker_id: String,
    pub worker_name: String,
    pub loop_id: String,
    pub backend: String,
    pub workspace_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_hat: Option<String>,
    pub status: WorkerStatus,
    pub last_heartbeat_at: String,
}

impl WorkerRecord {
    fn validate(&self) -> Result<(), ApiError> {
        validate_required_string("workerId", &self.worker_id)?;
        validate_required_string("workerName", &self.worker_name)?;
        validate_required_string("loopId", &self.loop_id)?;
        validate_required_string("backend", &self.backend)?;
        validate_required_string("workspaceRoot", &self.workspace_root)?;
        validate_optional_string("currentTaskId", self.current_task_id.as_deref())?;
        validate_optional_string("currentHat", self.current_hat.as_deref())?;
        validate_required_string("lastHeartbeatAt", &self.last_heartbeat_at)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkerHeartbeatInput {
    pub worker_id: String,
    pub status: WorkerStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_hat: Option<String>,
    pub last_heartbeat_at: String,
}

impl WorkerHeartbeatInput {
    fn validate(&self) -> Result<(), ApiError> {
        validate_required_string("workerId", &self.worker_id)?;
        validate_optional_string("currentTaskId", self.current_task_id.as_deref())?;
        validate_optional_string("currentHat", self.current_hat.as_deref())?;
        validate_required_string("lastHeartbeatAt", &self.last_heartbeat_at)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WorkerSnapshot {
    workers: Vec<WorkerRecord>,
}

pub struct WorkerDomain {
    store_path: PathBuf,
}

impl WorkerDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Result<Self, ApiError> {
        let domain = Self {
            store_path: workspace_root.as_ref().join(".ralph/workers.json"),
        };
        domain.read_workers_with_shared_lock()?;
        Ok(domain)
    }

    pub fn list(&self) -> Result<Vec<WorkerRecord>, ApiError> {
        let workers = self.read_workers_with_shared_lock()?;
        Ok(Self::sorted_workers(&workers))
    }

    pub fn get(&self, worker_id: &str) -> Result<WorkerRecord, ApiError> {
        validate_required_string("workerId", worker_id)?;
        self.read_workers_with_shared_lock()?
            .get(worker_id)
            .cloned()
            .ok_or_else(|| worker_not_found_error(worker_id))
    }

    /// Inserts or replaces a worker registry entry by `worker_id`.
    pub fn register(&mut self, worker: WorkerRecord) -> Result<WorkerRecord, ApiError> {
        worker.validate()?;

        let worker_id = worker.worker_id.clone();
        self.modify_workers(|workers| {
            workers.insert(worker_id, worker.clone());
            Ok(())
        })?;

        Ok(worker)
    }

    pub fn deregister(&mut self, worker_id: &str) -> Result<(), ApiError> {
        validate_required_string("workerId", worker_id)?;

        let worker_id = worker_id.to_string();
        self.modify_workers(|workers| {
            workers
                .remove(&worker_id)
                .ok_or_else(|| worker_not_found_error(&worker_id))?;
            Ok(())
        })?;

        Ok(())
    }

    pub fn heartbeat(&mut self, input: WorkerHeartbeatInput) -> Result<WorkerRecord, ApiError> {
        input.validate()?;

        let WorkerHeartbeatInput {
            worker_id,
            status,
            current_task_id,
            current_hat,
            last_heartbeat_at,
        } = input;
        let mut updated_worker = None;

        self.modify_workers(|workers| {
            let worker = workers
                .get_mut(&worker_id)
                .ok_or_else(|| worker_not_found_error(&worker_id))?;
            worker.status = status;
            worker.current_task_id = current_task_id.clone();
            worker.current_hat = current_hat.clone();
            worker.last_heartbeat_at = last_heartbeat_at.clone();
            updated_worker = Some(worker.clone());
            Ok(())
        })?;

        updated_worker.ok_or_else(|| {
            ApiError::internal(format!(
                "failed updating worker heartbeat for '{}'",
                worker_id
            ))
        })
    }

    fn read_workers_with_shared_lock(&self) -> Result<BTreeMap<String, WorkerRecord>, ApiError> {
        if !self.store_path.exists() {
            return Ok(BTreeMap::new());
        }

        let lock = self.file_lock()?;
        let _guard = lock.shared().map_err(|error| {
            ApiError::internal(format!(
                "failed locking worker registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        self.read_workers_from_disk()
    }

    fn modify_workers<F>(&self, f: F) -> Result<(), ApiError>
    where
        F: FnOnce(&mut BTreeMap<String, WorkerRecord>) -> Result<(), ApiError>,
    {
        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking worker registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut workers = self.read_workers_from_disk()?;
        f(&mut workers)?;
        self.persist_workers_to_disk(&workers)?;
        Ok(())
    }

    fn read_workers_from_disk(&self) -> Result<BTreeMap<String, WorkerRecord>, ApiError> {
        if !self.store_path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = fs::read_to_string(&self.store_path).map_err(|error| {
            ApiError::internal(format!(
                "failed reading worker registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        if content.trim().is_empty() {
            return Err(ApiError::internal(format!(
                "failed parsing worker registry '{}': file is empty",
                self.store_path.display()
            )));
        }

        let snapshot: WorkerSnapshot = serde_json::from_str(&content).map_err(|error| {
            ApiError::internal(format!(
                "failed parsing worker registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut workers = BTreeMap::new();
        for worker in snapshot.workers {
            worker.validate().map_err(|error| {
                ApiError::internal(format!(
                    "failed parsing worker registry '{}': {}",
                    self.store_path.display(),
                    error.message
                ))
            })?;

            if workers
                .insert(worker.worker_id.clone(), worker.clone())
                .is_some()
            {
                return Err(ApiError::internal(format!(
                    "failed parsing worker registry '{}': duplicate workerId '{}'",
                    self.store_path.display(),
                    worker.worker_id
                )));
            }
        }

        Ok(workers)
    }

    fn persist_workers_to_disk(
        &self,
        workers: &BTreeMap<String, WorkerRecord>,
    ) -> Result<(), ApiError> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed creating worker registry directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let snapshot = WorkerSnapshot {
            workers: workers.values().cloned().collect(),
        };

        let payload = serde_json::to_string_pretty(&snapshot).map_err(|error| {
            ApiError::internal(format!("failed serializing worker registry: {error}"))
        })?;

        fs::write(&self.store_path, payload).map_err(|error| {
            ApiError::internal(format!(
                "failed writing worker registry '{}': {error}",
                self.store_path.display()
            ))
        })
    }

    fn file_lock(&self) -> Result<FileLock, ApiError> {
        FileLock::new(&self.store_path).map_err(|error| {
            ApiError::internal(format!(
                "failed preparing worker registry lock '{}': {error}",
                self.store_path.display()
            ))
        })
    }

    fn sorted_workers(workers: &BTreeMap<String, WorkerRecord>) -> Vec<WorkerRecord> {
        workers.values().cloned().collect()
    }
}

fn validate_required_string(field_name: &str, value: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::invalid_params(format!(
            "worker.{field_name} must be a non-empty string"
        )));
    }

    Ok(())
}

fn validate_optional_string(field_name: &str, value: Option<&str>) -> Result<(), ApiError> {
    if let Some(value) = value {
        validate_required_string(field_name, value)?;
    }

    Ok(())
}

fn worker_not_found_error(worker_id: &str) -> ApiError {
    ApiError::not_found(format!("Worker with id '{worker_id}' not found"))
        .with_details(serde_json::json!({ "workerId": worker_id }))
}
