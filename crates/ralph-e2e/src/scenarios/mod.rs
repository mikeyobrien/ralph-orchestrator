//! Test scenarios for E2E testing.
//!
//! This module defines the `TestScenario` trait and provides implementations
//! for various test scenarios across different backends.
//!
//! # Architecture
//!
//! Each scenario follows a setup → run → cleanup lifecycle:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      TestScenario                               │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  1. setup()   → Creates ralph.yml, prompt files in workspace    │
//! │  2. run()     → Executes via RalphExecutor, checks assertions   │
//! │  3. cleanup() → Optional post-test cleanup                      │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```no_run
//! use ralph_e2e::scenarios::{TestScenario, ConnectivityScenario};
//! use ralph_e2e::executor::RalphExecutor;
//! use ralph_e2e::Backend;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() {
//!     let scenario = ConnectivityScenario::new();
//!     let workspace = Path::new(".e2e-tests/connect");
//!
//!     // Setup creates config files for the target backend
//!     let config = scenario.setup(workspace, Backend::Claude).unwrap();
//!
//!     // Run executes and validates
//!     let executor = RalphExecutor::new(workspace.to_path_buf());
//!     let result = scenario.run(&executor, &config).await.unwrap();
//!
//!     println!("Test passed: {}", result.passed);
//! }
//! ```

mod capabilities;
mod connectivity;
mod errors;
mod events;
mod hats;
mod incremental;
mod memory;
mod orchestration;
mod tasks;

pub use capabilities::{StreamingScenario, ToolUseScenario};
pub use connectivity::ConnectivityScenario;
pub use errors::{
    AuthFailureScenario, BackendUnavailableScenario, MaxIterationsScenario, TimeoutScenario,
};
pub use events::{BackpressureScenario, EventsScenario};
pub use hats::{
    HatBackendOverrideScenario, HatEventRoutingScenario, HatInstructionsScenario,
    HatMultiWorkflowScenario, HatSingleScenario,
};
pub use incremental::{ChainedLoopScenario, IncrementalFeatureScenario};
pub use memory::{
    MemoryAddScenario, MemoryCorruptedFileScenario, MemoryInjectionScenario,
    MemoryLargeContentScenario, MemoryMissingFileScenario, MemoryPersistenceScenario,
    MemoryRapidWriteScenario, MemorySearchScenario,
};
pub use orchestration::{CompletionScenario, MultiIterScenario, SingleIterScenario};
pub use tasks::{TaskAddScenario, TaskCloseScenario, TaskCompletionScenario, TaskReadyScenario};

use crate::Backend;
use crate::executor::{ExecutionResult, RalphExecutor, ScenarioConfig};
use crate::models::{Assertion, TestResult};
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during scenario execution.
#[derive(Debug, Error)]
pub enum ScenarioError {
    /// Failed to set up the scenario workspace.
    #[error("setup failed: {0}")]
    SetupError(String),

    /// Failed to execute the scenario.
    #[error("execution failed: {0}")]
    ExecutionError(String),

    /// IO error during setup or cleanup.
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A test scenario that can be executed against a backend.
///
/// Each scenario is a self-contained test case that knows how to:
/// - Set up the necessary configuration files
/// - Execute against a backend via RalphExecutor
/// - Validate the results with assertions
#[async_trait]
pub trait TestScenario: Send + Sync {
    /// Unique identifier for the scenario (e.g., "connect").
    ///
    /// Note: This is the base ID without backend suffix. When running for
    /// specific backends, the runner will append the backend name (e.g., "connect-claude").
    fn id(&self) -> &str;

    /// Human-readable description of what the scenario tests.
    fn description(&self) -> &str;

    /// The tier this scenario belongs to (e.g., "Tier 1: Connectivity").
    fn tier(&self) -> &str;

    /// Returns the list of backends this scenario supports.
    ///
    /// Default implementation returns all backends. Override this to restrict
    /// the scenario to specific backends.
    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude, Backend::Kiro, Backend::OpenCode]
    }

    /// Sets up the scenario by creating necessary files in the workspace.
    ///
    /// The `backend` parameter specifies which backend to configure for.
    /// Returns the configuration to use when running the scenario.
    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError>;

    /// Runs the scenario and returns the test result.
    ///
    /// This method executes the scenario using the provided executor and
    /// validates the results against expected assertions.
    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError>;

    /// Cleans up after the scenario (optional).
    ///
    /// The workspace is typically cleaned automatically, but scenarios can
    /// implement custom cleanup logic if needed.
    fn cleanup(&self, _workspace: &Path) -> Result<(), ScenarioError> {
        Ok(())
    }
}

/// Builder for creating assertions with a fluent API.
#[derive(Debug, Clone)]
pub struct AssertionBuilder {
    name: String,
    expected: String,
    actual: String,
    passed: bool,
}

impl AssertionBuilder {
    /// Creates a new assertion with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            expected: String::new(),
            actual: String::new(),
            passed: false,
        }
    }

    /// Sets the expected value.
    pub fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = expected.into();
        self
    }

    /// Sets the actual value.
    pub fn actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = actual.into();
        self
    }

    /// Marks the assertion as passed.
    pub fn passed(mut self) -> Self {
        self.passed = true;
        self
    }

    /// Marks the assertion as failed.
    pub fn failed(mut self) -> Self {
        self.passed = false;
        self
    }

    /// Builds the assertion.
    pub fn build(self) -> Assertion {
        Assertion {
            name: self.name,
            passed: self.passed,
            expected: self.expected,
            actual: self.actual,
        }
    }
}

/// Assertion helpers for common validation patterns.
pub struct Assertions;

impl Assertions {
    /// Asserts that the execution received a response (non-empty stdout).
    pub fn response_received(result: &ExecutionResult) -> Assertion {
        let has_response = !result.stdout.trim().is_empty();
        AssertionBuilder::new("Response received")
            .expected("Non-empty response from agent")
            .actual(if has_response {
                format!("Received {} bytes", result.stdout.len())
            } else {
                "Empty response".to_string()
            })
            .passed()
            .build()
            .with_passed(has_response)
    }

    /// Asserts that the exit code matches the expected value.
    pub fn exit_code(result: &ExecutionResult, expected: i32) -> Assertion {
        let actual_code = result.exit_code;
        let passed = actual_code == Some(expected);
        AssertionBuilder::new("Exit code")
            .expected(format!("Exit code {}", expected))
            .actual(match actual_code {
                Some(code) => format!("Exit code {}", code),
                None => "Process killed by signal".to_string(),
            })
            .build()
            .with_passed(passed)
    }

    /// Asserts that no errors occurred (empty stderr or known-safe warnings).
    pub fn no_errors(result: &ExecutionResult) -> Assertion {
        let stderr = result.stderr.trim();
        // Consider stderr empty or containing only warnings as "no errors"
        let has_error = !stderr.is_empty()
            && !stderr.contains("warning:")
            && !stderr.contains("Compiling")
            && !stderr.contains("Finished");

        AssertionBuilder::new("No errors")
            .expected("Empty stderr (or only warnings)")
            .actual(if stderr.is_empty() {
                "No stderr output".to_string()
            } else if has_error {
                format!("stderr: {}", truncate(stderr, 100))
            } else {
                "Only warnings/build output (OK)".to_string()
            })
            .build()
            .with_passed(!has_error)
    }

    /// Asserts that stdout contains the expected substring.
    pub fn output_contains(result: &ExecutionResult, expected: &str) -> Assertion {
        let contains = result.stdout.contains(expected);
        AssertionBuilder::new(format!("Output contains '{}'", truncate(expected, 30)))
            .expected(format!("Contains: {}", expected))
            .actual(if contains {
                "Found in output".to_string()
            } else {
                format!("Not found. Output: {}", truncate(&result.stdout, 100))
            })
            .build()
            .with_passed(contains)
    }

    /// Asserts that a specific event was emitted.
    pub fn event_emitted(result: &ExecutionResult, topic: &str) -> Assertion {
        let found = result.events.iter().any(|e| e.topic == topic);
        AssertionBuilder::new(format!("Event '{}' emitted", topic))
            .expected(format!("Event with topic '{}'", topic))
            .actual(if found {
                "Event found".to_string()
            } else {
                format!(
                    "Not found. Events: {:?}",
                    result.events.iter().map(|e| &e.topic).collect::<Vec<_>>()
                )
            })
            .build()
            .with_passed(found)
    }

    /// Asserts that the execution completed within the expected iteration count.
    pub fn iterations_within(result: &ExecutionResult, max: u32) -> Assertion {
        let within = result.iterations <= max;
        AssertionBuilder::new(format!("Iterations ≤ {}", max))
            .expected(format!("At most {} iterations", max))
            .actual(format!("{} iterations", result.iterations))
            .build()
            .with_passed(within)
    }

    /// Asserts that the execution did not time out.
    pub fn no_timeout(result: &ExecutionResult) -> Assertion {
        AssertionBuilder::new("No timeout")
            .expected("Execution completes without timeout")
            .actual(if result.timed_out {
                format!("Timed out after {:?}", result.duration)
            } else {
                format!("Completed in {:?}", result.duration)
            })
            .build()
            .with_passed(!result.timed_out)
    }

    /// Asserts that the execution completed within the given duration.
    pub fn duration_within(result: &ExecutionResult, max: Duration) -> Assertion {
        let within = result.duration <= max;
        AssertionBuilder::new(format!("Duration ≤ {:?}", max))
            .expected(format!("At most {:?}", max))
            .actual(format!("{:?}", result.duration))
            .build()
            .with_passed(within)
    }

    /// Asserts that exit code is 0 (completion) or 2 (limit reached).
    ///
    /// Ralph's exit codes:
    /// - **0**: Completion promise detected (success)
    /// - **1**: Consecutive failures, loop thrashing, validation failure (failure)
    /// - **2**: Max iterations, max runtime, or max cost exceeded (limit)
    /// - **130**: User interrupt (SIGINT)
    ///
    /// Use this when the test verifies functional behavior regardless of whether
    /// Ralph completed via the completion promise or hit iteration limits.
    pub fn exit_code_success_or_limit(result: &ExecutionResult) -> Assertion {
        let actual_code = result.exit_code;
        let passed = matches!(actual_code, Some(0 | 2));
        AssertionBuilder::new("Exit code (success or limit)")
            .expected("Exit code 0 or 2")
            .actual(match actual_code {
                Some(code) => format!("Exit code {}", code),
                None => "Process killed by signal".to_string(),
            })
            .build()
            .with_passed(passed)
    }
}

/// Extension trait for Assertion to allow chained modification.
trait AssertionExt {
    fn with_passed(self, passed: bool) -> Self;
}

impl AssertionExt for Assertion {
    fn with_passed(mut self, passed: bool) -> Self {
        self.passed = passed;
        self
    }
}

/// Truncates a string to the given length, adding "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Note: `max_len` is a byte-count upper bound.
        // We must back off to a valid UTF-8 character boundary; otherwise slicing `&s[..N]` can
        // panic when the output contains multi-byte characters (e.g. CJK, emoji).
        let mut boundary = max_len.min(s.len());
        while boundary > 0 && !s.is_char_boundary(boundary) {
            boundary -= 1;
        }
        format!("{}...", &s[..boundary])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::EventRecord;

    #[test]
    fn test_truncate_does_not_panic_on_multibyte_chars() {
        let s = format!("{}✅{}", "x".repeat(99), "y".repeat(10));
        let out = truncate(&s, 100);
        for _ in out.chars() {}
    }

    fn mock_execution_result() -> ExecutionResult {
        ExecutionResult {
            exit_code: Some(0),
            stdout: "Hello, I'm Claude!".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(5),
            scratchpad: None,
            events: vec![EventRecord {
                topic: "build.done".to_string(),
                payload: "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"
                    .to_string(),
            }],
            iterations: 1,
            termination_reason: Some("LOOP_COMPLETE".to_string()),
            timed_out: false,
        }
    }

    #[test]
    fn test_assertion_builder() {
        let assertion = AssertionBuilder::new("Test assertion")
            .expected("expected value")
            .actual("actual value")
            .passed()
            .build();

        assert_eq!(assertion.name, "Test assertion");
        assert_eq!(assertion.expected, "expected value");
        assert_eq!(assertion.actual, "actual value");
        assert!(assertion.passed);
    }

    #[test]
    fn test_assertion_builder_failed() {
        let assertion = AssertionBuilder::new("Failed test")
            .expected("foo")
            .actual("bar")
            .failed()
            .build();

        assert!(!assertion.passed);
    }

    #[test]
    fn test_response_received_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::response_received(&result);
        assert!(assertion.passed);
        assert_eq!(assertion.name, "Response received");
    }

    #[test]
    fn test_response_received_failed() {
        let mut result = mock_execution_result();
        result.stdout = String::new();
        let assertion = Assertions::response_received(&result);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_exit_code_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::exit_code(&result, 0);
        assert!(assertion.passed);
    }

    #[test]
    fn test_exit_code_failed() {
        let mut result = mock_execution_result();
        result.exit_code = Some(1);
        let assertion = Assertions::exit_code(&result, 0);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_no_errors_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::no_errors(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_no_errors_failed() {
        let mut result = mock_execution_result();
        result.stderr = "Error: something went wrong".to_string();
        let assertion = Assertions::no_errors(&result);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_no_errors_allows_warnings() {
        let mut result = mock_execution_result();
        result.stderr = "warning: unused variable".to_string();
        let assertion = Assertions::no_errors(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_output_contains_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::output_contains(&result, "Claude");
        assert!(assertion.passed);
    }

    #[test]
    fn test_output_contains_failed() {
        let result = mock_execution_result();
        let assertion = Assertions::output_contains(&result, "NotPresent");
        assert!(!assertion.passed);
    }

    #[test]
    fn test_event_emitted_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::event_emitted(&result, "build.done");
        assert!(assertion.passed);
    }

    #[test]
    fn test_event_emitted_failed() {
        let result = mock_execution_result();
        let assertion = Assertions::event_emitted(&result, "nonexistent.event");
        assert!(!assertion.passed);
    }

    #[test]
    fn test_iterations_within_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::iterations_within(&result, 3);
        assert!(assertion.passed);
    }

    #[test]
    fn test_iterations_within_failed() {
        let mut result = mock_execution_result();
        result.iterations = 5;
        let assertion = Assertions::iterations_within(&result, 3);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_no_timeout_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::no_timeout(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_no_timeout_failed() {
        let mut result = mock_execution_result();
        result.timed_out = true;
        let assertion = Assertions::no_timeout(&result);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_duration_within_passed() {
        let result = mock_execution_result();
        let assertion = Assertions::duration_within(&result, Duration::from_secs(10));
        assert!(assertion.passed);
    }

    #[test]
    fn test_duration_within_failed() {
        let result = mock_execution_result();
        let assertion = Assertions::duration_within(&result, Duration::from_secs(1));
        assert!(!assertion.passed);
    }

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("this is a long string", 10), "this is a ...");
    }

    #[test]
    fn test_exit_code_success_or_limit_passed_with_0() {
        let result = mock_execution_result();
        let assertion = Assertions::exit_code_success_or_limit(&result);
        assert!(assertion.passed);
        assert_eq!(assertion.name, "Exit code (success or limit)");
    }

    #[test]
    fn test_exit_code_success_or_limit_passed_with_2() {
        let mut result = mock_execution_result();
        result.exit_code = Some(2);
        let assertion = Assertions::exit_code_success_or_limit(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_exit_code_success_or_limit_failed_with_1() {
        let mut result = mock_execution_result();
        result.exit_code = Some(1);
        let assertion = Assertions::exit_code_success_or_limit(&result);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_exit_code_success_or_limit_failed_with_signal() {
        let mut result = mock_execution_result();
        result.exit_code = None;
        let assertion = Assertions::exit_code_success_or_limit(&result);
        assert!(!assertion.passed);
        assert!(assertion.actual.contains("signal"));
    }
}
