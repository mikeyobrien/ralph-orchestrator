//! CLI executor for running prompts through backends.
//!
//! Executes prompts via CLI tools with real-time streaming output.
//! Supports optional execution timeout with graceful SIGTERM termination.

#[cfg(test)]
use crate::cli_backend::PromptMode;
use crate::cli_backend::{CliBackend, OutputFormat};
use crate::copilot_stream::CopilotStreamParser;
use crate::pi_family::PiFamilySessionState;
use crate::stream_handler::{SessionResult, StreamHandler};
#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;
use std::env;
use std::io::Write;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, warn};

const TEXT_POST_EVENT_GRACE_TIMEOUT: Duration = Duration::from_secs(5);
const TERMINATION_GRACE_TIMEOUT: Duration = Duration::from_secs(2);

/// Result of a CLI execution.
#[derive(Debug)]
pub struct ExecutionResult {
    /// The full output from the CLI.
    pub output: String,
    /// Whether the execution succeeded (exit code 0).
    pub success: bool,
    /// The exit code.
    pub exit_code: Option<i32>,
    /// Whether the execution was terminated due to timeout.
    pub timed_out: bool,
    /// Whether the post-event grace deadline caused the executor to terminate
    /// the process instead of the normal inactivity timeout.
    pub post_event_grace_expired: bool,
    /// Parsed assistant text for Pi-family NDJSON backends (the processor owns
    /// extraction; the mandatory final-text fallback is already applied). `None`
    /// for backends that keep using raw/normalized output (Claude stream-json,
    /// Copilot, Text).
    pub extracted_text: Option<String>,
    /// Structured session result (metrics + merged `is_error`) for Pi-family
    /// NDJSON backends. `None` for backends that do not parse a session.
    pub session_result: Option<SessionResult>,
    /// Protocol-mismatch error surfaced when a successful Pi-family process
    /// produced no usable signal or no recoverable assistant text. `None`
    /// otherwise. When `Some`, `success` is `false` (exit code is preserved).
    pub protocol_error: Option<String>,
}

/// Executor for running prompts through CLI backends.
#[derive(Debug)]
pub struct CliExecutor {
    backend: CliBackend,
}

enum StreamEvent {
    StdoutLine(String),
    StderrLine(String),
    StdoutEof,
    StderrEof,
}

enum StreamKind {
    Stdout,
    Stderr,
}

impl CliExecutor {
    /// Creates a new executor with the given backend.
    pub fn new(backend: CliBackend) -> Self {
        Self { backend }
    }

    /// Executes a prompt and streams output to the provided writer.
    ///
    /// Output is streamed line-by-line to the writer while being accumulated
    /// for the return value. If `timeout` is provided and the execution produces
    /// no stdout/stderr activity for longer than that duration, the process
    /// receives SIGTERM and the result indicates timeout.
    ///
    /// When `verbose` is true, stderr output is also written to the output writer
    /// with a `[stderr]` prefix. When false, stderr is captured but not displayed.
    pub async fn execute<W: Write + Send>(
        &self,
        prompt: &str,
        mut output_writer: W,
        timeout: Option<Duration>,
        verbose: bool,
    ) -> std::io::Result<ExecutionResult> {
        // Note: _temp_file is kept alive for the duration of this function scope.
        // Some Arg-mode backends use temp-file indirection for very large prompts.
        let (cmd, args, stdin_input, _temp_file) = self.backend.build_command(prompt, false);

        let mut command = Command::new(&cmd);
        command.args(&args);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        #[cfg(unix)]
        command.process_group(0);

        // Set working directory to current directory (mirrors PTY executor behavior)
        // Use fallback to "." if current_dir fails (e.g., E2E test workspaces)
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        command.current_dir(&cwd);
        inject_ralph_runtime_env(&mut command, &cwd);

        // Apply backend-specific environment variables (e.g., Agent Teams env var)
        command.envs(self.backend.env_vars.iter().map(|(k, v)| (k, v)));

        debug!(
            command = %cmd,
            args = ?args,
            cwd = ?cwd,
            "Spawning CLI command"
        );

        if stdin_input.is_some() {
            command.stdin(Stdio::piped());
        }

        let mut child = command.spawn()?;

        // Write to stdin if needed. Some short-lived commands can exit before
        // consuming stdin, which surfaces as BrokenPipe. Treat that as benign
        // and continue collecting output/exit status from the child.
        if let Some(input) = stdin_input
            && let Some(mut stdin) = child.stdin.take()
        {
            if let Err(err) = stdin.write_all(input.as_bytes()).await
                && err.kind() != std::io::ErrorKind::BrokenPipe
            {
                return Err(err);
            }
            drop(stdin); // Close stdin to signal EOF
        }

        let mut timed_out = false;
        let mut post_event_grace_expired = false;
        let mut post_event_deadline: Option<tokio::time::Instant> = None;
        let mut terminated_status = None;

        // Take both stdout and stderr handles upfront to read concurrently.
        // Each emitted line resets the inactivity timeout.
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(256);

        let stdout_task = stdout_handle.map(|stdout| {
            let tx = event_tx.clone();
            tokio::spawn(async move { read_stream(stdout, tx, StreamKind::Stdout).await })
        });
        let stderr_task = stderr_handle.map(|stderr| {
            let tx = event_tx.clone();
            tokio::spawn(async move { read_stream(stderr, tx, StreamKind::Stderr).await })
        });
        drop(event_tx);

        let mut stdout_done = stdout_task.is_none();
        let mut stderr_done = stderr_task.is_none();
        let mut accumulated_output = String::new();

        // Pi-family NDJSON is routed through the shared processor so the
        // user-facing writer receives readable assistant text (not raw JSON)
        // while `accumulated_output` still stores the raw stream for loop
        // post-processing. The processor owns extracted text, metrics, stream
        // counts, and the two-case protocol-mismatch check (applied at
        // finalization).
        let is_pi_family_stream = matches!(
            self.backend.output_format,
            OutputFormat::PiStreamJson | OutputFormat::OmpStreamJson
        );
        let mut family_state = PiFamilySessionState::new();
        // OMP diagnostics say OMP even though parsing is shared (design Q1/TR9).
        family_state.flavor_label =
            if matches!(self.backend.output_format, OutputFormat::OmpStreamJson) {
                "OMP"
            } else {
                "Pi"
            };
        let start_time = Instant::now();

        if let Some(duration) = timeout {
            debug!(
                timeout_secs = duration.as_secs(),
                "Executing with inactivity timeout"
            );
        }

        while !stdout_done || !stderr_done {
            let now = tokio::time::Instant::now();
            let (effective_timeout, timeout_is_post_event) = match (timeout, post_event_deadline) {
                (_, Some(deadline)) if deadline <= now => (Some(Duration::ZERO), true),
                (Some(duration), Some(deadline)) => {
                    let post_event_remaining = deadline.saturating_duration_since(now);
                    if post_event_remaining <= duration {
                        (Some(post_event_remaining), true)
                    } else {
                        (Some(duration), false)
                    }
                }
                (None, Some(deadline)) => (Some(deadline.saturating_duration_since(now)), true),
                (Some(duration), None) => (Some(duration), false),
                (None, None) => (None, false),
            };

            let next_event = match effective_timeout {
                Some(duration) => match tokio::time::timeout(duration, event_rx.recv()).await {
                    Ok(event) => event,
                    Err(_) => {
                        warn!(
                            timeout_secs = duration.as_secs(),
                            "Execution inactivity timeout reached, sending SIGTERM"
                        );
                        timed_out = true;
                        post_event_grace_expired = timeout_is_post_event;
                        terminated_status = Some(Self::terminate_child_and_wait(&mut child).await?);
                        break;
                    }
                },
                None => event_rx.recv().await,
            };

            match next_event {
                Some(StreamEvent::StdoutLine(line)) => {
                    if self.backend.output_format == OutputFormat::Text
                        && line_signals_event_emitted(&line)
                    {
                        post_event_deadline.get_or_insert_with(|| {
                            tokio::time::Instant::now() + TEXT_POST_EVENT_GRACE_TIMEOUT
                        });
                    }
                    if self.backend.output_format == OutputFormat::CopilotStreamJson {
                        if let Some(text) = CopilotStreamParser::extract_text(&line) {
                            write!(output_writer, "{text}")?;
                            if !text.ends_with('\n') {
                                writeln!(output_writer)?;
                            }
                        }
                    } else if is_pi_family_stream {
                        // Pi-family: route through the shared processor with a
                        // writer-backed handler so the user sees readable text
                        // (mirrors the Copilot arm). Raw NDJSON is still
                        // accumulated below for loop post-processing.
                        let mut handler = WriterStreamHandler::new(&mut output_writer);
                        family_state.process_line(&line, &mut handler, verbose);
                    } else {
                        writeln!(output_writer, "{line}")?;
                    }
                    output_writer.flush()?;
                    accumulated_output.push_str(&line);
                    accumulated_output.push('\n');
                }
                Some(StreamEvent::StderrLine(line)) => {
                    if self.backend.output_format == OutputFormat::Text
                        && line_signals_event_emitted(&line)
                    {
                        post_event_deadline.get_or_insert_with(|| {
                            tokio::time::Instant::now() + TEXT_POST_EVENT_GRACE_TIMEOUT
                        });
                    }
                    if verbose {
                        writeln!(output_writer, "[stderr] {line}")?;
                        output_writer.flush()?;
                    }
                    accumulated_output.push_str("[stderr] ");
                    accumulated_output.push_str(&line);
                    accumulated_output.push('\n');
                }
                Some(StreamEvent::StdoutEof) => stdout_done = true,
                Some(StreamEvent::StderrEof) => stderr_done = true,
                None => {
                    stdout_done = true;
                    stderr_done = true;
                }
            }
        }

        let status = if let Some(status) = terminated_status {
            status
        } else {
            child.wait().await?
        };

        if let Some(handle) = stdout_task {
            handle.await.map_err(join_error_to_io)??;
        }
        if let Some(handle) = stderr_task {
            handle.await.map_err(join_error_to_io)??;
        }

        // Finalize the Pi-family session — applies the mandatory final-text
        // fallback and the two-case protocol-mismatch check — and surface the
        // structured result. Non-Pi backends leave these fields `None`.
        let (success, extracted_text, session_result, protocol_error) = if is_pi_family_stream {
            let process_success = status.success() && !timed_out;
            // Snapshot the delta-only text before `finalize` applies the
            // fallback, so recovered text is streamed to the writer only when
            // deltas were swallowed (never double-written).
            let delta_only = family_state.extracted_text().to_string();
            let summary = family_state.finalize(process_success, start_time.elapsed());
            if delta_only.is_empty() && !summary.extracted_text.is_empty() {
                write!(output_writer, "{}", summary.extracted_text)?;
                output_writer.flush()?;
            }
            // Surface a protocol mismatch as a visible, actionable error so a
            // successful-exit-but-empty/garbage stream is not a silent failure.
            if let Some(ref reason) = summary.protocol_error {
                writeln!(output_writer, "[Error] {reason}")?;
                output_writer.flush()?;
            }
            let session_result = summary.session_result;
            let success = !session_result.is_error;
            (
                success,
                Some(summary.extracted_text),
                Some(session_result),
                summary.protocol_error,
            )
        } else {
            (status.success() && !timed_out, None, None, None)
        };

        Ok(ExecutionResult {
            output: accumulated_output,
            success,
            exit_code: status.code(),
            timed_out,
            post_event_grace_expired,
            extracted_text,
            session_result,
            protocol_error,
        })
    }

    /// Terminates the child process with SIGTERM, then SIGKILL if it ignores graceful shutdown.
    async fn terminate_child_and_wait(
        child: &mut Child,
    ) -> std::io::Result<std::process::ExitStatus> {
        #[cfg(not(unix))]
        {
            child.start_kill()?;
            return child.wait().await;
        }

        #[cfg(unix)]
        if let Some(pid) = child.id() {
            #[allow(clippy::cast_possible_wrap)]
            let pid = Pid::from_raw(pid as i32);
            let pgid = Pid::from_raw(-pid.as_raw());
            debug!(%pid, "Sending SIGTERM to child process group");
            let _ = kill(pgid, Signal::SIGTERM);
            match tokio::time::timeout(TERMINATION_GRACE_TIMEOUT, child.wait()).await {
                Ok(status) => status,
                Err(_) => {
                    warn!(%pid, "Child process ignored SIGTERM, sending SIGKILL");
                    let _ = kill(pgid, Signal::SIGKILL);
                    child.wait().await
                }
            }
        } else {
            child.wait().await
        }
    }

    /// Executes a prompt without streaming (captures all output).
    ///
    /// Uses no timeout by default. For timed execution, use `execute_capture_with_timeout`.
    pub async fn execute_capture(&self, prompt: &str) -> std::io::Result<ExecutionResult> {
        self.execute_capture_with_timeout(prompt, None).await
    }

    /// Executes a prompt without streaming, with optional timeout.
    pub async fn execute_capture_with_timeout(
        &self,
        prompt: &str,
        timeout: Option<Duration>,
    ) -> std::io::Result<ExecutionResult> {
        // Use a sink that discards output for non-streaming execution
        // verbose=false since output is being discarded anyway
        let sink = std::io::sink();
        self.execute(prompt, sink, timeout, false).await
    }
}

fn line_signals_event_emitted(line: &str) -> bool {
    line.contains("Event emitted:")
}

async fn read_stream<R>(
    stream: R,
    tx: tokio::sync::mpsc::Sender<StreamEvent>,
    stream_kind: StreamKind,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        let event = match stream_kind {
            StreamKind::Stdout => StreamEvent::StdoutLine(line),
            StreamKind::Stderr => StreamEvent::StderrLine(line),
        };
        if tx.send(event).await.is_err() {
            return Ok(());
        }
    }

    let eof_event = match stream_kind {
        StreamKind::Stdout => StreamEvent::StdoutEof,
        StreamKind::Stderr => StreamEvent::StderrEof,
    };
    let _ = tx.send(eof_event).await;
    Ok(())
}

fn join_error_to_io(error: tokio::task::JoinError) -> std::io::Error {
    std::io::Error::other(error.to_string())
}

fn inject_ralph_runtime_env(command: &mut Command, workspace_root: &std::path::Path) {
    let Ok(current_exe) = env::current_exe() else {
        return;
    };
    let Some(bin_dir) = current_exe.parent() else {
        return;
    };

    let mut path_entries = vec![bin_dir.to_path_buf()];
    if let Some(existing_path) = env::var_os("PATH") {
        path_entries.extend(env::split_paths(&existing_path));
    }

    if let Ok(joined_path) = env::join_paths(path_entries) {
        command.env("PATH", joined_path);
    }
    command.env("RALPH_BIN", &current_exe);
    command.env("RALPH_WORKSPACE_ROOT", workspace_root);

    // Propagate RALPH_EVENTS_FILE so `ralph emit` from any CWD writes to the correct events file
    let marker = workspace_root.join(".ralph/current-events");
    if let Ok(relative) = std::fs::read_to_string(&marker) {
        let abs = workspace_root.join(relative.trim());
        command.env("RALPH_EVENTS_FILE", &abs);
    }

    if std::path::Path::new("/var/tmp").is_dir() {
        command.env("TMPDIR", "/var/tmp");
        command.env("TMP", "/var/tmp");
        command.env("TEMP", "/var/tmp");
    }
}

/// `StreamHandler` that renders Pi-family events to an arbitrary writer as
/// readable text.
///
/// Used by the no-TUI [`CliExecutor`] so Pi-family NDJSON streams as assistant
/// text (plus tool/error lines) instead of raw JSON. The processor owns the
/// accumulated `extracted_text`; this handler only mirrors readable output to
/// the writer. Write errors are ignored — the writer may be a sink
/// (`execute_capture`) and must never abort the stream.
struct WriterStreamHandler<'a, W: Write> {
    writer: &'a mut W,
}

impl<'a, W: Write> WriterStreamHandler<'a, W> {
    fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }
}

impl<W: Write + Send> StreamHandler for WriterStreamHandler<'_, W> {
    fn on_text(&mut self, text: &str) {
        let _ = self.writer.write_all(text.as_bytes());
    }

    fn on_tool_call(&mut self, name: &str, _id: &str, input: &serde_json::Value) {
        match crate::tool_preview::format_tool_summary(name, input) {
            Some(summary) => {
                let _ = writeln!(self.writer, "[Tool] {name}: {summary}");
            }
            None => {
                let _ = writeln!(self.writer, "[Tool] {name}");
            }
        }
    }

    fn on_tool_result(&mut self, _id: &str, output: &str) {
        let display = crate::tool_preview::format_tool_result(output);
        if !display.is_empty() {
            let _ = writeln!(self.writer, "[Result] {display}");
        }
    }

    fn on_error(&mut self, error: &str) {
        let _ = writeln!(self.writer, "[Error] {error}");
    }

    fn on_complete(&mut self, _result: &SessionResult) {
        // The no-TUI executor builds the SessionResult via the processor's
        // `finalize()`; nothing to render here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_echo() {
        // Use echo as a simple test backend
        let backend = CliBackend {
            command: "echo".to_string(),
            args: vec![],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();

        let result = executor
            .execute("hello world", &mut output, None, true)
            .await
            .unwrap();

        assert!(result.success);
        assert!(!result.timed_out);
        assert!(result.output.contains("hello world"));
    }

    #[tokio::test]
    async fn test_execute_stdin() {
        // Use cat to test stdin mode
        let backend = CliBackend {
            command: "cat".to_string(),
            args: vec![],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let result = executor.execute_capture("stdin test").await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("stdin test"));
    }

    #[tokio::test]
    async fn test_execute_failure() {
        let backend = CliBackend {
            command: "false".to_string(), // Always exits with code 1
            args: vec![],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let result = executor.execute_capture("").await.unwrap();

        assert!(!result.success);
        assert!(!result.timed_out);
        assert_eq!(result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        // Use sleep to test timeout behavior
        // The sleep command ignores stdin, so we use PromptMode::Stdin
        // to avoid appending the prompt as an argument
        let backend = CliBackend {
            command: "sleep".to_string(),
            args: vec!["10".to_string()],   // Sleep for 10 seconds
            prompt_mode: PromptMode::Stdin, // Use stdin mode so prompt doesn't interfere
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);

        // Execute with a 100ms timeout - should trigger timeout
        let timeout = Some(Duration::from_millis(100));
        let result = executor
            .execute_capture_with_timeout("", timeout)
            .await
            .unwrap();

        assert!(result.timed_out, "Expected execution to time out");
        assert!(
            !result.post_event_grace_expired,
            "Normal inactivity timeout must not be classified as post-event grace expiry"
        );
        assert!(
            !result.success,
            "Timed out execution should not be successful"
        );
    }

    #[tokio::test]
    async fn test_execute_timeout_resets_on_output_activity() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let timeout = Some(Duration::from_millis(500));
        let result = executor
            .execute_capture_with_timeout(
                "printf 'start\\n'; sleep 0.1; printf 'middle\\n'; sleep 0.1; printf 'done\\n'",
                timeout,
            )
            .await
            .unwrap();

        assert!(
            !result.timed_out,
            "Periodic output should reset the inactivity timeout"
        );
        assert!(result.success, "Periodic-output command should succeed");
        assert!(result.output.contains("start"));
        assert!(result.output.contains("middle"));
        assert!(result.output.contains("done"));
    }

    #[tokio::test]
    async fn test_execute_streams_output_before_inactivity_timeout() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "printf 'hello\\n'; sleep 10".to_string()],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();
        let result = executor
            .execute("", &mut output, Some(Duration::from_millis(200)), false)
            .await
            .unwrap();

        assert!(
            result.timed_out,
            "Expected inactivity timeout after output stops"
        );
        assert_eq!(String::from_utf8(output).unwrap(), "hello\n");
        assert!(result.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_timeout_force_kills_processes_that_ignore_sigterm() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "trap '' TERM; while :; do sleep 1; done".to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let started = std::time::Instant::now();
        let result = executor
            .execute_capture_with_timeout("", Some(Duration::from_millis(100)))
            .await
            .unwrap();

        assert!(
            result.timed_out,
            "Expected ignored-SIGTERM command to time out"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "Executor should force-kill ignored-SIGTERM processes instead of hanging"
        );
    }

    #[tokio::test]
    async fn test_execute_uses_short_post_event_grace_timeout() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'Event emitted: task.done\\n'; sleep 30".to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let started = std::time::Instant::now();
        let result = executor
            .execute_capture_with_timeout("", Some(Duration::from_secs(30)))
            .await
            .unwrap();

        assert!(
            result.timed_out,
            "Expected lingering post-event process to be terminated"
        );
        assert!(
            result.post_event_grace_expired,
            "Expected termination to be attributed to the post-event grace deadline"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "Event-emitting backends should use the short post-event grace timeout instead of the full inactivity timeout"
        );
        assert!(result.output.contains("Event emitted: task.done"));
    }

    #[tokio::test]
    async fn test_execute_short_inactivity_timeout_wins_over_post_event_grace() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'Event emitted: task.done\\n'; sleep 10".to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let result = executor
            .execute_capture_with_timeout("", Some(Duration::from_millis(100)))
            .await
            .unwrap();

        assert!(result.timed_out);
        assert!(
            !result.post_event_grace_expired,
            "The shorter inactivity timeout must win over the post-event grace deadline"
        );
    }

    #[tokio::test]
    async fn test_execute_post_event_deadline_does_not_reset_on_output_activity() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'Event emitted: task.done\\n'; while :; do printf 'heartbeat\\n'; sleep 1; done"
                    .to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let started = std::time::Instant::now();
        let result = executor
            .execute_capture_with_timeout("", Some(Duration::from_secs(30)))
            .await
            .unwrap();

        assert!(
            result.timed_out,
            "Expected noisy post-event process to be terminated"
        );
        assert!(
            result.post_event_grace_expired,
            "Expected the fixed post-event grace deadline to terminate the noisy process"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "Event-emitting backends should respect the fixed post-event grace deadline even if they keep producing output"
        );
        assert!(result.output.contains("Event emitted: task.done"));
        assert!(result.output.contains("heartbeat"));
    }

    #[tokio::test]
    async fn test_execute_claude_stream_waits_for_result_after_final_assistant_message() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                r#"printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"Event emitted: task.done"}]}}'; sleep 6; printf '%s\n' '{"type":"result","duration_ms":6000,"total_cost_usd":0.01,"num_turns":1,"is_error":false}'"#.to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::StreamJson,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let result = executor
            .execute_capture_with_timeout("", Some(Duration::from_secs(10)))
            .await
            .unwrap();

        assert!(
            !result.timed_out,
            "Claude stream should receive its terminal result event during the post-assistant quiet window"
        );
        assert!(result.success, "Claude stream should exit successfully");
        assert!(result.output.contains(r#"{"type":"result""#));
    }

    #[tokio::test]
    async fn test_execute_claude_stream_still_times_out_without_result() {
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                r#"printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"Event emitted: task.done"}]}}'; sleep 10"#.to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::StreamJson,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let result = executor
            .execute_capture_with_timeout("", Some(Duration::from_millis(200)))
            .await
            .unwrap();

        assert!(
            result.timed_out,
            "Claude stream without a result event should retain inactivity timeout protection"
        );
        assert!(!result.success, "Timed-out Claude stream must fail");
    }

    #[tokio::test]
    async fn test_execute_no_timeout_when_fast() {
        // Use echo which completes immediately
        let backend = CliBackend {
            command: "echo".to_string(),
            args: vec![],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);

        // Execute with a generous timeout - should complete before timeout
        let timeout = Some(Duration::from_secs(10));
        let result = executor
            .execute_capture_with_timeout("fast", timeout)
            .await
            .unwrap();

        assert!(!result.timed_out, "Fast command should not time out");
        assert!(result.success);
        assert!(result.output.contains("fast"));
    }

    #[tokio::test]
    async fn test_execute_copilot_stream_writes_extracted_text() {
        let backend = CliBackend {
            command: "printf".to_string(),
            args: vec![
                "%s\n%s\n".to_string(),
                r#"{"type":"assistant.turn_start","data":{"turnId":"0"}}"#.to_string(),
                r#"{"type":"assistant.message","data":{"content":"hello from copilot"}}"#
                    .to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::CopilotStreamJson,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();

        let result = executor
            .execute("ignored", &mut output, None, false)
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("\"assistant.message\""));
        assert_eq!(String::from_utf8(output).unwrap(), "hello from copilot\n");
    }

    #[tokio::test]
    async fn test_execute_pi_stream_writes_extracted_text() {
        // Pi-family NDJSON routed through the shared processor. The user-facing
        // writer must receive readable assistant text (not raw JSON), the
        // accumulated output must keep the raw NDJSON for loop post-processing,
        // and the structured result must carry the parsed text + metrics.
        let backend = CliBackend {
            command: "printf".to_string(),
            args: vec![
                "%s\n%s\n%s\n".to_string(),
                r#"{"type":"session","version":3}"#.to_string(),
                r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"hello from pi"}}"#
                    .to_string(),
                r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop"}}"#.to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::PiStreamJson,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();

        let result = executor
            .execute("ignored", &mut output, None, false)
            .await
            .unwrap();

        assert!(result.success, "clean Pi stream should succeed");
        // accumulated_output keeps the raw NDJSON (loop post-processes it).
        assert!(
            result.output.contains("text_delta"),
            "raw NDJSON must be retained in output"
        );
        // The user-facing writer gets readable text only — no raw JSON envelope.
        let written = String::from_utf8(output).unwrap();
        assert_eq!(written, "hello from pi");
        assert!(
            !written.contains("assistantMessageEvent"),
            "no raw JSON envelope in user-facing output"
        );
        // Structured result carries parsed text + metrics from the processor.
        assert_eq!(result.extracted_text, Some("hello from pi".to_string()));
        let session = result
            .session_result
            .expect("Pi stream must produce a structured session result");
        assert!(!session.is_error);
        assert!(
            result.protocol_error.is_none(),
            "clean stream must not surface a protocol error"
        );
    }

    #[tokio::test]
    async fn test_execute_pi_stream_protocol_mismatch_no_usable_events() {
        // A successful Pi process that emits only header/unknown records must
        // NOT be a silent empty success. It surfaces a protocol error (case 1)
        // with success flipped to false and the exit code preserved.
        let backend = CliBackend {
            command: "printf".to_string(),
            args: vec![
                "%s\n%s\n".to_string(),
                r#"{"type":"session","version":3}"#.to_string(),
                r#"{"type":"agent_start"}"#.to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::PiStreamJson,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();

        let result = executor
            .execute("ignored", &mut output, None, false)
            .await
            .unwrap();

        // printf exits 0, but the protocol mismatch flips success to false.
        assert!(
            !result.success,
            "header-only stream must not be a silent success"
        );
        assert_eq!(result.exit_code, Some(0), "exit code is preserved");
        let session = result
            .session_result
            .expect("Pi stream must still produce a structured session result");
        assert!(session.is_error);
        let reason = result
            .protocol_error
            .expect("must surface a protocol-mismatch reason");
        assert!(
            reason.contains("no usable"),
            "case-1 wording must be actionable: {reason}"
        );
        // The actionable reason is mirrored to the user-facing writer.
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("protocol mismatch"),
            "mismatch reason must be visible in user-facing output"
        );
    }

    #[tokio::test]
    async fn test_execute_omp_stream_writes_extracted_text() {
        // OMP routes through the same shared Pi-family processor as Pi (TR6).
        // The user-facing writer must receive readable assistant text (not raw
        // JSON), the accumulated output keeps the raw NDJSON, and the structured
        // result carries parsed text + metrics. `isError` is omitted on the tool
        // end (OMP-optional, defaults false) and `agent_end` is ignored.
        let backend = CliBackend {
            command: "printf".to_string(),
            args: vec![
                "%s\n%s\n%s\n%s\n".to_string(),
                r#"{"type":"session","version":3}"#.to_string(),
                r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"hello from omp"}}"#
                    .to_string(),
                r#"{"type":"turn_end","message":{"content":[{"type":"text","text":"hello from omp"}],"stopReason":"stop","usage":{"input":9,"output":8,"cacheRead":2,"cacheWrite":1,"cost":{"total":0.04}}}}"#
                    .to_string(),
                r#"{"type":"agent_end"}"#.to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();

        let result = executor
            .execute("ignored", &mut output, None, false)
            .await
            .unwrap();

        assert!(result.success, "clean OMP stream should succeed");
        // Raw NDJSON retained for loop post-processing.
        assert!(
            result.output.contains("text_delta"),
            "raw NDJSON must be retained in output"
        );
        assert!(
            result.output.contains("agent_end"),
            "OMP agent_end record is retained in the raw stream"
        );
        // User-facing writer gets readable text only — no raw JSON envelope.
        let written = String::from_utf8(output).unwrap();
        assert_eq!(written, "hello from omp");
        assert!(
            !written.contains("assistantMessageEvent"),
            "no raw JSON envelope in user-facing output"
        );
        // Structured result carries parsed text + metrics.
        assert_eq!(result.extracted_text, Some("hello from omp".to_string()));
        let session = result
            .session_result
            .expect("OMP stream must produce a structured session result");
        assert!(!session.is_error, "OMP isError defaults false");
        assert_eq!(session.input_tokens, 11); // peak: input(9) + cacheRead(2)
        assert_eq!(session.output_tokens, 8);
        assert_eq!(session.cache_read_tokens, 2);
        assert_eq!(session.cache_write_tokens, 1);
        assert!((session.total_cost_usd - 0.04).abs() < 1e-10);
        assert!(
            result.protocol_error.is_none(),
            "clean OMP stream must not surface a protocol error"
        );
    }

    #[tokio::test]
    async fn test_execute_omp_stream_protocol_mismatch_no_usable_events() {
        // OMP shares the two-case mismatch check. A successful process emitting
        // only lifecycle/header records must NOT be a silent success.
        let backend = CliBackend {
            command: "printf".to_string(),
            args: vec![
                "%s\n%s\n".to_string(),
                r#"{"type":"session","version":3}"#.to_string(),
                r#"{"type":"agent_start"}"#.to_string(),
                r#"{"type":"agent_end"}"#.to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();

        let result = executor
            .execute("ignored", &mut output, None, false)
            .await
            .unwrap();

        assert!(
            !result.success,
            "OMP header-only stream must not be a silent success"
        );
        assert_eq!(result.exit_code, Some(0), "exit code is preserved");
        let reason = result
            .protocol_error
            .expect("must surface an OMP protocol-mismatch reason");
        assert!(reason.contains("no usable"), "case-1 wording: {reason}");
        // Design Q1 / TR9: OMP diagnostics must say OMP (shared parser, distinct label).
        assert!(
            reason.contains("OMP"),
            "OMP mismatch must be OMP-labelled: {reason}"
        );
    }
}
