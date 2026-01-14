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

use crate::cli_backend::CliBackend;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Result of a PTY execution.
#[derive(Debug)]
pub struct PtyExecutionResult {
    /// The accumulated output (ANSI sequences preserved).
    pub output: String,
    /// The ANSI-stripped output for event parsing.
    pub stripped_output: String,
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
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            interactive: true,
            idle_timeout_secs: 30,
            cols: 80,
            rows: 24,
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
}

impl PtyExecutor {
    /// Creates a new PTY executor with the given backend and configuration.
    pub fn new(backend: CliBackend, config: PtyConfig) -> Self {
        Self { backend, config }
    }

    /// Spawns Claude in a PTY and returns the PTY pair and child process.
    fn spawn_pty(&self, prompt: &str) -> io::Result<(PtyPair, Box<dyn portable_pty::Child + Send>)> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: self.config.rows,
                cols: self.config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::other(e.to_string()))?;

        let (cmd, args, stdin_input, _temp_file) = self.backend.build_command(prompt, self.config.interactive);

        let mut cmd_builder = CommandBuilder::new(&cmd);
        cmd_builder.args(&args);

        // Set explicit working directory
        let cwd = std::env::current_dir()
            .map_err(|e| io::Error::other(format!("Failed to get current directory: {}", e)))?;
        cmd_builder.cwd(&cwd);

        // Set up environment for PTY
        cmd_builder.env("TERM", "xterm-256color");

        debug!(
            command = %cmd,
            args = ?args,
            cwd = ?cwd,
            cols = self.config.cols,
            rows = self.config.rows,
            "Spawning process in PTY"
        );

        let child = pair
            .slave
            .spawn_command(cmd_builder)
            .map_err(|e| io::Error::other(e.to_string()))?;

        // If we need to write to stdin, do it now
        if let Some(input) = stdin_input {
            let mut writer = pair.master.take_writer()
                .map_err(|e| io::Error::other(e.to_string()))?;
            writer.write_all(input.as_bytes())?;
        }

        Ok((pair, child))
    }

    /// Runs in observe mode (output-only, no input forwarding).
    ///
    /// Returns when the process exits or idle timeout triggers.
    ///
    /// # Errors
    ///
    /// Returns an error if PTY allocation fails, the command cannot be spawned,
    /// or an I/O error occurs during output handling.
    pub fn run_observe(&self, prompt: &str) -> io::Result<PtyExecutionResult> {
        let (pair, mut child) = self.spawn_pty(prompt)?;

        let mut reader = pair.master.try_clone_reader()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Drop the slave to signal EOF when master closes
        drop(pair.slave);

        let mut output = Vec::new();
        let mut buf = [0u8; 4096];
        let mut last_activity = Instant::now();
        let timeout = if self.config.idle_timeout_secs > 0 {
            Some(Duration::from_secs(u64::from(self.config.idle_timeout_secs)))
        } else {
            None
        };

        let mut termination = TerminationType::Natural;

        loop {
            // Check if child has exited
            if let Some(status) = child.try_wait()
                .map_err(|e| io::Error::other(e.to_string()))?
            {
                debug!(exit_status = ?status, "Child process exited");
                // Drain any remaining output
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            io::stdout().write_all(&buf[..n])?;
                            io::stdout().flush()?;
                            output.extend_from_slice(&buf[..n]);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    }
                }

                let exit_code = status.exit_code();
                let success = status.success();

                return Ok(PtyExecutionResult {
                    output: String::from_utf8_lossy(&output).to_string(),
                    stripped_output: strip_ansi(&output),
                    success,
                    exit_code: Some(exit_code as i32),
                    termination,
                });
            }

            // Check idle timeout
            if let Some(timeout_duration) = timeout {
                if last_activity.elapsed() > timeout_duration {
                    warn!(
                        timeout_secs = self.config.idle_timeout_secs,
                        "Idle timeout triggered"
                    );
                    termination = TerminationType::IdleTimeout;
                    self.terminate_child(&mut child, true)?;
                    break;
                }
            }

            // Read output (non-blocking would be ideal, but we use small timeout)
            // For simplicity, we do blocking reads with a timeout check
            match reader.read(&mut buf) {
                Ok(0) => {
                    // EOF - process likely exited
                    break;
                }
                Ok(n) => {
                    // Write to stdout in real-time
                    io::stdout().write_all(&buf[..n])?;
                    io::stdout().flush()?;

                    // Accumulate for return
                    output.extend_from_slice(&buf[..n]);

                    // Reset activity timer
                    last_activity = Instant::now();
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No data available, sleep briefly
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    // Real error or EOF
                    debug!(error = %e, "PTY read error");
                    break;
                }
            }
        }

        // Wait for child to fully exit
        let status = child.wait()
            .map_err(|e| io::Error::other(e.to_string()))?;

        Ok(PtyExecutionResult {
            output: String::from_utf8_lossy(&output).to_string(),
            stripped_output: strip_ansi(&output),
            success: status.success(),
            exit_code: Some(status.exit_code() as i32),
            termination,
        })
    }

    /// Runs in interactive mode (bidirectional I/O).
    ///
    /// Uses `tokio::select!` for non-blocking I/O multiplexing between:
    /// 1. PTY output (from blocking reader via channel)
    /// 2. User input (from stdin thread via channel)
    /// 3. Idle timeout
    ///
    /// This design ensures Ctrl+C is always responsive, even when the PTY
    /// has no output (e.g., during long-running tool calls).
    ///
    /// # Errors
    ///
    /// Returns an error if PTY allocation fails, the command cannot be spawned,
    /// or an I/O error occurs during bidirectional communication.
    #[allow(clippy::too_many_lines)] // Complex state machine requires cohesive implementation
    pub async fn run_interactive(&self, prompt: &str) -> io::Result<PtyExecutionResult> {
        let (pair, mut child) = self.spawn_pty(prompt)?;

        let reader = pair.master.try_clone_reader()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let mut writer = pair.master.take_writer()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Drop the slave to signal EOF when master closes
        drop(pair.slave);

        let mut output = Vec::new();
        let timeout_duration = if self.config.idle_timeout_secs > 0 {
            Some(Duration::from_secs(u64::from(self.config.idle_timeout_secs)))
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

        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];

            loop {
                if should_terminate_output.load(Ordering::SeqCst) {
                    break;
                }

                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF - PTY closed
                        let _ = output_tx.blocking_send(OutputEvent::Eof);
                        break;
                    }
                    Ok(n) => {
                        if output_tx.blocking_send(OutputEvent::Data(buf[..n].to_vec())).is_err() {
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
                        let _ = output_tx.blocking_send(OutputEvent::Error(e.to_string()));
                        break;
                    }
                }
            }
        });

        // Spawn input reading task
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<InputEvent>();
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
                            3 => InputEvent::CtrlC,           // Ctrl+C
                            28 => InputEvent::CtrlBackslash,  // Ctrl+\
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

        // Main select loop - this is the key fix for blocking I/O
        // We use tokio::select! to multiplex between output, input, and timeout
        loop {
            // Check if child has exited (non-blocking check before select)
            if let Some(status) = child.try_wait()
                .map_err(|e| io::Error::other(e.to_string()))?
            {
                debug!(exit_status = ?status, "Child process exited");

                // Drain remaining output from channel
                while let Ok(event) = output_rx.try_recv() {
                    if let OutputEvent::Data(data) = event {
                        io::stdout().write_all(&data)?;
                        io::stdout().flush()?;
                        output.extend_from_slice(&data);
                    }
                }

                should_terminate.store(true, Ordering::SeqCst);

                return Ok(PtyExecutionResult {
                    output: String::from_utf8_lossy(&output).to_string(),
                    stripped_output: strip_ansi(&output),
                    success: status.success(),
                    exit_code: Some(status.exit_code() as i32),
                    termination,
                });
            }

            // Build the timeout future (or a never-completing one if disabled)
            let timeout_future = async {
                match timeout_duration {
                    Some(d) => {
                        let elapsed = last_activity.elapsed();
                        if elapsed >= d {
                            tokio::time::sleep(Duration::ZERO).await
                        } else {
                            tokio::time::sleep(d - elapsed).await
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
                            io::stdout().write_all(&data)?;
                            io::stdout().flush()?;
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

                // User input received
                input_event = async { input_rx.recv().await } => {
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
                                    self.terminate_child(&mut child, true)?;
                                    break;
                                }
                            }
                        }
                        Some(InputEvent::CtrlBackslash) => {
                            info!("Ctrl+\\ detected, force killing");
                            termination = TerminationType::ForceKill;
                            should_terminate.store(true, Ordering::SeqCst);
                            self.terminate_child(&mut child, false)?;
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

                // Idle timeout expired
                _ = timeout_future => {
                    warn!(
                        timeout_secs = self.config.idle_timeout_secs,
                        "Idle timeout triggered"
                    );
                    termination = TerminationType::IdleTimeout;
                    should_terminate.store(true, Ordering::SeqCst);
                    self.terminate_child(&mut child, true)?;
                    break;
                }
            }
        }

        // Ensure termination flag is set for spawned threads
        should_terminate.store(true, Ordering::SeqCst);

        // Wait for child to fully exit
        let status = child.wait()
            .map_err(|e| io::Error::other(e.to_string()))?;

        Ok(PtyExecutionResult {
            output: String::from_utf8_lossy(&output).to_string(),
            stripped_output: strip_ansi(&output),
            success: status.success(),
            exit_code: Some(status.exit_code() as i32),
            termination,
        })
    }

    /// Terminates the child process.
    ///
    /// If `graceful` is true, sends SIGTERM and waits up to 5 seconds before SIGKILL.
    /// If `graceful` is false, sends SIGKILL immediately.
    #[allow(clippy::unused_self)] // Self is conceptually the right receiver for this method
    fn terminate_child(&self, child: &mut Box<dyn portable_pty::Child + Send>, graceful: bool) -> io::Result<()> {
        let pid = match child.process_id() {
            Some(id) => Pid::from_raw(id as i32),
            None => return Ok(()), // Already exited
        };

        if graceful {
            debug!(pid = %pid, "Sending SIGTERM");
            let _ = kill(pid, Signal::SIGTERM);

            // Wait up to 5 seconds for graceful exit
            let grace_period = Duration::from_secs(5);
            let start = Instant::now();

            while start.elapsed() < grace_period {
                if child.try_wait()
                    .map_err(|e| io::Error::other(e.to_string()))?
                    .is_some()
                {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            // Still running after grace period - force kill
            debug!(pid = %pid, "Grace period expired, sending SIGKILL");
        }

        debug!(pid = %pid, "Sending SIGKILL");
        let _ = kill(pid, Signal::SIGKILL);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut term = vt100::Parser::new(24, 80, 10_000);

        // Simulate Claude output with heavy ANSI formatting
        term.process(b"\x1b[1;36m  Thinking...\x1b[0m\r\n");
        term.process(b"\x1b[2K\x1b[1;32m  Done!\x1b[0m\r\n");
        term.process(b"\x1b[33mLOOP_COMPLETE\x1b[0m\r\n");

        let stripped = term.screen().contents();

        // Event parser sees clean text
        assert!(stripped.contains("LOOP_COMPLETE"));
        assert!(!stripped.contains("\x1b["));
    }

    #[test]
    fn test_event_tag_extraction() {
        let mut term = vt100::Parser::new(24, 80, 10_000);

        // Event tags may be wrapped in ANSI codes
        term.process(b"\x1b[90m<event topic=\"build.done\">\x1b[0m\r\n");
        term.process(b"Task completed successfully\r\n");
        term.process(b"\x1b[90m</event>\x1b[0m\r\n");

        let stripped = term.screen().contents();

        assert!(stripped.contains("<event topic=\"build.done\">"));
        assert!(stripped.contains("</event>"));
    }

    #[test]
    fn test_large_output_preserves_early_events() {
        // Regression test: ensure event tags aren't lost when output exceeds terminal height
        let mut term = vt100::Parser::new(24, 80, 10_000);

        // Event tag at the beginning
        term.process(b"<event topic=\"build.task\">Implement feature X</event>\r\n");

        // Simulate 50 lines of verbose output (exceeds 24-line terminal)
        for i in 0..50 {
            term.process(format!("Line {}: Processing step {}...\r\n", i, i).as_bytes());
        }

        let stripped = term.screen().contents();

        // Event tag should still be present in scrollback
        assert!(
            stripped.contains("<event topic=\"build.task\">"),
            "Event tag was lost due to scrollback overflow"
        );
        assert!(stripped.contains("Implement feature X"));
    }

    #[test]
    fn test_pty_config_defaults() {
        let config = PtyConfig::default();
        assert!(config.interactive);
        assert_eq!(config.idle_timeout_secs, 30);
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
    }
}
