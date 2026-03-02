use std::sync::OnceLock;

use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::errors::ApiError;
use crate::loop_support::now_ts;

pub const API_VERSION: &str = "v1";
pub const STREAM_NAME: &str = "events.v1";

pub const KNOWN_METHODS: &[&str] = &[
    "system.health",
    "system.version",
    "system.capabilities",
    "task.list",
    "task.get",
    "task.ready",
    "task.create",
    "task.update",
    "task.close",
    "task.archive",
    "task.unarchive",
    "task.delete",
    "task.clear",
    "task.run",
    "task.run_all",
    "task.retry",
    "task.cancel",
    "task.status",
    "loop.list",
    "loop.status",
    "loop.process",
    "loop.prune",
    "loop.retry",
    "loop.discard",
    "loop.stop",
    "loop.merge",
    "loop.merge_button_state",
    "loop.trigger_merge_task",
    "planning.list",
    "planning.get",
    "planning.start",
    "planning.respond",
    "planning.resume",
    "planning.delete",
    "planning.get_artifact",
    "config.get",
    "config.update",
    "preset.list",
    "collection.list",
    "collection.get",
    "collection.create",
    "collection.update",
    "collection.delete",
    "collection.import",
    "collection.export",
    "stream.subscribe",
    "stream.unsubscribe",
    "stream.ack",
];

pub const MUTATING_METHODS: &[&str] = &[
    "task.create",
    "task.update",
    "task.close",
    "task.archive",
    "task.unarchive",
    "task.delete",
    "task.clear",
    "task.run",
    "task.run_all",
    "task.retry",
    "task.cancel",
    "loop.process",
    "loop.prune",
    "loop.retry",
    "loop.discard",
    "loop.stop",
    "loop.merge",
    "loop.trigger_merge_task",
    "planning.start",
    "planning.respond",
    "planning.resume",
    "planning.delete",
    "config.update",
    "collection.create",
    "collection.update",
    "collection.delete",
    "collection.import",
];

pub const STREAM_TOPICS: &[&str] = &[
    "system.heartbeat",
    "system.lifecycle",
    "task.log.line",
    "task.status.changed",
    "loop.status.changed",
    "loop.merge.progress",
    "planning.prompt.issued",
    "planning.response.recorded",
    "planning.artifact.updated",
    "config.updated",
    "collection.updated",
    "preset.refreshed",
    "error.raised",
    "stream.keepalive",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcRequestEnvelope {
    pub api_version: String,
    pub id: String,
    pub method: String,
    pub params: Value,
    pub meta: Option<RequestMeta>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMeta {
    pub idempotency_key: Option<String>,
    pub auth: Option<AuthMeta>,
    pub timeout_ms: Option<u64>,
    pub request_ts: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthMeta {
    pub mode: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseMeta {
    served_by: String,
    served_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SuccessEnvelope {
    api_version: String,
    id: String,
    method: String,
    result: Value,
    meta: ResponseMeta,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    api_version: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    error: crate::errors::RpcErrorBody,
    meta: ResponseMeta,
}

pub fn is_known_method(method: &str) -> bool {
    KNOWN_METHODS.contains(&method)
}

pub fn is_mutating_method(method: &str) -> bool {
    MUTATING_METHODS.contains(&method)
}

pub fn parse_json_value(body: &[u8]) -> Result<Value, ApiError> {
    serde_json::from_slice::<Value>(body)
        .map_err(|err| ApiError::invalid_request(format!("invalid JSON body: {err}")))
}

pub fn request_context(raw: &Value) -> (String, Option<String>) {
    let request_id = raw
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let method = raw
        .get("method")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string);
    (request_id, method)
}

pub fn parse_request(raw: &Value) -> Result<RpcRequestEnvelope, ApiError> {
    serde_json::from_value::<RpcRequestEnvelope>(raw.clone())
        .map_err(|err| ApiError::invalid_request(format!("invalid request envelope: {err}")))
}

pub fn validate_request_schema(raw: &Value) -> Result<(), Vec<String>> {
    let validator = request_schema_validator();
    match validator.validate(raw) {
        Ok(()) => Ok(()),
        Err(errors) => Err(errors.map(|error| error.to_string()).collect()),
    }
}

pub fn success_envelope(request: &RpcRequestEnvelope, result: Value, served_by: &str) -> Value {
    serde_json::to_value(SuccessEnvelope {
        api_version: API_VERSION.to_string(),
        id: request.id.clone(),
        method: request.method.clone(),
        result,
        meta: response_meta(served_by),
    })
    .expect("success envelope should always serialize")
}

pub fn error_envelope(error: &ApiError, served_by: &str) -> Value {
    serde_json::to_value(ErrorEnvelope {
        api_version: API_VERSION.to_string(),
        id: error.request_id.clone(),
        method: error.method.clone(),
        error: error.as_body(),
        meta: response_meta(served_by),
    })
    .expect("error envelope should always serialize")
}

fn response_meta(served_by: &str) -> ResponseMeta {
    ResponseMeta {
        served_by: served_by.to_string(),
        served_at: now_ts(),
    }
}

fn request_schema_validator() -> &'static JSONSchema {
    static REQUEST_VALIDATOR: OnceLock<JSONSchema> = OnceLock::new();
    REQUEST_VALIDATOR.get_or_init(|| {
        let raw_schema = include_str!("../data/rpc-v1-schema.json");
        let root_schema: Value =
            serde_json::from_str(raw_schema).expect("embedded rpc-v1 schema must be valid JSON");
        let defs = root_schema
            .get("$defs")
            .cloned()
            .expect("rpc-v1 schema must include $defs");

        let request_schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": defs,
            "$ref": "#/$defs/requestEnvelope"
        });

        JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&request_schema)
            .expect("request envelope schema must compile")
    })
}
