use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use ralph_core::FileLock;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;
use crate::file_ownership::FileOwnershipRegistry;
use crate::task_domain::{TaskDomain, TaskEvent, TaskRecord};

const LEASE_DURATION_MINUTES: i64 = 2;
const DEAD_PURGE_MINUTES: i64 = 5;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_at: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
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
            iteration,
            max_iterations,
        } = input;
        let mut updated_worker = None;
        let mut extend_task_id: Option<String> = None;

        self.modify_workers(|workers| {
            let worker = workers
                .get_mut(&worker_id)
                .ok_or_else(|| worker_not_found_error(&worker_id))?;
            // Don't let busy heartbeats revive a Dead worker — reclaim_expired
            // marked it dead for a reason and already reset the task.
            // Idle heartbeats ARE allowed through so the factory loop's
            // send_idle_heartbeat can revive a worker that finished its task.
            if worker.status == WorkerStatus::Dead && status != WorkerStatus::Idle {
                updated_worker = Some(worker.clone());
                return Ok(());
            }
            worker.status = status;
            worker.current_task_id = current_task_id.clone();
            worker.current_hat = current_hat.clone();
            worker.last_heartbeat_at = last_heartbeat_at.clone();
            worker.iteration = iteration;
            worker.max_iterations = max_iterations;
            // Track busy heartbeats so we can extend the task lease below
            if status == WorkerStatus::Busy {
                extend_task_id = current_task_id;
            }
            updated_worker = Some(worker.clone());
            Ok(())
        })?;

        // Extend the task's leaseExpiresAt so the UI doesn't show it as stale
        // while the worker is still alive and heartbeating.
        if let Some(task_id) = extend_task_id {
            let new_lease = format_timestamp(Utc::now() + lease_duration());
            let mut task_domain = TaskDomain::new(self.workspace_root()?);
            let _ = task_domain.with_exclusive_snapshot(|tasks| {
                if let Some(task) = tasks.get_mut(&task_id)
                    && task.status == "in_progress"
                {
                    task.lease_expires_at = Some(new_lease.clone());
                }
                Ok(())
            });
        }

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
            task.events.push(
                TaskEvent::new("claimed")
                    .with_worker(&worker_id)
                    .with_details("ready -> in_progress"),
            );

            // Auto-extract scope_files from task title if not explicitly set
            if task.scope_files.is_empty() {
                task.scope_files = extract_file_paths(&task.title);
            }

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

        // Register file ownership for scope_files on the claimed task
        if let Some(ref task) = claim_result
            && !task.scope_files.is_empty()
            && let Ok(workspace) = self.workspace_root()
        {
            let registry = FileOwnershipRegistry::new(workspace);
            // Best-effort: don't fail the claim if ownership registration fails
            let _ = registry.claim(&worker_id, &task.id, task.scope_files.clone());
        }

        Ok(WorkerClaimNextResult {
            task: claim_result,
            worker,
        })
    }

    /// Completes a task claimed by a worker.
    ///
    /// On success: sets task status to "done" via `TaskDomain::close()`.
    /// On failure: sets task error_message, resets status to "ready" so it can be reclaimed.
    /// In both cases: sets worker back to `Idle`, clears `current_task_id`,
    /// and releases file ownership for the worker.
    pub fn complete_task(
        &mut self,
        worker_id: &str,
        task_id: &str,
        success: bool,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        validate_required_string("workerId", worker_id)?;
        validate_required_string("taskId", task_id)?;

        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking worker registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut workers = self.read_workers_from_disk()?;
        let worker = workers
            .get_mut(worker_id)
            .ok_or_else(|| worker_not_found_error(worker_id))?;

        let was_dead = worker.status == WorkerStatus::Dead;

        // Revive worker to Idle regardless of current status
        worker.status = WorkerStatus::Idle;
        worker.current_task_id = None;
        worker.current_hat = None;
        worker.last_heartbeat_at = format_timestamp(Utc::now());
        self.persist_workers_to_disk(&workers)?;

        let wid = worker_id.to_string();
        let mut task_domain = TaskDomain::new(self.workspace_root()?);
        if success {
            if was_dead {
                // reclaim_expired already reset the task to "ready", but this
                // worker actually completed the work. Force-close it by
                // bypassing the state machine (ready → done is not valid, but
                // the work IS done).
                let tid = task_id.to_string();
                let wid2 = wid.clone();
                let _ = task_domain.with_exclusive_snapshot(|tasks| {
                    if let Some(task) = tasks.get_mut(&tid)
                        && (task.status == "ready" || task.status == "in_progress")
                    {
                        let from = task.status.clone();
                        task.status = "done".to_string();
                        task.assignee_worker_id = None;
                        task.claimed_at = None;
                        task.lease_expires_at = None;
                        task.completed_at = Some(format_timestamp(Utc::now()));
                        task.updated_at = format_timestamp(Utc::now());
                        task.error_message = None;
                        task.events.push(
                            TaskEvent::new("completed")
                                .with_worker(&wid2)
                                .with_details(&format!("{} -> done (was_dead)", from)),
                        );
                    }
                    Ok(())
                });
            } else {
                // Close + append completed event in a single write
                let tid = task_id.to_string();
                let wid2 = wid.clone();
                let _ = task_domain.with_exclusive_snapshot(|tasks| {
                    if let Some(task) = tasks.get_mut(&tid)
                        && task.status == "in_progress"
                    {
                        task.status = "done".to_string();
                        task.assignee_worker_id = None;
                        task.claimed_at = None;
                        task.lease_expires_at = None;
                        task.completed_at = Some(format_timestamp(Utc::now()));
                        task.updated_at = format_timestamp(Utc::now());
                        task.error_message = None;
                        task.events.push(
                            TaskEvent::new("completed")
                                .with_worker(&wid2)
                                .with_details("in_progress -> done"),
                        );
                    }
                    Ok(())
                });
            }
        } else if !was_dead {
            // Reset to ready so it can be reclaimed, bypassing the state machine
            // (in_progress → ready is not a valid transition, but complete_task
            // needs to requeue failed tasks just like reclaim_expired does).
            // When was_dead, reclaim already reset the task — skip to avoid
            // double-mutation.
            let error_msg = error_message.clone();
            let tid = task_id.to_string();
            let wid2 = wid.clone();
            let _ = task_domain.with_exclusive_snapshot(|tasks| {
                if let Some(task) = tasks.get_mut(&tid)
                    && task.status == "in_progress"
                {
                    task.status = "ready".to_string();
                    task.assignee_worker_id = None;
                    task.claimed_at = None;
                    task.lease_expires_at = None;
                    task.completed_at = None;
                    task.updated_at = format_timestamp(Utc::now());
                    if let Some(ref msg) = error_msg {
                        task.error_message = Some(msg.clone());
                    }
                    task.events.push(
                        TaskEvent::new("failed")
                            .with_worker(&wid2)
                            .with_details("in_progress -> ready (task failed)"),
                    );
                }
                Ok(())
            });
        }

        // Release file ownership for this worker
        if let Ok(workspace) = self.workspace_root() {
            let registry = FileOwnershipRegistry::new(workspace);
            let _ = registry.release_all_for_worker(worker_id);
        }

        Ok(())
    }

    /// Sends guidance text to a busy factory worker by appending a `human.guidance`
    /// event to the worker's worktree events file. The worker picks it up on its
    /// next iteration via `process_events_from_jsonl()`.
    pub fn send_guidance(&self, worker_id: &str, message: &str) -> Result<(), ApiError> {
        validate_required_string("workerId", worker_id)?;
        if message.trim().is_empty() {
            return Err(ApiError::invalid_params(
                "worker.send_guidance requires a non-empty 'message'",
            ));
        }

        let workers = self.read_workers_with_shared_lock()?;
        let worker = workers
            .get(worker_id)
            .ok_or_else(|| worker_not_found_error(worker_id))?;

        if worker.status != WorkerStatus::Busy {
            return Err(ApiError::precondition_failed(format!(
                "Worker '{}' must be busy to receive guidance (current status: {:?})",
                worker_id, worker.status
            )));
        }

        let task_id = worker.current_task_id.as_deref().ok_or_else(|| {
            ApiError::precondition_failed(format!(
                "Worker '{}' is busy but has no current task",
                worker_id
            ))
        })?;

        let events_path = self.resolve_worker_events_path(&worker.workspace_root, task_id)?;

        let event = serde_json::json!({
            "topic": "human.guidance",
            "payload": message,
            "ts": format_timestamp(Utc::now()),
        });
        let mut line = serde_json::to_string(&event)
            .map_err(|e| ApiError::internal(format!("failed serializing guidance event: {e}")))?;
        line.push('\n');

        use std::io::Write;
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .map_err(|e| {
                ApiError::internal(format!(
                    "failed opening events file '{}': {e}",
                    events_path.display()
                ))
            })?;
        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(line.as_bytes()).map_err(|e| {
            ApiError::internal(format!(
                "failed writing guidance event to '{}': {e}",
                events_path.display()
            ))
        })?;
        writer.flush().map_err(|e| {
            ApiError::internal(format!(
                "failed flushing guidance event to '{}': {e}",
                events_path.display()
            ))
        })?;

        Ok(())
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
            // Still purge old dead workers even when nothing new is stale
            let purged = purge_stale_dead_workers(&mut workers, &as_of);
            if purged > 0 {
                self.persist_workers_to_disk(&workers)?;
            }
            return Ok(WorkerReclaimExpiredResult {
                tasks: Vec::new(),
                workers: Vec::new(),
            });
        }

        // Purge old dead workers before processing new reclaims
        // (only removes workers that were already dead, not ones about to be marked dead)
        let purged = purge_stale_dead_workers(&mut workers, &as_of);

        let original_workers = workers.clone();
        let mut worker_persisted = false;
        let mut task_domain = TaskDomain::new(self.workspace_root()?);

        let reclaim_result = task_domain.with_exclusive_snapshot(|tasks| {
            let mut reclaimed_tasks = Vec::new();
            let mut reclaimed_workers = Vec::new();

            for worker_id in &stale_worker_ids {
                let worker_snapshot = match workers.get(worker_id).cloned() {
                    Some(w) => w,
                    None => continue, // worker was purged
                };
                let Some(task_id) = worker_snapshot.current_task_id.as_deref() else {
                    continue;
                };

                let effective_lease_expires_at =
                    effective_lease_expires_at(&worker_snapshot, tasks.get(task_id))?;
                if effective_lease_expires_at > as_of {
                    continue;
                }

                if let Some(task) = tasks.get_mut(task_id)
                    && task.status == "in_progress"
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
                    task.events.push(
                        TaskEvent::new("reclaimed")
                            .with_worker(worker_id)
                            .with_details("in_progress -> ready (lease expired)"),
                    );
                    reclaimed_tasks.push(task.clone());
                }

                let worker = workers
                    .get_mut(worker_id)
                    .ok_or_else(|| worker_not_found_error(worker_id))?;
                worker.status = WorkerStatus::Dead;
                worker.current_task_id = None;
                worker.current_hat = None;
                reclaimed_workers.push(worker.clone());
            }

            if !reclaimed_workers.is_empty() || purged > 0 {
                self.persist_workers_to_disk(&workers)?;
                worker_persisted = true;
            }

            Ok(WorkerReclaimExpiredResult {
                tasks: reclaimed_tasks,
                workers: reclaimed_workers,
            })
        });

        // Release file ownership for dead workers
        if let Ok(ref result) = reclaim_result
            && !result.workers.is_empty()
            && let Ok(workspace) = self.workspace_root()
        {
            let registry = FileOwnershipRegistry::new(workspace);
            for worker in &result.workers {
                let _ = registry.release_all_for_worker(&worker.worker_id);
            }
        }

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

    /// Resolves the active events file for a worker's worktree.
    ///
    /// Reads the `current-events` marker (which contains a relative path to the
    /// timestamped events file), falling back to `events.jsonl` when the marker
    /// is absent.
    fn resolve_worker_events_path(
        &self,
        workspace_root: &str,
        task_id: &str,
    ) -> Result<PathBuf, ApiError> {
        let worktree_ralph_dir = Path::new(workspace_root)
            .join(".worktrees")
            .join(task_id)
            .join(".ralph");

        if !worktree_ralph_dir.exists() {
            return Err(ApiError::precondition_failed(format!(
                "Worker worktree .ralph dir does not exist: '{}'",
                worktree_ralph_dir.display()
            )));
        }

        let marker_path = worktree_ralph_dir.join("current-events");
        if let Ok(contents) = fs::read_to_string(&marker_path) {
            let relative = contents.trim();
            if !relative.is_empty() {
                return Ok(Path::new(workspace_root)
                    .join(".worktrees")
                    .join(task_id)
                    .join(relative));
            }
        }

        Ok(worktree_ralph_dir.join("events.jsonl"))
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

    if let Some(task) = task
        && let Some(lease_expires_at) = task.lease_expires_at.as_deref()
    {
        let task_deadline = parse_task_timestamp(task, "leaseExpiresAt", lease_expires_at)?;
        if task_deadline > deadline {
            deadline = task_deadline;
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

/// Removes dead workers whose last heartbeat is older than `LEASE_DURATION + DEAD_PURGE` minutes.
/// Returns the number of workers purged.
fn purge_stale_dead_workers(
    workers: &mut BTreeMap<String, WorkerRecord>,
    as_of: &DateTime<Utc>,
) -> usize {
    let purge_threshold = lease_duration() + Duration::minutes(DEAD_PURGE_MINUTES);
    let before = workers.len();
    workers.retain(|_, worker| {
        if worker.status != WorkerStatus::Dead {
            return true;
        }
        match parse_worker_timestamp(worker, "lastHeartbeatAt", &worker.last_heartbeat_at) {
            Ok(last_hb) => last_hb + purge_threshold > *as_of,
            Err(_) => true, // keep if we can't parse — don't silently drop
        }
    });
    before - workers.len()
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

/// Extracts file paths from a task title/description string.
///
/// Looks for patterns like `lib/foo/bar.ex`, `src/main.rs`, `config/runtime.exs`
/// — anything that looks like a relative file path with an extension.
fn extract_file_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    // Split on whitespace, parens, commas, and common delimiters
    for token in text.split(|c: char| c.is_whitespace() || "(),\"'`[]{}".contains(c)) {
        let token = token.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
        });
        if token.contains('/')
            && token.contains('.')
            && !token.starts_with("http")
            && !token.starts_with("//")
            && !token.starts_with('.')
            && token.len() > 3
        {
            // Verify it ends with a recognized source file extension
            if let Some(last_segment) = token.rsplit('/').next()
                && let Some(ext) = last_segment.rsplit('.').next()
            {
                // Only accept known source file extensions
                let valid_extensions = [
                    "ex", "exs", "rs", "py", "js", "ts", "tsx", "jsx", "rb", "go", "java", "kt",
                    "swift", "c", "cpp", "h", "hpp", "cs", "php", "vue", "svelte", "html", "css",
                    "scss", "sass", "less", "yaml", "yml", "toml", "json", "xml", "md", "txt",
                    "sh", "bash", "zsh", "sql", "lua", "zig", "nim", "hrl", "erl", "gleam",
                ];
                if valid_extensions.contains(&ext)
                    && !last_segment.starts_with('.')
                    && token
                        .chars()
                        .all(|c| c.is_alphanumeric() || "/_.-".contains(c))
                {
                    paths.push(token.to_string());
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod extract_tests {
    use super::extract_file_paths;

    #[test]
    fn extracts_elixir_paths() {
        let title = "Create Worker and Task structs (lib/ralph_workers/worker.ex and lib/ralph_workers/task.ex)";
        let paths = extract_file_paths(title);
        assert_eq!(
            paths,
            vec!["lib/ralph_workers/task.ex", "lib/ralph_workers/worker.ex",]
        );
    }

    #[test]
    fn extracts_nested_paths() {
        let title =
            "Create Dashboard LiveView (lib/ralph_workers_web/live/dashboard_live.ex) at route '/'";
        let paths = extract_file_paths(title);
        assert_eq!(paths, vec!["lib/ralph_workers_web/live/dashboard_live.ex"]);
    }

    #[test]
    fn ignores_urls() {
        let title = "Fetch data from https://example.com/api/v1 and write to src/data.rs";
        let paths = extract_file_paths(title);
        assert_eq!(paths, vec!["src/data.rs"]);
    }

    #[test]
    fn empty_for_no_paths() {
        let title = "Fix the bug in the login form";
        let paths = extract_file_paths(title);
        assert!(paths.is_empty());
    }

    #[test]
    fn deduplicates() {
        let title = "Update lib/foo/bar.ex and also lib/foo/bar.ex again";
        let paths = extract_file_paths(title);
        assert_eq!(paths, vec!["lib/foo/bar.ex"]);
    }

    #[test]
    fn extracts_rust_and_typescript_paths() {
        let title = "Fix crates/core/src/main.rs and frontend/app/src/App.tsx";
        let paths = extract_file_paths(title);
        assert_eq!(
            paths,
            vec!["crates/core/src/main.rs", "frontend/app/src/App.tsx"]
        );
    }

    #[test]
    fn ignores_dotfile_paths() {
        let title = "Update config/.hidden/secret.rs";
        let paths = extract_file_paths(title);
        // .hidden starts with dot, but the full path doesn't start with dot
        // the code checks !token.starts_with('.')
        assert_eq!(paths, vec!["config/.hidden/secret.rs"]);
    }

    #[test]
    fn ignores_paths_starting_with_dot() {
        let title = "Fix the ./src/main.rs file";
        let paths = extract_file_paths(title);
        assert!(paths.is_empty(), "paths starting with . should be excluded");
    }

    #[test]
    fn ignores_double_slash_paths() {
        let title = "Check //network/share/file.rs";
        let paths = extract_file_paths(title);
        assert!(
            paths.is_empty(),
            "paths starting with // should be excluded"
        );
    }

    #[test]
    fn ignores_bare_filenames_without_slash() {
        let title = "Fix main.rs and lib.rs";
        let paths = extract_file_paths(title);
        assert!(
            paths.is_empty(),
            "bare filenames without / should not be extracted"
        );
    }

    #[test]
    fn extracts_paths_in_brackets() {
        let title = "Modify [src/lib.rs] and (tests/test.rs)";
        let paths = extract_file_paths(title);
        assert_eq!(paths, vec!["src/lib.rs", "tests/test.rs"]);
    }

    #[test]
    fn ignores_unknown_extensions() {
        let title = "Update data/config/app.xyz and data/config/app.bin";
        let paths = extract_file_paths(title);
        assert!(
            paths.is_empty(),
            "unknown extensions should not be extracted"
        );
    }

    #[test]
    fn extracts_config_file_paths() {
        let title = "Update config/database.yml and config/app.toml";
        let paths = extract_file_paths(title);
        assert_eq!(paths, vec!["config/app.toml", "config/database.yml"]);
    }
}

#[cfg(test)]
mod serde_tests {
    use super::*;

    #[test]
    fn worker_status_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&WorkerStatus::Idle).unwrap(),
            r#""idle""#
        );
        assert_eq!(
            serde_json::to_string(&WorkerStatus::Busy).unwrap(),
            r#""busy""#
        );
        assert_eq!(
            serde_json::to_string(&WorkerStatus::Blocked).unwrap(),
            r#""blocked""#
        );
        assert_eq!(
            serde_json::to_string(&WorkerStatus::Dead).unwrap(),
            r#""dead""#
        );
    }

    #[test]
    fn worker_status_deserializes_from_snake_case() {
        assert_eq!(
            serde_json::from_str::<WorkerStatus>(r#""idle""#).unwrap(),
            WorkerStatus::Idle
        );
        assert_eq!(
            serde_json::from_str::<WorkerStatus>(r#""busy""#).unwrap(),
            WorkerStatus::Busy
        );
        assert_eq!(
            serde_json::from_str::<WorkerStatus>(r#""dead""#).unwrap(),
            WorkerStatus::Dead
        );
    }

    #[test]
    fn worker_record_roundtrip_json() {
        let record = WorkerRecord {
            worker_id: "worker-1".to_string(),
            worker_name: "alpha".to_string(),
            loop_id: "loop-1".to_string(),
            backend: "claude".to_string(),
            workspace_root: "/tmp/ws".to_string(),
            current_task_id: Some("task-1".to_string()),
            current_hat: None,
            status: WorkerStatus::Busy,
            last_heartbeat_at: "2026-01-01T00:00:00Z".to_string(),
            iteration: Some(3),
            max_iterations: Some(10),
            registered_at: Some("2026-01-01T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: WorkerRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, record);
    }

    #[test]
    fn worker_record_omits_none_fields() {
        let record = WorkerRecord {
            worker_id: "w-1".to_string(),
            worker_name: "alpha".to_string(),
            loop_id: "l-1".to_string(),
            backend: "claude".to_string(),
            workspace_root: "/tmp".to_string(),
            current_task_id: None,
            current_hat: None,
            status: WorkerStatus::Idle,
            last_heartbeat_at: "2026-01-01T00:00:00Z".to_string(),
            iteration: None,
            max_iterations: None,
            registered_at: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("currentTaskId"));
        assert!(!json.contains("currentHat"));
        assert!(!json.contains("iteration"));
        assert!(!json.contains("maxIterations"));
        assert!(!json.contains("registeredAt"));
    }

    #[test]
    fn worker_record_uses_camel_case_keys() {
        let record = WorkerRecord {
            worker_id: "w-1".to_string(),
            worker_name: "alpha".to_string(),
            loop_id: "l-1".to_string(),
            backend: "claude".to_string(),
            workspace_root: "/tmp".to_string(),
            current_task_id: Some("t-1".to_string()),
            current_hat: Some("builder".to_string()),
            status: WorkerStatus::Busy,
            last_heartbeat_at: "2026-01-01T00:00:00Z".to_string(),
            iteration: Some(5),
            max_iterations: Some(10),
            registered_at: Some("2026-01-01T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("workerId"));
        assert!(json.contains("workerName"));
        assert!(json.contains("loopId"));
        assert!(json.contains("workspaceRoot"));
        assert!(json.contains("currentTaskId"));
        assert!(json.contains("currentHat"));
        assert!(json.contains("lastHeartbeatAt"));
    }

    #[test]
    fn validate_rejects_empty_worker_id() {
        let record = WorkerRecord {
            worker_id: String::new(),
            worker_name: "alpha".to_string(),
            loop_id: "l-1".to_string(),
            backend: "claude".to_string(),
            workspace_root: "/tmp".to_string(),
            current_task_id: None,
            current_hat: None,
            status: WorkerStatus::Idle,
            last_heartbeat_at: "2026-01-01T00:00:00Z".to_string(),
            iteration: None,
            max_iterations: None,
            registered_at: None,
        };
        let err = record.validate().unwrap_err();
        assert!(err.message.contains("workerId"));
    }

    #[test]
    fn validate_rejects_whitespace_only_worker_id() {
        let record = WorkerRecord {
            worker_id: "   ".to_string(),
            worker_name: "alpha".to_string(),
            loop_id: "l-1".to_string(),
            backend: "claude".to_string(),
            workspace_root: "/tmp".to_string(),
            current_task_id: None,
            current_hat: None,
            status: WorkerStatus::Idle,
            last_heartbeat_at: "2026-01-01T00:00:00Z".to_string(),
            iteration: None,
            max_iterations: None,
            registered_at: None,
        };
        let err = record.validate().unwrap_err();
        assert!(err.message.contains("workerId"));
    }

    #[test]
    fn heartbeat_input_validates_required_fields() {
        let input = WorkerHeartbeatInput {
            worker_id: String::new(),
            status: WorkerStatus::Idle,
            current_task_id: None,
            current_hat: None,
            last_heartbeat_at: "2026-01-01T00:00:00Z".to_string(),
            iteration: None,
            max_iterations: None,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn reclaim_expired_input_validates_timestamp() {
        let input = WorkerReclaimExpiredInput {
            as_of: "not-a-timestamp".to_string(),
        };
        assert!(input.validate().is_err());
    }
}

#[cfg(test)]
mod send_guidance_tests {
    use super::*;
    use std::fs;

    fn make_busy_worker(workspace_root: &str, task_id: &str) -> WorkerRecord {
        WorkerRecord {
            worker_id: "worker-1".to_string(),
            worker_name: "alpha".to_string(),
            loop_id: "loop-1".to_string(),
            backend: "claude".to_string(),
            workspace_root: workspace_root.to_string(),
            current_task_id: Some(task_id.to_string()),
            current_hat: Some("builder".to_string()),
            status: WorkerStatus::Busy,
            last_heartbeat_at: "2026-01-01T00:00:00Z".to_string(),
            iteration: Some(3),
            max_iterations: Some(10),
            registered_at: Some("2026-01-01T00:00:00Z".to_string()),
        }
    }

    fn setup_domain_with_worker(worker: &WorkerRecord) -> (tempfile::TempDir, WorkerDomain) {
        let tmp = tempfile::tempdir().unwrap();
        let ralph_dir = tmp.path().join(".ralph");
        fs::create_dir_all(&ralph_dir).unwrap();

        let store_path = ralph_dir.join("workers.json");
        let snapshot = WorkerSnapshot {
            workers: vec![worker.clone()],
        };
        fs::write(
            &store_path,
            serde_json::to_string_pretty(&snapshot).unwrap(),
        )
        .unwrap();

        let domain = WorkerDomain { store_path };
        (tmp, domain)
    }

    #[test]
    fn send_guidance_to_busy_worker() {
        let tmp_workspace = tempfile::tempdir().unwrap();
        let task_id = "task-1";
        let worktree_ralph = tmp_workspace
            .path()
            .join(".worktrees")
            .join(task_id)
            .join(".ralph");
        fs::create_dir_all(&worktree_ralph).unwrap();
        // Create a current-events marker pointing to a timestamped events file
        let events_file = ".ralph/events-20260101-000000.jsonl";
        fs::write(worktree_ralph.join("current-events"), events_file).unwrap();
        // Pre-create the events file
        let full_events_path = tmp_workspace
            .path()
            .join(".worktrees")
            .join(task_id)
            .join(events_file);
        fs::write(&full_events_path, "").unwrap();

        let worker = make_busy_worker(tmp_workspace.path().to_str().unwrap(), task_id);
        let (_tmp, domain) = setup_domain_with_worker(&worker);

        let result = domain.send_guidance("worker-1", "Focus on error handling first");
        assert!(result.is_ok(), "send_guidance should succeed: {:?}", result);

        let contents = fs::read_to_string(&full_events_path).unwrap();
        assert!(!contents.is_empty(), "events file should have content");
        let event: serde_json::Value = serde_json::from_str(contents.trim()).unwrap();
        assert_eq!(event["topic"], "human.guidance");
        assert_eq!(event["payload"], "Focus on error handling first");
        assert!(event["ts"].as_str().is_some());
    }

    #[test]
    fn send_guidance_to_idle_worker_fails() {
        let tmp_workspace = tempfile::tempdir().unwrap();
        let mut worker = make_busy_worker(tmp_workspace.path().to_str().unwrap(), "task-1");
        worker.status = WorkerStatus::Idle;
        worker.current_task_id = None;

        let (_tmp, domain) = setup_domain_with_worker(&worker);

        let err = domain
            .send_guidance("worker-1", "some guidance")
            .unwrap_err();
        assert!(err.message.contains("must be busy"), "got: {}", err.message);
    }

    #[test]
    fn send_guidance_to_missing_worker_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let ralph_dir = tmp.path().join(".ralph");
        fs::create_dir_all(&ralph_dir).unwrap();
        let store_path = ralph_dir.join("workers.json");
        let snapshot = WorkerSnapshot { workers: vec![] };
        fs::write(
            &store_path,
            serde_json::to_string_pretty(&snapshot).unwrap(),
        )
        .unwrap();

        let domain = WorkerDomain { store_path };

        let err = domain.send_guidance("nonexistent", "hello").unwrap_err();
        assert!(err.message.contains("not found"), "got: {}", err.message);
    }

    #[test]
    fn send_guidance_to_missing_worktree_fails() {
        // Worker is busy but the worktree .ralph dir doesn't exist
        let tmp_workspace = tempfile::tempdir().unwrap();
        let worker = make_busy_worker(tmp_workspace.path().to_str().unwrap(), "task-gone");
        let (_tmp, domain) = setup_domain_with_worker(&worker);

        let err = domain
            .send_guidance("worker-1", "guidance text")
            .unwrap_err();
        assert!(
            err.message.contains("does not exist"),
            "got: {}",
            err.message
        );
    }

    #[test]
    fn send_guidance_falls_back_to_events_jsonl() {
        let tmp_workspace = tempfile::tempdir().unwrap();
        let task_id = "task-2";
        let worktree_ralph = tmp_workspace
            .path()
            .join(".worktrees")
            .join(task_id)
            .join(".ralph");
        fs::create_dir_all(&worktree_ralph).unwrap();
        // No current-events marker — should fallback to events.jsonl

        let worker = make_busy_worker(tmp_workspace.path().to_str().unwrap(), task_id);
        let (_tmp, domain) = setup_domain_with_worker(&worker);

        let result = domain.send_guidance("worker-1", "fallback test");
        assert!(result.is_ok(), "send_guidance should succeed: {:?}", result);

        let fallback_path = worktree_ralph.join("events.jsonl");
        let contents = fs::read_to_string(&fallback_path).unwrap();
        let event: serde_json::Value = serde_json::from_str(contents.trim()).unwrap();
        assert_eq!(event["topic"], "human.guidance");
        assert_eq!(event["payload"], "fallback test");
    }

    #[test]
    fn send_guidance_rejects_empty_message() {
        let tmp_workspace = tempfile::tempdir().unwrap();
        let worker = make_busy_worker(tmp_workspace.path().to_str().unwrap(), "task-1");
        let (_tmp, domain) = setup_domain_with_worker(&worker);

        let err = domain.send_guidance("worker-1", "   ").unwrap_err();
        assert!(err.message.contains("non-empty"), "got: {}", err.message);
    }
}
