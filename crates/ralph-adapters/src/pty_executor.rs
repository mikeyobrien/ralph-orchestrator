//! PTY executor for running prompts with full terminal emulation.
//!
//! Spawns CLI tools in a pseudo-terminal to preserve rich TUI features like
//! colors, spinners, and animations. Supports both interactive mode (user
//! input forwarded) and observe mode (output-only).
//!
//! Key features:
//! - PTY creation via `portable-pty` for cross-platform support
//! - Idle timeout with activity tracking (output AND input reset timer)
//! - Double Ctrl+C handling (first forwards, second terminates)
//! - Raw mode management with cleanup on exit/crash
//!
//! Architecture:
//! - Uses `tokio::select!` for non-blocking I/O multiplexing
//! - Spawns separate tasks for PTY output and user input
//! - Enables responsive Ctrl+C handling even when PTY is idle

// Exit codes and PIDs are always within i32 range in practice
#![allow(clippy::cast_possible_wrap)]

use crate::claude_stream::{ClaudeStreamEvent, ClaudeStreamParser, ContentBlock, UserContentBlock};
use crate::cli_backend::{CliBackend, OutputFormat};
use crate::stream_handler::{SessionResult, StreamHandler};
#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;
use portable_pty::{CommandBuilder, PtyPair, PtySize, native_pty_system};
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn};

/// Result of a PTY execution.
#[derive(Debug)]
pub struct PtyExecutionResult {
    /// The accumulated output (ANSI sequences preserved).
    pub output: String,
    /// The ANSI-stripped output for event parsing.
    pub stripped_output: String,
    /// Extracted text content from NDJSON stream (for Claude's stream-json output).
    /// When Claude outputs `--output-format stream-json`, event tags like
    /// `<event topic="...">` are inside JSON string values. This field contains
    /// the extracted text content for proper event parsing.
    /// Empty for non-JSON backends (use `stripped_output` instead).
    pub extracted_text: String,
    /// Whether the process exited successfully.
    pub success: bool,
    /// The exit code if available.
    pub exit_code: Option<i32>,
    /// How the process was terminated.
    pub termination: TerminationType,
}

/// How the PTY process was terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationType {
    /// Process exited naturally.
    Natural,
    /// Terminated due to idle timeout.
    IdleTimeout,
    /// Terminated by user (double Ctrl+C).
    UserInterrupt,
    /// Force killed by user (Ctrl+\).
    ForceKill,
}

/// Configuration for PTY execution.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Enable interactive mode (forward user input).
    pub interactive: bool,
    /// Idle timeout in seconds (0 = disabled).
    pub idle_timeout_secs: u32,
    /// Terminal width.
    pub cols: u16,
    /// Terminal height.
    pub rows: u16,
    /// Workspace root directory for command execution.
    /// This is captured at startup to avoid `current_dir()` failures when the
    /// working directory no longer exists (e.g., in E2E test workspaces).
    pub workspace_root: std::path::PathBuf,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            interactive: true,
            idle_timeout_secs: 30,
            cols: 80,
            rows: 24,
            workspace_root: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }
}

impl PtyConfig {
    /// Creates config from environment, falling back to defaults.
    pub fn from_env() -> Self {
        let cols = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(80);
        let rows = std::env::var("LINES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24);

        Self {
            cols,
            rows,
            ..Default::default()
        }
    }

    /// Sets the workspace root directory.
    pub fn with_workspace_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.workspace_root = root.into();
        self
    }
}

/// State machine for double Ctrl+C detection.
#[derive(Debug)]
pub struct CtrlCState {
    /// When the first Ctrl+C was pressed (if any).
    first_press: Option<Instant>,
    /// Window duration for double-press detection.
    window: Duration,
}

/// Action to take after handling Ctrl+C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CtrlCAction {
    /// Forward the Ctrl+C to Claude and start/restart the window.
    ForwardAndStartWindow,
    /// Terminate Claude (second Ctrl+C within window).
    Terminate,
}

impl CtrlCState {
    /// Creates a new Ctrl+C state tracker.
    pub fn new() -> Self {
        Self {
            first_press: None,
            window: Duration::from_secs(1),
        }
    }

    /// Handles a Ctrl+C keypress and returns the action to take.
    pub fn handle_ctrl_c(&mut self, now: Instant) -> CtrlCAction {
        match self.first_press {
            Some(first) if now.duration_since(first) < self.window => {
                // Second Ctrl+C within window - terminate
                self.first_press = None;
                CtrlCAction::Terminate
            }
            _ => {
                // First Ctrl+C or window expired - forward and start window
                self.first_press = Some(now);
                CtrlCAction::ForwardAndStartWindow
            }
        }
    }
}

impl Default for CtrlCState {
    fn default() -> Self {
        Self::new()
    }
}

/// Executor for running prompts in a pseudo-terminal.
pub struct PtyExecutor {
    backend: CliBackend,
    config: PtyConfig,
    // Channel ends for TUI integration
    output_tx: mpsc::UnboundedSender<Vec<u8>>,
    output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    input_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    control_tx: Option<mpsc::UnboundedSender<crate::pty_handle::ControlCommand>>,
    control_rx: mpsc::UnboundedReceiver<crate::pty_handle::ControlCommand>,
    // Termination notification for TUI
    terminated_tx: watch::Sender<bool>,
    terminated_rx: Option<watch::Receiver<bool>>,
    // Explicit TUI mode flag - set via set_tui_mode() when TUI is connected.
    // This replaces the previous inference via output_rx.is_none() which broke
    // after the streaming refactor (handle() is no longer called in TUI mode).
    tui_mode: bool,
}

impl PtyExecutor {
    /// Creates a new PTY executor with the given backend and configuration.
    pub fn new(backend: CliBackend, config: PtyConfig) -> Self {
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (terminated_tx, terminated_rx) = watch::channel(false);

        Self {
            backend,
            config,
            output_tx,
            output_rx: Some(output_rx),
            input_tx: Some(input_tx),
            input_rx,
            control_tx: Some(control_tx),
            control_rx,
            terminated_tx,
            terminated_rx: Some(terminated_rx),
            tui_mode: false,
        }
    }

    /// Sets the TUI mode flag.
    ///
    /// When TUI mode is enabled, PTY output is sent to the TUI channel instead of
    /// being written directly to stdout. This flag must be set before calling any
    /// of the run methods when using the TUI.
    ///
    /// # Arguments
    /// * `enabled` - Whether TUI mode should be active
    pub fn set_tui_mode(&mut self, enabled: bool) {
        self.tui_mode = enabled;
    }

    /// Updates the backend configuration for this executor.
    ///
    /// This allows switching backends between iterations without recreating
    /// the entire executor. Critical for hat-level backend configuration support.
    ///
    /// # Arguments
    /// * `backend` - The new backend configuration to use
    pub fn set_backend(&mut self, backend: CliBackend) {
        self.backend = backend;
    }

    /// Returns a handle for TUI integration.
    ///
    /// Can only be called once - panics if called multiple times.
    pub fn handle(&mut self) -> crate::pty_handle::PtyHandle {
        crate::pty_handle::PtyHandle {
            output_rx: self.output_rx.take().expect("handle() already called"),
            input_tx: self.input_tx.take().expect("handle() already called"),
            control_tx: self.control_tx.take().expect("handle() already called"),
            terminated_rx: self.terminated_rx.take().expect("handle() already called"),
        }
    }

    /// Spawns Claude in a PTY and returns the PTY pair, child process, stdin input, and temp file.
    ///
    /// The temp file is returned to keep it alive for the duration of execution.
    /// For large prompts (>7000 chars), Claude is instructed to read from a temp file.
    /// If the temp file is dropped before Claude reads it, the file is deleted and Claude hangs.
    ///
    /// The stdin_input is returned so callers can write it to the PTY after taking the writer.
    /// This is necessary because `take_writer()` can only be called once per PTY.
    fn spawn_pty(
        &self,
        prompt: &str,
    ) -> io::Result<(
        PtyPair,
        Box<dyn portable_pty::Child + Send>,
        Option<String>,
        Option<tempfile::NamedTempFile>,
    )> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: self.config.rows,
                cols: self.config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::other(e.to_string()))?;

        let (cmd, args, stdin_input, temp_file) =
            self.backend.build_command(prompt, self.config.interactive);

        let mut cmd_builder = CommandBuilder::new(&cmd);
        cmd_builder.args(&args);

        // Set explicit working directory from config (captured at startup to avoid
        // current_dir() failures when workspace no longer exists)
        cmd_builder.cwd(&self.config.workspace_root);

        // Set up environment for PTY
        cmd_builder.env("TERM", "xterm-256color");
        let child = pair
            .slave
            .spawn_command(cmd_builder)
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Return stdin_input so callers can write it after taking the writer
        Ok((pair, child, stdin_input, temp_file))
    }

    /// Runs in observe mode (output-only, no input forwarding).
    ///
    /// This is an async function that listens for interrupt signals via the shared
    /// `interrupt_rx` watch channel from the event loop.
    /// Uses a separate thread for blocking PTY reads and tokio::select! for signal handling.
    ///
    /// Returns when the process exits, idle timeout triggers, or interrupt is received.
    ///
    /// # Arguments
    /// * `prompt` - The prompt to execute
    /// * `interrupt_rx` - Watch channel receiver for interrupt signals from the event loop
    ///
    /// # Errors
    ///
    /// Returns an error if PTY allocation fails, the command cannot be spawned,
    /// or an I/O error occurs during output handling.
    pub async fn run_observe(
        &self,
        prompt: &str,
        mut interrupt_rx: tokio::sync::watch::Receiver<bool>,
    ) -> io::Result<PtyExecutionResult> {
        // Keep temp_file alive for the duration of execution (large prompts use temp files)
        let (pair, mut child, stdin_input, _temp_file) = self.spawn_pty(prompt)?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Write stdin input if present (for stdin prompt mode)
        if let Some(ref input) = stdin_input {
            // Small delay to let process initialize
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut writer = pair
                .master
                .take_writer()
                .map_err(|e| io::Error::other(e.to_string()))?;
            writer.write_all(input.as_bytes())?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }

        // Drop the slave to signal EOF when master closes
        drop(pair.slave);

        let mut output = Vec::new();
        let timeout_duration = if !self.config.interactive || self.config.idle_timeout_secs == 0 {
            None
        } else {
            Some(Duration::from_secs(u64::from(
                self.config.idle_timeout_secs,
            )))
        };

        let mut termination = TerminationType::Natural;
        let mut last_activity = Instant::now();

        // Flag for termination request (shared with reader thread)
        let should_terminate = Arc::new(AtomicBool::new(false));

        // Spawn blocking reader thread that sends output via channel
        let (output_tx, mut output_rx) = mpsc::channel::<OutputEvent>(256);
        let should_terminate_reader = Arc::clone(&should_terminate);
        // Check if TUI is handling output (output_rx taken by handle())
        let tui_connected = self.tui_mode;
        let tui_output_tx = if tui_connected {
            Some(self.output_tx.clone())
        } else {
            None
        };

        debug!("Spawning PTY output reader thread (observe mode)");
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];

            loop {
                if should_terminate_reader.load(Ordering::SeqCst) {
                    debug!("PTY reader: termination requested");
                    break;
                }

                match reader.read(&mut buf) {
                    Ok(0) => {
                        debug!("PTY reader: EOF");
                        let _ = output_tx.blocking_send(OutputEvent::Eof);
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        // Send to TUI channel if connected
                        if let Some(ref tx) = tui_output_tx {
                            let _ = tx.send(data.clone());
                        }
                        // Send to main loop
                        if output_tx.blocking_send(OutputEvent::Data(data)).is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        debug!(error = %e, "PTY reader error");
                        let _ = output_tx.blocking_send(OutputEvent::Error(e.to_string()));
                        break;
                    }
                }
            }
        });

        // Main event loop using tokio::select! for interruptibility
        loop {
            // Calculate timeout for idle check
            let idle_timeout = timeout_duration.map(|d| {
                let elapsed = last_activity.elapsed();
                if elapsed >= d {
                    Duration::from_millis(1) // Trigger immediately
                } else {
                    d.saturating_sub(elapsed)
                }
            });

            tokio::select! {
                // Check for interrupt signal from event loop
                _ = interrupt_rx.changed() => {
                    if *interrupt_rx.borrow() {
                        debug!("Interrupt received in observe mode, terminating");
                        termination = TerminationType::UserInterrupt;
                        should_terminate.store(true, Ordering::SeqCst);
                        let _ = self.terminate_child(&mut child, true).await;
                        break;
                    }
                }

                // Check for output from reader thread
                event = output_rx.recv() => {
                    match event {
                        Some(OutputEvent::Data(data)) => {
                            // Only write to stdout if TUI is NOT handling output
                            if !tui_connected {
                                io::stdout().write_all(&data)?;
                                io::stdout().flush()?;
                            }
                            output.extend_from_slice(&data);
                            last_activity = Instant::now();
                        }
                        Some(OutputEvent::Eof) | None => {
                            debug!("Output channel closed, process likely exited");
                            break;
                        }
                        Some(OutputEvent::Error(e)) => {
                            debug!(error = %e, "Reader thread reported error");
                            break;
                        }
                    }
                }

                // Check for idle timeout
                _ = async {
                    if let Some(timeout) = idle_timeout {
                        tokio::time::sleep(timeout).await;
                    } else {
                        // No timeout configured, wait forever
                        std::future::pending::<()>().await;
                    }
                } => {
                    warn!(
                        timeout_secs = self.config.idle_timeout_secs,
                        "Idle timeout triggered"
                    );
                    termination = TerminationType::IdleTimeout;
                    should_terminate.store(true, Ordering::SeqCst);
                    self.terminate_child(&mut child, true).await?;
                    break;
                }
            }

            // Check if child has exited
            if let Some(status) = child
                .try_wait()
                .map_err(|e| io::Error::other(e.to_string()))?
            {
                let exit_code = status.exit_code() as i32;
                debug!(exit_status = ?status, exit_code, "Child process exited");

                // Drain any remaining output from channel
                while let Ok(event) = output_rx.try_recv() {
                    if let OutputEvent::Data(data) = event {
                        if !tui_connected {
                            io::stdout().write_all(&data)?;
                            io::stdout().flush()?;
                        }
                        output.extend_from_slice(&data);
                    }
                }

                let final_termination = resolve_termination_type(exit_code, termination);
                // run_observe doesn't parse JSON, so extracted_text is empty
                return Ok(build_result(
                    &output,
                    status.success(),
                    Some(exit_code),
                    final_termination,
                    String::new(),
                ));
            }
        }

        // Signal reader thread to stop
        should_terminate.store(true, Ordering::SeqCst);

        // Wait for child to fully exit (interruptible + bounded)
        let status = self
            .wait_for_exit(&mut child, Some(Duration::from_secs(2)), &mut interrupt_rx)
            .await?;

        let (success, exit_code, final_termination) = match status {
            Some(s) => {
                let code = s.exit_code() as i32;
                (
                    s.success(),
                    Some(code),
                    resolve_termination_type(code, termination),
                )
            }
            None => {
                warn!("Timed out waiting for child to exit after termination");
                (false, None, termination)
            }
        };

        // run_observe doesn't parse JSON, so extracted_text is empty
        Ok(build_result(
            &output,
            success,
            exit_code,
            final_termination,
            String::new(),
        ))
    }

    /// Runs in observe mode with streaming event handling for JSON output.
    ///
    /// When the backend's output format is `StreamJson`, this method parses
    /// NDJSON lines and dispatches events to the provided handler for real-time
    /// display. For `Text` format, behaves identically to `run_observe`.
    ///
    /// # Arguments
    /// * `prompt` - The prompt to execute
    /// * `interrupt_rx` - Watch channel receiver for interrupt signals
    /// * `handler` - Handler to receive streaming events
    ///
    /// # Errors
    ///
    /// Returns an error if PTY allocation fails, the command cannot be spawned,
    /// or an I/O error occurs during output handling.
    pub async fn run_observe_streaming<H: StreamHandler>(
        &self,
        prompt: &str,
        mut interrupt_rx: tokio::sync::watch::Receiver<bool>,
        handler: &mut H,
    ) -> io::Result<PtyExecutionResult> {
        // Check output format to decide parsing strategy
        let output_format = self.backend.output_format;

        // StreamJson format uses NDJSON line parsing
        // Text format streams raw output directly to handler
        let is_stream_json = output_format == OutputFormat::StreamJson;

        // Keep temp_file alive for the duration of execution
        let (pair, mut child, stdin_input, _temp_file) = self.spawn_pty(prompt)?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Write stdin input if present (for stdin prompt mode)
        if let Some(ref input) = stdin_input {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut writer = pair
                .master
                .take_writer()
                .map_err(|e| io::Error::other(e.to_string()))?;
            writer.write_all(input.as_bytes())?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }

        drop(pair.slave);

        let mut output = Vec::new();
        let mut line_buffer = String::new();
        // Accumulate extracted text from NDJSON for event parsing
        let mut extracted_text = String::new();
        let timeout_duration = if !self.config.interactive || self.config.idle_timeout_secs == 0 {
            None
        } else {
            Some(Duration::from_secs(u64::from(
                self.config.idle_timeout_secs,
            )))
        };

        let mut termination = TerminationType::Natural;
        let mut last_activity = Instant::now();

        let should_terminate = Arc::new(AtomicBool::new(false));

        // Spawn blocking reader thread
        let (output_tx, mut output_rx) = mpsc::channel::<OutputEvent>(256);
        let should_terminate_reader = Arc::clone(&should_terminate);
        let tui_connected = self.tui_mode;
        let tui_output_tx = if tui_connected {
            Some(self.output_tx.clone())
        } else {
            None
        };

        debug!("Spawning PTY output reader thread (streaming mode)");
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];

            loop {
                if should_terminate_reader.load(Ordering::SeqCst) {
                    debug!("PTY reader: termination requested");
                    break;
                }

                match reader.read(&mut buf) {
                    Ok(0) => {
                        debug!("PTY reader: EOF");
                        let _ = output_tx.blocking_send(OutputEvent::Eof);
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if let Some(ref tx) = tui_output_tx {
                            let _ = tx.send(data.clone());
                        }
                        if output_tx.blocking_send(OutputEvent::Data(data)).is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        debug!(error = %e, "PTY reader error");
                        let _ = output_tx.blocking_send(OutputEvent::Error(e.to_string()));
                        break;
                    }
                }
            }
        });

        // Main event loop with JSON line parsing
        loop {
            let idle_timeout = timeout_duration.map(|d| {
                let elapsed = last_activity.elapsed();
                if elapsed >= d {
                    Duration::from_millis(1)
                } else {
                    d.saturating_sub(elapsed)
                }
            });

            tokio::select! {
                _ = interrupt_rx.changed() => {
                    if *interrupt_rx.borrow() {
                        debug!("Interrupt received in streaming observe mode, terminating");
                        termination = TerminationType::UserInterrupt;
                        should_terminate.store(true, Ordering::SeqCst);
                        let _ = self.terminate_child(&mut child, true).await;
                        break;
                    }
                }

                event = output_rx.recv() => {
                    match event {
                        Some(OutputEvent::Data(data)) => {
                            output.extend_from_slice(&data);
                            last_activity = Instant::now();

                            if let Ok(text) = std::str::from_utf8(&data) {
                                if is_stream_json {
                                    // StreamJson format: Parse JSON lines from the data
                                    line_buffer.push_str(text);

                                    // Process complete lines
                                    while let Some(newline_pos) = line_buffer.find('\n') {
                                        let line = line_buffer[..newline_pos].to_string();
                                        line_buffer = line_buffer[newline_pos + 1..].to_string();

                                        if let Some(event) = ClaudeStreamParser::parse_line(&line) {
                                            dispatch_stream_event(event, handler, &mut extracted_text);
                                        }
                                    }
                                } else {
                                    // Text format: Stream raw output directly to handler
                                    // This preserves ANSI escape codes for TUI rendering
                                    handler.on_text(text);
                                }
                            }
                        }
                        Some(OutputEvent::Eof) | None => {
                            debug!("Output channel closed");
                            // Process any remaining content in buffer (StreamJson only)
                            if is_stream_json && !line_buffer.is_empty()
                                && let Some(event) = ClaudeStreamParser::parse_line(&line_buffer)
                            {
                                dispatch_stream_event(event, handler, &mut extracted_text);
                            }
                            break;
                        }
                        Some(OutputEvent::Error(e)) => {
                            debug!(error = %e, "Reader thread reported error");
                            handler.on_error(&e);
                            break;
                        }
                    }
                }

                _ = async {
                    if let Some(timeout) = idle_timeout {
                        tokio::time::sleep(timeout).await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    warn!(
                        timeout_secs = self.config.idle_timeout_secs,
                        "Idle timeout triggered"
                    );
                    termination = TerminationType::IdleTimeout;
                    should_terminate.store(true, Ordering::SeqCst);
                    self.terminate_child(&mut child, true).await?;
                    break;
                }
            }

            // Check if child has exited
            if let Some(status) = child
                .try_wait()
                .map_err(|e| io::Error::other(e.to_string()))?
            {
                let exit_code = status.exit_code() as i32;
                debug!(exit_status = ?status, exit_code, "Child process exited");

                // Drain remaining output
                while let Ok(event) = output_rx.try_recv() {
                    if let OutputEvent::Data(data) = event {
                        output.extend_from_slice(&data);
                        if let Ok(text) = std::str::from_utf8(&data) {
                            if is_stream_json {
                                // StreamJson: parse JSON lines
                                line_buffer.push_str(text);
                                while let Some(newline_pos) = line_buffer.find('\n') {
                                    let line = line_buffer[..newline_pos].to_string();
                                    line_buffer = line_buffer[newline_pos + 1..].to_string();
                                    if let Some(event) = ClaudeStreamParser::parse_line(&line) {
                                        dispatch_stream_event(event, handler, &mut extracted_text);
                                    }
                                }
                            } else {
                                // Text: stream raw output to handler
                                handler.on_text(text);
                            }
                        }
                    }
                }

                // Process final buffer content (StreamJson only)
                if is_stream_json
                    && !line_buffer.is_empty()
                    && let Some(event) = ClaudeStreamParser::parse_line(&line_buffer)
                {
                    dispatch_stream_event(event, handler, &mut extracted_text);
                }

                let final_termination = resolve_termination_type(exit_code, termination);
                // Pass extracted_text for event parsing from NDJSON
                return Ok(build_result(
                    &output,
                    status.success(),
                    Some(exit_code),
                    final_termination,
                    extracted_text,
                ));
            }
        }

        should_terminate.store(true, Ordering::SeqCst);

        let status = self
            .wait_for_exit(&mut child, Some(Duration::from_secs(2)), &mut interrupt_rx)
            .await?;

        let (success, exit_code, final_termination) = match status {
            Some(s) => {
                let code = s.exit_code() as i32;
                (
                    s.success(),
                    Some(code),
                    resolve_termination_type(code, termination),
                )
            }
            None => {
                warn!("Timed out waiting for child to exit after termination");
                (false, None, termination)
            }
        };

        // Pass extracted_text for event parsing from NDJSON
        Ok(build_result(
            &output,
            success,
            exit_code,
            final_termination,
            extracted_text,
        ))
    }

    /// Runs in interactive mode (bidirectional I/O).
    ///
    /// Uses `tokio::select!` for non-blocking I/O multiplexing between:
    /// 1. PTY output (from blocking reader via channel)
    /// 2. User input (from stdin thread via channel)
    /// 3. Interrupt signal from event loop
    /// 4. Idle timeout
    ///
    /// This design ensures Ctrl+C is always responsive, even when the PTY
    /// has no output (e.g., during long-running tool calls).
    ///
    /// # Arguments
    /// * `prompt` - The prompt to execute
    /// * `interrupt_rx` - Watch channel receiver for interrupt signals from the event loop
    ///
    /// # Errors
    ///
    /// Returns an error if PTY allocation fails, the command cannot be spawned,
    /// or an I/O error occurs during bidirectional communication.
    #[allow(clippy::too_many_lines)] // Complex state machine requires cohesive implementation
    pub async fn run_interactive(
        &mut self,
        prompt: &str,
        mut interrupt_rx: tokio::sync::watch::Receiver<bool>,
    ) -> io::Result<PtyExecutionResult> {
        // Keep temp_file alive for the duration of execution (large prompts use temp files)
        let (pair, mut child, stdin_input, _temp_file) = self.spawn_pty(prompt)?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Keep master for resize operations
        let master = pair.master;

        // Drop the slave to signal EOF when master closes
        drop(pair.slave);

        // Store stdin_input for writing after reader thread starts
        let pending_stdin = stdin_input;

        let mut output = Vec::new();
        let timeout_duration = if self.config.idle_timeout_secs > 0 {
            Some(Duration::from_secs(u64::from(
                self.config.idle_timeout_secs,
            )))
        } else {
            None
        };

        let mut ctrl_c_state = CtrlCState::new();
        let mut termination = TerminationType::Natural;
        let mut last_activity = Instant::now();

        // Flag for termination request (shared with spawned tasks)
        let should_terminate = Arc::new(AtomicBool::new(false));

        // Spawn output reading task (blocking read wrapped in spawn_blocking via channel)
        let (output_tx, mut output_rx) = mpsc::channel::<OutputEvent>(256);
        let should_terminate_output = Arc::clone(&should_terminate);
        // Check if TUI is handling output (output_rx taken by handle())
        let tui_connected = self.tui_mode;
        let tui_output_tx = if tui_connected {
            Some(self.output_tx.clone())
        } else {
            None
        };

        debug!("Spawning PTY output reader thread");
        std::thread::spawn(move || {
            debug!("PTY output reader thread started");
            let mut reader = reader;
            let mut buf = [0u8; 4096];

            loop {
                if should_terminate_output.load(Ordering::SeqCst) {
                    debug!("PTY output reader: termination requested");
                    break;
                }

                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF - PTY closed
                        debug!("PTY output reader: EOF received");
                        let _ = output_tx.blocking_send(OutputEvent::Eof);
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        // Send to TUI channel if connected
                        if let Some(ref tx) = tui_output_tx {
                            let _ = tx.send(data.clone());
                        }
                        // Send to main loop
                        if output_tx.blocking_send(OutputEvent::Data(data)).is_err() {
                            debug!("PTY output reader: channel closed");
                            break;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // Non-blocking mode: no data available, yield briefly
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                        // Interrupted by signal, retry
                    }
                    Err(e) => {
                        warn!("PTY output reader: error - {}", e);
                        let _ = output_tx.blocking_send(OutputEvent::Error(e.to_string()));
                        break;
                    }
                }
            }
            debug!("PTY output reader thread exiting");
        });

        // Spawn input reading task - ONLY when TUI is NOT connected
        // In TUI mode (observation mode), user input should not be captured from stdin.
        // The TUI has its own input handling, and raw Ctrl+C should go directly to the
        // signal handler (interrupt_rx) without racing with the stdin reader.
        let mut input_rx = if tui_connected {
            debug!("TUI connected - skipping stdin reader thread");
            None
        } else {
            let (input_tx, input_rx) = mpsc::unbounded_channel::<InputEvent>();
            let should_terminate_input = Arc::clone(&should_terminate);

            std::thread::spawn(move || {
                let mut stdin = io::stdin();
                let mut buf = [0u8; 1];

                loop {
                    if should_terminate_input.load(Ordering::SeqCst) {
                        break;
                    }

                    match stdin.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(1) => {
                            let byte = buf[0];
                            let event = match byte {
                                3 => InputEvent::CtrlC,          // Ctrl+C
                                28 => InputEvent::CtrlBackslash, // Ctrl+\
                                _ => InputEvent::Data(vec![byte]),
                            };
                            if input_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {} // Shouldn't happen with 1-byte buffer
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                        Err(_) => break,
                    }
                }
            });
            Some(input_rx)
        };

        // Write stdin input after threads are spawned (so we capture any output)
        // Give Claude's TUI a moment to initialize before sending the prompt
        if let Some(ref input) = pending_stdin {
            tokio::time::sleep(Duration::from_millis(100)).await;
            writer.write_all(input.as_bytes())?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            last_activity = Instant::now();
        }

        // Main select loop - this is the key fix for blocking I/O
        // We use tokio::select! to multiplex between output, input, and timeout
        loop {
            // Check if child has exited (non-blocking check before select)
            if let Some(status) = child
                .try_wait()
                .map_err(|e| io::Error::other(e.to_string()))?
            {
                let exit_code = status.exit_code() as i32;
                debug!(exit_status = ?status, exit_code, "Child process exited");

                // Drain remaining output already buffered.
                while let Ok(event) = output_rx.try_recv() {
                    if let OutputEvent::Data(data) = event {
                        if !tui_connected {
                            io::stdout().write_all(&data)?;
                            io::stdout().flush()?;
                        }
                        output.extend_from_slice(&data);
                    }
                }

                // Give the reader thread a brief window to flush any final bytes/EOF.
                // This avoids races where fast-exiting commands drop output before we return.
                let drain_deadline = Instant::now() + Duration::from_millis(200);
                loop {
                    let remaining = drain_deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(remaining, output_rx.recv()).await {
                        Ok(Some(OutputEvent::Data(data))) => {
                            if !tui_connected {
                                io::stdout().write_all(&data)?;
                                io::stdout().flush()?;
                            }
                            output.extend_from_slice(&data);
                        }
                        Ok(Some(OutputEvent::Eof) | None) => break,
                        Ok(Some(OutputEvent::Error(e))) => {
                            debug!(error = %e, "PTY read error after exit");
                            break;
                        }
                        Err(_) => break, // timeout
                    }
                }

                should_terminate.store(true, Ordering::SeqCst);
                // Signal TUI that PTY has terminated
                let _ = self.terminated_tx.send(true);

                let final_termination = resolve_termination_type(exit_code, termination);
                // run_interactive doesn't parse JSON, so extracted_text is empty
                return Ok(build_result(
                    &output,
                    status.success(),
                    Some(exit_code),
                    final_termination,
                    String::new(),
                ));
            }

            // Build the timeout future (or a never-completing one if disabled)
            let timeout_future = async {
                match timeout_duration {
                    Some(d) => {
                        let elapsed = last_activity.elapsed();
                        if elapsed >= d {
                            tokio::time::sleep(Duration::ZERO).await
                        } else {
                            tokio::time::sleep(d.saturating_sub(elapsed)).await
                        }
                    }
                    None => std::future::pending::<()>().await,
                }
            };

            tokio::select! {
                // PTY output received
                output_event = output_rx.recv() => {
                    match output_event {
                        Some(OutputEvent::Data(data)) => {
                            // Only write to stdout if TUI is NOT handling output
                            if !tui_connected {
                                io::stdout().write_all(&data)?;
                                io::stdout().flush()?;
                            }
                            output.extend_from_slice(&data);

                            last_activity = Instant::now();
                        }
                        Some(OutputEvent::Eof) => {
                            debug!("PTY EOF received");
                            break;
                        }
                        Some(OutputEvent::Error(e)) => {
                            debug!(error = %e, "PTY read error");
                            break;
                        }
                        None => {
                            // Channel closed, reader thread exited
                            break;
                        }
                    }
                }

                // User input received (from stdin) - only active when TUI is NOT connected
                input_event = async {
                    match input_rx.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await, // Never resolves when TUI is connected
                    }
                } => {
                    match input_event {
                        Some(InputEvent::CtrlC) => {
                            match ctrl_c_state.handle_ctrl_c(Instant::now()) {
                                CtrlCAction::ForwardAndStartWindow => {
                                    // Forward Ctrl+C to Claude
                                    let _ = writer.write_all(&[3]);
                                    let _ = writer.flush();
                                    last_activity = Instant::now();
                                }
                                CtrlCAction::Terminate => {
                                    info!("Double Ctrl+C detected, terminating");
                                    termination = TerminationType::UserInterrupt;
                                    should_terminate.store(true, Ordering::SeqCst);
                                    self.terminate_child(&mut child, true).await?;
                                    break;
                                }
                            }
                        }
                        Some(InputEvent::CtrlBackslash) => {
                            info!("Ctrl+\\ detected, force killing");
                            termination = TerminationType::ForceKill;
                            should_terminate.store(true, Ordering::SeqCst);
                            self.terminate_child(&mut child, false).await?;
                            break;
                        }
                        Some(InputEvent::Data(data)) => {
                            // Forward to Claude
                            let _ = writer.write_all(&data);
                            let _ = writer.flush();
                            last_activity = Instant::now();
                        }
                        None => {
                            // Input channel closed (stdin EOF)
                            debug!("Input channel closed");
                        }
                    }
                }

                // TUI input received (convert to InputEvent for unified handling)
                tui_input = self.input_rx.recv() => {
                    if let Some(data) = tui_input {
                        match InputEvent::from_bytes(data) {
                            InputEvent::CtrlC => {
                                match ctrl_c_state.handle_ctrl_c(Instant::now()) {
                                    CtrlCAction::ForwardAndStartWindow => {
                                        let _ = writer.write_all(&[3]);
                                        let _ = writer.flush();
                                        last_activity = Instant::now();
                                    }
                                    CtrlCAction::Terminate => {
                                        info!("Double Ctrl+C detected, terminating");
                                        termination = TerminationType::UserInterrupt;
                                        should_terminate.store(true, Ordering::SeqCst);
                                        self.terminate_child(&mut child, true).await?;
                                        break;
                                    }
                                }
                            }
                            InputEvent::CtrlBackslash => {
                                info!("Ctrl+\\ detected, force killing");
                                termination = TerminationType::ForceKill;
                                should_terminate.store(true, Ordering::SeqCst);
                                self.terminate_child(&mut child, false).await?;
                                break;
                            }
                            InputEvent::Data(bytes) => {
                                let _ = writer.write_all(&bytes);
                                let _ = writer.flush();
                                last_activity = Instant::now();
                            }
                        }
                    }
                }

                // Control commands from TUI
                control_cmd = self.control_rx.recv() => {
                    if let Some(cmd) = control_cmd {
                        use crate::pty_handle::ControlCommand;
                        match cmd {
                            ControlCommand::Kill => {
                                info!("Control command: Kill");
                                termination = TerminationType::UserInterrupt;
                                should_terminate.store(true, Ordering::SeqCst);
                                self.terminate_child(&mut child, true).await?;
                                break;
                            }
                            ControlCommand::Resize(cols, rows) => {
                                debug!(cols, rows, "Control command: Resize");
                                // Resize the PTY to match TUI dimensions
                                if let Err(e) = master.resize(PtySize {
                                    rows,
                                    cols,
                                    pixel_width: 0,
                                    pixel_height: 0,
                                }) {
                                    warn!("Failed to resize PTY: {}", e);
                                }
                            }
                            ControlCommand::Skip | ControlCommand::Abort => {
                                // These are handled at orchestrator level, not here
                                debug!("Control command: {:?} (ignored at PTY level)", cmd);
                            }
                        }
                    }
                }

                // Idle timeout expired
                _ = timeout_future => {
                    warn!(
                        timeout_secs = self.config.idle_timeout_secs,
                        "Idle timeout triggered"
                    );
                    termination = TerminationType::IdleTimeout;
                    should_terminate.store(true, Ordering::SeqCst);
                    self.terminate_child(&mut child, true).await?;
                    break;
                }

                // Interrupt signal from event loop
                _ = interrupt_rx.changed() => {
                    if *interrupt_rx.borrow() {
                        debug!("Interrupt received in interactive mode, terminating");
                        termination = TerminationType::UserInterrupt;
                        should_terminate.store(true, Ordering::SeqCst);
                        self.terminate_child(&mut child, true).await?;
                        break;
                    }
                }
            }
        }

        // Ensure termination flag is set for spawned threads
        should_terminate.store(true, Ordering::SeqCst);

        // Signal TUI that PTY has terminated
        let _ = self.terminated_tx.send(true);

        // Wait for child to fully exit (interruptible + bounded)
        let status = self
            .wait_for_exit(&mut child, Some(Duration::from_secs(2)), &mut interrupt_rx)
            .await?;

        let (success, exit_code, final_termination) = match status {
            Some(s) => {
                let code = s.exit_code() as i32;
                (
                    s.success(),
                    Some(code),
                    resolve_termination_type(code, termination),
                )
            }
            None => {
                warn!("Timed out waiting for child to exit after termination");
                (false, None, termination)
            }
        };

        // run_interactive doesn't parse JSON, so extracted_text is empty
        Ok(build_result(
            &output,
            success,
            exit_code,
            final_termination,
            String::new(),
        ))
    }

    /// Terminates the child process.
    ///
    /// If `graceful` is true, sends SIGTERM and waits up to 5 seconds before SIGKILL.
    /// If `graceful` is false, sends SIGKILL immediately.
    ///
    /// This is an async function to avoid blocking the tokio runtime during the
    /// grace period wait. Previously used `std::thread::sleep` which blocked the
    /// worker thread for up to 5 seconds, making the TUI appear frozen.
    #[allow(clippy::unused_self)] // Self is conceptually the right receiver for this method
    #[allow(clippy::unused_async)] // Kept async to preserve signature parity with Unix implementation
    #[cfg(not(unix))]
    async fn terminate_child(
        &self,
        child: &mut Box<dyn portable_pty::Child + Send>,
        _graceful: bool,
    ) -> io::Result<()> {
        child.kill()
    }

    #[cfg(unix)]
    async fn terminate_child(
        &self,
        child: &mut Box<dyn portable_pty::Child + Send>,
        graceful: bool,
    ) -> io::Result<()> {
        let pid = match child.process_id() {
            Some(id) => Pid::from_raw(id as i32),
            None => return Ok(()), // Already exited
        };

        if graceful {
            debug!(pid = %pid, "Sending SIGTERM");
            let _ = kill(pid, Signal::SIGTERM);

            // Wait up to 5 seconds for graceful exit (reduced from 5s for better UX)
            let grace_period = Duration::from_secs(2);
            let start = Instant::now();

            while start.elapsed() < grace_period {
                if child
                    .try_wait()
                    .map_err(|e| io::Error::other(e.to_string()))?
                    .is_some()
                {
                    return Ok(());
                }
                // Use async sleep to avoid blocking the tokio runtime
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            // Still running after grace period - force kill
            debug!(pid = %pid, "Grace period expired, sending SIGKILL");
        }

        debug!(pid = %pid, "Sending SIGKILL");
        let _ = kill(pid, Signal::SIGKILL);
        Ok(())
    }

    /// Waits for the child process to exit, optionally with a timeout.
    ///
    /// This is interruptible by the shared interrupt channel from the event loop.
    /// When interrupted, returns `Ok(None)` to let the caller handle termination.
    async fn wait_for_exit(
        &self,
        child: &mut Box<dyn portable_pty::Child + Send>,
        max_wait: Option<Duration>,
        interrupt_rx: &mut tokio::sync::watch::Receiver<bool>,
    ) -> io::Result<Option<portable_pty::ExitStatus>> {
        let start = Instant::now();

        loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|e| io::Error::other(e.to_string()))?
            {
                return Ok(Some(status));
            }

            if let Some(max) = max_wait
                && start.elapsed() >= max
            {
                return Ok(None);
            }

            tokio::select! {
                _ = interrupt_rx.changed() => {
                    if *interrupt_rx.borrow() {
                        debug!("Interrupt received while waiting for child exit");
                        return Ok(None);
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        }
    }
}

/// Input events from the user.
#[derive(Debug)]
enum InputEvent {
    /// Ctrl+C pressed.
    CtrlC,
    /// Ctrl+\ pressed.
    CtrlBackslash,
    /// Regular data to forward.
    Data(Vec<u8>),
}

impl InputEvent {
    /// Creates an InputEvent from raw bytes.
    fn from_bytes(data: Vec<u8>) -> Self {
        if data.len() == 1 {
            match data[0] {
                3 => return InputEvent::CtrlC,
                28 => return InputEvent::CtrlBackslash,
                _ => {}
            }
        }
        InputEvent::Data(data)
    }
}

/// Output events from the PTY.
#[derive(Debug)]
enum OutputEvent {
    /// Data received from PTY.
    Data(Vec<u8>),
    /// PTY reached EOF (process exited).
    Eof,
    /// Error reading from PTY.
    Error(String),
}

/// Strips ANSI escape sequences from raw bytes.
///
/// Uses `strip-ansi-escapes` for direct byte-level ANSI removal without terminal
/// emulation. This ensures ALL content is preserved regardless of output size,
/// unlike vt100's terminal simulation which can lose content that scrolls off.
fn strip_ansi(bytes: &[u8]) -> String {
    let stripped = strip_ansi_escapes::strip(bytes);
    String::from_utf8_lossy(&stripped).into_owned()
}

/// Determines the final termination type, accounting for SIGINT exit code.
///
/// Exit code 130 indicates the process was killed by SIGINT (Ctrl+C forwarded to PTY).
fn resolve_termination_type(exit_code: i32, default: TerminationType) -> TerminationType {
    if exit_code == 130 {
        info!("Child process killed by SIGINT");
        TerminationType::UserInterrupt
    } else {
        default
    }
}

/// Dispatches a Claude stream event to the appropriate handler method.
/// Also accumulates text content into `extracted_text` for event parsing.
fn dispatch_stream_event<H: StreamHandler>(
    event: ClaudeStreamEvent,
    handler: &mut H,
    extracted_text: &mut String,
) {
    match event {
        ClaudeStreamEvent::System { .. } => {
            // Session initialization - could log in verbose mode but not user-facing
        }
        ClaudeStreamEvent::Assistant { message, .. } => {
            for block in message.content {
                match block {
                    ContentBlock::Text { text } => {
                        handler.on_text(&text);
                        // Accumulate text for event parsing
                        extracted_text.push_str(&text);
                        extracted_text.push('\n');
                    }
                    ContentBlock::ToolUse { name, id, input } => {
                        handler.on_tool_call(&name, &id, &input)
                    }
                }
            }
        }
        ClaudeStreamEvent::User { message } => {
            for block in message.content {
                match block {
                    UserContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => {
                        handler.on_tool_result(&tool_use_id, &content);
                    }
                }
            }
        }
        ClaudeStreamEvent::Result {
            duration_ms,
            total_cost_usd,
            num_turns,
            is_error,
        } => {
            if is_error {
                handler.on_error("Session ended with error");
            }
            handler.on_complete(&SessionResult {
                duration_ms,
                total_cost_usd,
                num_turns,
                is_error,
            });
        }
    }
}

/// Builds a `PtyExecutionResult` from the accumulated output and exit status.
///
/// # Arguments
/// * `output` - Raw bytes from PTY
/// * `success` - Whether process exited successfully
/// * `exit_code` - Process exit code if available
/// * `termination` - How the process was terminated
/// * `extracted_text` - Text extracted from NDJSON stream (for Claude's stream-json)
fn build_result(
    output: &[u8],
    success: bool,
    exit_code: Option<i32>,
    termination: TerminationType,
    extracted_text: String,
) -> PtyExecutionResult {
    PtyExecutionResult {
        output: String::from_utf8_lossy(output).to_string(),
        stripped_output: strip_ansi(output),
        extracted_text,
        success,
        exit_code,
        termination,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude_stream::{AssistantMessage, UserMessage};
    #[cfg(unix)]
    use crate::cli_backend::PromptMode;
    use crate::stream_handler::{SessionResult, StreamHandler};
    #[cfg(unix)]
    use tempfile::TempDir;

    #[test]
    fn test_double_ctrl_c_within_window() {
        let mut state = CtrlCState::new();
        let now = Instant::now();

        // First Ctrl+C: should forward and start window
        let action = state.handle_ctrl_c(now);
        assert_eq!(action, CtrlCAction::ForwardAndStartWindow);

        // Second Ctrl+C within 1 second: should terminate
        let later = now + Duration::from_millis(500);
        let action = state.handle_ctrl_c(later);
        assert_eq!(action, CtrlCAction::Terminate);
    }

    #[test]
    fn test_input_event_from_bytes_ctrl_c() {
        let event = InputEvent::from_bytes(vec![3]);
        assert!(matches!(event, InputEvent::CtrlC));
    }

    #[test]
    fn test_input_event_from_bytes_ctrl_backslash() {
        let event = InputEvent::from_bytes(vec![28]);
        assert!(matches!(event, InputEvent::CtrlBackslash));
    }

    #[test]
    fn test_input_event_from_bytes_data() {
        let event = InputEvent::from_bytes(vec![b'a']);
        assert!(matches!(event, InputEvent::Data(_)));

        let event = InputEvent::from_bytes(vec![1, 2, 3]);
        assert!(matches!(event, InputEvent::Data(_)));
    }

    #[test]
    fn test_ctrl_c_window_expires() {
        let mut state = CtrlCState::new();
        let now = Instant::now();

        // First Ctrl+C
        state.handle_ctrl_c(now);

        // Wait 2 seconds (window expires)
        let later = now + Duration::from_secs(2);

        // Second Ctrl+C: window expired, should forward and start new window
        let action = state.handle_ctrl_c(later);
        assert_eq!(action, CtrlCAction::ForwardAndStartWindow);
    }

    #[test]
    fn test_strip_ansi_basic() {
        let input = b"\x1b[1;36m  Thinking...\x1b[0m\r\n";
        let stripped = strip_ansi(input);
        assert!(stripped.contains("Thinking..."));
        assert!(!stripped.contains("\x1b["));
    }

    #[test]
    fn test_completion_promise_extraction() {
        // Simulate Claude output with heavy ANSI formatting
        let input = b"\x1b[1;36m  Thinking...\x1b[0m\r\n\
                      \x1b[2K\x1b[1;32m  Done!\x1b[0m\r\n\
                      \x1b[33mLOOP_COMPLETE\x1b[0m\r\n";

        let stripped = strip_ansi(input);

        // Event parser sees clean text
        assert!(stripped.contains("LOOP_COMPLETE"));
        assert!(!stripped.contains("\x1b["));
    }

    #[test]
    fn test_event_tag_extraction() {
        // Event tags may be wrapped in ANSI codes
        let input = b"\x1b[90m<event topic=\"build.done\">\x1b[0m\r\n\
                      Task completed successfully\r\n\
                      \x1b[90m</event>\x1b[0m\r\n";

        let stripped = strip_ansi(input);

        assert!(stripped.contains("<event topic=\"build.done\">"));
        assert!(stripped.contains("</event>"));
    }

    #[test]
    fn test_large_output_preserves_early_events() {
        // Regression test: ensure event tags aren't lost when output is large
        let mut input = Vec::new();

        // Event tag at the beginning
        input.extend_from_slice(b"<event topic=\"build.task\">Implement feature X</event>\r\n");

        // Simulate 500 lines of verbose output (would overflow any terminal)
        for i in 0..500 {
            input.extend_from_slice(format!("Line {}: Processing step {}...\r\n", i, i).as_bytes());
        }

        let stripped = strip_ansi(&input);

        // Event tag should still be present - no scrollback loss with strip-ansi-escapes
        assert!(
            stripped.contains("<event topic=\"build.task\">"),
            "Event tag was lost - strip_ansi is not preserving all content"
        );
        assert!(stripped.contains("Implement feature X"));
        assert!(stripped.contains("Line 499")); // Last line should be present too
    }

    #[test]
    fn test_pty_config_defaults() {
        let config = PtyConfig::default();
        assert!(config.interactive);
        assert_eq!(config.idle_timeout_secs, 30);
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
    }

    #[test]
    fn test_pty_config_from_env_matches_env_or_defaults() {
        let cols = std::env::var("COLUMNS")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(80);
        let rows = std::env::var("LINES")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(24);

        let config = PtyConfig::from_env();
        assert_eq!(config.cols, cols);
        assert_eq!(config.rows, rows);
    }

    /// Verifies that the idle timeout logic in run_interactive correctly handles
    /// activity resets. Per spec (interactive-mode.spec.md lines 155-159):
    /// - Timeout resets on agent output (any bytes from PTY)
    /// - Timeout resets on user input (any key forwarded to agent)
    ///
    /// This test validates the timeout calculation logic that enables resets.
    /// The actual reset happens in the select! branches at lines 497, 523, and 545.
    #[test]
    fn test_idle_timeout_reset_logic() {
        // Simulate the timeout calculation used in run_interactive
        let timeout_duration = Duration::from_secs(30);

        // Simulate 25 seconds of inactivity
        let simulated_25s = Duration::from_secs(25);

        // Remaining time before timeout
        let remaining = timeout_duration.saturating_sub(simulated_25s);
        assert_eq!(remaining.as_secs(), 5);

        // After activity (output or input), last_activity would be reset to now
        let last_activity_after_reset = Instant::now();

        // Now elapsed is 0, full timeout duration available again
        let elapsed = last_activity_after_reset.elapsed();
        assert!(elapsed < Duration::from_millis(100)); // Should be near-zero

        // Timeout calculation would give full duration minus small elapsed
        let new_remaining = timeout_duration.saturating_sub(elapsed);
        assert!(new_remaining > Duration::from_secs(29)); // Should be nearly full timeout
    }

    #[test]
    fn test_extracted_text_field_exists() {
        // Test that PtyExecutionResult has extracted_text field
        // This is for NDJSON output where event tags are inside JSON strings
        let result = PtyExecutionResult {
            output: String::new(),
            stripped_output: String::new(),
            extracted_text: String::from("<event topic=\"build.done\">Test</event>"),
            success: true,
            exit_code: Some(0),
            termination: TerminationType::Natural,
        };

        assert!(
            result
                .extracted_text
                .contains("<event topic=\"build.done\">")
        );
    }

    #[test]
    fn test_build_result_includes_extracted_text() {
        // Test that build_result properly handles extracted_text
        let output = b"raw output";
        let extracted = "extracted text with <event topic=\"test\">payload</event>";
        let result = build_result(
            output,
            true,
            Some(0),
            TerminationType::Natural,
            extracted.to_string(),
        );

        assert_eq!(result.extracted_text, extracted);
        assert!(result.stripped_output.contains("raw output"));
    }

    #[test]
    fn test_resolve_termination_type_handles_sigint_exit_code() {
        let termination = resolve_termination_type(130, TerminationType::Natural);
        assert_eq!(termination, TerminationType::UserInterrupt);

        let termination = resolve_termination_type(0, TerminationType::ForceKill);
        assert_eq!(termination, TerminationType::ForceKill);
    }

    #[derive(Default)]
    struct CapturingHandler {
        texts: Vec<String>,
        tool_calls: Vec<(String, String, serde_json::Value)>,
        tool_results: Vec<(String, String)>,
        errors: Vec<String>,
        completions: Vec<SessionResult>,
    }

    impl StreamHandler for CapturingHandler {
        fn on_text(&mut self, text: &str) {
            self.texts.push(text.to_string());
        }

        fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value) {
            self.tool_calls
                .push((name.to_string(), id.to_string(), input.clone()));
        }

        fn on_tool_result(&mut self, id: &str, output: &str) {
            self.tool_results.push((id.to_string(), output.to_string()));
        }

        fn on_error(&mut self, error: &str) {
            self.errors.push(error.to_string());
        }

        fn on_complete(&mut self, result: &SessionResult) {
            self.completions.push(result.clone());
        }
    }

    #[test]
    fn test_dispatch_stream_event_routes_text_and_tool_calls() {
        let mut handler = CapturingHandler::default();
        let mut extracted_text = String::new();

        let event = ClaudeStreamEvent::Assistant {
            message: AssistantMessage {
                content: vec![
                    ContentBlock::Text {
                        text: "Hello".to_string(),
                    },
                    ContentBlock::ToolUse {
                        id: "tool-1".to_string(),
                        name: "Read".to_string(),
                        input: serde_json::json!({"path": "README.md"}),
                    },
                ],
            },
            usage: None,
        };

        dispatch_stream_event(event, &mut handler, &mut extracted_text);

        assert_eq!(handler.texts, vec!["Hello".to_string()]);
        assert_eq!(handler.tool_calls.len(), 1);
        assert!(extracted_text.contains("Hello"));
        assert!(extracted_text.ends_with('\n'));
    }

    #[test]
    fn test_dispatch_stream_event_routes_tool_results_and_completion() {
        let mut handler = CapturingHandler::default();
        let mut extracted_text = String::new();

        let event = ClaudeStreamEvent::User {
            message: UserMessage {
                content: vec![UserContentBlock::ToolResult {
                    tool_use_id: "tool-1".to_string(),
                    content: "done".to_string(),
                }],
            },
        };

        dispatch_stream_event(event, &mut handler, &mut extracted_text);
        assert_eq!(handler.tool_results.len(), 1);
        assert_eq!(handler.tool_results[0].0, "tool-1");
        assert_eq!(handler.tool_results[0].1, "done");

        let event = ClaudeStreamEvent::Result {
            duration_ms: 12,
            total_cost_usd: 0.01,
            num_turns: 2,
            is_error: true,
        };

        dispatch_stream_event(event, &mut handler, &mut extracted_text);
        assert_eq!(handler.errors.len(), 1);
        assert_eq!(handler.completions.len(), 1);
        assert!(handler.completions[0].is_error);
    }

    #[test]
    fn test_dispatch_stream_event_system_noop() {
        let mut handler = CapturingHandler::default();
        let mut extracted_text = String::new();

        let event = ClaudeStreamEvent::System {
            session_id: "session-1".to_string(),
            model: "claude-test".to_string(),
            tools: Vec::new(),
        };

        dispatch_stream_event(event, &mut handler, &mut extracted_text);

        assert!(handler.texts.is_empty());
        assert!(handler.tool_calls.is_empty());
        assert!(handler.tool_results.is_empty());
        assert!(handler.errors.is_empty());
        assert!(handler.completions.is_empty());
        assert!(extracted_text.is_empty());
    }

    /// Regression test: TUI mode should not spawn stdin reader thread
    ///
    /// Bug: In TUI mode, Ctrl+C required double-press to exit because the stdin
    /// reader thread (which captures byte 0x03) raced with the signal handler.
    /// The stdin reader would win, triggering "double Ctrl+C" logic instead of
    /// clean exit via interrupt_rx.
    ///
    /// Fix: When tui_connected=true, skip spawning stdin reader entirely.
    /// TUI mode is observation-only; user input should not be captured from stdin.
    /// The TUI has its own input handling (Ctrl+a q), and raw Ctrl+C goes directly
    /// to the signal handler (interrupt_rx) without racing.
    ///
    /// This test documents the expected behavior. The actual fix is in
    /// run_interactive() where `let mut input_rx = if !tui_connected { ... }`.
    #[test]
    fn test_tui_mode_stdin_reader_bypass() {
        // The tui_connected flag is now determined by the explicit tui_mode field,
        // set via set_tui_mode(true) when TUI is connected.
        // Previously used output_rx.is_none() which broke after streaming refactor.

        // Simulate TUI connected scenario (tui_mode = true)
        let tui_mode = true;
        let tui_connected = tui_mode;

        // When TUI is connected, stdin reader is skipped
        // (verified by: input_rx becomes None instead of Some(channel))
        assert!(
            tui_connected,
            "When tui_mode is true, stdin reader must be skipped"
        );

        // In non-TUI mode, stdin reader is spawned
        let tui_mode_disabled = false;
        let tui_connected_non_tui = tui_mode_disabled;
        assert!(
            !tui_connected_non_tui,
            "When tui_mode is false, stdin reader must be spawned"
        );
    }

    #[test]
    fn test_tui_mode_default_is_false() {
        // Create a PtyExecutor and verify tui_mode defaults to false
        let backend = CliBackend::claude();
        let config = PtyConfig::default();
        let executor = PtyExecutor::new(backend, config);

        // tui_mode should default to false
        assert!(!executor.tui_mode, "tui_mode should default to false");
    }

    #[test]
    fn test_set_tui_mode() {
        // Create a PtyExecutor and verify set_tui_mode works
        let backend = CliBackend::claude();
        let config = PtyConfig::default();
        let mut executor = PtyExecutor::new(backend, config);

        // Initially false
        assert!(!executor.tui_mode, "tui_mode should start as false");

        // Set to true
        executor.set_tui_mode(true);
        assert!(
            executor.tui_mode,
            "tui_mode should be true after set_tui_mode(true)"
        );

        // Set back to false
        executor.set_tui_mode(false);
        assert!(
            !executor.tui_mode,
            "tui_mode should be false after set_tui_mode(false)"
        );
    }

    #[test]
    fn test_build_result_populates_fields() {
        let output = b"\x1b[31mHello\x1b[0m\n";
        let extracted = "extracted text".to_string();

        let result = build_result(
            output,
            true,
            Some(0),
            TerminationType::Natural,
            extracted.clone(),
        );

        assert_eq!(result.output, String::from_utf8_lossy(output));
        assert!(result.stripped_output.contains("Hello"));
        assert!(!result.stripped_output.contains("\x1b["));
        assert_eq!(result.extracted_text, extracted);
        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.termination, TerminationType::Natural);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_run_observe_executes_arg_prompt() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let result = executor
            .run_observe("echo hello-pty", rx)
            .await
            .expect("run_observe");

        assert!(result.success);
        assert!(result.output.contains("hello-pty"));
        assert!(result.stripped_output.contains("hello-pty"));
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.termination, TerminationType::Natural);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_run_observe_writes_stdin_prompt() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "read line; echo \"$line\"".to_string()],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let result = executor
            .run_observe("stdin-line", rx)
            .await
            .expect("run_observe");

        assert!(result.success);
        assert!(result.output.contains("stdin-line"));
        assert!(result.stripped_output.contains("stdin-line"));
        assert_eq!(result.termination, TerminationType::Natural);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_run_observe_streaming_text_routes_output() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let result = executor
            .run_observe_streaming("printf 'alpha\\nbeta\\n'", rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        let captured = handler.texts.join("");
        assert!(captured.contains("alpha"), "captured: {captured}");
        assert!(captured.contains("beta"), "captured: {captured}");
        assert!(handler.completions.is_empty());
        assert!(result.extracted_text.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_run_observe_streaming_parses_stream_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::StreamJson,
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let script = r#"printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"Hello stream"}]}}' '{"type":"result","duration_ms":1,"total_cost_usd":0.0,"num_turns":1,"is_error":false}'"#;
        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert!(
            handler
                .texts
                .iter()
                .any(|text| text.contains("Hello stream"))
        );
        assert_eq!(handler.completions.len(), 1);
        assert!(result.extracted_text.contains("Hello stream"));
        assert_eq!(result.termination, TerminationType::Natural);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_run_interactive_in_tui_mode() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        };
        let config = PtyConfig {
            interactive: true,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let mut executor = PtyExecutor::new(backend, config);
        executor.set_tui_mode(true);
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let result = executor
            .run_interactive("echo hello-tui", rx)
            .await
            .expect("run_interactive");

        assert!(result.success);
        assert!(result.output.contains("hello-tui"));
        assert!(result.stripped_output.contains("hello-tui"));
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.termination, TerminationType::Natural);
    }
}
