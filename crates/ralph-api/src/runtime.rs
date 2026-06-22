mod dispatch;

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tracing::debug;

use crate::auth::{Authenticator, from_config};
use crate::collection_domain::CollectionDomain;
use crate::config::ApiConfig;
use crate::config_domain::ConfigDomain;
use crate::errors::{ApiError, RpcErrorCode};
use crate::idempotency::{
    IdempotencyCheck, IdempotencyStore, InMemoryIdempotencyStore, StoredResponse,
};
use crate::loop_domain::LoopDomain;
use crate::planning_domain::PlanningDomain;
use crate::preset_domain::PresetDomain;
use crate::protocol::{
    API_VERSION, KNOWN_METHODS, RpcRequestEnvelope, STREAM_TOPICS, error_envelope, is_known_method,
    is_mutating_method, parse_json_value, parse_request, request_context, success_envelope,
    validate_request_schema,
};
use crate::robot_domain::RobotDomain;
use crate::stream_domain::StreamDomain;
use crate::task_domain::TaskDomain;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IdOnlyParams {
    pub(crate) id: String,
}

#[derive(Clone)]
pub struct RpcRuntime {
    pub(crate) config: ApiConfig,
    auth: Arc<dyn Authenticator>,
    idempotency: Arc<dyn IdempotencyStore>,
    tasks: Arc<Mutex<TaskDomain>>,
    loops: Arc<Mutex<LoopDomain>>,
    planning: Arc<Mutex<PlanningDomain>>,
    collections: Arc<Mutex<CollectionDomain>>,
    streams: StreamDomain,
    config_domain: ConfigDomain,
    preset_domain: PresetDomain,
    robot_domain: RobotDomain,
}

enum ExecutionOutcome {
    Fresh(Value),
    Replay(StoredResponse),
}

impl RpcRuntime {
    pub fn new(config: ApiConfig) -> anyhow::Result<Self> {
        config.validate()?;

        let auth = from_config(&config)?;
        let idempotency = Arc::new(InMemoryIdempotencyStore::new(Duration::from_secs(
            config.idempotency_ttl_secs,
        )));

        Ok(Self::with_components(config, auth, idempotency))
    }

    pub fn with_components(
        config: ApiConfig,
        auth: Arc<dyn Authenticator>,
        idempotency: Arc<dyn IdempotencyStore>,
    ) -> Self {
        let tasks = Arc::new(Mutex::new(TaskDomain::new(&config.workspace_root)));
        let loops = Arc::new(Mutex::new(LoopDomain::new(
            &config.workspace_root,
            config.loop_process_interval_ms,
            config.ralph_command.clone(),
        )));
        let planning = Arc::new(Mutex::new(PlanningDomain::new(&config.workspace_root)));
        let collections = Arc::new(Mutex::new(CollectionDomain::new(&config.workspace_root)));
        let streams = StreamDomain::new();
        let config_domain = ConfigDomain::new(&config.workspace_root);
        let preset_domain = PresetDomain::new(&config.workspace_root);
        let robot_domain = RobotDomain::new(&config.workspace_root);

        Self {
            config,
            auth,
            idempotency,
            tasks,
            loops,
            planning,
            collections,
            streams,
            config_domain,
            preset_domain,
            robot_domain,
        }
    }

    pub fn health_payload(&self) -> Value {
        json!({
            "status": "ok",
            "timestamp": crate::loop_support::now_ts()
        })
    }

    pub fn capabilities_payload(&self) -> Value {
        json!({
            "methods": KNOWN_METHODS,
            "streamTopics": STREAM_TOPICS,
            "auth": {
                "mode": self.auth.mode().as_contract_mode(),
                "supportedModes": ["trusted_local", "token"]
            },
            "idempotency": {
                "requiredForMutations": true,
                "retentionSeconds": self.config.idempotency_ttl_secs
            }
        })
    }

    pub fn invoke_method(
        &self,
        request_id: impl Into<String>,
        method: &str,
        params: Value,
        principal: &str,
        idempotency_key: Option<String>,
    ) -> Result<Value, ApiError> {
        let request_id = request_id.into();
        let mut raw = json!({
            "apiVersion": API_VERSION,
            "id": request_id,
            "method": method,
            "params": params,
        });

        if let Some(idempotency_key) = idempotency_key {
            raw["meta"] = json!({
                "idempotencyKey": idempotency_key,
            });
        }

        let request = self.parse_and_validate_request_value(raw)?;
        match self.execute_request(&request, principal)? {
            ExecutionOutcome::Fresh(result) => Ok(result),
            ExecutionOutcome::Replay(response) => self.replay_stored_response(&request, response),
        }
    }

    pub fn handle_http_request(&self, body: &[u8], headers: &HeaderMap) -> (StatusCode, Value) {
        let request = match self.parse_and_validate_request(body) {
            Ok(request) => request,
            Err(error) => {
                let status = error.status;
                let envelope = error_envelope(&error, &self.config.served_by);
                return (status, envelope);
            }
        };

        let principal =
            match self.auth.authorize(&request, headers).map_err(|error| {
                error.with_context(request.id.clone(), Some(request.method.clone()))
            }) {
                Ok(p) => p,
                Err(error) => {
                    let status = error.status;
                    let envelope = error_envelope(&error, &self.config.served_by);
                    return (status, envelope);
                }
            };

        let (status, envelope) = match self.execute_request(&request, &principal) {
            Ok(ExecutionOutcome::Fresh(result)) => (
                StatusCode::OK,
                success_envelope(&request, result, &self.config.served_by),
            ),
            Ok(ExecutionOutcome::Replay(response)) => (
                StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK),
                response.envelope,
            ),
            Err(error) => {
                let status = error.status;
                let envelope = error_envelope(&error, &self.config.served_by);
                (status, envelope)
            }
        };

        (status, envelope)
    }

    pub fn authenticate_websocket(&self, headers: &HeaderMap) -> Result<String, ApiError> {
        let dummy_request = crate::protocol::RpcRequestEnvelope {
            api_version: "v1".to_string(),
            id: "ws-upgrade".to_string(),
            method: "stream.subscribe".to_string(),
            params: serde_json::Value::Object(serde_json::Map::new()),
            meta: None,
        };

        self.auth
            .authorize(&dummy_request, headers)
            .map_err(|error| error.with_context("ws-upgrade", Some("stream.subscribe".to_string())))
    }

    pub(crate) fn task_domain_mut(&self) -> Result<MutexGuard<'_, TaskDomain>, ApiError> {
        self.tasks
            .lock()
            .map_err(|_| ApiError::internal("task domain lock poisoned"))
    }

    pub(crate) fn loop_domain_mut(&self) -> Result<MutexGuard<'_, LoopDomain>, ApiError> {
        self.loops
            .lock()
            .map_err(|_| ApiError::internal("loop domain lock poisoned"))
    }

    pub(crate) fn planning_domain_mut(&self) -> Result<MutexGuard<'_, PlanningDomain>, ApiError> {
        self.planning
            .lock()
            .map_err(|_| ApiError::internal("planning domain lock poisoned"))
    }

    pub(crate) fn collection_domain_mut(
        &self,
    ) -> Result<MutexGuard<'_, CollectionDomain>, ApiError> {
        self.collections
            .lock()
            .map_err(|_| ApiError::internal("collection domain lock poisoned"))
    }

    pub(crate) fn stream_domain(&self) -> StreamDomain {
        self.streams.clone()
    }

    pub(crate) fn config_domain(&self) -> &ConfigDomain {
        &self.config_domain
    }

    pub(crate) fn preset_domain(&self) -> &PresetDomain {
        &self.preset_domain
    }

    pub(crate) fn robot_domain(&self) -> &RobotDomain {
        &self.robot_domain
    }

    pub(crate) fn parse_params<T>(&self, request: &RpcRequestEnvelope) -> Result<T, ApiError>
    where
        T: DeserializeOwned,
    {
        serde_json::from_value(request.params.clone()).map_err(|error| {
            ApiError::invalid_params(format!(
                "invalid params for method '{}': {error}",
                request.method
            ))
        })
    }

    fn parse_and_validate_request(&self, body: &[u8]) -> Result<RpcRequestEnvelope, ApiError> {
        let raw = parse_json_value(body)?;
        let (request_id, method) = request_context(&raw);
        if method.as_deref() == Some("_internal.publish") {
            return Err(ApiError::method_not_found("_internal.publish")
                .with_context(request_id, Some("_internal.publish".to_string())));
        }

        self.parse_and_validate_request_value(raw)
    }

    fn parse_and_validate_request_value(&self, raw: Value) -> Result<RpcRequestEnvelope, ApiError> {
        let (request_id, method) = request_context(&raw);

        if !raw.is_object() {
            return Err(
                ApiError::invalid_request("request body must be a JSON object")
                    .with_context(request_id, method),
            );
        }

        let method = method.ok_or_else(|| {
            ApiError::invalid_request("missing required field 'method'")
                .with_context(request_id.clone(), None)
        })?;

        if method == "_internal.publish" {
            return self.parse_and_validate_internal_publish_request(raw, request_id, method);
        }

        if !is_known_method(&method) {
            return Err(
                ApiError::method_not_found(method.clone()).with_context(request_id, Some(method))
            );
        }

        if let Err(errors) = validate_request_schema(&raw) {
            return Err(
                ApiError::invalid_params("request does not match rpc-v1 schema")
                    .with_context(request_id, Some(method))
                    .with_details(json!({ "errors": errors })),
            );
        }

        let request = parse_request(&raw)
            .map_err(|error| error.with_context(request_id.clone(), Some(method.clone())))?;

        if request.api_version != API_VERSION {
            return Err(ApiError::invalid_request(format!(
                "unsupported apiVersion '{}'; expected '{API_VERSION}'",
                request.api_version
            ))
            .with_context(request.id, Some(request.method)));
        }

        Ok(request)
    }

    fn parse_and_validate_internal_publish_request(
        &self,
        raw: Value,
        request_id: String,
        method: String,
    ) -> Result<RpcRequestEnvelope, ApiError> {
        let request = parse_request(&raw)
            .map_err(|error| error.with_context(request_id.clone(), Some(method.clone())))?;

        if request.api_version != API_VERSION {
            return Err(ApiError::invalid_request(format!(
                "unsupported apiVersion '{}'; expected '{API_VERSION}'",
                request.api_version
            ))
            .with_context(request.id, Some(request.method)));
        }

        validate_internal_publish_params(&request.params).map_err(|error| {
            error.with_context(request.id.clone(), Some(request.method.clone()))
        })?;

        Ok(request)
    }

    fn execute_request(
        &self,
        request: &RpcRequestEnvelope,
        principal: &str,
    ) -> Result<ExecutionOutcome, ApiError> {
        let mut idempotency_context: Option<String> = None;
        if is_mutating_method(&request.method) {
            let key = match request
                .meta
                .as_ref()
                .and_then(|meta| meta.idempotency_key.as_deref())
            {
                Some(key) => key,
                None => {
                    return Err(ApiError::invalid_params(
                        "mutating methods require meta.idempotencyKey",
                    )
                    .with_context(request.id.clone(), Some(request.method.clone())));
                }
            };

            match self
                .idempotency
                .check(&request.method, key, &request.params)
            {
                IdempotencyCheck::Replay(response) => {
                    debug!(
                        method = %request.method,
                        request_id = %request.id,
                        "idempotency replay"
                    );
                    return Ok(ExecutionOutcome::Replay(response));
                }
                IdempotencyCheck::Conflict => {
                    return Err(ApiError::idempotency_conflict(
                        "idempotency key was already used with different parameters",
                    )
                    .with_context(request.id.clone(), Some(request.method.clone()))
                    .with_details(json!({
                        "method": request.method.clone(),
                        "idempotencyKey": key
                    })));
                }
                IdempotencyCheck::New => {
                    idempotency_context = Some(key.to_string());
                }
            }
        }

        let result = self
            .dispatch(request, principal)
            .map_err(|error| error.with_context(request.id.clone(), Some(request.method.clone())));

        if let Some(key) = idempotency_context {
            let (status, envelope) = match &result {
                Ok(value) => (
                    StatusCode::OK.as_u16(),
                    success_envelope(request, value.clone(), &self.config.served_by),
                ),
                Err(error) => (
                    error.status.as_u16(),
                    error_envelope(error, &self.config.served_by),
                ),
            };
            self.idempotency.store(
                &request.method,
                &key,
                &request.params,
                &StoredResponse { status, envelope },
            );
        }

        result.map(ExecutionOutcome::Fresh)
    }

    fn replay_stored_response(
        &self,
        request: &RpcRequestEnvelope,
        response: StoredResponse,
    ) -> Result<Value, ApiError> {
        if let Some(result) = response.envelope.get("result") {
            return Ok(result.clone());
        }

        let error_body = response
            .envelope
            .get("error")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ApiError::internal("stored idempotency response was missing an error payload")
                    .with_context(request.id.clone(), Some(request.method.clone()))
            })?;

        let code = error_body
            .get("code")
            .and_then(Value::as_str)
            .and_then(RpcErrorCode::from_contract)
            .unwrap_or(RpcErrorCode::Internal);
        let message = error_body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("stored idempotency replay failed");
        let retryable = error_body
            .get("retryable")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut error = ApiError::new(code, message)
            .with_context(request.id.clone(), Some(request.method.clone()));
        error.retryable = retryable;
        error.details = error_body.get("details").cloned();
        error.status = StatusCode::from_u16(response.status).unwrap_or(error.status);
        Err(error)
    }
}

fn validate_internal_publish_params(params: &Value) -> Result<(), ApiError> {
    let object = params
        .as_object()
        .ok_or_else(|| ApiError::invalid_params("_internal.publish params must be an object"))?;

    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "topic" | "resourceType" | "resourceId" | "payload"
        ) {
            return Err(ApiError::invalid_params(format!(
                "_internal.publish does not accept '{key}'"
            )));
        }
    }

    let topic = required_non_empty_string(object.get("topic"), "topic")?;
    if !STREAM_TOPICS.contains(&topic) {
        return Err(ApiError::invalid_params(format!(
            "_internal.publish topic '{topic}' is not registered"
        )));
    }

    required_non_empty_string(object.get("resourceType"), "resourceType")?;
    required_non_empty_string(object.get("resourceId"), "resourceId")?;

    if !matches!(object.get("payload"), Some(Value::Object(_))) {
        return Err(ApiError::invalid_params(
            "_internal.publish requires object 'payload'",
        ));
    }

    Ok(())
}

fn required_non_empty_string<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a str, ApiError> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::invalid_params(format!("_internal.publish requires '{field}'")))
}
