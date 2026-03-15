use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Duration, SecondsFormat, Utc};
use ralph_core::FileLock;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;
use crate::task_domain::{TaskDomain, TaskRecord};

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerClaimNextResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskRecord>,
    pub worker: WorkerRecord,
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

    pub fn claim_next(&mut self, worker_id: &str) -> Result<WorkerClaimNextResult, ApiError> {
        validate_required_string("workerId", worker_id)?;
        let worker_id = worker_id.to_string();

        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking worker registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut workers = self.read_workers_from_disk()?;
        let worker = workers
            .get(&worker_id)
            .cloned()
            .ok_or_else(|| worker_not_found_error(&worker_id))?;
        if worker.status != WorkerStatus::Idle || worker.current_task_id.is_some() {
            return Err(worker_not_idle_error(&worker));
        }

        let original_workers = workers.clone();
        let (claimed_at, lease_expires_at) = claim_timestamps();
        let mut worker_persisted = false;
        let mut task_domain = TaskDomain::new(self.workspace_root()?);

        let claim_result = task_domain.with_exclusive_snapshot(|tasks| {
            let Some(next_ready_task) = TaskDomain::ready_from_tasks(tasks).into_iter().next()
            else {
                return Ok(None);
            };

            let task = tasks.get_mut(&next_ready_task.id).ok_or_else(|| {
                ApiError::internal(format!(
                    "failed claiming ready task '{}': snapshot changed before it could be updated",
                    next_ready_task.id
                ))
            })?;
            task.status = "in_progress".to_string();
            task.assignee_worker_id = Some(worker_id.clone());
            task.claimed_at = Some(claimed_at.clone());
            task.lease_expires_at = Some(lease_expires_at.clone());
            task.updated_at = claimed_at.clone();
            task.completed_at = None;
            task.error_message = None;
            let claimed_task = task.clone();

            let worker = workers
                .get_mut(&worker_id)
                .ok_or_else(|| worker_not_found_error(&worker_id))?;
            worker.status = WorkerStatus::Busy;
            worker.current_task_id = Some(claimed_task.id.clone());
            worker.current_hat = None;
            worker.last_heartbeat_at = claimed_at.clone();

            self.persist_workers_to_disk(&workers)?;
            worker_persisted = true;

            Ok(Some(claimed_task))
        });

        let claim_result = match claim_result {
            Ok(result) => result,
            Err(error) => {
                if worker_persisted {
                    self.persist_workers_to_disk(&original_workers)
                        .map_err(|rollback_error| {
                            ApiError::internal(format!(
                                "failed finalizing claim_next for worker '{}': {}; rollback failed: {}",
                                worker_id, error.message, rollback_error.message
                            ))
                        })?;

                    return Err(ApiError::internal(format!(
                        "failed finalizing claim_next for worker '{}': {}; rolled back worker registry",
                        worker_id, error.message
                    )));
                }

                return Err(error);
            }
        };

        let worker = if claim_result.is_some() {
            workers
                .get(&worker_id)
                .cloned()
                .ok_or_else(|| worker_not_found_error(&worker_id))?
        } else {
            worker
        };

        Ok(WorkerClaimNextResult {
            task: claim_result,
            worker,
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

    fn workspace_root(&self) -> Result<&Path, ApiError> {
        self.store_path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                ApiError::internal(format!(
                    "failed resolving workspace root from worker registry '{}': unexpected path layout",
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

fn claim_timestamps() -> (String, String) {
    let claimed_at = Utc::now();
    let lease_expires_at = claimed_at + Duration::minutes(2);
    (
        claimed_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        lease_expires_at.to_rfc3339_opts(SecondsFormat::Secs, true),
    )
}

fn worker_not_idle_error(worker: &WorkerRecord) -> ApiError {
    ApiError::precondition_failed(format!(
        "Worker with id '{}' must be idle and unassigned before claiming the next task",
        worker.worker_id
    ))
    .with_details(serde_json::json!({
        "workerId": worker.worker_id,
        "status": worker.status,
        "currentTaskId": worker.current_task_id,
    }))
}

fn worker_not_found_error(worker_id: &str) -> ApiError {
    ApiError::not_found(format!("Worker with id '{worker_id}' not found"))
        .with_details(serde_json::json!({ "workerId": worker_id }))
}
