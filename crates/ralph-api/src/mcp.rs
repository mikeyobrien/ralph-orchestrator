use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use jsonschema::{Draft, JSONSchema};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ErrorCode, ErrorData, Implementation, InitializeResult,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, Tool, ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, ServiceExt};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::sync::{Mutex as AsyncMutex, broadcast};

use crate::config::ApiConfig;
use crate::errors::ApiError;
use crate::protocol::{KNOWN_METHODS, MUTATING_METHODS};
use crate::runtime::RpcRuntime;
use crate::stream_domain::StreamEventEnvelope;

const MCP_PRINCIPAL: &str = "trusted_local";
const TOOL_PAGE_SIZE: usize = 128;
const DEFAULT_STREAM_NEXT_WAIT_MS: u64 = 30_000;
const MAX_STREAM_NEXT_WAIT_MS: u64 = 120_000;
const DEFAULT_STREAM_NEXT_EVENTS: u16 = 50;
const MAX_STREAM_NEXT_EVENTS: u16 = 200;

#[derive(Clone)]
pub struct RalphMcpServer {
    runtime: RpcRuntime,
    catalog: Arc<ToolCatalog>,
    subscriptions: Arc<AsyncMutex<HashMap<String, Arc<AsyncMutex<McpSubscriptionState>>>>>,
}

impl RalphMcpServer {
    pub fn new(config: ApiConfig) -> Result<Self> {
        let runtime = RpcRuntime::new(config)?;
        let catalog = Arc::new(tool_catalog().clone());
        Ok(Self {
            runtime,
            catalog,
            subscriptions: Arc::new(AsyncMutex::new(HashMap::new())),
        })
    }

    fn invoke_rpc_tool(
        &self,
        method: &str,
        arguments: Value,
        request_id: &str,
        tool_name: &str,
    ) -> Result<Value, ApiError> {
        let idempotency_key = MUTATING_METHODS
            .contains(&method)
            .then(|| format!("mcp:{tool_name}:{request_id}"));
        self.runtime.invoke_method(
            request_id.to_string(),
            method,
            arguments,
            MCP_PRINCIPAL,
            idempotency_key,
        )
    }

    fn call_rpc_tool(
        &self,
        method: &str,
        arguments: Value,
        request_id: &str,
        tool_name: &str,
    ) -> Result<CallToolResult, ApiError> {
        let result = self.invoke_rpc_tool(method, arguments, request_id, tool_name)?;
        Ok(CallToolResult::structured(result))
    }

    async fn call_stream_subscribe(
        &self,
        arguments: Value,
        request_id: &str,
        tool_name: &str,
    ) -> Result<CallToolResult, ApiError> {
        let live_rx = self.runtime.stream_domain().live_receiver();
        let result = self.invoke_rpc_tool("stream.subscribe", arguments, request_id, tool_name)?;
        let subscription_id = subscription_id_from_result(&result)
            .ok_or_else(|| ApiError::internal("stream.subscribe result missing subscriptionId"))
            .map_err(|error| {
                error.with_context(request_id.to_string(), Some("stream.subscribe".to_string()))
            })?;
        self.subscriptions.lock().await.insert(
            subscription_id.to_string(),
            Arc::new(AsyncMutex::new(McpSubscriptionState { live_rx })),
        );
        Ok(CallToolResult::structured(result))
    }

    async fn call_stream_unsubscribe(
        &self,
        arguments: Value,
        request_id: &str,
        tool_name: &str,
    ) -> Result<CallToolResult, ApiError> {
        let subscription_id = subscription_id_from_arguments(&arguments)
            .ok_or_else(|| ApiError::internal("stream.unsubscribe args missing subscriptionId"))
            .map_err(|error| {
                error.with_context(
                    request_id.to_string(),
                    Some("stream.unsubscribe".to_string()),
                )
            })?
            .to_string();
        let result =
            self.invoke_rpc_tool("stream.unsubscribe", arguments, request_id, tool_name)?;
        self.subscriptions.lock().await.remove(&subscription_id);
        Ok(CallToolResult::structured(result))
    }

    async fn call_stream_next(
        &self,
        arguments: Value,
        request_id: &str,
    ) -> Result<CallToolResult, ApiError> {
        let params: StreamNextParams = serde_json::from_value(arguments).map_err(|error| {
            ApiError::invalid_params(format!("invalid params for method 'stream.next': {error}"))
                .with_context(request_id.to_string(), Some("stream.next".to_string()))
        })?;
        let wait_ms = params
            .wait_ms
            .unwrap_or(DEFAULT_STREAM_NEXT_WAIT_MS)
            .clamp(1, MAX_STREAM_NEXT_WAIT_MS);
        let max_events = params
            .max_events
            .unwrap_or(DEFAULT_STREAM_NEXT_EVENTS)
            .clamp(1, MAX_STREAM_NEXT_EVENTS) as usize;

        let streams = self.runtime.stream_domain();
        if !streams.has_subscription(&params.subscription_id) {
            return Err(ApiError::not_found(format!(
                "subscription '{}' not found",
                params.subscription_id
            ))
            .with_context(request_id.to_string(), Some("stream.next".to_string()))
            .with_details(json!({ "subscriptionId": params.subscription_id })));
        }

        let subscription = self.lookup_subscription(&params.subscription_id).await?;
        let replay = streams
            .replay_for_subscription(&params.subscription_id)
            .map_err(|error| {
                error.with_context(request_id.to_string(), Some("stream.next".to_string()))
            })?;

        let mut events = replay.events;
        let mut dropped_count = replay.dropped_count;
        if !events.is_empty() || dropped_count > 0 {
            events.truncate(max_events);
            return Ok(stream_next_result(events, false, dropped_count));
        }

        let mut live_state = subscription.lock().await;
        let current_cursor = streams
            .subscription_cursor(&params.subscription_id)
            .map_err(|error| {
                error.with_context(request_id.to_string(), Some("stream.next".to_string()))
            })?;
        let current_cursor_sequence = streams
            .subscription_cursor_sequence(&params.subscription_id)
            .map_err(|error| {
                error.with_context(request_id.to_string(), Some("stream.next".to_string()))
            })?;

        if let Some(result) = collect_live_events(
            &streams,
            &params.subscription_id,
            &current_cursor,
            current_cursor_sequence,
            &mut live_state.live_rx,
            max_events,
            dropped_count,
        ) {
            return Ok(stream_next_result(
                result.events,
                false,
                result.dropped_count,
            ));
        }

        let timer = tokio::time::sleep(Duration::from_millis(wait_ms));
        tokio::pin!(timer);

        loop {
            tokio::select! {
                _ = &mut timer => {
                    return Ok(stream_next_result(Vec::new(), true, dropped_count));
                }
                message = live_state.live_rx.recv() => {
                    match message {
                        Ok(event) => {
                            if is_pending_live_event(
                                &streams,
                                &params.subscription_id,
                                &current_cursor,
                                current_cursor_sequence,
                                &event,
                            ) {
                                events.push(event);
                                let drained = collect_live_events(
                                    &streams,
                                    &params.subscription_id,
                                    &current_cursor,
                                    current_cursor_sequence,
                                    &mut live_state.live_rx,
                                    max_events - events.len(),
                                    dropped_count,
                                );
                                if let Some(drained) = drained {
                                    dropped_count = drained.dropped_count;
                                    events.extend(drained.events);
                                }
                                return Ok(stream_next_result(events, false, dropped_count));
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            dropped_count = dropped_count.saturating_add(skipped as usize);
                            return Ok(stream_next_result(Vec::new(), false, dropped_count));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            return Err(ApiError::service_unavailable("stream receiver closed")
                                .with_context(request_id.to_string(), Some("stream.next".to_string())));
                        }
                    }
                }
            }
        }
    }

    async fn lookup_subscription(
        &self,
        subscription_id: &str,
    ) -> Result<Arc<AsyncMutex<McpSubscriptionState>>, ApiError> {
        self.subscriptions
            .lock()
            .await
            .get(subscription_id)
            .cloned()
            .ok_or_else(|| {
                ApiError::not_found(format!(
                    "subscription '{}' is not managed by this MCP server",
                    subscription_id
                ))
                .with_details(json!({ "subscriptionId": subscription_id }))
            })
    }
}

impl ServerHandler for RalphMcpServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("ralph-mcp", env!("CARGO_PKG_VERSION"))
                    .with_title("Ralph MCP Server"),
            )
            .with_instructions(
                "Use the Ralph control-plane tools to inspect and manage local orchestration state.",
            )
    }

    fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        std::future::ready(
            self.catalog
                .list_tools(request.as_ref().and_then(|value| value.cursor.as_deref())),
        )
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.catalog.lookup(name).map(|entry| entry.tool.clone())
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let request_id = context.id.to_string();
        let Some(entry) = self.catalog.lookup(request.name.as_ref()) else {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("tool '{}' is not available", request.name),
                None,
            ));
        };

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        if let Err(errors) = entry.validate_input(&arguments) {
            let error = ApiError::invalid_params(format!(
                "tool '{}' arguments do not match the published schema",
                request.name
            ))
            .with_context(request_id.clone(), Some(entry.method_name().to_string()))
            .with_details(json!({ "errors": errors }));
            return Ok(tool_error_result(error));
        }

        let result = match &entry.target {
            ToolTarget::Rpc { method } => match method.as_str() {
                "stream.subscribe" => {
                    self.call_stream_subscribe(arguments, &request_id, entry.tool.name.as_ref())
                        .await
                }
                "stream.unsubscribe" => {
                    self.call_stream_unsubscribe(arguments, &request_id, entry.tool.name.as_ref())
                        .await
                }
                _ => self.call_rpc_tool(method, arguments, &request_id, entry.tool.name.as_ref()),
            },
            ToolTarget::StreamNext => self.call_stream_next(arguments, &request_id).await,
        };

        match result {
            Ok(result) => Ok(result),
            Err(error) => Ok(tool_error_result(error)),
        }
    }
}

pub async fn serve_stdio(config: ApiConfig) -> Result<()> {
    let server = RalphMcpServer::new(config)?;
    let running = server.serve(rmcp::transport::stdio()).await?;
    let _ = running.waiting().await?;
    Ok(())
}

#[derive(Clone)]
struct ToolCatalog {
    entries: Vec<ToolEntry>,
    by_name: HashMap<String, usize>,
}

impl ToolCatalog {
    fn load() -> Result<Self> {
        let root_schema: Value = serde_json::from_str(include_str!("../data/rpc-v1-schema.json"))
            .context("embedded rpc-v1 schema must be valid JSON")?;
        let request_variants = schema_method_map(&root_schema, "requestByMethod", "params")?;
        let response_variants = schema_method_map(&root_schema, "responseByMethod", "result")?;

        let mut entries = Vec::new();
        for method in KNOWN_METHODS {
            let name = rpc_method_to_tool_name(method);
            let input_ref = request_variants
                .get(*method)
                .with_context(|| format!("missing params schema for method '{method}'"))?;
            let output_ref = response_variants
                .get(*method)
                .with_context(|| format!("missing result schema for method '{method}'"))?;

            let input_schema = schema_ref_object(&root_schema, input_ref)?;
            let output_schema = schema_ref_object(&root_schema, output_ref)?;
            let tool = build_tool(name, method, &input_schema, Some(&output_schema));
            let input_validator = compile_validator(&input_schema)?;

            entries.push(ToolEntry {
                tool,
                target: ToolTarget::Rpc {
                    method: (*method).to_string(),
                },
                input_validator,
            });
        }

        let stream_next_input = stream_next_input_schema();
        entries.push(ToolEntry {
            tool: build_tool(
                "stream_next".to_string(),
                "stream.next",
                &stream_next_input,
                Some(&stream_next_output_schema()),
            ),
            target: ToolTarget::StreamNext,
            input_validator: compile_validator(&stream_next_input)?,
        });

        entries.sort_by(|left, right| left.tool.name.cmp(&right.tool.name));
        let by_name = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.tool.name.to_string(), index))
            .collect();

        Ok(Self { entries, by_name })
    }

    fn lookup(&self, name: &str) -> Option<&ToolEntry> {
        self.by_name
            .get(name)
            .and_then(|index| self.entries.get(*index))
    }

    fn list_tools(&self, cursor: Option<&str>) -> Result<ListToolsResult, ErrorData> {
        let offset = cursor
            .map(|value| {
                value.parse::<usize>().map_err(|_| {
                    ErrorData::invalid_params(format!("invalid tools/list cursor '{value}'"), None)
                })
            })
            .transpose()?
            .unwrap_or(0);

        if offset > self.entries.len() {
            return Err(ErrorData::invalid_params(
                format!("tools/list cursor '{}' is out of range", offset),
                None,
            ));
        }

        let end = (offset + TOOL_PAGE_SIZE).min(self.entries.len());
        let mut result = ListToolsResult::with_all_items(
            self.entries[offset..end]
                .iter()
                .map(|entry| entry.tool.clone())
                .collect(),
        );
        if end < self.entries.len() {
            result.next_cursor = Some(end.to_string());
        }
        Ok(result)
    }
}

#[derive(Clone)]
struct ToolEntry {
    tool: Tool,
    target: ToolTarget,
    input_validator: Arc<JSONSchema>,
}

impl ToolEntry {
    fn method_name(&self) -> &str {
        match &self.target {
            ToolTarget::Rpc { method } => method,
            ToolTarget::StreamNext => "stream.next",
        }
    }

    fn validate_input(&self, arguments: &Value) -> Result<(), Vec<String>> {
        match self.input_validator.validate(arguments) {
            Ok(()) => Ok(()),
            Err(errors) => Err(errors.map(|error| error.to_string()).collect()),
        }
    }
}

#[derive(Clone)]
enum ToolTarget {
    Rpc { method: String },
    StreamNext,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamNextParams {
    subscription_id: String,
    wait_ms: Option<u64>,
    max_events: Option<u16>,
}

struct McpSubscriptionState {
    live_rx: broadcast::Receiver<StreamEventEnvelope>,
}

struct LiveEventBatch {
    events: Vec<StreamEventEnvelope>,
    dropped_count: usize,
}

fn subscription_id_from_result(result: &Value) -> Option<&str> {
    result.get("subscriptionId").and_then(Value::as_str)
}

fn subscription_id_from_arguments(arguments: &Value) -> Option<&str> {
    arguments.get("subscriptionId").and_then(Value::as_str)
}

fn is_pending_live_event(
    streams: &crate::stream_domain::StreamDomain,
    subscription_id: &str,
    current_cursor: &str,
    cursor_sequence: u64,
    event: &StreamEventEnvelope,
) -> bool {
    streams.matches_subscription(subscription_id, event)
        && (event.sequence > cursor_sequence
            || (event.sequence == cursor_sequence && event.cursor != current_cursor))
}

fn collect_live_events(
    streams: &crate::stream_domain::StreamDomain,
    subscription_id: &str,
    current_cursor: &str,
    cursor_sequence: u64,
    live_rx: &mut broadcast::Receiver<StreamEventEnvelope>,
    max_events: usize,
    dropped_count: usize,
) -> Option<LiveEventBatch> {
    if max_events == 0 {
        return Some(LiveEventBatch {
            events: Vec::new(),
            dropped_count,
        });
    }

    let mut events = Vec::new();
    let mut dropped_count = dropped_count;
    loop {
        match live_rx.try_recv() {
            Ok(event) => {
                if is_pending_live_event(
                    streams,
                    subscription_id,
                    current_cursor,
                    cursor_sequence,
                    &event,
                ) {
                    events.push(event);
                    if events.len() >= max_events {
                        break;
                    }
                }
            }
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                dropped_count = dropped_count.saturating_add(skipped as usize);
                break;
            }
            Err(broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed) => {
                break;
            }
        }
    }

    (!events.is_empty() || dropped_count > 0).then_some(LiveEventBatch {
        events,
        dropped_count,
    })
}

fn tool_catalog() -> &'static ToolCatalog {
    static CATALOG: OnceLock<ToolCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        ToolCatalog::load().expect("embedded rpc-v1 schema must be convertible into an MCP catalog")
    })
}

fn rpc_method_to_tool_name(method: &str) -> String {
    method.replace('.', "_")
}

fn build_tool(
    name: String,
    method: &str,
    input_schema: &Map<String, Value>,
    output_schema: Option<&Map<String, Value>>,
) -> Tool {
    let description: Cow<'static, str> = match method {
        "system.health" => "Return the Ralph control-plane health snapshot.".into(),
        "system.version" => "Return the Ralph control-plane API and server version.".into(),
        "system.capabilities" => {
            "List supported Ralph control-plane methods and stream topics.".into()
        }
        "task.list" => "List Ralph tasks, with optional filters.".into(),
        "task.ready" => "List open tasks that are ready to run.".into(),
        "task.run_all" => "Enqueue every open or queued Ralph task.".into(),
        "loop.status" => "Return the current primary loop and merge status.".into(),
        "loop.trigger_merge_task" => "Create a merge task for a completed loop.".into(),
        "planning.get_artifact" => "Read a generated planning artifact by filename.".into(),
        "config.get" => "Read the current Ralph YAML configuration.".into(),
        "config.update" => "Replace the Ralph YAML configuration after validation.".into(),
        "preset.list" => "List all available Ralph presets.".into(),
        "collection.import" => "Import a preset collection from YAML.".into(),
        "collection.export" => "Export a preset collection to YAML.".into(),
        "stream.subscribe" => "Create a Ralph event stream subscription.".into(),
        "stream.unsubscribe" => "Close a Ralph event stream subscription.".into(),
        "stream.ack" => "Advance a Ralph event stream subscription cursor.".into(),
        "stream.next" => "Poll for the next batch of Ralph stream events.".into(),
        _ => format!("Invoke Ralph control-plane method `{method}`.").into(),
    };

    let mut tool = Tool::new_with_raw(name, Some(description), Arc::new(input_schema.clone()));
    tool.title = Some(method.to_string());
    tool.output_schema = output_schema.cloned().map(Arc::new);
    tool.annotations = Some(
        ToolAnnotations::with_title(method)
            .read_only(!MUTATING_METHODS.contains(&method))
            .destructive(MUTATING_METHODS.contains(&method))
            .open_world(false),
    );
    tool
}

fn schema_method_map(
    root_schema: &Value,
    def_name: &str,
    property_name: &str,
) -> Result<HashMap<String, String>> {
    let variants = root_schema
        .pointer(&format!("/$defs/{def_name}/oneOf"))
        .and_then(Value::as_array)
        .with_context(|| format!("schema def '{def_name}' must expose a oneOf array"))?;

    let mut map = HashMap::new();
    for variant in variants {
        let method = variant
            .pointer("/properties/method/const")
            .and_then(Value::as_str)
            .context("schema variant is missing properties.method.const")?;
        let schema_ref = variant
            .pointer(&format!("/properties/{property_name}/$ref"))
            .and_then(Value::as_str)
            .with_context(|| {
                format!("schema variant '{method}' is missing {property_name}.$ref")
            })?;
        map.insert(method.to_string(), schema_ref.to_string());
    }

    Ok(map)
}

fn schema_ref_object(root_schema: &Value, schema_ref: &str) -> Result<Map<String, Value>> {
    let defs = root_schema
        .get("$defs")
        .cloned()
        .context("schema must expose $defs")?;
    let schema = json!({
        "$schema": root_schema
            .get("$schema")
            .cloned()
            .unwrap_or_else(|| json!("https://json-schema.org/draft/2020-12/schema")),
        "$defs": defs,
        "$ref": schema_ref,
    });
    schema
        .as_object()
        .cloned()
        .context("generated schema object must be a JSON object")
}

fn compile_validator(schema: &Map<String, Value>) -> Result<Arc<JSONSchema>> {
    let schema_value = Value::Object(schema.clone());
    let validator = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&schema_value)
        .map_err(|error| anyhow!("tool schema must compile: {error}"))?;
    Ok(Arc::new(validator))
}

fn stream_next_input_schema() -> Map<String, Value> {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "subscriptionId": {
                "type": "string",
                "minLength": 1
            },
            "waitMs": {
                "type": "integer",
                "minimum": 1,
                "maximum": MAX_STREAM_NEXT_WAIT_MS
            },
            "maxEvents": {
                "type": "integer",
                "minimum": 1,
                "maximum": MAX_STREAM_NEXT_EVENTS
            }
        },
        "required": ["subscriptionId"]
    })
    .as_object()
    .cloned()
    .expect("stream.next input schema must be an object")
}

fn stream_next_output_schema() -> Map<String, Value> {
    let event_schema: Value = serde_json::from_str(include_str!("../data/rpc-v1-events.json"))
        .expect("embedded rpc-v1 event schema must be valid JSON");
    let defs = event_schema
        .get("$defs")
        .cloned()
        .expect("embedded rpc-v1 event schema must expose $defs");

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": defs,
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "events": {
                "type": "array",
                "items": { "$ref": "#/$defs/eventEnvelope" }
            },
            "timedOut": { "type": "boolean" },
            "droppedCount": { "type": "integer", "minimum": 0 }
        },
        "required": ["events", "timedOut", "droppedCount"]
    })
    .as_object()
    .cloned()
    .expect("stream.next output schema must be an object")
}

fn stream_next_result(
    events: Vec<StreamEventEnvelope>,
    timed_out: bool,
    dropped_count: usize,
) -> CallToolResult {
    CallToolResult::structured(json!({
        "events": events,
        "timedOut": timed_out,
        "droppedCount": dropped_count,
    }))
}

fn tool_error_result(error: ApiError) -> CallToolResult {
    CallToolResult::structured_error(json!({
        "code": error.code.as_str(),
        "message": error.message,
        "details": error.details,
        "requestId": error.request_id,
        "method": error.method,
        "retryable": error.retryable,
    }))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn test_server() -> (RalphMcpServer, TempDir) {
        let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
        let config = ApiConfig {
            workspace_root: workspace.path().to_path_buf(),
            ..ApiConfig::default()
        };
        (
            RalphMcpServer::new(config).expect("MCP server should initialize"),
            workspace,
        )
    }

    #[test]
    fn catalog_maps_known_tools() {
        let catalog = tool_catalog();
        assert!(catalog.lookup("task_list").is_some());
        assert!(catalog.lookup("loop_trigger_merge_task").is_some());
        assert!(catalog.lookup("stream_next").is_some());
    }

    #[test]
    fn catalog_publishes_output_schemas() {
        let catalog = tool_catalog();
        let task_create = catalog.lookup("task_create").expect("task_create tool");
        assert!(task_create.tool.output_schema.is_some());
        let stream_next = catalog.lookup("stream_next").expect("stream_next tool");
        assert!(stream_next.tool.output_schema.is_some());
    }

    #[tokio::test]
    async fn stream_next_times_out_without_events() {
        let (server, _workspace) = test_server();
        let subscribed = server
            .call_stream_subscribe(
                json!({ "topics": ["task.status.changed"] }),
                "req-stream-subscribe-1",
                "stream_subscribe",
            )
            .await
            .expect("stream.subscribe should succeed");
        let subscription_id = subscribed.structured_content.unwrap()["subscriptionId"]
            .as_str()
            .expect("subscription id")
            .to_string();
        let result = server
            .call_stream_next(
                json!({
                    "subscriptionId": subscription_id,
                    "waitMs": 1,
                }),
                "req-stream-next-timeout-1",
            )
            .await
            .expect("stream.next should succeed");

        assert_eq!(result.structured_content.unwrap()["timedOut"], true);
    }

    #[tokio::test]
    async fn stream_next_returns_matching_events() {
        let (server, _workspace) = test_server();
        let subscribed = server
            .call_stream_subscribe(
                json!({ "topics": ["task.status.changed"] }),
                "req-stream-subscribe-2",
                "stream_subscribe",
            )
            .await
            .expect("stream.subscribe should succeed");
        let subscription_id = subscribed.structured_content.unwrap()["subscriptionId"]
            .as_str()
            .expect("subscription id")
            .to_string();

        server.runtime.stream_domain().publish(
            "task.status.changed",
            "task",
            "task-123",
            json!({ "from": "open", "to": "running" }),
        );

        let result = server
            .call_stream_next(
                json!({
                    "subscriptionId": subscription_id,
                    "waitMs": 25,
                    "maxEvents": 5,
                }),
                "req-stream-next-events-1",
            )
            .await
            .expect("stream.next should succeed");

        let payload = result.structured_content.expect("stream.next payload");
        assert_eq!(payload["timedOut"], false);
        assert_eq!(payload["events"].as_array().unwrap().len(), 1);
        assert_eq!(payload["events"][0]["topic"], "task.status.changed");
    }
}
