//! ACP (Agent Client Protocol) executor for kiro-acp backend.
//!
//! Implements the ACP lifecycle: spawn → initialize → session/new → session/prompt.
//! Uses `agent-client-protocol` crate for bidirectional JSON-RPC over stdio.
//!
//! The ACP `Client` trait is `!Send`, so the protocol runs on a dedicated
//! single-threaded runtime inside `spawn_blocking`. Events are streamed back
//! to the caller via an unbounded channel for handler dispatch.

use std::path::PathBuf;
use std::time::Instant;

use agent_client_protocol::{
    Agent, ClientSideConnection, ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest,
    ProtocolVersion, RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, SessionUpdate, StopReason, TextContent,
    ToolCallStatus,
};
use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::{debug, warn};

use crate::cli_backend::CliBackend;
use crate::pty_executor::{PtyExecutionResult, TerminationType};
use crate::stream_handler::{SessionResult, StreamHandler};

/// Events dispatched from the ACP Client impl to the executor.
enum AcpEvent {
    Text(String),
    ToolCall {
        name: String,
        id: String,
        input: serde_json::Value,
    },
    ToolResult {
        id: String,
        output: String,
    },
    #[allow(dead_code)]
    Error(String),
    /// Prompt completed with a stop reason.
    Done(StopReason),
    /// ACP lifecycle failed.
    Failed(String),
}

/// Ralph's implementation of the ACP `Client` trait.
///
/// Auto-approves all permissions and forwards session notifications
/// as `AcpEvent`s through a channel.
struct RalphAcpClient {
    tx: mpsc::UnboundedSender<AcpEvent>,
}

#[async_trait::async_trait(?Send)]
impl agent_client_protocol::Client for RalphAcpClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> agent_client_protocol::Result<RequestPermissionResponse> {
        let option_id = args
            .options
            .first()
            .map(|o| o.option_id.clone())
            .unwrap_or_else(|| "allowed".into());
        Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id)),
        ))
    }

    async fn session_notification(
        &self,
        args: SessionNotification,
    ) -> agent_client_protocol::Result<()> {
        match args.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    let _ = self.tx.send(AcpEvent::Text(text.text));
                }
            }
            SessionUpdate::ToolCall(tc) => {
                let _ = self.tx.send(AcpEvent::ToolCall {
                    name: tc.title.clone(),
                    id: tc.tool_call_id.to_string(),
                    input: tc.raw_input.unwrap_or(serde_json::Value::Null),
                });
            }
            SessionUpdate::ToolCallUpdate(update) => {
                if update.fields.status == Some(ToolCallStatus::Completed) {
                    // Try structured content first, fall back to raw_output
                    let output = update
                        .fields
                        .content
                        .as_ref()
                        .and_then(|c| {
                            c.iter().find_map(|block| {
                                if let agent_client_protocol::ToolCallContent::Content(content) =
                                    block
                                    && let ContentBlock::Text(t) = &content.content
                                {
                                    return Some(t.text.clone());
                                }
                                None
                            })
                        })
                        .or_else(|| {
                            update.fields.raw_output.as_ref().map(|v| match v {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            })
                        })
                        .unwrap_or_default();
                    let _ = self.tx.send(AcpEvent::ToolResult {
                        id: update.tool_call_id.to_string(),
                        output,
                    });
                }
            }
            SessionUpdate::Plan(plan) => {
                let text = plan
                    .entries
                    .iter()
                    .map(|e| format!("- {}", e.content))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    let _ = self
                        .tx
                        .send(AcpEvent::Text(format!("\n## Plan\n{}\n", text)));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Executor for ACP-based backends (kiro-acp).
pub struct AcpExecutor {
    backend: CliBackend,
    workspace_root: PathBuf,
}

impl AcpExecutor {
    pub fn new(backend: CliBackend, workspace_root: PathBuf) -> Self {
        Self {
            backend,
            workspace_root,
        }
    }

    /// Execute a single prompt turn via ACP.
    ///
    /// The ACP protocol runs on a dedicated thread (Client trait is `!Send`).
    /// Events stream back via channel for real-time handler dispatch.
    pub async fn execute<H: StreamHandler>(
        &self,
        prompt: &str,
        handler: &mut H,
    ) -> Result<PtyExecutionResult> {
        let start = Instant::now();
        let mut text_output = String::new();

        let (tx, mut rx) = mpsc::unbounded_channel::<AcpEvent>();
        let backend = self.backend.clone();
        let workspace_root = self.workspace_root.clone();
        let prompt_owned = prompt.to_string();

        // Run ACP lifecycle on a blocking thread with its own runtime
        // (ClientSideConnection / Client trait is !Send)
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build ACP runtime");
            let local = tokio::task::LocalSet::new();
            local.block_on(
                &rt,
                run_acp_lifecycle(backend, workspace_root, prompt_owned, tx),
            );
        });

        // Process streamed events until Done/Failed
        let mut stop_reason = None;
        let mut error_msg = None;
        while let Some(event) = rx.recv().await {
            match event {
                AcpEvent::Text(t) => {
                    text_output.push_str(&t);
                    handler.on_text(&t);
                }
                AcpEvent::ToolCall { name, id, input } => {
                    handler.on_tool_call(&name, &id, &input);
                }
                AcpEvent::ToolResult { id, output } => {
                    handler.on_tool_result(&id, &output);
                }
                AcpEvent::Error(e) => {
                    handler.on_error(&e);
                }
                AcpEvent::Done(reason) => {
                    stop_reason = Some(reason);
                    break;
                }
                AcpEvent::Failed(msg) => {
                    error_msg = Some(msg);
                    break;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let (success, is_error) = if let Some(reason) = stop_reason {
            match reason {
                StopReason::EndTurn | StopReason::MaxTokens | StopReason::MaxTurnRequests => {
                    (true, false)
                }
                _ => (false, true),
            }
        } else if let Some(msg) = error_msg {
            handler.on_error(&format!("ACP session failed: {}", msg));
            (false, true)
        } else {
            warn!("ACP channel closed without completion");
            (false, true)
        };

        handler.on_complete(&SessionResult {
            duration_ms,
            total_cost_usd: 0.0,
            num_turns: 1,
            is_error,
        });

        Ok(PtyExecutionResult {
            output: text_output.clone(),
            stripped_output: text_output.clone(),
            extracted_text: text_output,
            success,
            exit_code: if success { Some(0) } else { Some(1) },
            termination: TerminationType::Natural,
        })
    }
}

/// Runs the full ACP lifecycle on a LocalSet (single-threaded).
async fn run_acp_lifecycle(
    backend: CliBackend,
    workspace_root: PathBuf,
    prompt: String,
    tx: mpsc::UnboundedSender<AcpEvent>,
) {
    if let Err(e) = run_acp_lifecycle_inner(&backend, &workspace_root, &prompt, &tx).await {
        let _ = tx.send(AcpEvent::Failed(e.to_string()));
    }
}

async fn run_acp_lifecycle_inner(
    backend: &CliBackend,
    workspace_root: &PathBuf,
    prompt: &str,
    tx: &mpsc::UnboundedSender<AcpEvent>,
) -> Result<()> {
    // Spawn child process
    let mut child = tokio::process::Command::new(&backend.command)
        .args(&backend.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("Failed to spawn ACP process")?;

    let child_stdin = child.stdin.take().context("No stdin")?;
    let child_stdout = child.stdout.take().context("No stdout")?;

    let client = RalphAcpClient { tx: tx.clone() };

    let (conn, io_task) = ClientSideConnection::new(
        client,
        child_stdin.compat_write(),
        child_stdout.compat(),
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );

    tokio::task::spawn_local(async move {
        if let Err(e) = io_task.await {
            debug!("ACP IO task ended: {}", e);
        }
    });

    // Initialize
    let init_req = InitializeRequest::new(ProtocolVersion::LATEST).client_info(
        agent_client_protocol::Implementation::new("ralph-orchestrator", env!("CARGO_PKG_VERSION")),
    );
    conn.initialize(init_req)
        .await
        .context("ACP initialize failed")?;

    // New session
    let session = conn
        .new_session(NewSessionRequest::new(workspace_root))
        .await
        .context("ACP session/new failed")?;

    debug!("ACP session created: {}", session.session_id);

    // Send prompt
    let response = conn
        .prompt(PromptRequest::new(
            session.session_id,
            vec![ContentBlock::Text(TextContent::new(prompt))],
        ))
        .await
        .context("ACP session/prompt failed")?;

    let _ = tx.send(AcpEvent::Done(response.stop_reason));

    // Kill child
    let _ = child.kill().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_executor_new() {
        let backend = CliBackend::kiro_acp();
        let executor = AcpExecutor::new(backend, PathBuf::from("/tmp"));
        assert_eq!(executor.backend.command, "kiro-cli");
        assert_eq!(executor.workspace_root, PathBuf::from("/tmp"));
    }

    /// AcpEvent::Failed should produce a graceful error, not crash the loop.
    #[tokio::test]
    async fn test_acp_failed_event_returns_error_not_panic() {
        let (tx, rx) = mpsc::unbounded_channel::<AcpEvent>();

        // Simulate a failed ACP session
        tx.send(AcpEvent::Text("partial output".to_string()))
            .unwrap();
        tx.send(AcpEvent::Failed("session/prompt failed".to_string()))
            .unwrap();
        drop(tx);

        // Process events the same way execute() does
        let mut handler = TestHandler::default();
        let mut text_output = String::new();
        let mut stop_reason = None;
        let mut error_msg = None;
        let mut rx = rx;

        while let Some(event) = rx.recv().await {
            match event {
                AcpEvent::Text(t) => {
                    text_output.push_str(&t);
                    handler.on_text(&t);
                }
                AcpEvent::ToolCall { name, id, input } => {
                    handler.on_tool_call(&name, &id, &input);
                }
                AcpEvent::ToolResult { id, output } => {
                    handler.on_tool_result(&id, &output);
                }
                AcpEvent::Error(e) => {
                    handler.on_error(&e);
                }
                AcpEvent::Done(reason) => {
                    stop_reason = Some(reason);
                    break;
                }
                AcpEvent::Failed(msg) => {
                    error_msg = Some(msg);
                    break;
                }
            }
        }

        // Should have captured the error, not panicked
        assert!(stop_reason.is_none());
        assert!(error_msg.is_some());
        assert!(error_msg.unwrap().contains("session/prompt failed"));
        assert!(text_output.contains("partial"));
    }

    #[derive(Default)]
    struct TestHandler {
        errors: Vec<String>,
    }

    impl StreamHandler for TestHandler {
        fn on_text(&mut self, _: &str) {}
        fn on_tool_call(&mut self, _: &str, _: &str, _: &serde_json::Value) {}
        fn on_tool_result(&mut self, _: &str, _: &str) {}
        fn on_error(&mut self, error: &str) {
            self.errors.push(error.to_string());
        }
        fn on_complete(&mut self, _: &SessionResult) {}
    }
}
