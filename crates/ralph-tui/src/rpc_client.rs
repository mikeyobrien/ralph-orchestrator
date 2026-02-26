//! RPC v1 client for connecting the TUI to a remote ralph-api server.
//!
//! Provides HTTP request/response and WebSocket streaming for consuming
//! the same RPC v1 API that the web dashboard uses. This enables the TUI
//! to attach to a running orchestration loop from any terminal.

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Request / response types (mirrors ralph-api protocol)
// ---------------------------------------------------------------------------

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_request_id() -> String {
    let n = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("tui-{}-{:04x}", chrono::Utc::now().timestamp_millis(), n)
}

fn next_idempotency_key(method: &str) -> String {
    let n = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "idem-tui-{}-{}-{:04x}",
        method.replace('.', "-"),
        chrono::Utc::now().timestamp_millis(),
        n
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RpcRequest {
    api_version: String,
    id: String,
    method: String,
    params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<RequestMeta>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestMeta {
    idempotency_key: String,
    request_ts: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcResponse {
    #[allow(dead_code)]
    api_version: String,
    #[allow(dead_code)]
    id: String,
    result: Option<Value>,
    error: Option<RpcErrorBody>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

// ---------------------------------------------------------------------------
// Stream event types
// ---------------------------------------------------------------------------

/// A stream event received over WebSocket from ralph-api.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvent {
    #[allow(dead_code)]
    pub api_version: String,
    #[allow(dead_code)]
    pub stream: String,
    pub topic: String,
    pub cursor: String,
    pub sequence: u64,
    #[allow(dead_code)]
    pub ts: String,
    pub resource: StreamResource,
    pub replay: StreamReplay,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamResource {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamReplay {
    pub mode: String,
    #[allow(dead_code)]
    pub requested_cursor: Option<String>,
    #[allow(dead_code)]
    pub batch: Option<u64>,
}

// ---------------------------------------------------------------------------
// Subscribe result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeResult {
    pub subscription_id: String,
    pub accepted_topics: Vec<String>,
    pub cursor: String,
}

// ---------------------------------------------------------------------------
// Domain types returned by RPC methods
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopRecord {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResult {
    #[serde(default)]
    pub config: Value,
}

// ---------------------------------------------------------------------------
// RPC client
// ---------------------------------------------------------------------------

/// An RPC v1 client targeting a single ralph-api server.
#[derive(Clone)]
pub struct RpcClient {
    http: reqwest::Client,
    /// Base URL, e.g. `http://127.0.0.1:3000`
    base_url: url::Url,
}

impl RpcClient {
    /// Create a new client pointed at the given base URL.
    pub fn new(base_url: &str) -> Result<Self> {
        let base_url = url::Url::parse(base_url)
            .with_context(|| format!("invalid ralph-api base URL: {base_url}"))?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http, base_url })
    }

    /// Issue an RPC call and return the `result` value.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let is_mutating = is_mutating(method);
        let request = RpcRequest {
            api_version: "v1".to_string(),
            id: next_request_id(),
            method: method.to_string(),
            params,
            meta: if is_mutating {
                Some(RequestMeta {
                    idempotency_key: next_idempotency_key(method),
                    request_ts: chrono::Utc::now().to_rfc3339(),
                })
            } else {
                None
            },
        };

        let url = self
            .base_url
            .join("/rpc/v1")
            .context("failed to build RPC endpoint URL")?;

        let response = self
            .http
            .post(url)
            .json(&request)
            .send()
            .await
            .context("RPC HTTP request failed")?;

        let status = response.status();
        let body: RpcResponse = response
            .json()
            .await
            .context("failed to parse RPC response JSON")?;

        if let Some(err) = body.error {
            anyhow::bail!(
                "RPC error ({status}): [{code}] {msg}",
                code = err.code,
                msg = err.message
            );
        }

        body.result
            .ok_or_else(|| anyhow::anyhow!("RPC response missing result"))
    }

    // -- convenience wrappers ------------------------------------------------

    /// Fetch all tasks.
    pub async fn task_list(&self) -> Result<Vec<TaskRecord>> {
        let result = self.call("task.list", json!({})).await?;
        let tasks: Vec<TaskRecord> =
            serde_json::from_value(result.get("tasks").cloned().unwrap_or(Value::Array(vec![])))
                .context("failed to parse task list")?;
        Ok(tasks)
    }

    /// Fetch all loops.
    pub async fn loop_list(&self) -> Result<Vec<LoopRecord>> {
        let result = self
            .call("loop.list", json!({ "includeTerminal": true }))
            .await?;
        let loops: Vec<LoopRecord> =
            serde_json::from_value(result.get("loops").cloned().unwrap_or(Value::Array(vec![])))
                .context("failed to parse loop list")?;
        Ok(loops)
    }

    /// Fetch config.
    pub async fn config_get(&self) -> Result<Value> {
        self.call("config.get", json!({})).await
    }

    /// Create a stream subscription, returning the subscription ID and cursor.
    pub async fn stream_subscribe(
        &self,
        topics: &[&str],
        cursor: Option<&str>,
    ) -> Result<SubscribeResult> {
        let mut params = json!({
            "topics": topics,
        });
        if let Some(c) = cursor {
            params["cursor"] = Value::String(c.to_string());
        }
        let result = self.call("stream.subscribe", params).await?;
        serde_json::from_value(result).context("failed to parse subscribe result")
    }

    /// Build the WebSocket URL for the given subscription ID.
    pub fn stream_ws_url(&self, subscription_id: &str) -> Result<String> {
        let mut ws_url = self.base_url.clone();
        let scheme = match ws_url.scheme() {
            "https" => "wss",
            _ => "ws",
        };
        ws_url
            .set_scheme(scheme)
            .map_err(|()| anyhow::anyhow!("failed to set WebSocket scheme"))?;
        ws_url.set_path("/rpc/v1/stream");
        ws_url
            .query_pairs_mut()
            .append_pair("subscriptionId", subscription_id);
        Ok(ws_url.to_string())
    }

    /// Send a `stream.ack` to checkpoint the cursor.
    pub async fn stream_ack(&self, subscription_id: &str, cursor: &str) -> Result<()> {
        self.call(
            "stream.ack",
            json!({
                "subscriptionId": subscription_id,
                "cursor": cursor,
            }),
        )
        .await?;
        Ok(())
    }
}

fn is_mutating(method: &str) -> bool {
    matches!(
        method,
        "task.create"
            | "task.update"
            | "task.close"
            | "task.archive"
            | "task.unarchive"
            | "task.delete"
            | "task.clear"
            | "task.run"
            | "task.run_all"
            | "task.retry"
            | "task.cancel"
            | "loop.process"
            | "loop.prune"
            | "loop.retry"
            | "loop.discard"
            | "loop.stop"
            | "loop.merge"
            | "loop.trigger_merge_task"
            | "planning.start"
            | "planning.respond"
            | "planning.resume"
            | "planning.delete"
            | "config.update"
            | "collection.create"
            | "collection.update"
            | "collection.delete"
            | "collection.import"
    )
}
