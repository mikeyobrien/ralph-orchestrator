use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use ralph_core::FileLock;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;
use crate::task_domain::{TaskDomain, TaskRecord};

const LEASE_DURATION_MINUTES: i64 = 2;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReclaimExpiredInput {
    pub as_of: String,
}

impl WorkerReclaimExpiredInput {
    fn validate(&self) -> Result<(), ApiError> {
        parse_input_timestamp("asOf", &self.as_of)?;
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReclaimExpiredResult {
    pub tasks: Vec<TaskRecord>,
    pub workers: Vec<WorkerRecord>,
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

    pub fn reclaim_expired(
        &mut self,
        input: WorkerReclaimExpiredInput,
    ) -> Result<WorkerReclaimExpiredResult, ApiError> {
        input.validate()?;
        let as_of = parse_input_timestamp("asOf", &input.as_of)?;

        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking worker registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut workers = self.read_workers_from_disk()?;
        let stale_worker_ids = workers
            .iter()
            .filter_map(|(worker_id, worker)| {
                worker.current_task_id.as_ref()?;
                match worker_lease_deadline(worker) {
                    Ok(deadline) if deadline <= as_of => Some(Ok(worker_id.clone())),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        if stale_worker_ids.is_empty() {
            return Ok(WorkerReclaimExpiredResult {
                tasks: Vec::new(),
                workers: Vec::new(),
            });
        }

        let original_workers = workers.clone();
        let mut worker_persisted = false;
        let mut task_domain = TaskDomain::new(self.workspace_root()?);

        let reclaim_result = task_domain.with_exclusive_snapshot(|tasks| {
            let mut reclaimed_tasks = Vec::new();
            let mut reclaimed_workers = Vec::new();

            for worker_id in &stale_worker_ids {
                let worker_snapshot = workers
                    .get(worker_id)
                    .cloned()
                    .ok_or_else(|| worker_not_found_error(worker_id))?;
                let Some(task_id) = worker_snapshot.current_task_id.as_deref() else {
                    continue;
                };

                let effective_lease_expires_at =
                    effective_lease_expires_at(&worker_snapshot, tasks.get(task_id))?;
                if effective_lease_expires_at > as_of {
                    continue;
                }

                if let Some(task) = tasks.get_mut(task_id) {
                    if task.status == "in_progress"
                        && task.assignee_worker_id.as_deref() == Some(worker_id.as_str())
                    {
                        task.status = "ready".to_string();
                        task.assignee_worker_id = None;
                        task.claimed_at = None;
                        task.lease_expires_at = None;
                        task.updated_at = input.as_of.clone();
                        task.completed_at = None;
                        task.error_message = Some(reclaim_reason(
                            &worker_snapshot,
                            task_id,
                            &effective_lease_expires_at,
                            &input.as_of,
                        ));
                        reclaimed_tasks.push(task.clone());
                    }
                }

                let worker = workers
                    .get_mut(worker_id)
                    .ok_or_else(|| worker_not_found_error(worker_id))?;
                worker.status = WorkerStatus::Dead;
                worker.current_task_id = None;
                worker.current_hat = None;
                reclaimed_workers.push(worker.clone());
            }

            if !reclaimed_workers.is_empty() {
                self.persist_workers_to_disk(&workers)?;
                worker_persisted = true;
            }

            Ok(WorkerReclaimExpiredResult {
                tasks: reclaimed_tasks,
                workers: reclaimed_workers,
            })
        });

        match reclaim_result {
            Ok(result) => Ok(result),
            Err(error) => {
                if worker_persisted {
                    self.persist_workers_to_disk(&original_workers)
                        .map_err(|rollback_error| {
                            ApiError::internal(format!(
                                "failed finalizing reclaim_expired at '{}': {}; rollback failed: {}",
                                input.as_of, error.message, rollback_error.message
                            ))
                        })?;

                    return Err(ApiError::internal(format!(
                        "failed finalizing reclaim_expired at '{}': {}; rolled back worker registry",
                        input.as_of, error.message
                    )));
                }

                Err(error)
            }
        }
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
    let lease_expires_at = claimed_at + lease_duration();
    (
        format_timestamp(claimed_at),
        format_timestamp(lease_expires_at),
    )
}

fn lease_duration() -> Duration {
    Duration::minutes(LEASE_DURATION_MINUTES)
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn parse_input_timestamp(field_name: &str, value: &str) -> Result<DateTime<Utc>, ApiError> {
    validate_required_string(field_name, value)?;
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            ApiError::invalid_params(format!(
                "worker.{field_name} must be a valid RFC3339 timestamp: {error}"
            ))
        })
}

fn parse_worker_timestamp(
    worker: &WorkerRecord,
    field_name: &str,
    value: &str,
) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            ApiError::internal(format!(
                "failed parsing worker {field_name} for '{}': {error}",
                worker.worker_id
            ))
        })
}

fn parse_task_timestamp(
    task: &TaskRecord,
    field_name: &str,
    value: &str,
) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            ApiError::internal(format!(
                "failed parsing task {field_name} for '{}': {error}",
                task.id
            ))
        })
}

fn worker_lease_deadline(worker: &WorkerRecord) -> Result<DateTime<Utc>, ApiError> {
    Ok(
        parse_worker_timestamp(worker, "lastHeartbeatAt", &worker.last_heartbeat_at)?
            + lease_duration(),
    )
}

fn effective_lease_expires_at(
    worker: &WorkerRecord,
    task: Option<&TaskRecord>,
) -> Result<DateTime<Utc>, ApiError> {
    let mut deadline = worker_lease_deadline(worker)?;

    if let Some(task) = task {
        if let Some(lease_expires_at) = task.lease_expires_at.as_deref() {
            let task_deadline = parse_task_timestamp(task, "leaseExpiresAt", lease_expires_at)?;
            if task_deadline > deadline {
                deadline = task_deadline;
            }
        }
    }

    Ok(deadline)
}

fn reclaim_reason(
    worker: &WorkerRecord,
    task_id: &str,
    lease_expires_at: &DateTime<Utc>,
    as_of: &str,
) -> String {
    format!(
        "Task '{task_id}' reclaimed after worker '{}' lease expired at {} (last heartbeat {}, as of {as_of})",
        worker.worker_id,
        format_timestamp(*lease_expires_at),
        worker.last_heartbeat_at,
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
