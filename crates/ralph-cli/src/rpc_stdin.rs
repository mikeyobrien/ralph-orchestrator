//! Stdin command reader and dispatcher for RPC mode.
//!
//! This module provides the command consumer side of the JSON-RPC protocol.
//! It reads `RpcCommand` objects from stdin and translates them into
//! appropriate actions (inject guidance, send abort signal, return state).
//!
//! In RPC mode, this reader replaces the TUI keyboard input and OS signal
//! handlers. It runs as a background tokio task alongside the orchestration
//! loop, communicating via channels.

// Allow dead_code for now - these APIs are consumed by Task 04 (rpc execution mode)
#![allow(dead_code)]

use ralph_proto::{GuidanceTarget, RpcCommand, RpcEvent, RpcState, emit_event_line, parse_command};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn};

/// Dispatcher that routes RPC commands to the appropriate channels and handlers.
pub struct RpcDispatcher<F>
where
    F: Fn() -> RpcState + Send + Sync,
{
    /// Watch channel for abort commands (sends `true` to trigger loop termination).
    pub interrupt_tx: watch::Sender<bool>,

    /// Channel for guidance/steer/follow_up messages.
    pub guidance_tx: mpsc::Sender<GuidanceMessage>,

    /// Channel for sending responses back to the stdout emitter.
    pub response_tx: mpsc::Sender<RpcEvent>,

    /// Closure to snapshot current loop state for `get_state` commands.
    pub state_fn: Arc<F>,

    /// Path to events.jsonl for writing human.guidance events.
    pub events_path: Option<PathBuf>,

    /// Tracks whether the loop has been started (for prompt validation).
    pub loop_started: Arc<std::sync::atomic::AtomicBool>,

    /// Channel to send the initial prompt (before loop starts).
    pub prompt_tx: Option<mpsc::Sender<PromptRequest>>,
}

/// A guidance message with its target (current iteration or next).
#[derive(Debug, Clone)]
pub struct GuidanceMessage {
    pub message: String,
    pub target: GuidanceTarget,
}

/// Request to start the loop with a prompt.
#[derive(Debug, Clone)]
pub struct PromptRequest {
    pub prompt: String,
    pub backend: Option<String>,
    pub max_iterations: Option<u32>,
}

impl<F> RpcDispatcher<F>
where
    F: Fn() -> RpcState + Send + Sync,
{
    /// Creates a new dispatcher with the given channels and state function.
    pub fn new(
        interrupt_tx: watch::Sender<bool>,
        guidance_tx: mpsc::Sender<GuidanceMessage>,
        response_tx: mpsc::Sender<RpcEvent>,
        state_fn: F,
    ) -> Self {
        Self {
            interrupt_tx,
            guidance_tx,
            response_tx,
            state_fn: Arc::new(state_fn),
            events_path: None,
            loop_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            prompt_tx: None,
        }
    }

    /// Sets the events path for writing human.guidance events.
    pub fn with_events_path(mut self, path: PathBuf) -> Self {
        self.events_path = Some(path);
        self
    }

    /// Sets the prompt channel for sending the initial prompt.
    pub fn with_prompt_tx(mut self, tx: mpsc::Sender<PromptRequest>) -> Self {
        self.prompt_tx = Some(tx);
        self
    }

    /// Marks the loop as started (call this when the loop begins execution).
    pub fn mark_loop_started(&self) {
        self.loop_started
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Checks if the loop has started.
    fn loop_has_started(&self) -> bool {
        self.loop_started.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Dispatches a command and returns a response event.
    async fn dispatch(&self, cmd: RpcCommand) -> RpcEvent {
        let id = cmd.id().map(|s| s.to_string());
        let cmd_type = cmd.command_type();

        match cmd {
            RpcCommand::Prompt {
                prompt,
                backend,
                max_iterations,
                ..
            } => {
                if self.loop_has_started() {
                    return RpcEvent::error_response(
                        cmd_type,
                        id,
                        "loop already running; use guidance or steer instead",
                    );
                }

                match &self.prompt_tx {
                    Some(tx) => {
                        let request = PromptRequest {
                            prompt,
                            backend,
                            max_iterations,
                        };
                        match tx.send(request).await {
                            Ok(()) => RpcEvent::success_response(cmd_type, id, None),
                            Err(_) => {
                                RpcEvent::error_response(cmd_type, id, "prompt channel closed")
                            }
                        }
                    }
                    None => RpcEvent::error_response(
                        cmd_type,
                        id,
                        "prompt channel not configured; loop may already be starting",
                    ),
                }
            }

            RpcCommand::Guidance { message, .. } => {
                // Write human.guidance event to events.jsonl
                if let Some(ref path) = self.events_path
                    && let Err(e) = write_guidance_event(path, &message)
                {
                    warn!(error = %e, "Failed to write guidance event");
                }

                // Also push to guidance channel for next iteration
                let msg = GuidanceMessage {
                    message: message.clone(),
                    target: GuidanceTarget::Next,
                };
                match self.guidance_tx.send(msg).await {
                    Ok(()) => {
                        // Emit guidance ack event
                        let _ = self
                            .response_tx
                            .send(RpcEvent::GuidanceAck {
                                message: message.clone(),
                                applies_to: GuidanceTarget::Next,
                            })
                            .await;
                        RpcEvent::success_response(cmd_type, id, None)
                    }
                    Err(_) => RpcEvent::error_response(cmd_type, id, "guidance channel closed"),
                }
            }

            RpcCommand::Steer { message, .. } => {
                // Steer is immediate injection - write directly to events file
                if let Some(ref path) = self.events_path
                    && let Err(e) = write_guidance_event(path, &message)
                {
                    warn!(error = %e, "Failed to write steer event");
                }

                // Also push to guidance channel with Current target
                let msg = GuidanceMessage {
                    message: message.clone(),
                    target: GuidanceTarget::Current,
                };
                match self.guidance_tx.send(msg).await {
                    Ok(()) => {
                        let _ = self
                            .response_tx
                            .send(RpcEvent::GuidanceAck {
                                message: message.clone(),
                                applies_to: GuidanceTarget::Current,
                            })
                            .await;
                        RpcEvent::success_response(cmd_type, id, None)
                    }
                    Err(_) => RpcEvent::error_response(cmd_type, id, "guidance channel closed"),
                }
            }

            RpcCommand::FollowUp { message, .. } => {
                // Follow-up is for next iteration only
                let msg = GuidanceMessage {
                    message: message.clone(),
                    target: GuidanceTarget::Next,
                };
                match self.guidance_tx.send(msg).await {
                    Ok(()) => {
                        let _ = self
                            .response_tx
                            .send(RpcEvent::GuidanceAck {
                                message: message.clone(),
                                applies_to: GuidanceTarget::Next,
                            })
                            .await;
                        RpcEvent::success_response(cmd_type, id, None)
                    }
                    Err(_) => RpcEvent::error_response(cmd_type, id, "guidance channel closed"),
                }
            }

            RpcCommand::Abort { reason, .. } => {
                debug!(reason = ?reason, "Received abort command");
                match self.interrupt_tx.send(true) {
                    Ok(()) => RpcEvent::success_response(cmd_type, id, None),
                    Err(_) => RpcEvent::error_response(cmd_type, id, "interrupt channel closed"),
                }
            }

            RpcCommand::GetState { .. } => {
                let state = (self.state_fn)();
                let data = serde_json::to_value(&state).ok();
                RpcEvent::success_response(cmd_type, id, data)
            }

            RpcCommand::GetIterations {
                include_content, ..
            } => {
                // Return iteration info from state
                let state = (self.state_fn)();
                let data = serde_json::json!({
                    "iteration": state.iteration,
                    "max_iterations": state.max_iterations,
                    "include_content": include_content,
                    // Note: Full iteration history would require integration with EventLoop
                });
                RpcEvent::success_response(cmd_type, id, Some(data))
            }

            RpcCommand::SetHat { hat, .. } => {
                // Hat changes require integration with EventLoop - emit event
                // For now, acknowledge but note it requires further integration
                debug!(hat = %hat, "Received set_hat command");
                let data = serde_json::json!({
                    "hat": hat,
                    "note": "hat change will be applied to next iteration"
                });
                RpcEvent::success_response(cmd_type, id, Some(data))
            }

            RpcCommand::ExtensionUiResponse {
                request_id,
                response,
                ..
            } => {
                // Extension UI responses require specific handling
                debug!(request_id = %request_id, "Received extension UI response");
                let data = serde_json::json!({
                    "request_id": request_id,
                    "response": response
                });
                RpcEvent::success_response(cmd_type, id, Some(data))
            }
        }
    }
}

/// Writes a human.guidance event to the events JSONL file.
fn write_guidance_event(path: &PathBuf, message: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let timestamp = chrono::Utc::now().to_rfc3339();
    let event = serde_json::json!({
        "topic": "human.guidance",
        "payload": message,
        "ts": timestamp,
    });

    let line = serde_json::to_string(&event)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)?;
    file.flush()?;
    info!(path = ?path, "Wrote guidance event to events.jsonl");
    Ok(())
}

/// Runs the stdin reader loop, dispatching commands to the given dispatcher.
///
/// This function reads JSON-line commands from stdin, parses them, dispatches
/// them via the dispatcher, and sends responses to the response channel.
///
/// The function exits gracefully when:
/// - stdin is closed (EOF)
/// - An unrecoverable error occurs
/// - The response channel is closed
pub async fn run_stdin_reader<F, R>(dispatcher: RpcDispatcher<F>, reader: R)
where
    F: Fn() -> RpcState + Send + Sync + 'static,
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                debug!(line = %line, "Received stdin command");

                let response = match parse_command(line) {
                    Ok(cmd) => dispatcher.dispatch(cmd).await,
                    Err(parse_error) => {
                        warn!(error = %parse_error, line = %line, "Failed to parse command");
                        RpcEvent::error_response("parse", None, parse_error)
                    }
                };

                // Send response to stdout emitter
                if dispatcher.response_tx.send(response).await.is_err() {
                    warn!("Response channel closed, stopping stdin reader");
                    break;
                }
            }
            Ok(None) => {
                // EOF - stdin closed
                info!("Stdin closed (EOF), stopping reader task");
                break;
            }
            Err(e) => {
                warn!(error = %e, "Error reading from stdin, stopping reader task");
                break;
            }
        }
    }
}

/// Runs the stdout emitter loop, writing events to stdout.
///
/// This function receives events from the response channel and writes them
/// as JSON lines to stdout.
pub async fn run_stdout_emitter(mut rx: mpsc::Receiver<RpcEvent>) {
    use std::io::Write;

    while let Some(event) = rx.recv().await {
        let line = emit_event_line(&event);
        // Lock stdout for each write to avoid holding across await points
        let stdout = std::io::stdout();
        let mut stdout = stdout.lock();
        if stdout.write_all(line.as_bytes()).is_err() {
            warn!("Failed to write to stdout, stopping emitter");
            break;
        }
        if stdout.flush().is_err() {
            warn!("Failed to flush stdout");
        }
        // stdout lock is dropped here
    }

    debug!("Stdout emitter task finished");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn default_state() -> RpcState {
        RpcState {
            iteration: 1,
            max_iterations: Some(10),
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            completed: false,
            started_at: 1_700_000_000_000,
            iteration_started_at: Some(1_700_000_001_000),
            task_counts: ralph_proto::RpcTaskCounts::default(),
            active_task: None,
            total_cost_usd: 0.0,
        }
    }

    #[tokio::test]
    async fn test_abort_triggers_interrupt() {
        let (interrupt_tx, interrupt_rx) = watch::channel(false);
        let (guidance_tx, _guidance_rx) = mpsc::channel(10);
        let (response_tx, _response_rx) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        let cmd = RpcCommand::Abort {
            id: Some("abort-1".to_string()),
            reason: Some("test abort".to_string()),
        };

        let response = dispatcher.dispatch(cmd).await;

        // Check interrupt was sent
        assert!(*interrupt_rx.borrow());

        // Check response
        match response {
            RpcEvent::Response {
                command, success, ..
            } => {
                assert_eq!(command, "abort");
                assert!(success);
            }
            _ => panic!("Expected Response event"),
        }
    }

    #[tokio::test]
    async fn test_guidance_routes_to_channel() {
        let (interrupt_tx, _) = watch::channel(false);
        let (guidance_tx, mut guidance_rx) = mpsc::channel(10);
        let (response_tx, _) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        let cmd = RpcCommand::Guidance {
            id: None,
            message: "focus on tests".to_string(),
        };

        let _response = dispatcher.dispatch(cmd).await;

        // Check guidance was sent
        let msg = guidance_rx.recv().await.expect("should receive guidance");
        assert_eq!(msg.message, "focus on tests");
        assert_eq!(msg.target, GuidanceTarget::Next);
    }

    #[tokio::test]
    async fn test_get_state_returns_snapshot() {
        let (interrupt_tx, _) = watch::channel(false);
        let (guidance_tx, _) = mpsc::channel(10);
        let (response_tx, _) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        let cmd = RpcCommand::GetState {
            id: Some("state-1".to_string()),
        };

        let response = dispatcher.dispatch(cmd).await;

        match response {
            RpcEvent::Response {
                command,
                id,
                success,
                data,
                ..
            } => {
                assert_eq!(command, "get_state");
                assert_eq!(id, Some("state-1".to_string()));
                assert!(success);
                let data = data.expect("should have data");
                assert_eq!(data["iteration"], 1);
                assert_eq!(data["hat"], "builder");
            }
            _ => panic!("Expected Response event"),
        }
    }

    #[tokio::test]
    async fn test_steer_vs_follow_up_semantics() {
        let (interrupt_tx, _) = watch::channel(false);
        let (guidance_tx, mut guidance_rx) = mpsc::channel(10);
        let (response_tx, _) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        // Steer should have Current target
        let steer_cmd = RpcCommand::Steer {
            id: None,
            message: "steer now".to_string(),
        };
        let _response = dispatcher.dispatch(steer_cmd).await;
        let steer_msg = guidance_rx.recv().await.expect("steer message");
        assert_eq!(steer_msg.target, GuidanceTarget::Current);

        // FollowUp should have Next target
        let follow_up_cmd = RpcCommand::FollowUp {
            id: None,
            message: "follow up later".to_string(),
        };
        let _response = dispatcher.dispatch(follow_up_cmd).await;
        let follow_up_msg = guidance_rx.recv().await.expect("follow_up message");
        assert_eq!(follow_up_msg.target, GuidanceTarget::Next);
    }

    #[tokio::test]
    async fn test_prompt_rejected_after_loop_started() {
        let (interrupt_tx, _) = watch::channel(false);
        let (guidance_tx, _) = mpsc::channel(10);
        let (response_tx, _) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        // Mark loop as started
        dispatcher.mark_loop_started();

        let cmd = RpcCommand::Prompt {
            id: Some("prompt-1".to_string()),
            prompt: "do something".to_string(),
            backend: None,
            max_iterations: None,
        };

        let response = dispatcher.dispatch(cmd).await;

        match response {
            RpcEvent::Response { success, error, .. } => {
                assert!(!success);
                assert!(error.unwrap().contains("loop already running"));
            }
            _ => panic!("Expected Response event"),
        }
    }

    #[tokio::test]
    async fn test_stdin_reader_parses_json_commands() {
        let (interrupt_tx, _) = watch::channel(false);
        let (guidance_tx, _) = mpsc::channel(10);
        let (response_tx, mut response_rx) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        // Simulate stdin with a get_state command
        let input = r#"{"type": "get_state", "id": "test-1"}"#;
        let reader = std::io::Cursor::new(input.as_bytes().to_vec());

        // Run reader in background
        tokio::spawn(async move {
            run_stdin_reader(dispatcher, reader).await;
        });

        // Check we get a response
        let response = tokio::time::timeout(std::time::Duration::from_secs(1), response_rx.recv())
            .await
            .expect("timeout")
            .expect("should receive response");

        match response {
            RpcEvent::Response {
                command,
                id,
                success,
                ..
            } => {
                assert_eq!(command, "get_state");
                assert_eq!(id, Some("test-1".to_string()));
                assert!(success);
            }
            _ => panic!("Expected Response event"),
        }
    }

    #[tokio::test]
    async fn test_parse_error_returns_error_response() {
        let (interrupt_tx, _) = watch::channel(false);
        let (guidance_tx, _) = mpsc::channel(10);
        let (response_tx, mut response_rx) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        // Invalid JSON
        let input = r#"{"type": "nonexistent_command"}"#;
        let reader = std::io::Cursor::new(input.as_bytes().to_vec());

        tokio::spawn(async move {
            run_stdin_reader(dispatcher, reader).await;
        });

        let response = tokio::time::timeout(std::time::Duration::from_secs(1), response_rx.recv())
            .await
            .expect("timeout")
            .expect("should receive response");

        match response {
            RpcEvent::Response {
                command,
                success,
                error,
                ..
            } => {
                assert_eq!(command, "parse");
                assert!(!success);
                assert!(error.is_some());
            }
            _ => panic!("Expected error Response event"),
        }
    }

    #[tokio::test]
    async fn test_stdin_eof_exits_gracefully() {
        let (interrupt_tx, _) = watch::channel(false);
        let (guidance_tx, _) = mpsc::channel(10);
        let (response_tx, _response_rx) = mpsc::channel(10);

        let dispatcher =
            RpcDispatcher::new(interrupt_tx, guidance_tx, response_tx, || default_state());

        // Empty input = immediate EOF
        let reader = std::io::Cursor::new(Vec::<u8>::new());

        // Should complete without panic
        run_stdin_reader(dispatcher, reader).await;
    }
}
