//! Ralph execution for E2E tests.
//!
//! This module provides functionality to execute `ralph run` with test configurations
//! and capture all output including stdout, stderr, exit code, and artifacts from
//! the `.ralph/agent/` directory.
//!
//! # Example
//!
//! ```no_run
//! use ralph_e2e::executor::{RalphExecutor, ScenarioConfig, PromptSource};
//! use std::path::PathBuf;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let executor = RalphExecutor::new(PathBuf::from(".e2e-tests/test-scenario"));
//!
//!     let config = ScenarioConfig {
//!         config_file: PathBuf::from("ralph.yml"),
//!         prompt: PromptSource::Inline("Say hello".to_string()),
//!         max_iterations: 1,
//!         timeout: Duration::from_secs(60),
//!         extra_args: vec![],
//!     };
//!
//!     let result = executor.run(&config).await.unwrap();
//!     println!("Exit code: {:?}", result.exit_code);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Configuration for a test scenario.
#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    /// Path to ralph.yml for this test (relative to workspace).
    pub config_file: PathBuf,

    /// Prompt to send to the agent.
    pub prompt: PromptSource,

    /// Maximum iterations for this test.
    pub max_iterations: u32,

    /// Timeout for the entire test.
    pub timeout: Duration,

    /// Additional CLI arguments.
    pub extra_args: Vec<String>,
}

impl ScenarioConfig {
    /// Creates a minimal config for basic connectivity tests.
    pub fn minimal(prompt: impl Into<String>) -> Self {
        Self {
            config_file: PathBuf::from("ralph.yml"),
            prompt: PromptSource::Inline(prompt.into()),
            max_iterations: 1,
            timeout: Duration::from_secs(300), // 5 minutes - Claude iterations can take 60-120s
            extra_args: vec![],
        }
    }
}

/// Source of the prompt for a test.
#[derive(Debug, Clone)]
pub enum PromptSource {
    /// Prompt loaded from a file.
    File(PathBuf),
    /// Inline prompt string.
    Inline(String),
}

/// Result of executing Ralph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Exit code from the ralph process (None if killed by signal).
    pub exit_code: Option<i32>,

    /// Full stdout output.
    pub stdout: String,

    /// Full stderr output.
    pub stderr: String,

    /// How long the execution took.
    #[serde(with = "duration_serde")]
    pub duration: Duration,

    /// Content of scratchpad after execution, if present.
    pub scratchpad: Option<String>,

    /// Events parsed from the execution output.
    pub events: Vec<EventRecord>,

    /// Number of iterations completed.
    pub iterations: u32,

    /// Reason for termination, if detected.
    pub termination_reason: Option<String>,

    /// Whether the execution timed out.
    pub timed_out: bool,
}

/// A recorded event from Ralph execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    /// Event topic (e.g., "build.done", "task.complete").
    pub topic: String,

    /// Event payload content.
    pub payload: String,
}

/// Errors that can occur during Ralph execution.
#[derive(Debug, Error)]
pub enum ExecutorError {
    /// Failed to spawn the ralph process.
    #[error("failed to spawn ralph: {0}")]
    SpawnError(#[from] std::io::Error),

    /// Workspace directory doesn't exist.
    #[error("workspace does not exist: {0}")]
    WorkspaceNotFound(PathBuf),

    /// Config file doesn't exist.
    #[error("config file does not exist: {0}")]
    ConfigNotFound(PathBuf),

    /// Ralph binary not found.
    #[error("ralph binary not found")]
    RalphNotFound,

    /// Execution timed out.
    #[error("execution timed out after {0:?}")]
    Timeout(Duration),
}

/// Finds the workspace root by walking up from the current directory.
///
/// Returns the first directory containing a Cargo.toml file, or None if not found.
pub fn find_workspace_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            return Some(current);
        }

        current = current.parent()?.to_path_buf();
    }
}

/// Resolves the path to the ralph binary.
///
/// Resolution order:
/// 1. `target/release/ralph` (prefer optimized builds)
/// 2. `target/debug/ralph` (development builds)
/// 3. Falls back to "ralph" (PATH lookup)
///
/// This ensures e2e tests run against the locally built code, not a system-installed version.
pub fn resolve_ralph_binary() -> PathBuf {
    if let Some(root) = find_workspace_root() {
        // Check for release binary first (faster)
        let release_binary = root.join("target/release/ralph");
        if release_binary.exists() {
            return release_binary;
        }

        // Fall back to debug binary
        let debug_binary = root.join("target/debug/ralph");
        if debug_binary.exists() {
            return debug_binary;
        }
    }

    // Fall back to PATH lookup
    PathBuf::from("ralph")
}

/// Executes Ralph with test configurations.
#[derive(Debug, Clone)]
pub struct RalphExecutor {
    /// Path to the workspace directory for this scenario.
    workspace: PathBuf,

    /// Optional path to the ralph binary (defaults to finding it in PATH).
    ralph_binary: Option<PathBuf>,
}

impl RalphExecutor {
    /// Creates a new executor for the given workspace.
    ///
    /// The workspace should already exist and contain a ralph.yml config file.
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            ralph_binary: None,
        }
    }

    /// Creates a new executor with a specific ralph binary path.
    pub fn with_binary(workspace: PathBuf, ralph_binary: PathBuf) -> Self {
        Self {
            workspace,
            ralph_binary: Some(ralph_binary),
        }
    }

    /// Returns the workspace path.
    pub fn workspace(&self) -> &PathBuf {
        &self.workspace
    }

    /// Returns the ralph binary that will be used.
    pub fn ralph_binary(&self) -> PathBuf {
        self.ralph_binary
            .clone()
            .unwrap_or_else(|| PathBuf::from("ralph"))
    }

    /// Executes ralph with the given configuration.
    pub async fn run(&self, config: &ScenarioConfig) -> Result<ExecutionResult, ExecutorError> {
        self.run_with_timeout(config, config.timeout).await
    }

    /// Executes ralph with a specific timeout.
    pub async fn run_with_timeout(
        &self,
        config: &ScenarioConfig,
        timeout: Duration,
    ) -> Result<ExecutionResult, ExecutorError> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;
        use tokio::process::Command;
        use tokio::time::Instant;

        // Verify workspace exists
        if !self.workspace.exists() {
            return Err(ExecutorError::WorkspaceNotFound(self.workspace.clone()));
        }

        let config_path = self.workspace.join(&config.config_file);
        if !config_path.exists() {
            return Err(ExecutorError::ConfigNotFound(config_path));
        }

        let start = Instant::now();

        // Build the command
        // Note: Pass config_file (not full config_path) because current_dir is set to workspace
        let mut cmd = Command::new(self.ralph_binary());
        cmd.arg("run")
            .arg("-c")
            .arg(&config.config_file)
            .arg("--max-iterations")
            .arg(config.max_iterations.to_string())
            .current_dir(&self.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Always enable diagnostics for E2E tests to aid debugging
            .env("RALPH_DIAGNOSTICS", "1")
            // Pass workspace root so Ralph resolves paths correctly in E2E tests
            .env("RALPH_WORKSPACE_ROOT", &self.workspace)
            // Use Haiku for faster, cheaper E2E tests
            .env("CLAUDE_MODEL", "haiku");

        // Handle prompt
        match &config.prompt {
            PromptSource::File(path) => {
                cmd.arg("-p").arg(format!("@{}", path.display()));
            }
            PromptSource::Inline(prompt) => {
                cmd.arg("-p").arg(prompt);
            }
        }

        // Add extra args
        for arg in &config.extra_args {
            cmd.arg(arg);
        }

        // Spawn the process
        let mut child = cmd.spawn()?;

        // Close stdin to signal no more input
        if let Some(mut stdin) = child.stdin.take() {
            stdin.shutdown().await.ok();
        }

        // Wait with timeout - we need to handle the timeout separately
        // because wait_with_output consumes the child
        let wait_result =
            tokio::time::timeout(timeout, async { child.wait_with_output().await }).await;

        let duration = start.elapsed();

        match wait_result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                // Read scratchpad if it exists
                let scratchpad = self.read_scratchpad().await;

                // Read events from JSONL file (primary source)
                let events = self.read_events_from_jsonl().await;

                // Count iterations from output
                let iterations = self.count_iterations(&stdout);

                // Detect termination reason
                let termination_reason = self.detect_termination_reason(&stdout);

                Ok(ExecutionResult {
                    exit_code: output.status.code(),
                    stdout,
                    stderr,
                    duration,
                    scratchpad,
                    events,
                    iterations,
                    termination_reason,
                    timed_out: false,
                })
            }
            Ok(Err(e)) => Err(ExecutorError::SpawnError(e)),
            Err(_) => {
                // Timeout occurred - the child is already consumed by wait_with_output
                // The process may still be running, but we can't kill it directly
                // Return a timeout result indicating what happened
                Ok(ExecutionResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    duration,
                    scratchpad: self.read_scratchpad().await,
                    events: vec![],
                    iterations: 0,
                    termination_reason: Some("TIMEOUT".to_string()),
                    timed_out: true,
                })
            }
        }
    }

    /// Reads the scratchpad file from the workspace.
    async fn read_scratchpad(&self) -> Option<String> {
        let scratchpad_path = self.workspace.join(".agent").join("scratchpad.md");
        tokio::fs::read_to_string(scratchpad_path).await.ok()
    }

    /// Reads events from .ralph/events.jsonl file.
    ///
    /// Ralph writes events to JSONL format since commit dfb8f8de.
    /// Each line is a JSON object with "topic" and "payload" fields.
    async fn read_events_from_jsonl(&self) -> Vec<EventRecord> {
        // Find the current events file (uses marker file for timestamped paths)
        let events_marker = self.workspace.join(".ralph").join("current-events");
        let fallback_path = self.workspace.join(".ralph/events.jsonl");

        let events_path = match tokio::fs::read_to_string(&events_marker).await {
            Ok(path) => {
                let marker_path = self.workspace.join(path.trim());
                // Fall back to events.jsonl if marker-pointed file doesn't exist
                if tokio::fs::metadata(&marker_path).await.is_ok() {
                    marker_path
                } else {
                    fallback_path.clone()
                }
            }
            Err(_) => fallback_path.clone(), // marker file missing
        };

        let mut events = Vec::new();
        if let Ok(content) = tokio::fs::read_to_string(&events_path).await {
            for line in content.lines().filter(|l| !l.trim().is_empty()) {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(line)
                    && let (Some(topic), Some(payload)) = (
                        event.get("topic").and_then(|v| v.as_str()),
                        event.get("payload").and_then(|v| v.as_str()),
                    )
                {
                    events.push(EventRecord {
                        topic: topic.to_string(),
                        payload: payload.to_string(),
                    });
                }
            }
        }
        events
    }

    /// Counts iterations from the output.
    ///
    /// Ralph outputs iteration markers like "[Iteration 1]" or similar.
    fn count_iterations(&self, output: &str) -> u32 {
        // Look for patterns like "[Iteration N]" or "Iteration N" or "[iter N]"
        let iter_regex = regex::Regex::new(r"(?i)\[?\s*iter(?:ation)?\s*(\d+)\s*\]?").unwrap();

        let mut max_iter = 0;
        for cap in iter_regex.captures_iter(output) {
            if let Some(num) = cap.get(1)
                && let Ok(n) = num.as_str().parse::<u32>()
            {
                max_iter = max_iter.max(n);
            }
        }

        max_iter
    }

    /// Detects the termination reason from output.
    fn detect_termination_reason(&self, output: &str) -> Option<String> {
        if output.contains("LOOP_COMPLETE") {
            return Some("LOOP_COMPLETE".to_string());
        }
        if output.contains("max iterations") || output.contains("max-iterations") {
            return Some("MAX_ITERATIONS".to_string());
        }
        if output.contains("timeout") || output.contains("timed out") {
            return Some("TIMEOUT".to_string());
        }
        None
    }
}

/// Serde helper for Duration serialization.
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs_f64().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::Path;

    /// Creates a unique test workspace path.
    fn test_workspace(test_name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-executor-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    /// Sets up a test workspace with a minimal ralph.yml.
    fn setup_workspace(path: &Path) {
        fs::create_dir_all(path.join(".agent")).unwrap();
        fs::write(
            path.join("ralph.yml"),
            r"cli:
  backend: claude
  max_iterations: 1
",
        )
        .unwrap();
    }

    /// Cleans up a test workspace.
    fn cleanup_workspace(path: &PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    #[test]
    fn test_resolve_ralph_binary_finds_local_or_path() {
        let binary = super::resolve_ralph_binary();
        // Should return something - either a local build or "ralph" for PATH
        let binary_str = binary.to_string_lossy();
        assert!(
            binary_str.contains("target/debug/ralph")
                || binary_str.contains("target/release/ralph")
                || binary_str == "ralph",
            "Expected local build path or 'ralph', got: {}",
            binary_str
        );
    }

    #[test]
    fn test_executor_new() {
        let workspace = PathBuf::from("/tmp/test-workspace");
        let executor = RalphExecutor::new(workspace.clone());
        assert_eq!(executor.workspace(), &workspace);
        assert_eq!(executor.ralph_binary(), PathBuf::from("ralph"));
    }

    #[test]
    fn test_executor_with_binary() {
        let workspace = PathBuf::from("/tmp/test-workspace");
        let binary = PathBuf::from("/usr/local/bin/ralph");
        let executor = RalphExecutor::with_binary(workspace.clone(), binary.clone());
        assert_eq!(executor.workspace(), &workspace);
        assert_eq!(executor.ralph_binary(), binary);
    }

    #[test]
    fn test_scenario_config_minimal() {
        let config = ScenarioConfig::minimal("Say hello");
        assert_eq!(config.config_file, PathBuf::from("ralph.yml"));
        assert!(matches!(config.prompt, PromptSource::Inline(p) if p == "Say hello"));
        assert_eq!(config.max_iterations, 1);
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert!(config.extra_args.is_empty());
    }

    #[test]
    fn test_count_iterations_none() {
        let executor = RalphExecutor::new(PathBuf::from("/tmp"));
        let count = executor.count_iterations("no iteration markers here");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_iterations_single() {
        let executor = RalphExecutor::new(PathBuf::from("/tmp"));
        let count = executor.count_iterations("[Iteration 1] Starting...");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_count_iterations_multiple() {
        let executor = RalphExecutor::new(PathBuf::from("/tmp"));
        let output = "[Iteration 1] First\n[Iteration 2] Second\n[Iteration 3] Third";
        let count = executor.count_iterations(output);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_iterations_short_format() {
        let executor = RalphExecutor::new(PathBuf::from("/tmp"));
        let output = "[iter 1] First\n[iter 2] Second";
        let count = executor.count_iterations(output);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_detect_termination_loop_complete() {
        let executor = RalphExecutor::new(PathBuf::from("/tmp"));
        let reason = executor.detect_termination_reason("Task done. LOOP_COMPLETE");
        assert_eq!(reason, Some("LOOP_COMPLETE".to_string()));
    }

    #[test]
    fn test_detect_termination_max_iterations() {
        let executor = RalphExecutor::new(PathBuf::from("/tmp"));
        let reason = executor.detect_termination_reason("Reached max iterations, stopping");
        assert_eq!(reason, Some("MAX_ITERATIONS".to_string()));
    }

    #[test]
    fn test_detect_termination_none() {
        let executor = RalphExecutor::new(PathBuf::from("/tmp"));
        let reason = executor.detect_termination_reason("normal output");
        assert!(reason.is_none());
    }

    #[tokio::test]
    async fn test_run_workspace_not_found() {
        let workspace = PathBuf::from("/nonexistent/workspace");
        let executor = RalphExecutor::new(workspace.clone());
        let config = ScenarioConfig::minimal("test");

        let result = executor.run(&config).await;
        assert!(matches!(result, Err(ExecutorError::WorkspaceNotFound(_))));
    }

    #[tokio::test]
    async fn test_run_config_not_found() {
        let workspace = test_workspace("config-not-found");
        fs::create_dir_all(&workspace).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let config = ScenarioConfig::minimal("test");

        let result = executor.run(&config).await;
        assert!(matches!(result, Err(ExecutorError::ConfigNotFound(_))));

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_execution_result_serialization() {
        let result = ExecutionResult {
            exit_code: Some(0),
            stdout: "hello".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs_f64(1.5),
            scratchpad: Some("# Notes".to_string()),
            events: vec![EventRecord {
                topic: "build.done".to_string(),
                payload: "success".to_string(),
            }],
            iterations: 2,
            termination_reason: Some("LOOP_COMPLETE".to_string()),
            timed_out: false,
        };

        // Serialize to JSON
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"exit_code\":0"));
        assert!(json.contains("\"stdout\":\"hello\""));
        assert!(json.contains("\"duration\":1.5"));

        // Deserialize back
        let parsed: ExecutionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.exit_code, Some(0));
        assert_eq!(parsed.stdout, "hello");
        assert_eq!(parsed.iterations, 2);
    }

    // Integration test that requires ralph binary - skip in CI
    #[tokio::test]
    #[ignore = "requires ralph binary"]
    async fn test_run_real_ralph() {
        let workspace = test_workspace("real-ralph");
        setup_workspace(&workspace);

        let executor = RalphExecutor::new(workspace.clone());
        let config = ScenarioConfig::minimal("Say 'test passed'");

        let result = executor.run(&config).await;

        // Clean up regardless of result
        cleanup_workspace(&workspace);

        // Verify execution
        let result = result.expect("ralph should execute");
        assert!(
            !result.stdout.is_empty() || !result.stderr.is_empty(),
            "should have output"
        );
    }
}
