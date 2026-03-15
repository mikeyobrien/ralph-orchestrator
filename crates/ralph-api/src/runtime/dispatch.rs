use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use tracing::warn;

use super::{IdOnlyParams, RpcRuntime};
use crate::collection_domain::{
    CollectionCreateParams, CollectionImportParams, CollectionUpdateParams,
};
use crate::config_domain::ConfigUpdateParams;
use crate::errors::ApiError;
use crate::loop_domain::{
    LoopListParams, LoopRecord, LoopRetryParams, LoopStopMergeParams, LoopTriggerMergeTaskParams,
};
use crate::planning_domain::{
    PlanningGetArtifactParams, PlanningRespondParams, PlanningStartParams,
};
use crate::protocol::{API_VERSION, RpcRequestEnvelope};
use crate::stream_domain::{StreamAckParams, StreamSubscribeParams, StreamUnsubscribeParams};
use crate::task_domain::{TaskCreateParams, TaskListParams, TaskRecord, TaskUpdateInput};
use crate::worker_domain::{WorkerHeartbeatInput, WorkerReclaimExpiredInput, WorkerRecord};

impl RpcRuntime {
    pub(super) fn dispatch(
        &self,
        request: &RpcRequestEnvelope,
        principal: &str,
    ) -> Result<Value, ApiError> {
        let result = match request.method.as_str() {
            "system.health" => Ok(self.health_payload()),
            "system.version" => Ok(json!({
                "apiVersion": API_VERSION,
                "serverVersion": env!("CARGO_PKG_VERSION")
            })),
            "system.capabilities" => Ok(self.capabilities_payload()),
            method if method.starts_with("task.") => self.dispatch_task(request),
            method if method.starts_with("loop.") => self.dispatch_loop(request),
            method if method.starts_with("planning.") => self.dispatch_planning(request),
            method if method.starts_with("config.") => self.dispatch_config(request),
            method if method.starts_with("preset.") => self.dispatch_preset(request),
            method if method.starts_with("collection.") => self.dispatch_collection(request),
            method if method.starts_with("worker.") => self.dispatch_worker(request),
            method if method.starts_with("stream.") => self.dispatch_stream(request, principal),
            "_internal.publish" => self.dispatch_internal_publish(request),
            _ => {
                warn!(
                    method = %request.method,
                    "recognized method is not implemented in rpc runtime"
                );
                Err(ApiError::service_unavailable(format!(
                    "method '{}' is recognized but not implemented in rpc runtime",
                    request.method
                )))
            }
        };

        if let Ok(payload) = &result
            && !request.method.starts_with("stream.")
        {
            self.stream_domain()
                .publish_rpc_side_effect(&request.method, &request.params, payload);
        }

        result
    }

    fn dispatch_task(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        // Load worker snapshot once for enrichment (cheap in-memory read).
        // Falls back to empty map if worker domain is unavailable so task
        // RPCs never fail due to worker-domain issues.
        let workers: BTreeMap<String, WorkerRecord> = self
            .worker_domain_mut()
            .and_then(|wd| wd.list())
            .unwrap_or_default()
            .into_iter()
            .map(|w| (w.worker_id.clone(), w))
            .collect();

        match request.method.as_str() {
            "task.list" => {
                let params: TaskListParams = self.parse_params(request)?;
                let tasks = self.task_domain_mut()?.list(params);
                Ok(json!({ "tasks": enrich_tasks(tasks, &workers) }))
            }
            "task.get" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.get(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.ready" => {
                let tasks = self.task_domain_mut()?.ready();
                Ok(json!({ "tasks": enrich_tasks(tasks, &workers) }))
            }
            "task.in_review" => {
                let tasks = self.task_domain_mut()?.in_review();
                Ok(json!({ "tasks": enrich_tasks(tasks, &workers) }))
            }
            "task.create" => {
                let params: TaskCreateParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.create(params)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.update" => {
                let input = parse_task_update_input(request)?;
                let task = self.task_domain_mut()?.update(input)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.close" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.close(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.archive" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.archive(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.unarchive" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.unarchive(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.delete" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                self.task_domain_mut()?.delete(&params.id)?;
                Ok(json!({ "success": true }))
            }
            "task.clear" => {
                self.task_domain_mut()?.clear()?;
                Ok(json!({ "success": true }))
            }
            "task.retry" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.retry(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.promote" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.promote(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.submit_for_review" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.submit_for_review(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.request_changes" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.request_changes(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            "task.cancel" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let task = self.task_domain_mut()?.cancel(&params.id)?;
                Ok(json!({ "task": enrich_task(task, &workers) }))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }

    fn dispatch_loop(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        // Load worker snapshot once for loop enrichment (same pattern as dispatch_task).
        // Falls back to empty map if worker domain is unavailable.
        let workers: BTreeMap<String, WorkerRecord> = self
            .worker_domain_mut()
            .and_then(|wd| wd.list())
            .unwrap_or_default()
            .into_iter()
            .map(|w| (w.worker_id.clone(), w))
            .collect();

        match request.method.as_str() {
            "loop.list" => {
                let params: LoopListParams = self.parse_params(request)?;
                let loops = self.loop_domain_mut()?.list(params)?;
                Ok(json!({ "loops": enrich_loops(loops, &workers) }))
            }
            "loop.status" => {
                let status = self.loop_domain_mut()?.status();
                Ok(json!(status))
            }
            "loop.process" => {
                self.loop_domain_mut()?.process()?;
                Ok(json!({ "success": true }))
            }
            "loop.prune" => {
                self.loop_domain_mut()?.prune()?;
                Ok(json!({ "success": true }))
            }
            "loop.retry" => {
                let params: LoopRetryParams = self.parse_params(request)?;
                self.loop_domain_mut()?.retry(params)?;
                Ok(json!({ "success": true }))
            }
            "loop.discard" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                self.loop_domain_mut()?.discard(&params.id)?;
                Ok(json!({ "success": true }))
            }
            "loop.stop" => {
                let params: LoopStopMergeParams = self.parse_params(request)?;
                self.loop_domain_mut()?.stop(params)?;
                Ok(json!({ "success": true }))
            }
            "loop.merge" => {
                let params: LoopStopMergeParams = self.parse_params(request)?;
                self.loop_domain_mut()?.merge(params)?;
                Ok(json!({ "success": true }))
            }
            "loop.merge_button_state" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let state = self.loop_domain_mut()?.merge_button_state(&params.id)?;
                Ok(json!(state))
            }
            "loop.trigger_merge_task" => {
                let params: LoopTriggerMergeTaskParams = self.parse_params(request)?;
                let loops = self.loop_domain_mut()?;
                let mut tasks = self.task_domain_mut()?;
                let result = loops.trigger_merge_task(params, &mut tasks)?;
                Ok(json!(result))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }

    fn dispatch_planning(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        match request.method.as_str() {
            "planning.list" => {
                let sessions = self.planning_domain_mut()?.list()?;
                Ok(json!({ "sessions": sessions }))
            }
            "planning.get" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let session = self.planning_domain_mut()?.get(&params.id)?;
                Ok(json!({ "session": session }))
            }
            "planning.start" => {
                let params: PlanningStartParams = self.parse_params(request)?;
                let session = self.planning_domain_mut()?.start(params)?;
                Ok(json!({ "session": session }))
            }
            "planning.respond" => {
                let params: PlanningRespondParams = self.parse_params(request)?;
                self.planning_domain_mut()?.respond(params)?;
                Ok(json!({ "success": true }))
            }
            "planning.resume" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                self.planning_domain_mut()?.resume(&params.id)?;
                Ok(json!({ "success": true }))
            }
            "planning.delete" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                self.planning_domain_mut()?.delete(&params.id)?;
                Ok(json!({ "success": true }))
            }
            "planning.get_artifact" => {
                let params: PlanningGetArtifactParams = self.parse_params(request)?;
                let artifact = self.planning_domain_mut()?.get_artifact(params)?;
                Ok(json!(artifact))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }

    fn dispatch_config(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        match request.method.as_str() {
            "config.get" => {
                let config = self.config_domain().get()?;
                Ok(json!(config))
            }
            "config.update" => {
                let params: ConfigUpdateParams = self.parse_params(request)?;
                let result = self.config_domain().update(params)?;
                Ok(json!(result))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }

    fn dispatch_preset(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        match request.method.as_str() {
            "preset.list" => {
                let collections = self.collection_domain_mut()?.list();
                let presets = self.preset_domain().list(&collections);
                Ok(json!({ "presets": presets }))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }

    fn dispatch_collection(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        match request.method.as_str() {
            "collection.list" => {
                let collections = self.collection_domain_mut()?.list();
                Ok(json!({ "collections": collections }))
            }
            "collection.get" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let collection = self.collection_domain_mut()?.get(&params.id)?;
                Ok(json!({ "collection": collection }))
            }
            "collection.create" => {
                let params: CollectionCreateParams = self.parse_params(request)?;
                let collection = self.collection_domain_mut()?.create(params)?;
                Ok(json!({ "collection": collection }))
            }
            "collection.update" => {
                let params: CollectionUpdateParams = self.parse_params(request)?;
                let collection = self.collection_domain_mut()?.update(params)?;
                Ok(json!({ "collection": collection }))
            }
            "collection.delete" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                self.collection_domain_mut()?.delete(&params.id)?;
                Ok(json!({ "success": true }))
            }
            "collection.import" => {
                let params: CollectionImportParams = self.parse_params(request)?;
                let collection = self.collection_domain_mut()?.import(params)?;
                Ok(json!({ "collection": collection }))
            }
            "collection.export" => {
                let params: IdOnlyParams = self.parse_params(request)?;
                let yaml = self.collection_domain_mut()?.export(&params.id)?;
                Ok(json!({ "yaml": yaml }))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }

    fn dispatch_worker(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        match request.method.as_str() {
            "worker.list" => {
                let workers = self.worker_domain_mut()?.list()?;
                Ok(json!({ "workers": workers }))
            }
            "worker.get" => {
                let params: WorkerIdParams = self.parse_params(request)?;
                let worker = self.worker_domain_mut()?.get(&params.worker_id)?;
                Ok(json!({ "worker": worker }))
            }
            "worker.register" => {
                let record: WorkerRecord = self.parse_params(request)?;
                let worker = self.worker_domain_mut()?.register(record)?;
                Ok(json!({ "worker": worker }))
            }
            "worker.deregister" => {
                let params: WorkerIdParams = self.parse_params(request)?;
                self.worker_domain_mut()?.deregister(&params.worker_id)?;
                Ok(json!({ "success": true }))
            }
            "worker.heartbeat" => {
                let input: WorkerHeartbeatInput = self.parse_params(request)?;
                let worker = self.worker_domain_mut()?.heartbeat(input)?;
                Ok(json!({ "worker": worker }))
            }
            "worker.claim_next" => {
                let params: WorkerIdParams = self.parse_params(request)?;
                let result = self.worker_domain_mut()?.claim_next(&params.worker_id)?;
                Ok(json!(result))
            }
            "worker.reclaim_expired" => {
                let input: WorkerReclaimExpiredInput = self.parse_params(request)?;
                let result = self.worker_domain_mut()?.reclaim_expired(input)?;
                Ok(json!(result))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }

    fn dispatch_stream(
        &self,
        request: &RpcRequestEnvelope,
        principal: &str,
    ) -> Result<Value, ApiError> {
        match request.method.as_str() {
            "stream.subscribe" => {
                let params: StreamSubscribeParams = self.parse_params(request)?;
                let result = self.stream_domain().subscribe(params, principal)?;
                Ok(json!(result))
            }
            "stream.unsubscribe" => {
                let params: StreamUnsubscribeParams = self.parse_params(request)?;
                self.stream_domain().unsubscribe(params)?;
                Ok(json!({ "success": true }))
            }
            "stream.ack" => {
                let params: StreamAckParams = self.parse_params(request)?;
                self.stream_domain().ack(params)?;
                Ok(json!({ "success": true }))
            }
            _ => Err(ApiError::service_unavailable(format!(
                "method '{}' is recognized but not implemented",
                request.method
            ))),
        }
    }
}

use serde::Deserialize as InternalDeserialize;

#[derive(Debug, Clone, InternalDeserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerIdParams {
    worker_id: String,
}

#[derive(Debug, Clone, InternalDeserialize)]
#[serde(rename_all = "camelCase")]
struct InternalPublishParams {
    topic: String,
    resource_type: String,
    resource_id: String,
    payload: Value,
}

impl RpcRuntime {
    /// Internal-only method for the orchestration loop to inject events
    /// into the stream domain. Not part of the public RPC contract.
    fn dispatch_internal_publish(&self, request: &RpcRequestEnvelope) -> Result<Value, ApiError> {
        let params: InternalPublishParams = self.parse_params(request)?;
        self.stream_domain().publish(
            &params.topic,
            &params.resource_type,
            &params.resource_id,
            params.payload,
        );
        Ok(json!({ "success": true }))
    }
}

/// Enrich a single `TaskRecord` with computed fields derived from the worker snapshot.
///
/// Injected fields:
/// - `currentLoopId`  — loop the assigned worker belongs to, or `null`
/// - `currentHat`     — hat the assigned worker is wearing, or `null`
/// - `isClaimed`      — `true` when `assigneeWorkerId` is set
/// - `isStale`        — `true` when `leaseExpiresAt` is in the past
fn enrich_task(task: TaskRecord, workers: &BTreeMap<String, WorkerRecord>) -> Value {
    let is_claimed = task.assignee_worker_id.is_some();

    let is_stale = task
        .lease_expires_at
        .as_deref()
        .and_then(|ts| ts.parse::<DateTime<Utc>>().ok())
        .map_or(false, |expires| expires < Utc::now());

    let (current_loop_id, current_hat) = task
        .assignee_worker_id
        .as_deref()
        .and_then(|wid| workers.get(wid))
        .map(|w| (Some(w.loop_id.clone()), w.current_hat.clone()))
        .unwrap_or((None, None));

    let mut val = serde_json::to_value(&task).expect("TaskRecord is always serializable");
    let obj = val
        .as_object_mut()
        .expect("TaskRecord serializes to object");
    obj.insert("currentLoopId".to_string(), json!(current_loop_id));
    obj.insert("currentHat".to_string(), json!(current_hat));
    obj.insert("isClaimed".to_string(), json!(is_claimed));
    obj.insert("isStale".to_string(), json!(is_stale));
    val
}

/// Batch-enrich a list of tasks.
fn enrich_tasks(tasks: Vec<TaskRecord>, workers: &BTreeMap<String, WorkerRecord>) -> Vec<Value> {
    tasks.into_iter().map(|t| enrich_task(t, workers)).collect()
}

/// Enrich a single loop record with worker-facing fields.
///
/// Finds the worker whose `loop_id` matches this loop's `id` and injects
/// `workerId`, `workerStatus`, `currentTaskId`, `currentHat`, `lastHeartbeatAt`.
/// All fields are null when no worker is assigned to the loop.
fn enrich_loop(loop_rec: LoopRecord, workers: &BTreeMap<String, WorkerRecord>) -> Value {
    let worker = workers.values().find(|w| w.loop_id == loop_rec.id);

    let mut val = serde_json::to_value(&loop_rec).expect("LoopRecord is always serializable");
    let obj = val
        .as_object_mut()
        .expect("LoopRecord serializes to object");

    if let Some(w) = worker {
        obj.insert("workerId".to_string(), json!(w.worker_id));
        obj.insert("workerStatus".to_string(), json!(w.status));
        obj.insert("currentTaskId".to_string(), json!(w.current_task_id));
        obj.insert("currentHat".to_string(), json!(w.current_hat));
        obj.insert("lastHeartbeatAt".to_string(), json!(w.last_heartbeat_at));
    } else {
        obj.insert("workerId".to_string(), json!(null));
        obj.insert("workerStatus".to_string(), json!(null));
        obj.insert("currentTaskId".to_string(), json!(null));
        obj.insert("currentHat".to_string(), json!(null));
        obj.insert("lastHeartbeatAt".to_string(), json!(null));
    }

    val
}

/// Batch-enrich a list of loop records.
fn enrich_loops(loops: Vec<LoopRecord>, workers: &BTreeMap<String, WorkerRecord>) -> Vec<Value> {
    loops.into_iter().map(|l| enrich_loop(l, workers)).collect()
}

fn parse_optional_nullable_string_field(
    object: &serde_json::Map<String, Value>,
    field_name: &'static str,
) -> Result<Option<Option<String>>, ApiError> {
    if !object.contains_key(field_name) {
        return Ok(None);
    }

    let value = object
        .get(field_name)
        .expect("contains_key check guarantees field exists");
    if value.is_null() {
        return Ok(Some(None));
    }

    let value = value.as_str().ok_or_else(|| {
        ApiError::invalid_params(format!("task.update {field_name} must be a string or null"))
    })?;

    Ok(Some(Some(value.to_string())))
}

fn parse_task_update_input(request: &RpcRequestEnvelope) -> Result<TaskUpdateInput, ApiError> {
    let object = request.params.as_object().ok_or_else(|| {
        ApiError::invalid_params("task.update params must be an object")
            .with_details(json!({ "method": request.method }))
    })?;

    let id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| ApiError::invalid_params("task.update requires non-empty 'id'"))?
        .to_string();

    let title = object
        .get("title")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string);

    let status = object
        .get("status")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string);

    let priority = object
        .get("priority")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok());

    let blocked_by = parse_optional_nullable_string_field(object, "blockedBy")?;
    let assignee_worker_id = parse_optional_nullable_string_field(object, "assigneeWorkerId")?;
    let claimed_at = parse_optional_nullable_string_field(object, "claimedAt")?;
    let lease_expires_at = parse_optional_nullable_string_field(object, "leaseExpiresAt")?;

    Ok(TaskUpdateInput {
        id,
        title,
        status,
        priority,
        blocked_by,
        assignee_worker_id,
        claimed_at,
        lease_expires_at,
    })
}
