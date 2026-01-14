//! CLI executor for running prompts through backends.
//!
//! Executes prompts via CLI tools with real-time streaming output.

use crate::cli_backend::CliBackend;
#[cfg(test)]
use crate::cli_backend::PromptMode;
use std::io::Write;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Result of a CLI execution.
#[derive(Debug)]
pub struct ExecutionResult {
    /// The full output from the CLI.
    pub output: String,
    /// Whether the execution succeeded (exit code 0).
    pub success: bool,
    /// The exit code.
    pub exit_code: Option<i32>,
}

/// Executor for running prompts through CLI backends.
#[derive(Debug)]
pub struct CliExecutor {
    backend: CliBackend,
}

impl CliExecutor {
    /// Creates a new executor with the given backend.
    pub fn new(backend: CliBackend) -> Self {
        Self { backend }
    }

    /// Executes a prompt and streams output to the provided writer.
    ///
    /// Output is streamed line-by-line to the writer while being accumulated
    /// for the return value.
    pub async fn execute<W: Write + Send>(
        &self,
        prompt: &str,
        mut output_writer: W,
    ) -> std::io::Result<ExecutionResult> {
        let (cmd, args, stdin_input, _temp_file) = self.backend.build_command(prompt, false);

        let mut command = Command::new(&cmd);
        command.args(&args);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        if stdin_input.is_some() {
            command.stdin(Stdio::piped());
        }

        let mut child = command.spawn()?;

        // Write to stdin if needed
        if let Some(input) = stdin_input {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(input.as_bytes()).await?;
                drop(stdin); // Close stdin to signal EOF
            }
        }

        let mut accumulated_output = String::new();

        // Stream stdout
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Some(line) = lines.next_line().await? {
                // Write to output writer (real-time streaming)
                writeln!(output_writer, "{line}")?;
                output_writer.flush()?;

                // Accumulate for return value
                accumulated_output.push_str(&line);
                accumulated_output.push('\n');
            }
        }

        // Also capture stderr (append to output)
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Some(line) = lines.next_line().await? {
                writeln!(output_writer, "[stderr] {line}")?;
                output_writer.flush()?;

                accumulated_output.push_str("[stderr] ");
                accumulated_output.push_str(&line);
                accumulated_output.push('\n');
            }
        }

        let status = child.wait().await?;

        Ok(ExecutionResult {
            output: accumulated_output,
            success: status.success(),
            exit_code: status.code(),
        })
    }

    /// Executes a prompt without streaming (captures all output).
    pub async fn execute_capture(&self, prompt: &str) -> std::io::Result<ExecutionResult> {
        // Use a sink that discards output for non-streaming execution
        let sink = std::io::sink();
        self.execute(prompt, sink).await
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
        };

        let executor = CliExecutor::new(backend);
        let mut output = Vec::new();

        let result = executor.execute("hello world", &mut output).await.unwrap();

        assert!(result.success);
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
        };

        let executor = CliExecutor::new(backend);
        let result = executor.execute_capture("").await.unwrap();

        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
    }
}
