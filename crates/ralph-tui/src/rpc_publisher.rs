//! Publishes orchestration events to a running ralph-api RPC stream.
//!
//! When the orchestration loop runs alongside a ralph-api server (e.g.,
//! `ralph web` or `ralph run` with the API enabled), this publisher bridges
//! in-process `EventBus` events to the RPC stream so remote TUI and web
//! clients can observe them in real time.
//!
//! This is an *optional* enhancement — the TUI works in-process without it,
//! and the RPC bridge works independently when consuming an API started by
//! a separate process.

use std::sync::Arc;

use ratatui::text::Line;
use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::rpc_client::RpcClient;

/// Publishes log lines and lifecycle events to the RPC stream.
///
/// Cloneable and cheap — holds only an `Arc<RpcClient>` and metadata.
#[derive(Clone)]
pub struct RpcPublisher {
    client: Arc<RpcClient>,
    iteration: u32,
    hat: Option<String>,
    backend: Option<String>,
}

impl RpcPublisher {
    /// Create a new publisher connected to the given ralph-api server.
    pub fn new(client: RpcClient) -> Self {
        Self {
            client: Arc::new(client),
            iteration: 0,
            hat: None,
            backend: None,
        }
    }

    /// Update iteration metadata (called at the start of each iteration).
    pub fn set_iteration(&mut self, iteration: u32, hat: Option<String>, backend: Option<String>) {
        self.iteration = iteration;
        self.hat = hat;
        self.backend = backend;
    }

    /// Publish a batch of ratatui `Line`s as `task.log.line` events.
    ///
    /// Each line is serialized as plain text (spans concatenated). This is
    /// intentionally lossy — styling is TUI-local, not part of the RPC wire
    /// format.
    pub async fn publish_lines(&self, lines: &[Line<'_>]) {
        for line in lines {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if text.is_empty() {
                continue;
            }

            let payload = json!({
                "line": text,
                "iteration": self.iteration,
                "hat": self.hat,
                "backend": self.backend,
            });

            if let Err(e) = self.publish_event("task.log.line", payload).await {
                debug!(error = %e, "Failed to publish log line to RPC stream");
            }
        }
    }

    /// Publish a lifecycle event (iteration start/end, loop completion).
    pub async fn publish_lifecycle(&self, status: &str, details: Option<Value>) {
        let mut payload = json!({
            "status": status,
            "iteration": self.iteration,
        });
        if let Some(hat) = &self.hat {
            payload["hat"] = Value::String(hat.clone());
        }
        if let Some(backend) = &self.backend {
            payload["backend"] = Value::String(backend.clone());
        }
        if let Some(d) = details
            && let Value::Object(map) = d
            && let Value::Object(ref mut p) = payload
        {
            p.extend(map);
        }

        if let Err(e) = self.publish_event("loop.status.changed", payload).await {
            warn!(error = %e, "Failed to publish lifecycle event to RPC stream");
        }
    }

    /// Low-level publish: POST an internal event to the ralph-api.
    ///
    /// This uses the `_internal.publish` method which is not part of the
    /// public RPC contract — it's an internal bridge endpoint for the
    /// orchestration loop to inject events into the stream domain.
    ///
    /// If the API server doesn't support this endpoint (older version or
    /// not running), failures are silently ignored.
    async fn publish_event(&self, topic: &str, payload: Value) -> anyhow::Result<()> {
        // Use the standard RPC call mechanism. The server-side handler for
        // _internal.publish inserts directly into the stream domain.
        self.client
            .call(
                "_internal.publish",
                json!({
                    "topic": topic,
                    "resourceType": "loop",
                    "resourceId": "primary",
                    "payload": payload,
                }),
            )
            .await
            .map(|_| ())
    }
}
