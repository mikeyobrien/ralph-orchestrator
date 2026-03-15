//! CLI executor for running prompts through backends.
//!
//! Executes prompts via CLI tools with real-time streaming output.
//! Supports optional execution timeout with graceful SIGTERM termination.

use crate::cli_backend::CliBackend;
#[cfg(test)]
use crate::cli_backend::{OutputFormat, PromptMode};
#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;
use std::env;
use std::io::Write;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{debug, warn};

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
        // For large prompts (>7000 chars), Claude reads from the temp file.
        let (cmd, args, stdin_input, _temp_file) = self.backend.build_command(prompt, false);

        let mut command = Command::new(&cmd);
        command.args(&args);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

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

        // Write to stdin if needed
        if let Some(input) = stdin_input
            && let Some(mut stdin) = child.stdin.take()
        {
            stdin.write_all(input.as_bytes()).await?;
            drop(stdin); // Close stdin to signal EOF
        }

        let mut timed_out = false;

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

        if let Some(duration) = timeout {
            debug!(
                timeout_secs = duration.as_secs(),
                "Executing with inactivity timeout"
            );
        }

        while !stdout_done || !stderr_done {
            let next_event = match timeout {
                Some(duration) => match tokio::time::timeout(duration, event_rx.recv()).await {
                    Ok(event) => event,
                    Err(_) => {
                        warn!(
                            timeout_secs = duration.as_secs(),
                            "Execution inactivity timeout reached, sending SIGTERM"
                        );
                        timed_out = true;
                        Self::terminate_child(&mut child)?;
                        break;
                    }
                },
                None => event_rx.recv().await,
            };

            match next_event {
                Some(StreamEvent::StdoutLine(line)) => {
                    writeln!(output_writer, "{line}")?;
                    output_writer.flush()?;
                    accumulated_output.push_str(&line);
                    accumulated_output.push('\n');
                }
                Some(StreamEvent::StderrLine(line)) => {
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

        let status = child.wait().await?;

        if let Some(handle) = stdout_task {
            handle.await.map_err(join_error_to_io)??;
        }
        if let Some(handle) = stderr_task {
            handle.await.map_err(join_error_to_io)??;
        }

        Ok(ExecutionResult {
            output: accumulated_output,
            success: status.success() && !timed_out,
            exit_code: status.code(),
            timed_out,
        })
    }

    /// Terminates the child process with SIGTERM.
    fn terminate_child(child: &mut tokio::process::Child) -> std::io::Result<()> {
        #[cfg(not(unix))]
        {
            // SIGTERM doesn't exist on Windows. Best-effort termination:
            // On Unix this would be SIGKILL, on Windows it maps to process termination.
            child.start_kill()
        }

        #[cfg(unix)]
        if let Some(pid) = child.id() {
            #[allow(clippy::cast_possible_wrap)]
            let pid = Pid::from_raw(pid as i32);
            debug!(%pid, "Sending SIGTERM to child process");
            let _ = kill(pid, Signal::SIGTERM);
            Ok(())
        } else {
            Ok(())
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
    if std::path::Path::new("/var/tmp").is_dir() {
        command.env("TMPDIR", "/var/tmp");
        command.env("TMP", "/var/tmp");
        command.env("TEMP", "/var/tmp");
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
        let timeout = Some(Duration::from_millis(300));
        let result = executor
            .execute_capture_with_timeout(
                "printf 'start\\n'; sleep 0.2; printf 'middle\\n'; sleep 0.2; printf 'done\\n'",
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
}
