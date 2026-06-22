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
use tokio::io::{AsyncRead, AsyncReadExt};
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
    child: Option<tokio::process::Child>,
    pid: Option<u32>,
    output: Rc<RefCell<Vec<u8>>>,
    output_truncated: Rc<RefCell<bool>>,
    output_byte_limit: Option<u64>,
    exit_status: Rc<RefCell<Option<TerminalExitStatus>>>,
    pending_exit_status: Rc<RefCell<Option<TerminalExitStatus>>>,
    cleanup_in_progress: Rc<RefCell<bool>>,
    release_requested: Rc<RefCell<bool>>,
    reader: Option<tokio::task::JoinHandle<()>>,
}

type Terminals = Rc<RefCell<HashMap<String, TerminalState>>>;

const TERMINAL_READER_DRAIN_TIMEOUT: Duration = Duration::from_millis(250);

struct PendingTerminalWait {
    terminals: Terminals,
    terminal_id: String,
    child: Option<tokio::process::Child>,
    reader: Option<tokio::task::JoinHandle<()>>,
    cleanup_in_progress: Option<Rc<RefCell<bool>>>,
}

impl Drop for PendingTerminalWait {
    fn drop(&mut self) {
        let mut reset_cleanup = false;

        if let Ok(mut terminals) = self.terminals.try_borrow_mut()
            && let Some(state) = terminals.get_mut(&self.terminal_id)
            && state.exit_status.borrow().is_none()
        {
            reset_cleanup = true;
            if state.child.is_none() {
                state.child = self.child.take();
            }
            if state.reader.is_none() {
                state.reader = self.reader.take();
            }
        }

        if reset_cleanup && let Some(cleanup_in_progress) = &self.cleanup_in_progress {
            *cleanup_in_progress.borrow_mut() = false;
        }

        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        if let Some(reader) = self.reader.take() {
            reader.abort();
        }
    }
}

fn render_utf8_suffix_with_byte_limit(bytes: &[u8], max: usize) -> String {
    if max == 0 {
        return String::new();
    }

    let mut start = bytes.len().saturating_sub(max);
    while start < bytes.len() && bytes[start] & 0b1100_0000 == 0b1000_0000 {
        start += 1;
    }

    match std::str::from_utf8(&bytes[start..]) {
        Ok(text) => text.to_string(),
        Err(err) if err.error_len().is_none() => {
            let valid_end = start + err.valid_up_to();
            String::from_utf8_lossy(&bytes[start..valid_end]).into_owned()
        }
        Err(_) => {
            let text = String::from_utf8_lossy(&bytes[start..]).into_owned();
            let mut suffix_start = text.len().saturating_sub(max);
            while !text.is_char_boundary(suffix_start) {
                suffix_start += 1;
            }
            text[suffix_start..].to_string()
        }
    }
}

fn append_terminal_output(
    output: &Rc<RefCell<Vec<u8>>>,
    truncated: &Rc<RefCell<bool>>,
    chunk: &[u8],
    limit: Option<u64>,
) {
    let mut buf = output.borrow_mut();
    buf.extend_from_slice(chunk);

    if let Some(max) = limit {
        let max = usize::try_from(max).unwrap_or(usize::MAX);
        let retained = max.saturating_add(3);
        if max == 0 {
            if !buf.is_empty() {
                *truncated.borrow_mut() = true;
            }
            buf.clear();
        } else if buf.len() > retained {
            let drop_len = buf.len() - retained;
            buf.drain(..drop_len);
            *truncated.borrow_mut() = true;
        }
    }
}

async fn read_terminal_stream<R: AsyncRead + Unpin>(
    mut stream: R,
    output: Rc<RefCell<Vec<u8>>>,
    truncated: Rc<RefCell<bool>>,
    limit: Option<u64>,
) {
    let mut tmp = vec![0u8; 8192];
    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => break,
            Ok(n) => append_terminal_output(&output, &truncated, &tmp[..n], limit),
            Err(_) => break,
        }
    }
}

async fn drain_terminal_reader(reader: tokio::task::JoinHandle<()>) {
    let mut reader = Some(reader);
    drain_terminal_reader_in_place(&mut reader).await;
}

async fn drain_terminal_reader_in_place(reader: &mut Option<tokio::task::JoinHandle<()>>) {
    let Some(handle) = reader.as_mut() else {
        return;
    };

    if tokio::time::timeout(TERMINAL_READER_DRAIN_TIMEOUT, handle)
        .await
        .is_err()
        && let Some(handle) = reader.as_ref()
    {
        handle.abort();
    }
    reader.take();
}

async fn terminate_terminal_process_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        let pgid = nix::unistd::Pid::from_raw(-(pid as i32));
        if nix::sys::signal::kill(pgid, nix::sys::signal::Signal::SIGTERM).is_ok() {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = nix::sys::signal::kill(pgid, nix::sys::signal::Signal::SIGKILL);
        }
    }

    #[cfg(not(unix))]
    let _ = pid;
}

fn terminal_exit_status(status: std::process::ExitStatus) -> TerminalExitStatus {
    let mut exit_status = TerminalExitStatus::new().exit_code(status.code().map(|c| c as u32));

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;

        if let Some(signal) = status.signal() {
            let signal_name = nix::sys::signal::Signal::try_from(signal)
                .map(|signal| format!("{signal:?}"))
                .unwrap_or_else(|_| signal.to_string());
            exit_status = exit_status.signal(signal_name);
        }
    }

    exit_status
}

/// Ralph's implementation of the ACP `Client` trait.
///
/// Auto-approves all permissions and forwards session notifications
/// as `AcpEvent`s through a channel.
struct RalphAcpClient {
    tx: mpsc::UnboundedSender<AcpEvent>,
    terminals: Terminals,
}

impl RalphAcpClient {
    fn clear_terminal_pid_after_cleanup(&self, terminal_id: &str, pid: Option<u32>) {
        let Some(pid) = pid else {
            return;
        };
        let mut terminals = self.terminals.borrow_mut();
        let Some(state) = terminals.get_mut(terminal_id) else {
            return;
        };

        if state.pid == Some(pid) {
            state.pid = None;
        }
    }
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
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);

        #[cfg(unix)]
        cmd.process_group(0);

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

        let pid = child.id();
        let id = format!("term-{}", pid.unwrap_or(0));
        let output_buf = Rc::new(RefCell::new(Vec::new()));
        let output_truncated = Rc::new(RefCell::new(false));
        let exit_status = Rc::new(RefCell::new(None));
        let pending_exit_status = Rc::new(RefCell::new(None));
        let cleanup_in_progress = Rc::new(RefCell::new(false));
        let release_requested = Rc::new(RefCell::new(false));

        // Spawn background reader for stdout
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let buf_clone = Rc::clone(&output_buf);
        let truncated_clone = Rc::clone(&output_truncated);
        let exit_clone = Rc::clone(&exit_status);
        let limit = args.output_byte_limit;

        let reader = tokio::task::spawn_local(async move {
            let stdout_buf = Rc::clone(&buf_clone);
            let stderr_buf = Rc::clone(&buf_clone);
            let stdout_truncated = Rc::clone(&truncated_clone);
            let stderr_truncated = Rc::clone(&truncated_clone);
            let stdout_reader = async move {
                if let Some(out) = stdout {
                    read_terminal_stream(out, stdout_buf, stdout_truncated, limit).await;
                }
            };
            let stderr_reader = async move {
                if let Some(err) = stderr {
                    read_terminal_stream(err, stderr_buf, stderr_truncated, limit).await;
                }
            };
            tokio::join!(stdout_reader, stderr_reader);
            // Mark as "reader done" — exit_status set by wait.
            let _ = exit_clone;
        });

        self.terminals.borrow_mut().insert(
            id.clone(),
            TerminalState {
                child: Some(child),
                pid,
                output: output_buf,
                output_truncated,
                output_byte_limit: args.output_byte_limit,
                exit_status,
                pending_exit_status,
                cleanup_in_progress,
                release_requested,
                reader: Some(reader),
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
        let (output, bounded_by_limit) = if let Some(limit) = state.output_byte_limit {
            let limit = usize::try_from(limit).unwrap_or(usize::MAX);
            (
                render_utf8_suffix_with_byte_limit(&buf, limit),
                buf.len() > limit,
            )
        } else {
            (String::from_utf8_lossy(&buf).into_owned(), false)
        };
        let truncated = *state.output_truncated.borrow() || bounded_by_limit;
        let exit_status = state.exit_status.borrow().clone();

        Ok(TerminalOutputResponse::new(output, truncated).exit_status(exit_status))
    }

    async fn wait_for_terminal_exit(
        &self,
        args: WaitForTerminalExitRequest,
    ) -> agent_client_protocol::Result<WaitForTerminalExitResponse> {
        enum WaitPlan {
            Done(TerminalExitStatus),
            AwaitExisting(Rc<RefCell<Option<TerminalExitStatus>>>),
            AwaitChild {
                terminal_id: String,
                child: tokio::process::Child,
                exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
                pending_exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
                cleanup_in_progress: Rc<RefCell<bool>>,
                release_requested: Rc<RefCell<bool>>,
                reader: Option<tokio::task::JoinHandle<()>>,
                pid: Option<u32>,
            },
            FinalizeExited {
                terminal_id: String,
                exit_status: TerminalExitStatus,
                exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
                pending_exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
                cleanup_in_progress: Rc<RefCell<bool>>,
                release_requested: Rc<RefCell<bool>>,
                reader: Option<tokio::task::JoinHandle<()>>,
                pid: Option<u32>,
            },
        }

        let terminal_id = args.terminal_id.0.to_string();
        loop {
            let plan = {
                let mut terminals = self.terminals.borrow_mut();
                let state = terminals.get_mut(terminal_id.as_str()).ok_or_else(|| {
                    let mut err = agent_client_protocol::Error::invalid_params();
                    err.message = format!("unknown terminal: {}", args.terminal_id);
                    err
                })?;
                let exit_rc = Rc::clone(&state.exit_status);
                let pending_exit_rc = Rc::clone(&state.pending_exit_status);
                let cleanup_in_progress = Rc::clone(&state.cleanup_in_progress);
                let release_requested = Rc::clone(&state.release_requested);
                // Check if already exited
                if let Some(status) = state.exit_status.borrow().as_ref() {
                    WaitPlan::Done(status.clone())
                } else if let Some(status) = state.pending_exit_status.borrow().as_ref() {
                    if *state.cleanup_in_progress.borrow() {
                        WaitPlan::AwaitExisting(exit_rc)
                    } else {
                        *state.cleanup_in_progress.borrow_mut() = true;
                        WaitPlan::FinalizeExited {
                            terminal_id: terminal_id.clone(),
                            exit_status: status.clone(),
                            exit_rc,
                            pending_exit_rc,
                            cleanup_in_progress,
                            release_requested,
                            reader: state.reader.take(),
                            pid: state.pid,
                        }
                    }
                } else if let Some(child) = state.child.as_mut() {
                    // Try non-blocking wait
                    if let Ok(Some(status)) = child.try_wait() {
                        let es = terminal_exit_status(status);
                        *state.pending_exit_status.borrow_mut() = Some(es.clone());
                        *state.cleanup_in_progress.borrow_mut() = true;
                        state.child.take();
                        WaitPlan::FinalizeExited {
                            terminal_id: terminal_id.clone(),
                            exit_status: es,
                            exit_rc,
                            pending_exit_rc,
                            cleanup_in_progress,
                            release_requested,
                            reader: state.reader.take(),
                            pid: state.pid,
                        }
                    } else {
                        *state.cleanup_in_progress.borrow_mut() = true;
                        WaitPlan::AwaitChild {
                            terminal_id: terminal_id.clone(),
                            child: state.child.take().expect("child checked above"),
                            exit_rc,
                            pending_exit_rc,
                            cleanup_in_progress,
                            release_requested,
                            reader: state.reader.take(),
                            pid: state.pid,
                        }
                    }
                } else {
                    WaitPlan::AwaitExisting(exit_rc)
                }
            };

            match plan {
                WaitPlan::Done(status) => return Ok(WaitForTerminalExitResponse::new(status)),
                WaitPlan::AwaitExisting(exit_rc) => {
                    if let Some(status) = exit_rc.borrow().clone() {
                        return Ok(WaitForTerminalExitResponse::new(status));
                    }
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    if let Some(status) = exit_rc.borrow().clone() {
                        return Ok(WaitForTerminalExitResponse::new(status));
                    }
                }
                WaitPlan::FinalizeExited {
                    terminal_id,
                    exit_status,
                    exit_rc,
                    pending_exit_rc,
                    cleanup_in_progress,
                    release_requested,
                    reader,
                    pid,
                } => {
                    let mut pending = PendingTerminalWait {
                        terminals: Rc::clone(&self.terminals),
                        terminal_id: terminal_id.clone(),
                        child: None,
                        reader,
                        cleanup_in_progress: Some(Rc::clone(&cleanup_in_progress)),
                    };
                    terminate_terminal_process_group(pid).await;
                    self.clear_terminal_pid_after_cleanup(&terminal_id, pid);
                    drain_terminal_reader_in_place(&mut pending.reader).await;
                    *pending_exit_rc.borrow_mut() = None;
                    *cleanup_in_progress.borrow_mut() = false;
                    *exit_rc.borrow_mut() = Some(exit_status.clone());
                    if *release_requested.borrow() {
                        self.terminals.borrow_mut().remove(terminal_id.as_str());
                    }
                    return Ok(WaitForTerminalExitResponse::new(exit_status));
                }
                WaitPlan::AwaitChild {
                    terminal_id,
                    child,
                    exit_rc,
                    pending_exit_rc,
                    cleanup_in_progress,
                    release_requested,
                    reader,
                    pid,
                } => {
                    let mut pending = PendingTerminalWait {
                        terminals: Rc::clone(&self.terminals),
                        terminal_id: terminal_id.clone(),
                        child: Some(child),
                        reader,
                        cleanup_in_progress: Some(Rc::clone(&cleanup_in_progress)),
                    };
                    let status = pending
                        .child
                        .as_mut()
                        .expect("child is present")
                        .wait()
                        .await
                        .map_err(|e| {
                            let mut err = agent_client_protocol::Error::internal_error();
                            err.message = format!("wait failed: {e}");
                            err
                        })?;

                    let es = terminal_exit_status(status);
                    *pending_exit_rc.borrow_mut() = Some(es.clone());
                    *cleanup_in_progress.borrow_mut() = true;
                    pending.child.take();
                    terminate_terminal_process_group(pid).await;
                    self.clear_terminal_pid_after_cleanup(&terminal_id, pid);
                    drain_terminal_reader_in_place(&mut pending.reader).await;
                    *pending_exit_rc.borrow_mut() = None;
                    *cleanup_in_progress.borrow_mut() = false;
                    *exit_rc.borrow_mut() = Some(es.clone());
                    if *release_requested.borrow() {
                        self.terminals.borrow_mut().remove(terminal_id.as_str());
                    }
                    return Ok(WaitForTerminalExitResponse::new(es));
                }
            }
        }
    }

    async fn release_terminal(
        &self,
        args: ReleaseTerminalRequest,
    ) -> agent_client_protocol::Result<ReleaseTerminalResponse> {
        let terminal_id = args.terminal_id.0.to_string();
        let mut saw_terminal = false;

        loop {
            enum ReleasePlan {
                Wait {
                    pid: Option<u32>,
                },
                Remove,
                Cleanup {
                    child: Option<tokio::process::Child>,
                    reader: Option<tokio::task::JoinHandle<()>>,
                    exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
                    pending_exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
                    cleanup_in_progress: Rc<RefCell<bool>>,
                    pid: Option<u32>,
                },
            }

            let plan = {
                let mut terminals = self.terminals.borrow_mut();
                let Some(state) = terminals.get_mut(terminal_id.as_str()) else {
                    if saw_terminal {
                        return Ok(ReleaseTerminalResponse::new());
                    }

                    let mut err = agent_client_protocol::Error::invalid_params();
                    err.message = format!("unknown terminal: {}", args.terminal_id);
                    return Err(err);
                };
                saw_terminal = true;
                *state.release_requested.borrow_mut() = true;

                if *state.cleanup_in_progress.borrow() {
                    ReleasePlan::Wait { pid: state.pid }
                } else if state.child.is_none() && state.reader.is_none() {
                    if state.exit_status.borrow().is_some()
                        || state.pending_exit_status.borrow().is_some()
                    {
                        ReleasePlan::Remove
                    } else {
                        ReleasePlan::Wait { pid: state.pid }
                    }
                } else {
                    *state.cleanup_in_progress.borrow_mut() = true;
                    ReleasePlan::Cleanup {
                        child: state.child.take(),
                        reader: state.reader.take(),
                        exit_rc: Rc::clone(&state.exit_status),
                        pending_exit_rc: Rc::clone(&state.pending_exit_status),
                        cleanup_in_progress: Rc::clone(&state.cleanup_in_progress),
                        pid: state.pid,
                    }
                }
            };

            match plan {
                ReleasePlan::Wait { pid } => {
                    terminate_terminal_process_group(pid).await;
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                ReleasePlan::Remove => {
                    self.terminals.borrow_mut().remove(terminal_id.as_str());
                    return Ok(ReleaseTerminalResponse::new());
                }
                ReleasePlan::Cleanup {
                    child,
                    reader,
                    exit_rc,
                    pending_exit_rc,
                    cleanup_in_progress,
                    pid,
                } => {
                    let mut pending = PendingTerminalWait {
                        terminals: Rc::clone(&self.terminals),
                        terminal_id: terminal_id.clone(),
                        child,
                        reader,
                        cleanup_in_progress: Some(Rc::clone(&cleanup_in_progress)),
                    };

                    terminate_terminal_process_group(pid).await;
                    self.clear_terminal_pid_after_cleanup(&terminal_id, pid);
                    if pending.child.is_some() {
                        if let Ok(status) = pending
                            .child
                            .as_mut()
                            .expect("child is present")
                            .wait()
                            .await
                        {
                            *pending_exit_rc.borrow_mut() = Some(terminal_exit_status(status));
                        }
                        pending.child.take();
                    }
                    drain_terminal_reader_in_place(&mut pending.reader).await;
                    let exit_status = pending_exit_rc.borrow().clone();
                    *pending_exit_rc.borrow_mut() = None;
                    *cleanup_in_progress.borrow_mut() = false;
                    if let Some(exit_status) = exit_status {
                        *exit_rc.borrow_mut() = Some(exit_status);
                    }
                    self.terminals.borrow_mut().remove(terminal_id.as_str());
                    return Ok(ReleaseTerminalResponse::new());
                }
            }
        }
    }

    async fn kill_terminal_command(
        &self,
        args: KillTerminalCommandRequest,
    ) -> agent_client_protocol::Result<KillTerminalCommandResponse> {
        let terminal_id = args.terminal_id.0.to_string();
        enum KillPlan {
            Done,
            SignalOnly { pid: Option<u32> },
            Cleanup(Box<KillCleanupPlan>),
        }

        struct KillCleanupPlan {
            child: Option<tokio::process::Child>,
            reader: Option<tokio::task::JoinHandle<()>>,
            exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
            pending_exit_rc: Rc<RefCell<Option<TerminalExitStatus>>>,
            cleanup_in_progress: Rc<RefCell<bool>>,
            release_requested: Rc<RefCell<bool>>,
            pid: Option<u32>,
        }

        let plan = {
            let mut terminals = self.terminals.borrow_mut();
            let state = terminals.get_mut(terminal_id.as_str()).ok_or_else(|| {
                let mut err = agent_client_protocol::Error::invalid_params();
                err.message = format!("unknown terminal: {}", args.terminal_id);
                err
            })?;

            if state.exit_status.borrow().is_some() {
                KillPlan::Done
            } else if *state.cleanup_in_progress.borrow() {
                KillPlan::SignalOnly { pid: state.pid }
            } else if state.pending_exit_status.borrow().is_some() || state.child.is_some() {
                *state.cleanup_in_progress.borrow_mut() = true;
                KillPlan::Cleanup(Box::new(KillCleanupPlan {
                    child: state.child.take(),
                    reader: state.reader.take(),
                    exit_rc: Rc::clone(&state.exit_status),
                    pending_exit_rc: Rc::clone(&state.pending_exit_status),
                    cleanup_in_progress: Rc::clone(&state.cleanup_in_progress),
                    release_requested: Rc::clone(&state.release_requested),
                    pid: state.pid,
                }))
            } else {
                KillPlan::SignalOnly { pid: state.pid }
            }
        };

        let (child, reader, exit_rc, pending_exit_rc, cleanup_in_progress, release_requested, pid) =
            match plan {
                KillPlan::Done => return Ok(KillTerminalCommandResponse::new()),
                KillPlan::SignalOnly { pid } => {
                    terminate_terminal_process_group(pid).await;
                    return Ok(KillTerminalCommandResponse::new());
                }
                KillPlan::Cleanup(cleanup) => {
                    let KillCleanupPlan {
                        child,
                        reader,
                        exit_rc,
                        pending_exit_rc,
                        cleanup_in_progress,
                        release_requested,
                        pid,
                    } = *cleanup;
                    (
                        child,
                        reader,
                        exit_rc,
                        pending_exit_rc,
                        cleanup_in_progress,
                        release_requested,
                        pid,
                    )
                }
            };

        let mut pending = PendingTerminalWait {
            terminals: Rc::clone(&self.terminals),
            terminal_id: terminal_id.clone(),
            child,
            reader,
            cleanup_in_progress: Some(Rc::clone(&cleanup_in_progress)),
        };

        terminate_terminal_process_group(pid).await;
        self.clear_terminal_pid_after_cleanup(&terminal_id, pid);

        if pending.child.is_some() {
            if let Ok(status) = pending
                .child
                .as_mut()
                .expect("child is present")
                .wait()
                .await
            {
                *pending_exit_rc.borrow_mut() = Some(terminal_exit_status(status));
            } else if pending_exit_rc.borrow().is_none() && exit_rc.borrow().is_none() {
                *pending_exit_rc.borrow_mut() = Some(TerminalExitStatus::new());
            }
            pending.child.take();
        }

        if pending_exit_rc.borrow().is_some() {
            drain_terminal_reader_in_place(&mut pending.reader).await;
            if let Some(status) = pending_exit_rc.borrow_mut().take() {
                *exit_rc.borrow_mut() = Some(status);
            }
        } else if let Some(reader) = pending.reader.take() {
            drain_terminal_reader(reader).await;
        }
        *cleanup_in_progress.borrow_mut() = false;
        if *release_requested.borrow() {
            self.terminals.borrow_mut().remove(terminal_id.as_str());
        }

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
    context_window: u64,
}

impl AcpExecutor {
    pub fn new(backend: CliBackend, workspace_root: PathBuf) -> Self {
        Self {
            backend,
            workspace_root,
            context_window: 0,
        }
    }

    /// Sets the resolved context-window ceiling (tokens) for this run.
    ///
    /// Threaded into `SessionResult.context_window` so downstream renderers can
    /// emit the `Context: NN% (KK/200K)` suffix when the user provides an
    /// explicit override (`event_loop.context_window_tokens`). For ACP backends
    /// without an override this stays at 0 (the suffix is suppressed).
    pub fn set_context_window(&mut self, context_window: u64) {
        self.context_window = context_window;
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
            context_window: self.context_window,
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
            num_turns: 1,
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
        terminate_terminal_process_group(state.pid).await;
        if let Some(mut child) = state.child.take() {
            let _ = child.wait().await;
        }
        if let Some(reader) = state.reader.take() {
            drain_terminal_reader(reader).await;
        }
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

    #[cfg(unix)]
    fn test_process_alive(pid: u32) -> bool {
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok()
    }

    #[cfg(unix)]
    async fn wait_for_process_exit(pid: u32) -> bool {
        for _ in 0..40 {
            if !test_process_alive(pid) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    #[test]
    fn render_utf8_suffix_with_byte_limit_preserves_character_boundary() {
        let bytes = "a\u{00e9}\u{00e9}\u{00e9}".as_bytes();

        let output = render_utf8_suffix_with_byte_limit(bytes, 5);

        assert_eq!(output, "\u{00e9}\u{00e9}");
        assert!(output.len() <= 5);
    }

    #[test]
    fn append_terminal_output_preserves_split_utf8_for_later_reads() {
        let output = Rc::new(RefCell::new(Vec::new()));
        let truncated = Rc::new(RefCell::new(false));

        append_terminal_output(&output, &truncated, b"a\xc3", Some(5));
        assert_eq!(render_utf8_suffix_with_byte_limit(&output.borrow(), 5), "a");

        append_terminal_output(&output, &truncated, b"\xa9\xc3\xa9\xc3\xa9", Some(5));
        let rendered = render_utf8_suffix_with_byte_limit(&output.borrow(), 5);

        assert_eq!(rendered, "\u{00e9}\u{00e9}");
        assert!(rendered.len() <= 5);
        assert!(!rendered.contains('\u{fffd}'));
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
    async fn test_terminal_output_limit_preserves_utf8_boundary() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec![
                        "-c".into(),
                        r"printf 'a\303\251\303\251\303\251'".into(),
                    ])
                    .output_byte_limit(5);
                let resp = client.create_terminal(req).await.unwrap();

                let wait_req =
                    WaitForTerminalExitRequest::new("test-session", resp.terminal_id.clone());
                let wait_resp = client.wait_for_terminal_exit(wait_req).await.unwrap();
                assert_eq!(wait_resp.exit_status.exit_code, Some(0));

                let out_req = TerminalOutputRequest::new("test-session", resp.terminal_id.clone());
                let out_resp = client.terminal_output(out_req).await.unwrap();

                assert_eq!(out_resp.output, "\u{00e9}\u{00e9}");
                assert!(out_resp.output.len() <= 5);
                assert!(out_resp.truncated);
                assert!(
                    !out_resp.output.contains('\u{fffd}'),
                    "output should not contain replacement characters: {:?}",
                    out_resp.output
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_wait_for_terminal_exit_does_not_hang_on_inherited_pipes() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec!["-c".into(), "echo done; sleep 2 &".into()]);
                let resp = client.create_terminal(req).await.unwrap();

                let wait_req =
                    WaitForTerminalExitRequest::new("test-session", resp.terminal_id.clone());
                let wait_resp = tokio::time::timeout(
                    Duration::from_secs(1),
                    client.wait_for_terminal_exit(wait_req),
                )
                .await
                .expect("wait_for_terminal_exit should not wait for inherited pipes")
                .unwrap();
                assert_eq!(wait_resp.exit_status.exit_code, Some(0));

                let out_req = TerminalOutputRequest::new("test-session", resp.terminal_id.clone());
                let out_resp = client.terminal_output(out_req).await.unwrap();
                assert!(
                    out_resp.output.contains("done"),
                    "expected inherited-pipe command output: {:?}",
                    out_resp.output
                );
            })
            .await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_wait_for_terminal_exit_cleans_inherited_pipe_descendant() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();
                let temp = tempfile::tempdir().unwrap();
                let pid_path = temp.path().join("descendant.pid");
                let script = format!("sleep 60 & echo $! > {}; echo done", pid_path.display());

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec!["-c".into(), script]);
                let resp = client.create_terminal(req).await.unwrap();

                let wait_req =
                    WaitForTerminalExitRequest::new("test-session", resp.terminal_id.clone());
                let wait_resp = tokio::time::timeout(
                    Duration::from_secs(2),
                    client.wait_for_terminal_exit(wait_req),
                )
                .await
                .expect("wait_for_terminal_exit should not wait for inherited pipes")
                .unwrap();
                assert_eq!(wait_resp.exit_status.exit_code, Some(0));

                let raw_pid = std::fs::read_to_string(&pid_path).unwrap();
                let descendant_pid = raw_pid.trim().parse::<u32>().unwrap();
                if !wait_for_process_exit(descendant_pid).await {
                    let pid = nix::unistd::Pid::from_raw(descendant_pid as i32);
                    let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL);
                    panic!("descendant process {descendant_pid} was not cleaned up");
                }
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
    async fn test_kill_terminal_during_wait_unblocks_waiter() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);

                let req =
                    CreateTerminalRequest::new("test-session", "sleep").args(vec!["60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let wait_client = Rc::clone(&client);
                let wait_tid = tid.clone();
                let waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", wait_tid);
                    wait_client.wait_for_terminal_exit(wait_req).await
                });

                tokio::time::sleep(Duration::from_millis(50)).await;

                let kill_req = KillTerminalCommandRequest::new("test-session", tid.clone());
                client.kill_terminal_command(kill_req).await.unwrap();

                let wait_resp = tokio::time::timeout(Duration::from_secs(2), waiter)
                    .await
                    .expect("wait should be unblocked by kill")
                    .unwrap()
                    .unwrap();

                assert_ne!(wait_resp.exit_status.exit_code, Some(0));
                assert!(terminals.borrow().contains_key(tid.0.as_ref()));
            })
            .await;
    }

    #[tokio::test]
    async fn test_cancelled_wait_returns_child_to_terminal_state() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);

                let req =
                    CreateTerminalRequest::new("test-session", "sleep").args(vec!["60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let wait_client = Rc::clone(&client);
                let wait_tid = tid.clone();
                let waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", wait_tid);
                    wait_client.wait_for_terminal_exit(wait_req).await
                });

                tokio::time::sleep(Duration::from_millis(50)).await;
                waiter.abort();
                assert!(waiter.await.unwrap_err().is_cancelled());

                let kill_req = KillTerminalCommandRequest::new("test-session", tid.clone());
                client.kill_terminal_command(kill_req).await.unwrap();

                let wait_req = WaitForTerminalExitRequest::new("test-session", tid.clone());
                let wait_resp = tokio::time::timeout(
                    Duration::from_secs(2),
                    client.wait_for_terminal_exit(wait_req),
                )
                .await
                .expect("second wait should not hang after cancelled waiter")
                .unwrap();

                assert_ne!(wait_resp.exit_status.exit_code, Some(0));
                assert!(terminals.borrow().contains_key(tid.0.as_ref()));
            })
            .await;
    }

    #[tokio::test]
    async fn test_second_wait_takes_restored_child_after_first_wait_cancelled() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();
                let client = Rc::new(client);

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec!["-c".into(), "sleep 0.4; exit 7".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let first_wait_client = Rc::clone(&client);
                let first_wait_tid = tid.clone();
                let first_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", first_wait_tid);
                    first_wait_client.wait_for_terminal_exit(wait_req).await
                });

                tokio::time::sleep(Duration::from_millis(50)).await;

                let second_wait_client = Rc::clone(&client);
                let second_wait_tid = tid.clone();
                let second_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", second_wait_tid);
                    second_wait_client.wait_for_terminal_exit(wait_req).await
                });

                tokio::time::sleep(Duration::from_millis(50)).await;
                first_waiter.abort();
                assert!(first_waiter.await.unwrap_err().is_cancelled());

                let second_wait = tokio::time::timeout(Duration::from_secs(3), second_waiter)
                    .await
                    .expect("second waiter should take restored child")
                    .unwrap()
                    .unwrap();

                assert_eq!(second_wait.exit_status.exit_code, Some(7));
            })
            .await;
    }

    #[tokio::test]
    async fn test_cancelled_cleanup_can_be_resumed_by_next_waiter() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec!["-c".into(), "sleep 60 & echo ready; exit 7".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();
                let terminal_key = tid.0.to_string();

                let first_wait_client = Rc::clone(&client);
                let first_wait_tid = tid.clone();
                let first_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", first_wait_tid);
                    first_wait_client.wait_for_terminal_exit(wait_req).await
                });

                for _ in 0..100 {
                    let cleanup_started = terminals
                        .borrow()
                        .get(terminal_key.as_str())
                        .map(|state| *state.cleanup_in_progress.borrow())
                        .unwrap_or(false);
                    if cleanup_started {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                assert!(
                    terminals
                        .borrow()
                        .get(terminal_key.as_str())
                        .map(|state| *state.cleanup_in_progress.borrow())
                        .unwrap_or(false),
                    "first waiter should have entered cleanup"
                );

                first_waiter.abort();
                assert!(first_waiter.await.unwrap_err().is_cancelled());

                let second_wait_req = WaitForTerminalExitRequest::new("test-session", tid.clone());
                let second_wait = tokio::time::timeout(
                    Duration::from_secs(2),
                    client.wait_for_terminal_exit(second_wait_req),
                )
                .await
                .expect("second wait should resume cancelled cleanup")
                .unwrap();

                assert_eq!(second_wait.exit_status.exit_code, Some(7));
                assert_eq!(
                    terminals
                        .borrow()
                        .get(terminal_key.as_str())
                        .and_then(|state| state.pid),
                    None
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_second_waiter_waits_for_output_drain_before_returning() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);
                let terminal_id = TerminalId::new("term-drain");
                let output = Rc::new(RefCell::new(Vec::new()));
                let output_truncated = Rc::new(RefCell::new(false));
                let exit_status = Rc::new(RefCell::new(None));
                let pending_exit_status = Rc::new(RefCell::new(Some(
                    TerminalExitStatus::new().exit_code(Some(0)),
                )));
                let cleanup_in_progress = Rc::new(RefCell::new(false));
                let release_requested = Rc::new(RefCell::new(false));
                let reader_output = Rc::clone(&output);
                let reader_truncated = Rc::clone(&output_truncated);
                let reader = tokio::task::spawn_local(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    append_terminal_output(&reader_output, &reader_truncated, b"drained", None);
                });

                terminals.borrow_mut().insert(
                    terminal_id.0.to_string(),
                    TerminalState {
                        child: None,
                        pid: None,
                        output,
                        output_truncated,
                        output_byte_limit: None,
                        exit_status,
                        pending_exit_status,
                        cleanup_in_progress,
                        release_requested,
                        reader: Some(reader),
                    },
                );

                let first_wait_client = Rc::clone(&client);
                let first_wait_tid = terminal_id.clone();
                let first_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", first_wait_tid);
                    first_wait_client.wait_for_terminal_exit(wait_req).await
                });

                tokio::time::sleep(Duration::from_millis(10)).await;

                let second_wait_client = Rc::clone(&client);
                let second_wait_tid = terminal_id.clone();
                let second_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", second_wait_tid);
                    second_wait_client.wait_for_terminal_exit(wait_req).await
                });

                let first_wait = first_waiter.await.unwrap().unwrap();
                let second_wait = second_waiter.await.unwrap().unwrap();
                assert_eq!(first_wait.exit_status.exit_code, Some(0));
                assert_eq!(second_wait.exit_status.exit_code, Some(0));

                let out_req = TerminalOutputRequest::new("test-session", terminal_id);
                let out_resp = client.terminal_output(out_req).await.unwrap();
                assert_eq!(out_resp.output, "drained");
            })
            .await;
    }

    #[tokio::test]
    async fn test_kill_during_wait_cleanup_does_not_publish_before_drain() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);
                let terminal_id = TerminalId::new("term-kill-during-cleanup");
                let output = Rc::new(RefCell::new(Vec::new()));
                let output_truncated = Rc::new(RefCell::new(false));
                let exit_status = Rc::new(RefCell::new(None));
                let pending_exit_status = Rc::new(RefCell::new(Some(
                    TerminalExitStatus::new().exit_code(Some(0)),
                )));
                let cleanup_in_progress = Rc::new(RefCell::new(false));
                let release_requested = Rc::new(RefCell::new(false));
                let reader_output = Rc::clone(&output);
                let reader_truncated = Rc::clone(&output_truncated);
                let reader = tokio::task::spawn_local(async move {
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    append_terminal_output(&reader_output, &reader_truncated, b"drained", None);
                });

                terminals.borrow_mut().insert(
                    terminal_id.0.to_string(),
                    TerminalState {
                        child: None,
                        pid: None,
                        output,
                        output_truncated,
                        output_byte_limit: None,
                        exit_status,
                        pending_exit_status,
                        cleanup_in_progress,
                        release_requested,
                        reader: Some(reader),
                    },
                );

                let wait_client = Rc::clone(&client);
                let wait_tid = terminal_id.clone();
                let waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", wait_tid);
                    wait_client.wait_for_terminal_exit(wait_req).await
                });

                for _ in 0..100 {
                    let cleanup_started = terminals
                        .borrow()
                        .get(terminal_id.0.as_ref())
                        .map(|state| *state.cleanup_in_progress.borrow())
                        .unwrap_or(false);
                    if cleanup_started {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                assert!(
                    terminals
                        .borrow()
                        .get(terminal_id.0.as_ref())
                        .map(|state| *state.cleanup_in_progress.borrow())
                        .unwrap_or(false),
                    "waiter should own cleanup before kill"
                );

                let kill_req = KillTerminalCommandRequest::new("test-session", terminal_id.clone());
                client.kill_terminal_command(kill_req).await.unwrap();

                let out_req = TerminalOutputRequest::new("test-session", terminal_id.clone());
                let out_resp = client.terminal_output(out_req).await.unwrap();
                assert!(
                    out_resp.exit_status.is_none(),
                    "kill should not publish exit status while waiter is still draining"
                );

                let wait_resp = waiter.await.unwrap().unwrap();
                assert_eq!(wait_resp.exit_status.exit_code, Some(0));

                let out_req = TerminalOutputRequest::new("test-session", terminal_id);
                let out_resp = client.terminal_output(out_req).await.unwrap();
                assert_eq!(out_resp.output, "drained");
                assert!(out_resp.exit_status.is_some());
            })
            .await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_cancelled_release_restores_terminal_for_cleanup() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec!["-c".into(), "trap '' TERM; sleep 60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let release_client = Rc::clone(&client);
                let release_tid = tid.clone();
                let releaser = tokio::task::spawn_local(async move {
                    let release_req = ReleaseTerminalRequest::new("test-session", release_tid);
                    release_client.release_terminal(release_req).await
                });

                tokio::time::sleep(Duration::from_millis(10)).await;
                releaser.abort();
                assert!(releaser.await.unwrap_err().is_cancelled());

                {
                    let borrowed = terminals.borrow();
                    let state = borrowed
                        .get(tid.0.as_ref())
                        .expect("cancelled release should keep terminal state");
                    assert!(state.child.is_some(), "child should be restored");
                    assert!(state.reader.is_some(), "reader should be restored");
                    assert!(
                        !*state.cleanup_in_progress.borrow(),
                        "cleanup ownership should be released"
                    );
                    assert!(
                        state.pid.is_some(),
                        "pid should remain available for cleanup"
                    );
                }

                let release_req = ReleaseTerminalRequest::new("test-session", tid.clone());
                tokio::time::timeout(Duration::from_secs(2), client.release_terminal(release_req))
                    .await
                    .expect("second release should complete cleanup")
                    .unwrap();

                assert!(
                    !terminals.borrow().contains_key(tid.0.as_ref()),
                    "successful release should remove terminal state"
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_release_waits_for_active_wait_owner_cleanup() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);

                let req =
                    CreateTerminalRequest::new("test-session", "sleep").args(vec!["60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let wait_client = Rc::clone(&client);
                let wait_tid = tid.clone();
                let waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", wait_tid);
                    wait_client.wait_for_terminal_exit(wait_req).await
                });

                for _ in 0..100 {
                    let wait_owns_cleanup = terminals
                        .borrow()
                        .get(tid.0.as_ref())
                        .map(|state| {
                            *state.cleanup_in_progress.borrow()
                                && state.child.is_none()
                                && state.reader.is_none()
                        })
                        .unwrap_or(false);
                    if wait_owns_cleanup {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                assert!(
                    terminals
                        .borrow()
                        .get(tid.0.as_ref())
                        .map(|state| {
                            *state.cleanup_in_progress.borrow()
                                && state.child.is_none()
                                && state.reader.is_none()
                        })
                        .unwrap_or(false),
                    "waiter should own child/reader before release"
                );

                let release_req = ReleaseTerminalRequest::new("test-session", tid.clone());
                tokio::time::timeout(Duration::from_secs(2), client.release_terminal(release_req))
                    .await
                    .expect("release should wait for active waiter cleanup")
                    .unwrap();

                let wait_resp = tokio::time::timeout(Duration::from_secs(2), waiter)
                    .await
                    .expect("waiter should complete after release signals")
                    .unwrap()
                    .unwrap();
                assert_ne!(wait_resp.exit_status.exit_code, Some(0));
                assert!(
                    !terminals.borrow().contains_key(tid.0.as_ref()),
                    "release should return only after terminal is removed"
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_waiters_return_status_after_release_removes_terminal() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);
                let terminal_id = TerminalId::new("term-waiters-release-race");
                let output = Rc::new(RefCell::new(Vec::new()));
                let output_truncated = Rc::new(RefCell::new(false));
                let exit_status = Rc::new(RefCell::new(None));
                let pending_exit_status = Rc::new(RefCell::new(Some(
                    TerminalExitStatus::new().exit_code(Some(7)),
                )));
                let cleanup_in_progress = Rc::new(RefCell::new(false));
                let release_requested = Rc::new(RefCell::new(false));
                let reader_output = Rc::clone(&output);
                let reader_truncated = Rc::clone(&output_truncated);
                let reader = tokio::task::spawn_local(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    append_terminal_output(&reader_output, &reader_truncated, b"drained", None);
                });

                terminals.borrow_mut().insert(
                    terminal_id.0.to_string(),
                    TerminalState {
                        child: None,
                        pid: None,
                        output,
                        output_truncated,
                        output_byte_limit: None,
                        exit_status,
                        pending_exit_status,
                        cleanup_in_progress,
                        release_requested,
                        reader: Some(reader),
                    },
                );

                let owner_client = Rc::clone(&client);
                let owner_tid = terminal_id.clone();
                let owner = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", owner_tid);
                    owner_client.wait_for_terminal_exit(wait_req).await
                });

                for _ in 0..100 {
                    let cleanup_started = terminals
                        .borrow()
                        .get(terminal_id.0.as_ref())
                        .map(|state| *state.cleanup_in_progress.borrow())
                        .unwrap_or(false);
                    if cleanup_started {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                assert!(
                    terminals
                        .borrow()
                        .get(terminal_id.0.as_ref())
                        .map(|state| *state.cleanup_in_progress.borrow())
                        .unwrap_or(false),
                    "owner should be draining before secondary waiters start"
                );

                let second_client = Rc::clone(&client);
                let second_tid = terminal_id.clone();
                let second_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", second_tid);
                    second_client.wait_for_terminal_exit(wait_req).await
                });
                let third_client = Rc::clone(&client);
                let third_tid = terminal_id.clone();
                let third_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", third_tid);
                    third_client.wait_for_terminal_exit(wait_req).await
                });

                tokio::time::sleep(Duration::from_millis(10)).await;

                let release_client = Rc::clone(&client);
                let release_tid = terminal_id.clone();
                let releaser = tokio::task::spawn_local(async move {
                    let release_req = ReleaseTerminalRequest::new("test-session", release_tid);
                    release_client.release_terminal(release_req).await
                });

                let owner_resp = owner.await.unwrap().unwrap();
                assert_eq!(owner_resp.exit_status.exit_code, Some(7));
                releaser.await.unwrap().unwrap();
                assert!(
                    !terminals.borrow().contains_key(terminal_id.0.as_ref()),
                    "release should remove terminal after owner cleanup"
                );

                let second_resp = tokio::time::timeout(Duration::from_secs(2), second_waiter)
                    .await
                    .expect("second waiter should return held exit status")
                    .unwrap()
                    .unwrap();
                let third_resp = tokio::time::timeout(Duration::from_secs(2), third_waiter)
                    .await
                    .expect("third waiter should return held exit status")
                    .unwrap()
                    .unwrap();

                assert_eq!(second_resp.exit_status.exit_code, Some(7));
                assert_eq!(third_resp.exit_status.exit_code, Some(7));
            })
            .await;
    }

    #[tokio::test]
    async fn test_waiter_returns_status_after_release_owned_cleanup_removes_terminal() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();
                let client = Rc::new(client);

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec!["-c".into(), "trap '' TERM; sleep 0.2; exit 7".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let release_client = Rc::clone(&client);
                let release_tid = tid.clone();
                let releaser = tokio::task::spawn_local(async move {
                    let release_req = ReleaseTerminalRequest::new("test-session", release_tid);
                    release_client.release_terminal(release_req).await
                });

                for _ in 0..100 {
                    let release_owns_cleanup = terminals
                        .borrow()
                        .get(tid.0.as_ref())
                        .map(|state| {
                            *state.cleanup_in_progress.borrow()
                                && state.child.is_none()
                                && state.reader.is_none()
                        })
                        .unwrap_or(false);
                    if release_owns_cleanup {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                assert!(
                    terminals
                        .borrow()
                        .get(tid.0.as_ref())
                        .map(|state| {
                            *state.cleanup_in_progress.borrow()
                                && state.child.is_none()
                                && state.reader.is_none()
                        })
                        .unwrap_or(false),
                    "release should own child/reader before waiter starts"
                );

                let wait_client = Rc::clone(&client);
                let wait_tid = tid.clone();
                let waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", wait_tid);
                    wait_client.wait_for_terminal_exit(wait_req).await
                });

                releaser.await.unwrap().unwrap();
                assert!(
                    !terminals.borrow().contains_key(tid.0.as_ref()),
                    "release should remove terminal after publishing status"
                );

                let wait_resp = tokio::time::timeout(Duration::from_secs(2), waiter)
                    .await
                    .expect("waiter should return release-owned cleanup status")
                    .unwrap()
                    .unwrap();
                assert!(
                    wait_resp.exit_status.exit_code.is_some()
                        || wait_resp.exit_status.signal.is_some(),
                    "waiter should receive release-owned completion status: {:?}",
                    wait_resp.exit_status
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_terminal_pid_is_cleared_after_wait_and_kill_cleanup() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, terminals) = test_client();

                let wait_req = CreateTerminalRequest::new("test-session", "true");
                let wait_resp = client.create_terminal(wait_req).await.unwrap();
                let wait_tid = wait_resp.terminal_id.clone();
                let exit_req = WaitForTerminalExitRequest::new("test-session", wait_tid.clone());
                client.wait_for_terminal_exit(exit_req).await.unwrap();

                assert_eq!(
                    terminals
                        .borrow()
                        .get(wait_tid.0.as_ref())
                        .and_then(|state| state.pid),
                    None
                );

                let kill_req =
                    CreateTerminalRequest::new("test-session", "sleep").args(vec!["60".into()]);
                let kill_resp = client.create_terminal(kill_req).await.unwrap();
                let kill_tid = kill_resp.terminal_id.clone();
                let command_req = KillTerminalCommandRequest::new("test-session", kill_tid.clone());
                client.kill_terminal_command(command_req).await.unwrap();

                assert_eq!(
                    terminals
                        .borrow()
                        .get(kill_tid.0.as_ref())
                        .and_then(|state| state.pid),
                    None
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_terminal_output_remains_available_while_kill_is_in_progress() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();
                let client = Rc::new(client);

                let req = CreateTerminalRequest::new("test-session", "sh")
                    .args(vec!["-c".into(), "echo ready; sleep 60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                tokio::time::sleep(Duration::from_millis(50)).await;

                let kill_client = Rc::clone(&client);
                let kill_tid = tid.clone();
                let killer = tokio::task::spawn_local(async move {
                    let kill_req = KillTerminalCommandRequest::new("test-session", kill_tid);
                    kill_client.kill_terminal_command(kill_req).await
                });

                tokio::time::sleep(Duration::from_millis(25)).await;

                let out_req = TerminalOutputRequest::new("test-session", tid.clone());
                let out_resp = client.terminal_output(out_req).await.unwrap();
                assert!(
                    out_resp.output.contains("ready"),
                    "expected terminal to remain addressable during kill: {:?}",
                    out_resp.output
                );

                tokio::time::timeout(Duration::from_secs(2), killer)
                    .await
                    .expect("kill should complete")
                    .unwrap()
                    .unwrap();
            })
            .await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_second_wait_during_kill_observes_real_signal_status() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (client, _rx, _terminals) = test_client();
                let client = Rc::new(client);

                let req =
                    CreateTerminalRequest::new("test-session", "sleep").args(vec!["60".into()]);
                let resp = client.create_terminal(req).await.unwrap();
                let tid = resp.terminal_id.clone();

                let first_wait_client = Rc::clone(&client);
                let first_wait_tid = tid.clone();
                let first_waiter = tokio::task::spawn_local(async move {
                    let wait_req = WaitForTerminalExitRequest::new("test-session", first_wait_tid);
                    first_wait_client.wait_for_terminal_exit(wait_req).await
                });

                tokio::time::sleep(Duration::from_millis(50)).await;

                let kill_req = KillTerminalCommandRequest::new("test-session", tid.clone());
                client.kill_terminal_command(kill_req).await.unwrap();

                let second_wait_req = WaitForTerminalExitRequest::new("test-session", tid);
                let second_wait = tokio::time::timeout(
                    Duration::from_secs(2),
                    client.wait_for_terminal_exit(second_wait_req),
                )
                .await
                .expect("second wait should not hang")
                .unwrap();
                assert!(
                    second_wait.exit_status.signal.is_some(),
                    "second wait should observe a real signal status, got {:?}",
                    second_wait.exit_status
                );

                let first_wait = tokio::time::timeout(Duration::from_secs(2), first_waiter)
                    .await
                    .expect("first wait should complete")
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    second_wait.exit_status.signal,
                    first_wait.exit_status.signal
                );
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
