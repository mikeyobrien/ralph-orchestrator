//! ACP (Agent Client Protocol) executor for kiro-acp backend.
//!
//! Implements the ACP lifecycle: spawn → initialize → session/new → session/prompt.
//! Uses `agent-client-protocol` crate for bidirectional JSON-RPC over stdio.
//!
//! The ACP `Client` trait is `!Send`, so the protocol runs on a dedicated
//! single-threaded runtime inside `spawn_blocking`. Events are streamed back
//! to the caller via an unbounded channel for handler dispatch.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use agent_client_protocol::{
    Agent, CancelNotification, ClientSideConnection, ContentBlock, CreateTerminalRequest,
    CreateTerminalResponse, InitializeRequest, KillTerminalCommandRequest,
    KillTerminalCommandResponse, NewSessionRequest, PromptRequest, ProtocolVersion,
    ReleaseTerminalRequest, ReleaseTerminalResponse, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionNotification, SessionUpdate, StopReason, TerminalExitStatus, TerminalId,
    TerminalOutputRequest, TerminalOutputResponse, TextContent, ToolCallStatus,
    WaitForTerminalExitRequest, WaitForTerminalExitResponse,
};
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::io::AsyncReadExt;
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

/// State for a single ACP terminal (child process + captured output).
struct TerminalState {
    child: tokio::process::Child,
    output: Rc<RefCell<Vec<u8>>>,
    exit_status: Rc<RefCell<Option<TerminalExitStatus>>>,
    output_byte_limit: Option<u64>,
}

type Terminals = Rc<RefCell<HashMap<String, TerminalState>>>;

/// Ralph's implementation of the ACP `Client` trait.
///
/// Auto-approves all permissions and forwards session notifications
/// as `AcpEvent`s through a channel.
struct RalphAcpClient {
    tx: mpsc::UnboundedSender<AcpEvent>,
    terminals: Terminals,
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
                // ACP sends two ToolCall notifications per tool:
                // 1. Initial: no raw_input, no locations (just "tool started")
                // 2. Update: has raw_input with actual parameters and a descriptive title
                // Skip the first one to avoid showing bare "[Tool] ls" with no details.
                if tc.raw_input.is_none() && tc.locations.is_empty() {
                    return Ok(());
                }

                let input = tc.raw_input.clone().unwrap_or_else(|| {
                    if let Some(loc) = tc.locations.first() {
                        serde_json::json!({"path": loc.path.display().to_string()})
                    } else {
                        serde_json::Value::Null
                    }
                });
                let _ = self.tx.send(AcpEvent::ToolCall {
                    name: tc.title.clone(),
                    id: tc.tool_call_id.to_string(),
                    input,
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

    async fn create_terminal(
        &self,
        args: CreateTerminalRequest,
    ) -> agent_client_protocol::Result<CreateTerminalResponse> {
        debug!("ACP create_terminal: {} {:?}", args.command, args.args);
        let mut cmd = tokio::process::Command::new(&args.command);
        cmd.args(&args.args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null());

        if let Some(cwd) = &args.cwd {
            cmd.current_dir(cwd);
        }
        for env_var in &args.env {
            cmd.env(&env_var.name, &env_var.value);
        }

        let mut child = cmd.spawn().map_err(|e| {
            let mut err = agent_client_protocol::Error::internal_error();
            err.message = format!("spawn failed: {e}");
            err
        })?;

        let id = format!("term-{}", child.id().unwrap_or(0));
        let output_buf = Rc::new(RefCell::new(Vec::new()));
        let exit_status = Rc::new(RefCell::new(None));

        // Spawn background reader for stdout
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let buf_clone = Rc::clone(&output_buf);
        let exit_clone = Rc::clone(&exit_status);
        let limit = args.output_byte_limit;

        tokio::task::spawn_local(async move {
            let mut combined = Vec::new();
            if let Some(mut out) = stdout {
                let mut tmp = vec![0u8; 8192];
                loop {
                    match out.read(&mut tmp).await {
                        Ok(0) => break,
                        Ok(n) => combined.extend_from_slice(&tmp[..n]),
                        Err(_) => break,
                    }
                }
            }
            if let Some(mut err) = stderr {
                let mut tmp = vec![0u8; 8192];
                loop {
                    match err.read(&mut tmp).await {
                        Ok(0) => break,
                        Ok(n) => combined.extend_from_slice(&tmp[..n]),
                        Err(_) => break,
                    }
                }
            }
            // Apply byte limit (truncate from beginning)
            if let Some(max) = limit {
                let max = max as usize;
                if combined.len() > max {
                    // Find a valid UTF-8 boundary
                    let start = combined.len() - max;
                    let s = String::from_utf8_lossy(&combined[start..]);
                    combined = s.into_owned().into_bytes();
                }
            }
            *buf_clone.borrow_mut() = combined;
            // Mark as "reader done" — exit_status set by wait
            let _ = exit_clone;
        });

        self.terminals.borrow_mut().insert(
            id.clone(),
            TerminalState {
                child,
                output: output_buf,
                exit_status,
                output_byte_limit: args.output_byte_limit,
            },
        );

        Ok(CreateTerminalResponse::new(TerminalId::new(id)))
    }

    async fn terminal_output(
        &self,
        args: TerminalOutputRequest,
    ) -> agent_client_protocol::Result<TerminalOutputResponse> {
        let terminals = self.terminals.borrow();
        let state = terminals.get(args.terminal_id.0.as_ref()).ok_or_else(|| {
            let mut err = agent_client_protocol::Error::invalid_params();
            err.message = format!("unknown terminal: {}", args.terminal_id);
            err
        })?;

        let buf = state.output.borrow();
        let output = String::from_utf8_lossy(&buf).into_owned();
        let truncated = state
            .output_byte_limit
            .is_some_and(|limit| buf.len() >= limit as usize);
        let exit_status = state.exit_status.borrow().clone();

        Ok(TerminalOutputResponse::new(output, truncated).exit_status(exit_status))
    }

    async fn wait_for_terminal_exit(
        &self,
        args: WaitForTerminalExitRequest,
    ) -> agent_client_protocol::Result<WaitForTerminalExitResponse> {
        // Take child out temporarily to await it (can't hold borrow across await)
        let (mut child, exit_rc) = {
            let mut terminals = self.terminals.borrow_mut();
            let state = terminals
                .get_mut(args.terminal_id.0.as_ref())
                .ok_or_else(|| {
                    let mut err = agent_client_protocol::Error::invalid_params();
                    err.message = format!("unknown terminal: {}", args.terminal_id);
                    err
                })?;
            let exit_rc = Rc::clone(&state.exit_status);
            // Check if already exited
            if let Some(status) = state.exit_status.borrow().as_ref() {
                return Ok(WaitForTerminalExitResponse::new(status.clone()));
            }
            // Try non-blocking wait
            if let Ok(Some(status)) = state.child.try_wait() {
                let es = TerminalExitStatus::new().exit_code(status.code().map(|c| c as u32));
                *state.exit_status.borrow_mut() = Some(es.clone());
                return Ok(WaitForTerminalExitResponse::new(es));
            }
            // Need to actually await — swap in a placeholder
            let placeholder_child = tokio::process::Command::new("true").spawn().map_err(|e| {
                let mut err = agent_client_protocol::Error::internal_error();
                err.message = format!("internal error: {e}");
                err
            })?;
            let real_child = std::mem::replace(&mut state.child, placeholder_child);
            (real_child, exit_rc)
        };

        let status = child.wait().await.map_err(|e| {
            let mut err = agent_client_protocol::Error::internal_error();
            err.message = format!("wait failed: {e}");
            err
        })?;

        let es = TerminalExitStatus::new().exit_code(status.code().map(|c| c as u32));
        *exit_rc.borrow_mut() = Some(es.clone());

        Ok(WaitForTerminalExitResponse::new(es))
    }

    async fn release_terminal(
        &self,
        args: ReleaseTerminalRequest,
    ) -> agent_client_protocol::Result<ReleaseTerminalResponse> {
        let mut state = self
            .terminals
            .borrow_mut()
            .remove(args.terminal_id.0.as_ref())
            .ok_or_else(|| {
                let mut err = agent_client_protocol::Error::invalid_params();
                err.message = format!("unknown terminal: {}", args.terminal_id);
                err
            })?;

        let _ = state.child.kill().await;
        Ok(ReleaseTerminalResponse::new())
    }

    async fn kill_terminal_command(
        &self,
        args: KillTerminalCommandRequest,
    ) -> agent_client_protocol::Result<KillTerminalCommandResponse> {
        let terminal_id = args.terminal_id.0.to_string();
        let mut state = self
            .terminals
            .borrow_mut()
            .remove(terminal_id.as_str())
            .ok_or_else(|| {
                let mut err = agent_client_protocol::Error::invalid_params();
                err.message = format!("unknown terminal: {}", args.terminal_id);
                err
            })?;

        let _ = state.child.kill().await;
        // Try to capture exit status after kill
        if let Ok(status) = state.child.try_wait()
            && let Some(s) = status
        {
            *state.exit_status.borrow_mut() =
                Some(TerminalExitStatus::new().exit_code(s.code().map(|c| c as u32)));
        }

        // Keep terminal state addressable after kill for subsequent output/wait requests.
        self.terminals.borrow_mut().insert(terminal_id, state);

        Ok(KillTerminalCommandResponse::new())
    }
}

/// Drop guard that terminates the ACP child process.
///
/// When the `execute` future is cancelled (e.g., by `tokio::select!` on
/// interrupt), destructors still run. This ensures the child process tree
/// is cleaned up even if the normal cleanup code is never reached.
/// Sends SIGTERM first for graceful shutdown, then SIGKILL.
struct ChildKillGuard(Arc<Mutex<Option<u32>>>);

impl Drop for ChildKillGuard {
    fn drop(&mut self) {
        if let Ok(guard) = self.0.lock()
            && let Some(pid) = *guard
        {
            // Kill the entire process group (negative PID) so grandchildren
            // (e.g. MCP servers) are also terminated — not just the direct child.
            let pgid = nix::unistd::Pid::from_raw(-(pid as i32));
            let _ = nix::sys::signal::kill(pgid, nix::sys::signal::Signal::SIGTERM);
            std::thread::sleep(Duration::from_millis(100));
            let _ = nix::sys::signal::kill(pgid, nix::sys::signal::Signal::SIGKILL);
        }
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

        // Shared child PID for cleanup. Wrapped in a drop guard so the child
        // is killed even when this future is cancelled by tokio::select!.
        let child_pid = Arc::new(Mutex::new(None::<u32>));
        let child_pid_inner = Arc::clone(&child_pid);
        let _kill_guard = ChildKillGuard(Arc::clone(&child_pid));

        // Run ACP lifecycle on a blocking thread with its own runtime
        // (ClientSideConnection / Client trait is !Send)
        let join_handle = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build ACP runtime");
            let local = tokio::task::LocalSet::new();
            local.block_on(
                &rt,
                run_acp_lifecycle(backend, workspace_root, prompt_owned, tx, child_pid_inner),
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

        // Ensure the entire process tree is killed even if the blocking task is still running.
        if let Ok(guard) = child_pid.lock()
            && let Some(pid) = *guard
        {
            let pgid = nix::unistd::Pid::from_raw(-(pid as i32));
            let _ = nix::sys::signal::kill(pgid, nix::sys::signal::Signal::SIGKILL);
        }

        // Wait for the blocking task to finish so it doesn't leak.
        let _ = join_handle.await;

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
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        });

        Ok(PtyExecutionResult {
            output: text_output.clone(),
            stripped_output: text_output.clone(),
            extracted_text: text_output,
            success,
            exit_code: if success { Some(0) } else { Some(1) },
            termination: TerminationType::Natural,
            total_cost_usd: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        })
    }
}

/// Runs the full ACP lifecycle on a LocalSet (single-threaded).
async fn run_acp_lifecycle(
    backend: CliBackend,
    workspace_root: PathBuf,
    prompt: String,
    tx: mpsc::UnboundedSender<AcpEvent>,
    child_pid: Arc<Mutex<Option<u32>>>,
) {
    if let Err(e) =
        run_acp_lifecycle_inner(&backend, &workspace_root, &prompt, &tx, &child_pid).await
    {
        let _ = tx.send(AcpEvent::Failed(e.to_string()));
    }
}

async fn run_acp_lifecycle_inner(
    backend: &CliBackend,
    workspace_root: &PathBuf,
    prompt: &str,
    tx: &mpsc::UnboundedSender<AcpEvent>,
    child_pid: &Arc<Mutex<Option<u32>>>,
) -> Result<()> {
    // Spawn child process in its own process group so we can kill the
    // entire tree (including MCP servers) with a single group signal.
    let mut cmd = tokio::process::Command::new(&backend.command);
    cmd.args(&backend.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd.spawn().context("Failed to spawn ACP process")?;

    // Record PID so the caller can kill the process if needed.
    if let Some(pid) = child.id()
        && let Ok(mut guard) = child_pid.lock()
    {
        *guard = Some(pid);
    }

    let child_stdin = child.stdin.take().context("No stdin")?;
    let child_stdout = child.stdout.take().context("No stdout")?;

    // Log stderr from kiro-cli so we can see errors
    if let Some(stderr) = child.stderr.take() {
        tokio::task::spawn_local(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut line = String::new();
            use tokio::io::AsyncBufReadExt;
            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                warn!("kiro-cli stderr: {}", line.trim_end());
                line.clear();
            }
        });
    }

    let terminals: Terminals = Rc::new(RefCell::new(HashMap::new()));
    let client = RalphAcpClient {
        tx: tx.clone(),
        terminals: Rc::clone(&terminals),
    };

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
    let init_req = InitializeRequest::new(ProtocolVersion::LATEST)
        .client_info(agent_client_protocol::Implementation::new(
            "ralph-orchestrator",
            env!("CARGO_PKG_VERSION"),
        ))
        .client_capabilities(agent_client_protocol::ClientCapabilities::new().terminal(true));
    conn.initialize(init_req)
        .await
        .context("ACP initialize failed")?;

    debug!("ACP initialize succeeded");

    // New session
    let session = conn
        .new_session(NewSessionRequest::new(workspace_root))
        .await
        .context("ACP session/new failed")?;

    debug!("ACP session created: {}", session.session_id);

    // Send prompt
    let session_id = session.session_id.clone();
    debug!("ACP sending prompt...");
    let response = conn
        .prompt(PromptRequest::new(
            session.session_id,
            vec![ContentBlock::Text(TextContent::new(prompt))],
        ))
        .await
        .context("ACP session/prompt failed")?;

    let _ = tx.send(AcpEvent::Done(response.stop_reason));

    // Kill all active terminals before shutting down
    let active_terminals: Vec<_> = terminals.borrow_mut().drain().collect();
    for (_, mut state) in active_terminals {
        let _ = state.child.kill().await;
    }

    // Graceful shutdown: cancel the session so kiro-cli can clean up MCP servers
    let _ = conn.cancel(CancelNotification::new(session_id)).await;

    // Give the process a moment to exit cleanly, then force-kill
    match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
        Ok(_) => {}
        Err(_) => {
            let _ = child.kill().await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::Client;

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

    /// Helper to create a RalphAcpClient with a terminals map for testing.
    fn test_client() -> (RalphAcpClient, mpsc::UnboundedReceiver<AcpEvent>, Terminals) {
        let (tx, rx) = mpsc::unbounded_channel();
        let terminals: Terminals = Rc::new(RefCell::new(HashMap::new()));
        let client = RalphAcpClient {
            tx,
            terminals: Rc::clone(&terminals),
        };
        (client, rx, terminals)
    }

    #[tokio::test]
    async fn test_create_terminal_and_output() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();

                let req = CreateTerminalRequest::new("test-session", "echo")
                    .args(vec!["hello world".into()]);
                let resp = client.create_terminal(req).await.unwrap();

                // Terminal should be tracked
                assert!(terminals.borrow().contains_key(resp.terminal_id.0.as_ref()));

                // Wait for exit
                let wait_req =
                    WaitForTerminalExitRequest::new("test-session", resp.terminal_id.clone());
                let wait_resp = client.wait_for_terminal_exit(wait_req).await.unwrap();
                assert_eq!(wait_resp.exit_status.exit_code, Some(0));

                // Give background reader a moment to finish
                tokio::time::sleep(Duration::from_millis(100)).await;
                tokio::task::yield_now().await;

                // Get output
                let out_req = TerminalOutputRequest::new("test-session", resp.terminal_id.clone());
                let out_resp = client.terminal_output(out_req).await.unwrap();
                assert!(
                    out_resp.output.contains("hello world"),
                    "expected 'hello world' in output: {:?}",
                    out_resp.output
                );
                assert!(out_resp.exit_status.is_some());
            })
            .await;
    }

    #[tokio::test]
    async fn test_release_terminal_removes_from_map() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();

                let req =
                    CreateTerminalRequest::new("test-session", "sleep").args(vec!["60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                assert!(terminals.borrow().contains_key(tid.0.as_ref()));

                let rel_req = ReleaseTerminalRequest::new("test-session", tid.clone());
                client.release_terminal(rel_req).await.unwrap();

                assert!(!terminals.borrow().contains_key(tid.0.as_ref()));
            })
            .await;
    }

    #[tokio::test]
    async fn test_kill_terminal_keeps_in_map() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();

                let req =
                    CreateTerminalRequest::new("test-session", "sleep").args(vec!["60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let kill_req = KillTerminalCommandRequest::new("test-session", tid.clone());
                client.kill_terminal_command(kill_req).await.unwrap();

                // Should still be in the map
                assert!(terminals.borrow().contains_key(tid.0.as_ref()));
            })
            .await;
    }

    #[tokio::test]
    async fn test_terminal_output_unknown_id_errors() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();

                let req = TerminalOutputRequest::new("test-session", "nonexistent");
                let result = client.terminal_output(req).await;
                assert!(result.is_err());
            })
            .await;
    }

    #[tokio::test]
    async fn test_terminal_failed_command_exit_code() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();

                let req = CreateTerminalRequest::new("test-session", "false");
                let resp = client.create_terminal(req).await.unwrap();

                let wait_req =
                    WaitForTerminalExitRequest::new("test-session", resp.terminal_id.clone());
                let wait_resp = client.wait_for_terminal_exit(wait_req).await.unwrap();
                assert_ne!(wait_resp.exit_status.exit_code, Some(0));
            })
            .await;
    }
}
