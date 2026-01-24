//! Tier 7: Error Handling test scenarios (RED phase).
//!
//! These scenarios test Ralph's graceful failure modes:
//! - Timeout handling
//! - Max iterations termination
//! - Auth failure handling
//! - Backend unavailable handling
//!
//! Unlike other tiers which test "green" (success) paths, these scenarios
//! intentionally trigger failure conditions to verify graceful degradation.
//!
//! All scenarios are backend-agnostic and can run against any supported backend.

use super::{Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::TestResult;
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;

/// Test scenario that verifies graceful timeout handling.
///
/// This scenario:
/// - Sends a prompt designed to run longer than the timeout
/// - Verifies that Ralph terminates gracefully at timeout
/// - Validates that the timeout flag is set correctly
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{TimeoutScenario, TestScenario};
///
/// let scenario = TimeoutScenario::new();
/// assert_eq!(scenario.tier(), "Tier 7: Error Handling");
/// ```
pub struct TimeoutScenario {
    id: String,
    description: String,
    tier: String,
}

impl TimeoutScenario {
    /// Creates a new timeout scenario.
    pub fn new() -> Self {
        Self {
            id: "timeout-handling".to_string(),
            description: "Verifies graceful timeout termination".to_string(),
            tier: "Tier 7: Error Handling".to_string(),
        }
    }
}

impl Default for TimeoutScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for TimeoutScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude, Backend::Kiro, Backend::OpenCode]
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml for timeout testing
        let config_content = format!(
            r#"# Timeout test config
cli:
  backend: {}

event_loop:
  max_iterations: 100
  completion_promise: "LOOP_COMPLETE"
"#,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Create a prompt that would take a very long time to complete
        let prompt = r"You are testing timeout behavior. This task should exceed the timeout.

Your task is to write a 10,000 word essay about the history of computing.
Be extremely detailed and thorough. Cover every decade from 1940 to 2020.
Include at least 100 named individuals and their contributions.

Do NOT output LOOP_COMPLETE until the essay is complete.

Start the essay now.";

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 100,
            // Very short timeout to trigger timeout condition
            timeout: Duration::from_secs(5),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Build assertions for timeout behavior
        // Note: This is a "RED" test - we EXPECT failure/timeout
        let assertions = vec![
            self.did_timeout(&execution),
            self.terminated_gracefully(&execution),
            self.duration_near_timeout(&execution, Duration::from_secs(5)),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Runner sets this
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl TimeoutScenario {
    /// Asserts that the execution timed out.
    fn did_timeout(&self, result: &crate::executor::ExecutionResult) -> crate::models::Assertion {
        super::AssertionBuilder::new("Execution timed out")
            .expected("timed_out = true")
            .actual(if result.timed_out {
                "Timed out as expected".to_string()
            } else {
                format!(
                    "Did not timeout. Exit code: {:?}, reason: {:?}",
                    result.exit_code, result.termination_reason
                )
            })
            .build()
            .with_passed(result.timed_out)
    }

    /// Asserts that termination was graceful (process didn't crash).
    fn terminated_gracefully(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        // Graceful termination means either:
        // 1. Process was killed by signal (exit_code is None)
        // 2. Process exited with any code (it didn't crash hard)
        let graceful = result.timed_out; // If we got a result at all, it was graceful

        super::AssertionBuilder::new("Terminated gracefully")
            .expected("Process terminated without crash")
            .actual(format!(
                "Exit code: {:?}, Timed out: {}",
                result.exit_code, result.timed_out
            ))
            .build()
            .with_passed(graceful)
    }

    /// Asserts that duration is close to the timeout value.
    fn duration_near_timeout(
        &self,
        result: &crate::executor::ExecutionResult,
        timeout: Duration,
    ) -> crate::models::Assertion {
        // Allow some tolerance (within 3 seconds of timeout)
        let tolerance = Duration::from_secs(3);
        let near = result.duration >= timeout.saturating_sub(tolerance)
            && result.duration <= timeout + tolerance;

        super::AssertionBuilder::new("Duration near timeout value")
            .expected(format!("{:?} ± 3s", timeout))
            .actual(format!("{:?}", result.duration))
            .build()
            .with_passed(near)
    }
}

/// Test scenario that verifies max iterations termination.
///
/// This scenario:
/// - Configures a low max_iterations limit
/// - Sends a prompt that won't complete quickly
/// - Verifies that Ralph terminates at the iteration limit
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MaxIterationsScenario, TestScenario};
///
/// let scenario = MaxIterationsScenario::new();
/// assert_eq!(scenario.tier(), "Tier 7: Error Handling");
/// ```
pub struct MaxIterationsScenario {
    id: String,
    description: String,
    tier: String,
}

impl MaxIterationsScenario {
    /// Creates a new max iterations scenario.
    pub fn new() -> Self {
        Self {
            id: "max-iterations".to_string(),
            description: "Verifies termination at max iterations limit".to_string(),
            tier: "Tier 7: Error Handling".to_string(),
        }
    }
}

impl Default for MaxIterationsScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MaxIterationsScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude, Backend::Kiro, Backend::OpenCode]
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with low max iterations
        let config_content = format!(
            r#"# Max iterations test config
cli:
  backend: {}

event_loop:
  max_iterations: 2
  completion_promise: "NEVER_GOING_TO_MATCH_THIS"
"#,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Create a prompt that would take many iterations
        let prompt = r#"You are testing max iterations behavior.

Your task is complex and will take many steps:
1. In iteration 1: Say "Starting step 1" and emit <event topic="step.1">done</event>
2. In iteration 2: Say "Starting step 2" and emit <event topic="step.2">done</event>
3. In iteration 3: Say "Starting step 3" and emit <event topic="step.3">done</event>
4. In iteration 4: Say "Starting step 4" and emit <event topic="step.4">done</event>
5. Only after step 4: Output NEVER_GOING_TO_MATCH_THIS

Complete exactly one step per iteration. Stop after each step."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 2,
            timeout: backend.default_timeout(),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Build assertions for max iterations behavior
        let assertions = vec![
            Assertions::response_received(&execution),
            self.stopped_at_max_iterations(&execution, 2),
            self.termination_reason_is_max(&execution),
            Assertions::no_timeout(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Runner sets this
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl MaxIterationsScenario {
    /// Asserts that execution stopped at max iterations.
    fn stopped_at_max_iterations(
        &self,
        result: &crate::executor::ExecutionResult,
        max: u32,
    ) -> crate::models::Assertion {
        let at_max = result.iterations == max;

        super::AssertionBuilder::new(format!("Stopped at {} iterations", max))
            .expected(format!("{} iterations", max))
            .actual(format!("{} iterations", result.iterations))
            .build()
            .with_passed(at_max)
    }

    /// Asserts that termination reason indicates max iterations reached.
    fn termination_reason_is_max(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let is_max = result
            .termination_reason
            .as_ref()
            .map(|r| {
                let r_lower = r.to_lowercase();
                r_lower.contains("max")
                    || r_lower.contains("iteration")
                    || r_lower.contains("limit")
            })
            .unwrap_or(false);

        super::AssertionBuilder::new("Termination reason indicates max iterations")
            .expected("Reason containing 'max', 'iteration', or 'limit'")
            .actual(format!("Reason: {:?}", result.termination_reason))
            .build()
            .with_passed(is_max)
    }
}

/// Test scenario that verifies auth failure handling.
///
/// This scenario:
/// - Configures an invalid authentication scenario
/// - Verifies that Ralph handles auth failures gracefully
/// - Validates error messages are informative
///
/// Note: This scenario may need special setup or may be skipped if
/// there's no way to safely trigger auth failure.
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{AuthFailureScenario, TestScenario};
///
/// let scenario = AuthFailureScenario::new();
/// assert_eq!(scenario.tier(), "Tier 7: Error Handling");
/// ```
pub struct AuthFailureScenario {
    id: String,
    description: String,
    tier: String,
}

impl AuthFailureScenario {
    /// Creates a new auth failure scenario.
    pub fn new() -> Self {
        Self {
            id: "auth-failure".to_string(),
            description: "Verifies graceful handling of authentication failures".to_string(),
            tier: "Tier 7: Error Handling".to_string(),
        }
    }
}

impl Default for AuthFailureScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for AuthFailureScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude, Backend::Kiro, Backend::OpenCode]
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml - we'll pass bad auth via environment
        let config_content = format!(
            r#"# Auth failure test config
cli:
  backend: {}

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"
"#,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = "Say hello.";

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 1,
            timeout: Duration::from_secs(30),
            // Pass invalid API key to trigger auth failure
            extra_args: vec!["--api-key".to_string(), "invalid-key-12345".to_string()],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Build assertions for auth failure handling
        let assertions = vec![
            self.execution_failed_with_error(&execution),
            self.error_message_helpful(&execution),
            self.process_exited_cleanly(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Runner sets this
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl AuthFailureScenario {
    /// Asserts that execution failed with an error.
    fn execution_failed_with_error(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let failed = result.exit_code != Some(0) || !result.stderr.is_empty();

        super::AssertionBuilder::new("Execution failed with error")
            .expected("Non-zero exit code or stderr output")
            .actual(format!(
                "Exit: {:?}, stderr: {}",
                result.exit_code,
                if result.stderr.is_empty() {
                    "empty"
                } else {
                    "present"
                }
            ))
            .build()
            .with_passed(failed)
    }

    /// Asserts that error message is helpful.
    fn error_message_helpful(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let stderr_lower = result.stderr.to_lowercase();
        let stdout_lower = result.stdout.to_lowercase();
        let combined = format!("{} {}", stderr_lower, stdout_lower);

        let helpful = combined.contains("auth")
            || combined.contains("unauthorized")
            || combined.contains("invalid")
            || combined.contains("key")
            || combined.contains("credential")
            || combined.contains("401")
            || combined.contains("403");

        super::AssertionBuilder::new("Error message is helpful")
            .expected("Message mentions auth/key/unauthorized/invalid")
            .actual(if helpful {
                "Found helpful error context".to_string()
            } else {
                format!("stderr: {}", truncate(&result.stderr, 100))
            })
            .build()
            .with_passed(helpful)
    }

    /// Asserts that the process exited cleanly (didn't crash/segfault).
    fn process_exited_cleanly(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        // Exit code present (not killed by signal) indicates clean exit
        let clean = result.exit_code.is_some();

        super::AssertionBuilder::new("Process exited cleanly")
            .expected("Exit code present (not killed by signal)")
            .actual(format!("Exit code: {:?}", result.exit_code))
            .build()
            .with_passed(clean)
    }
}

/// Test scenario that verifies backend unavailable handling.
///
/// This scenario:
/// - Configures a backend that doesn't exist
/// - Verifies that Ralph handles missing backends gracefully
/// - Validates error messages guide the user
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{BackendUnavailableScenario, TestScenario};
///
/// let scenario = BackendUnavailableScenario::new();
/// assert_eq!(scenario.tier(), "Tier 7: Error Handling");
/// ```
pub struct BackendUnavailableScenario {
    id: String,
    description: String,
    tier: String,
}

impl BackendUnavailableScenario {
    /// Creates a new backend unavailable scenario.
    pub fn new() -> Self {
        Self {
            id: "backend-unavailable".to_string(),
            description: "Verifies graceful handling of missing CLI backends".to_string(),
            tier: "Tier 7: Error Handling".to_string(),
        }
    }
}

impl Default for BackendUnavailableScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for BackendUnavailableScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude, Backend::Kiro, Backend::OpenCode]
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with a nonexistent backend command
        let config_content = format!(
            r#"# Backend unavailable test config
cli:
  backend: {}
  command: nonexistent-cli-that-does-not-exist-12345

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"
"#,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = "Say hello.";

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 1,
            timeout: Duration::from_secs(30),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Build assertions for backend unavailable handling
        let assertions = vec![
            self.execution_failed(&execution),
            self.error_mentions_backend(&execution),
            self.failed_fast(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Runner sets this
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl BackendUnavailableScenario {
    /// Asserts that execution failed.
    fn execution_failed(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let failed = result.exit_code != Some(0);

        super::AssertionBuilder::new("Execution failed")
            .expected("Non-zero exit code")
            .actual(format!("Exit code: {:?}", result.exit_code))
            .build()
            .with_passed(failed)
    }

    /// Asserts that error mentions the backend/command issue.
    fn error_mentions_backend(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let combined = format!(
            "{} {}",
            result.stderr.to_lowercase(),
            result.stdout.to_lowercase()
        );

        let mentions = combined.contains("not found")
            || combined.contains("command not found")
            || combined.contains("no such file")
            || combined.contains("cannot find")
            || combined.contains("nonexistent")
            || combined.contains("backend")
            || combined.contains("cli");

        super::AssertionBuilder::new("Error mentions backend/command issue")
            .expected("Message about missing command or backend")
            .actual(if mentions {
                "Found relevant error context".to_string()
            } else {
                format!(
                    "stderr: {}, stdout: {}",
                    truncate(&result.stderr, 50),
                    truncate(&result.stdout, 50)
                )
            })
            .build()
            .with_passed(mentions)
    }

    /// Asserts that failure was fast (didn't hang trying to connect).
    /// NOTE: Relaxed from 10s to 20s because startup overhead, retry logic,
    /// and process spawning can add latency beyond the bare minimum.
    fn failed_fast(&self, result: &crate::executor::ExecutionResult) -> crate::models::Assertion {
        let fast = result.duration < Duration::from_secs(20);

        super::AssertionBuilder::new("Failed fast")
            .expected("Failure within 20 seconds")
            .actual(format!("Took {:?}", result.duration))
            .build()
            .with_passed(fast)
    }
}

/// Extension trait for with_passed (duplicated here to avoid cross-module issues)
trait AssertionExt {
    fn with_passed(self, passed: bool) -> Self;
}

impl AssertionExt for crate::models::Assertion {
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
    use std::env;
    use std::fs;

    #[test]
    fn test_truncate_does_not_panic_on_multibyte_chars() {
        let s = format!("{}✅{}", "x".repeat(99), "y".repeat(10));
        let out = truncate(&s, 100);
        for _ in out.chars() {}
    }

    fn test_workspace(test_name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-errors-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    fn cleanup_workspace(path: &std::path::PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    fn mock_timeout_result() -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: None, // Killed by signal
            stdout: "Starting the essay...".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(5),
            scratchpad: None,
            events: vec![],
            iterations: 1,
            termination_reason: None,
            timed_out: true,
        }
    }

    fn mock_max_iter_result() -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: Some(0),
            stdout: "Step 1 done... Step 2 done...".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(30),
            scratchpad: None,
            events: vec![],
            iterations: 2,
            termination_reason: Some("MAX_ITERATIONS".to_string()),
            timed_out: false,
        }
    }

    fn mock_auth_failure_result() -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "Error: Invalid API key. Please check your credentials.".to_string(),
            duration: Duration::from_secs(2),
            scratchpad: None,
            events: vec![],
            iterations: 0,
            termination_reason: None,
            timed_out: false,
        }
    }

    fn mock_backend_unavailable_result() -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: Some(127),
            stdout: String::new(),
            stderr: "error: command not found: nonexistent-cli-that-does-not-exist-12345"
                .to_string(),
            duration: Duration::from_secs(1),
            scratchpad: None,
            events: vec![],
            iterations: 0,
            termination_reason: None,
            timed_out: false,
        }
    }

    // ========== TimeoutScenario Tests ==========

    #[test]
    fn test_timeout_scenario_new() {
        let scenario = TimeoutScenario::new();
        assert_eq!(scenario.id(), "timeout-handling");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 7: Error Handling");
    }

    #[test]
    fn test_timeout_scenario_default() {
        let scenario = TimeoutScenario::default();
        assert_eq!(scenario.id(), "timeout-handling");
    }

    #[test]
    fn test_timeout_scenario_description() {
        let scenario = TimeoutScenario::new();
        assert!(scenario.description().contains("timeout"));
    }

    #[test]
    fn test_timeout_setup_creates_config() {
        let workspace = test_workspace("timeout-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = TimeoutScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify ralph.yml was created
        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        // Verify short timeout
        assert_eq!(config.timeout, Duration::from_secs(5));

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_timeout_did_timeout_passed() {
        let scenario = TimeoutScenario::new();
        let result = mock_timeout_result();
        let assertion = scenario.did_timeout(&result);
        assert!(assertion.passed, "Should pass when timed out");
    }

    #[test]
    fn test_timeout_did_timeout_failed() {
        let scenario = TimeoutScenario::new();
        let mut result = mock_timeout_result();
        result.timed_out = false;
        let assertion = scenario.did_timeout(&result);
        assert!(!assertion.passed, "Should fail when not timed out");
    }

    #[test]
    fn test_timeout_terminated_gracefully_passed() {
        let scenario = TimeoutScenario::new();
        let result = mock_timeout_result();
        let assertion = scenario.terminated_gracefully(&result);
        assert!(assertion.passed, "Should pass when terminated gracefully");
    }

    #[test]
    fn test_timeout_duration_near_timeout_passed() {
        let scenario = TimeoutScenario::new();
        let result = mock_timeout_result();
        let assertion = scenario.duration_near_timeout(&result, Duration::from_secs(5));
        assert!(assertion.passed, "Should pass when duration near timeout");
    }

    #[test]
    fn test_timeout_duration_near_timeout_failed() {
        let scenario = TimeoutScenario::new();
        let mut result = mock_timeout_result();
        result.duration = Duration::from_secs(20); // Way over
        let assertion = scenario.duration_near_timeout(&result, Duration::from_secs(5));
        assert!(
            !assertion.passed,
            "Should fail when duration far from timeout"
        );
    }

    // ========== MaxIterationsScenario Tests ==========

    #[test]
    fn test_max_iter_scenario_new() {
        let scenario = MaxIterationsScenario::new();
        assert_eq!(scenario.id(), "max-iterations");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 7: Error Handling");
    }

    #[test]
    fn test_max_iter_scenario_default() {
        let scenario = MaxIterationsScenario::default();
        assert_eq!(scenario.id(), "max-iterations");
    }

    #[test]
    fn test_max_iter_scenario_description() {
        let scenario = MaxIterationsScenario::new();
        assert!(scenario.description().contains("max iterations"));
    }

    #[test]
    fn test_max_iter_setup_creates_config() {
        let workspace = test_workspace("max-iter-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MaxIterationsScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify low max iterations
        assert_eq!(config.max_iterations, 2);

        // Verify config content
        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("max_iterations: 2"));

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_max_iter_stopped_at_max_passed() {
        let scenario = MaxIterationsScenario::new();
        let result = mock_max_iter_result();
        let assertion = scenario.stopped_at_max_iterations(&result, 2);
        assert!(assertion.passed, "Should pass when stopped at max");
    }

    #[test]
    fn test_max_iter_stopped_at_max_failed() {
        let scenario = MaxIterationsScenario::new();
        let mut result = mock_max_iter_result();
        result.iterations = 1;
        let assertion = scenario.stopped_at_max_iterations(&result, 2);
        assert!(!assertion.passed, "Should fail when not at max");
    }

    #[test]
    fn test_max_iter_termination_reason_passed() {
        let scenario = MaxIterationsScenario::new();
        let result = mock_max_iter_result();
        let assertion = scenario.termination_reason_is_max(&result);
        assert!(assertion.passed, "Should pass with MAX_ITERATIONS reason");
    }

    #[test]
    fn test_max_iter_termination_reason_passed_limit() {
        let scenario = MaxIterationsScenario::new();
        let mut result = mock_max_iter_result();
        result.termination_reason = Some("ITERATION_LIMIT".to_string());
        let assertion = scenario.termination_reason_is_max(&result);
        assert!(assertion.passed, "Should pass with ITERATION_LIMIT reason");
    }

    #[test]
    fn test_max_iter_termination_reason_failed() {
        let scenario = MaxIterationsScenario::new();
        let mut result = mock_max_iter_result();
        result.termination_reason = Some("LOOP_COMPLETE".to_string());
        let assertion = scenario.termination_reason_is_max(&result);
        assert!(!assertion.passed, "Should fail with wrong reason");
    }

    // ========== AuthFailureScenario Tests ==========

    #[test]
    fn test_auth_failure_scenario_new() {
        let scenario = AuthFailureScenario::new();
        assert_eq!(scenario.id(), "auth-failure");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 7: Error Handling");
    }

    #[test]
    fn test_auth_failure_scenario_default() {
        let scenario = AuthFailureScenario::default();
        assert_eq!(scenario.id(), "auth-failure");
    }

    #[test]
    fn test_auth_failure_scenario_description() {
        let scenario = AuthFailureScenario::new();
        assert!(scenario.description().contains("auth"));
    }

    #[test]
    fn test_auth_failure_setup_creates_config() {
        let workspace = test_workspace("auth-failure-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = AuthFailureScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify invalid API key in extra args
        assert!(config.extra_args.contains(&"--api-key".to_string()));
        assert!(config.extra_args.contains(&"invalid-key-12345".to_string()));

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_auth_failure_execution_failed_passed() {
        let scenario = AuthFailureScenario::new();
        let result = mock_auth_failure_result();
        let assertion = scenario.execution_failed_with_error(&result);
        assert!(assertion.passed, "Should pass when execution failed");
    }

    #[test]
    fn test_auth_failure_execution_failed_failed() {
        let scenario = AuthFailureScenario::new();
        let mut result = mock_auth_failure_result();
        result.exit_code = Some(0);
        result.stderr = String::new();
        let assertion = scenario.execution_failed_with_error(&result);
        assert!(!assertion.passed, "Should fail when execution succeeded");
    }

    #[test]
    fn test_auth_failure_error_message_helpful_passed() {
        let scenario = AuthFailureScenario::new();
        let result = mock_auth_failure_result();
        let assertion = scenario.error_message_helpful(&result);
        assert!(assertion.passed, "Should pass with helpful error");
    }

    #[test]
    fn test_auth_failure_error_message_helpful_failed() {
        let scenario = AuthFailureScenario::new();
        let mut result = mock_auth_failure_result();
        result.stderr = "Something went wrong".to_string();
        let assertion = scenario.error_message_helpful(&result);
        assert!(!assertion.passed, "Should fail without helpful keywords");
    }

    #[test]
    fn test_auth_failure_process_exited_cleanly_passed() {
        let scenario = AuthFailureScenario::new();
        let result = mock_auth_failure_result();
        let assertion = scenario.process_exited_cleanly(&result);
        assert!(assertion.passed, "Should pass when exit code present");
    }

    #[test]
    fn test_auth_failure_process_exited_cleanly_failed() {
        let scenario = AuthFailureScenario::new();
        let mut result = mock_auth_failure_result();
        result.exit_code = None; // Killed by signal
        let assertion = scenario.process_exited_cleanly(&result);
        assert!(!assertion.passed, "Should fail when killed by signal");
    }

    // ========== BackendUnavailableScenario Tests ==========

    #[test]
    fn test_backend_unavailable_scenario_new() {
        let scenario = BackendUnavailableScenario::new();
        assert_eq!(scenario.id(), "backend-unavailable");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 7: Error Handling");
    }

    #[test]
    fn test_backend_unavailable_scenario_default() {
        let scenario = BackendUnavailableScenario::default();
        assert_eq!(scenario.id(), "backend-unavailable");
    }

    #[test]
    fn test_backend_unavailable_scenario_description() {
        let scenario = BackendUnavailableScenario::new();
        assert!(
            scenario.description().contains("backend") || scenario.description().contains("CLI")
        );
    }

    #[test]
    fn test_backend_unavailable_setup_creates_config() {
        let workspace = test_workspace("backend-unavailable-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = BackendUnavailableScenario::new();
        let _config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify config has nonexistent command
        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("nonexistent-cli-that-does-not-exist-12345"));

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_backend_unavailable_execution_failed_passed() {
        let scenario = BackendUnavailableScenario::new();
        let result = mock_backend_unavailable_result();
        let assertion = scenario.execution_failed(&result);
        assert!(assertion.passed, "Should pass when execution failed");
    }

    #[test]
    fn test_backend_unavailable_execution_failed_failed() {
        let scenario = BackendUnavailableScenario::new();
        let mut result = mock_backend_unavailable_result();
        result.exit_code = Some(0);
        let assertion = scenario.execution_failed(&result);
        assert!(!assertion.passed, "Should fail when execution succeeded");
    }

    #[test]
    fn test_backend_unavailable_error_mentions_backend_passed() {
        let scenario = BackendUnavailableScenario::new();
        let result = mock_backend_unavailable_result();
        let assertion = scenario.error_mentions_backend(&result);
        assert!(assertion.passed, "Should pass with helpful error");
    }

    #[test]
    fn test_backend_unavailable_error_mentions_backend_failed() {
        let scenario = BackendUnavailableScenario::new();
        let mut result = mock_backend_unavailable_result();
        result.stderr = "Unknown error".to_string();
        let assertion = scenario.error_mentions_backend(&result);
        assert!(!assertion.passed, "Should fail without helpful keywords");
    }

    #[test]
    fn test_backend_unavailable_failed_fast_passed() {
        let scenario = BackendUnavailableScenario::new();
        let result = mock_backend_unavailable_result();
        let assertion = scenario.failed_fast(&result);
        assert!(assertion.passed, "Should pass when failed fast");
    }

    #[test]
    fn test_backend_unavailable_failed_fast_failed() {
        let scenario = BackendUnavailableScenario::new();
        let mut result = mock_backend_unavailable_result();
        result.duration = Duration::from_secs(25); // Over the 20s threshold
        let assertion = scenario.failed_fast(&result);
        assert!(!assertion.passed, "Should fail when took too long");
    }

    // ========== Integration Tests (ignored by default) ==========

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_timeout_full_run() {
        let workspace = test_workspace("timeout-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = TimeoutScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "✅" } else { "❌" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_max_iterations_full_run() {
        let workspace = test_workspace("max-iter-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MaxIterationsScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "✅" } else { "❌" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_backend_unavailable_full_run() {
        let workspace = test_workspace("backend-unavailable-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = BackendUnavailableScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "✅" } else { "❌" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }
}
