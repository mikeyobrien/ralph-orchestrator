//! Tier 2: Orchestration Loop test scenarios (backend-agnostic).
//!
//! These scenarios test the full Ralph orchestration loop, including:
//! - Single iteration completion
//! - Multi-iteration workflows
//! - LOOP_COMPLETE detection
//!
//! These scenarios are backend-agnostic and support all configured backends.
//! They are more complex than Tier 1 connectivity tests and require prompts
//! designed to trigger specific orchestration behaviors.

use super::{Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::TestResult;
use async_trait::async_trait;
use std::path::Path;

/// Test scenario that verifies a single iteration completes successfully.
///
/// This scenario is backend-agnostic and:
/// - Configures max_iterations to 1
/// - Sends a simple task that completes in one turn
/// - Verifies exactly 1 iteration completed
/// - Verifies scratchpad was updated
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{SingleIterScenario, TestScenario};
/// use ralph_e2e::executor::RalphExecutor;
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() {
///     let scenario = SingleIterScenario::new();
///     assert_eq!(scenario.tier(), "Tier 2: Orchestration Loop");
/// }
/// ```
pub struct SingleIterScenario {
    id: String,
    description: String,
    tier: String,
}

impl SingleIterScenario {
    /// Creates a new single iteration scenario.
    pub fn new() -> Self {
        Self {
            id: "single-iter".to_string(),
            description: "Verifies single iteration completion with scratchpad update".to_string(),
            tier: "Tier 2: Orchestration Loop".to_string(),
        }
    }
}

impl Default for SingleIterScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for SingleIterScenario {
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

        // Create ralph.yml with single iteration config
        let config_content = format!(
            r#"# Single iteration test config
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

        // Create a prompt that completes in a single iteration
        let prompt = r"You are testing Ralph orchestration.
Complete this task in a single iteration:
1. Create a simple task list in the scratchpad
2. Mark the task as complete
3. Output LOOP_COMPLETE to signal completion

Write to the scratchpad first, then output LOOP_COMPLETE.";

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 1,
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

        // Execute ralph
        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Build assertions specific to single-iteration behavior
        // Note: We use exit_code_success_or_limit() because Ralph's exit code 2 means
        // "max iterations reached" which is valid when functional behavior succeeds.
        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            Assertions::iterations_within(&execution, 1),
            self.scratchpad_updated(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(),
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl SingleIterScenario {
    /// Asserts that the scratchpad was updated during execution.
    fn scratchpad_updated(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let updated = result
            .scratchpad
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());
        super::AssertionBuilder::new("Scratchpad updated")
            .expected("Scratchpad contains content")
            .actual(if updated {
                "Scratchpad has content".to_string()
            } else {
                "Scratchpad empty or missing".to_string()
            })
            .build()
            .with_passed(updated)
    }
}

/// Test scenario that verifies multi-iteration workflow.
///
/// This scenario is backend-agnostic and:
/// - Configures max_iterations to 3
/// - Sends a task requiring multiple steps
/// - Verifies iteration count progression
/// - Verifies events emitted between iterations
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MultiIterScenario, TestScenario};
///
/// let scenario = MultiIterScenario::new();
/// assert_eq!(scenario.id(), "multi-iter");
/// ```
pub struct MultiIterScenario {
    id: String,
    description: String,
    tier: String,
}

impl MultiIterScenario {
    /// Creates a new multi-iteration scenario.
    pub fn new() -> Self {
        Self {
            id: "multi-iter".to_string(),
            description: "Verifies multi-iteration workflow with event progression".to_string(),
            tier: "Tier 2: Orchestration Loop".to_string(),
        }
    }
}

impl Default for MultiIterScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MultiIterScenario {
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

        // Create ralph.yml allowing multiple iterations
        let config_content = format!(
            r#"# Multi-iteration test config
cli:
  backend: {}

event_loop:
  max_iterations: 3
  completion_promise: "LOOP_COMPLETE"
"#,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Create a prompt designed to require multiple iterations
        // NOTE: The prompt must emphasize the exact XML format since LLMs may paraphrase.
        // We also need to be explicit about emitting events in the agent's text output.
        let prompt = r#"You are testing Ralph orchestration with multiple iterations.

Your task requires emitting events using this EXACT XML format in your text output:

<event topic="TOPIC_NAME">
payload content here
</event>

Complete these phases:
1. First, emit: <event topic="phase.init">Starting phase 1</event>
2. Then emit: <event topic="phase.process">Processing in phase 2</event>
3. Finally emit: <event topic="phase.complete">Done</event>

IMPORTANT: You MUST include the literal XML tags above in your response text.
After all events are emitted, output LOOP_COMPLETE on its own line."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 3,
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

        // Build assertions for multi-iteration behavior
        // Note: We use exit_code_success_or_limit() because Ralph's exit code 2 means
        // "max iterations reached" which is valid when functional behavior succeeds.
        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.iterations_match(&execution, 3),
            self.events_progressed(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(),
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl MultiIterScenario {
    /// Asserts that exactly the expected number of iterations occurred.
    fn iterations_match(
        &self,
        result: &crate::executor::ExecutionResult,
        expected: u32,
    ) -> crate::models::Assertion {
        let matched = result.iterations == expected;
        super::AssertionBuilder::new(format!("Completed in {} iterations", expected))
            .expected(format!("{} iterations", expected))
            .actual(format!("{} iterations", result.iterations))
            .build()
            .with_passed(matched)
    }

    /// Asserts that multiple events were emitted showing progression.
    fn events_progressed(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        // We expect at least 2 events (init, process, and/or complete)
        let event_count = result.events.len();
        let progressed = event_count >= 2;
        super::AssertionBuilder::new("Events show progression")
            .expected("At least 2 events emitted")
            .actual(format!(
                "{} events emitted: {:?}",
                event_count,
                result.events.iter().map(|e| &e.topic).collect::<Vec<_>>()
            ))
            .build()
            .with_passed(progressed)
    }
}

/// Test scenario that verifies LOOP_COMPLETE detection.
///
/// This scenario is backend-agnostic and:
/// - Sends a task that explicitly outputs LOOP_COMPLETE
/// - Verifies the termination reason is detected correctly
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{CompletionScenario, TestScenario};
///
/// let scenario = CompletionScenario::new();
/// assert_eq!(scenario.id(), "completion");
/// ```
pub struct CompletionScenario {
    id: String,
    description: String,
    tier: String,
}

impl CompletionScenario {
    /// Creates a new completion detection scenario.
    pub fn new() -> Self {
        Self {
            id: "completion".to_string(),
            description: "Verifies LOOP_COMPLETE detection terminates orchestration".to_string(),
            tier: "Tier 2: Orchestration Loop".to_string(),
        }
    }
}

impl Default for CompletionScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for CompletionScenario {
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

        // Create ralph.yml - allow multiple iterations but expect early termination
        let config_content = format!(
            r#"# Completion detection test config
cli:
  backend: {}

event_loop:
  max_iterations: 5
  completion_promise: "LOOP_COMPLETE"
"#,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Create a prompt that outputs completion signal quickly
        // NOTE: Ralph uses a dual-confirmation pattern that requires the completion promise
        // to appear in two consecutive iterations. To speed this up, we make it very clear
        // that the agent should output LOOP_COMPLETE multiple times.
        let prompt = r"You are testing Ralph's completion detection.

Your ONLY task is to output the completion signal LOOP_COMPLETE.

IMPORTANT: Ralph uses dual-confirmation. You MUST output LOOP_COMPLETE in this response
and in any subsequent responses until the loop terminates.

Output LOOP_COMPLETE now:
LOOP_COMPLETE

Keep outputting LOOP_COMPLETE until the orchestration ends.";

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 5,
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

        // Build assertions for completion detection
        // Note: We use exit_code_success_or_limit() because Ralph's exit code 2 means
        // "max iterations reached" which is valid when functional behavior succeeds.
        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.loop_complete_detected(&execution),
            self.terminated_early(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(),
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl CompletionScenario {
    /// Asserts that LOOP_COMPLETE was detected as the termination reason.
    fn loop_complete_detected(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let detected = result.termination_reason.as_ref() == Some(&"LOOP_COMPLETE".to_string());
        super::AssertionBuilder::new("LOOP_COMPLETE detected")
            .expected("Termination reason: LOOP_COMPLETE")
            .actual(format!(
                "Termination reason: {:?}",
                result.termination_reason
            ))
            .build()
            .with_passed(detected)
    }

    /// Asserts that the loop terminated before max iterations.
    fn terminated_early(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        // Should complete in fewer than max_iterations (5)
        let early = result.iterations < 5 && result.iterations > 0;
        super::AssertionBuilder::new("Terminated before max iterations")
            .expected("1-4 iterations (early termination)")
            .actual(format!("{} iterations", result.iterations))
            .build()
            .with_passed(early)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::EventRecord;
    use std::env;
    use std::fs;
    use std::time::Duration;

    fn test_workspace(test_name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-orch-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    fn cleanup_workspace(path: &std::path::PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    fn mock_execution_result() -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: Some(0),
            stdout: "[Iteration 1] Working...".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(5),
            scratchpad: Some("## Tasks\n- [x] Test task".to_string()),
            events: vec![],
            iterations: 1,
            termination_reason: Some("LOOP_COMPLETE".to_string()),
            timed_out: false,
        }
    }

    // ========== SingleIterScenario Tests ==========

    #[test]
    fn test_single_iter_scenario_new() {
        let scenario = SingleIterScenario::new();
        assert_eq!(scenario.id(), "single-iter");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 2: Orchestration Loop");
    }

    #[test]
    fn test_single_iter_scenario_default() {
        let scenario = SingleIterScenario::default();
        assert_eq!(scenario.id(), "single-iter");
    }

    #[test]
    fn test_single_iter_setup_creates_config() {
        let workspace = test_workspace("single-iter-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = SingleIterScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify ralph.yml was created
        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        // Verify content includes backend and iterations
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: claude"));
        assert!(content.contains("max_iterations: 1"));
        assert!(content.contains("completion_promise:"));

        // Verify .agent directory was created
        assert!(workspace.join(".agent").exists());

        // Verify config struct
        assert_eq!(config.max_iterations, 1);
        assert_eq!(config.timeout, Backend::Claude.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_single_iter_scratchpad_assertion_passed() {
        let scenario = SingleIterScenario::new();
        let result = mock_execution_result();
        let assertion = scenario.scratchpad_updated(&result);
        assert!(assertion.passed, "Should pass when scratchpad has content");
    }

    #[test]
    fn test_single_iter_scratchpad_assertion_failed_empty() {
        let scenario = SingleIterScenario::new();
        let mut result = mock_execution_result();
        result.scratchpad = Some(String::new());
        let assertion = scenario.scratchpad_updated(&result);
        assert!(!assertion.passed, "Should fail when scratchpad is empty");
    }

    #[test]
    fn test_single_iter_scratchpad_assertion_failed_none() {
        let scenario = SingleIterScenario::new();
        let mut result = mock_execution_result();
        result.scratchpad = None;
        let assertion = scenario.scratchpad_updated(&result);
        assert!(!assertion.passed, "Should fail when scratchpad is missing");
    }

    #[test]
    fn test_single_iter_description() {
        let scenario = SingleIterScenario::new();
        assert!(scenario.description().contains("single iteration"));
    }

    // ========== MultiIterScenario Tests ==========

    #[test]
    fn test_multi_iter_scenario_new() {
        let scenario = MultiIterScenario::new();
        assert_eq!(scenario.id(), "multi-iter");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 2: Orchestration Loop");
    }

    #[test]
    fn test_multi_iter_scenario_default() {
        let scenario = MultiIterScenario::default();
        assert_eq!(scenario.id(), "multi-iter");
    }

    #[test]
    fn test_multi_iter_setup_creates_config() {
        let workspace = test_workspace("multi-iter-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MultiIterScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify ralph.yml was created
        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        // Verify content includes multi-iteration config
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: claude"));
        assert!(content.contains("max_iterations: 3"));

        // Verify config struct
        assert_eq!(config.max_iterations, 3);
        assert_eq!(config.timeout, Backend::Claude.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_multi_iter_iterations_match_passed() {
        let scenario = MultiIterScenario::new();
        let mut result = mock_execution_result();
        result.iterations = 3;
        let assertion = scenario.iterations_match(&result, 3);
        assert!(
            assertion.passed,
            "Should pass when iterations match expected"
        );
    }

    #[test]
    fn test_multi_iter_iterations_match_failed() {
        let scenario = MultiIterScenario::new();
        let mut result = mock_execution_result();
        result.iterations = 2;
        let assertion = scenario.iterations_match(&result, 3);
        assert!(!assertion.passed, "Should fail when iterations don't match");
    }

    #[test]
    fn test_multi_iter_events_progressed_passed() {
        let scenario = MultiIterScenario::new();
        let mut result = mock_execution_result();
        result.events = vec![
            EventRecord {
                topic: "phase.init".to_string(),
                payload: "Starting".to_string(),
            },
            EventRecord {
                topic: "phase.process".to_string(),
                payload: "Processing".to_string(),
            },
            EventRecord {
                topic: "phase.complete".to_string(),
                payload: "Done".to_string(),
            },
        ];
        let assertion = scenario.events_progressed(&result);
        assert!(assertion.passed, "Should pass when multiple events emitted");
    }

    #[test]
    fn test_multi_iter_events_progressed_failed() {
        let scenario = MultiIterScenario::new();
        let mut result = mock_execution_result();
        result.events = vec![EventRecord {
            topic: "single.event".to_string(),
            payload: "Only one".to_string(),
        }];
        let assertion = scenario.events_progressed(&result);
        assert!(!assertion.passed, "Should fail when only one event emitted");
    }

    #[test]
    fn test_multi_iter_description() {
        let scenario = MultiIterScenario::new();
        assert!(scenario.description().contains("multi-iteration"));
    }

    // ========== CompletionScenario Tests ==========

    #[test]
    fn test_completion_scenario_new() {
        let scenario = CompletionScenario::new();
        assert_eq!(scenario.id(), "completion");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 2: Orchestration Loop");
    }

    #[test]
    fn test_completion_scenario_default() {
        let scenario = CompletionScenario::default();
        assert_eq!(scenario.id(), "completion");
    }

    #[test]
    fn test_completion_setup_creates_config() {
        let workspace = test_workspace("completion-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = CompletionScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify ralph.yml was created
        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        // Verify content includes higher max iterations (to test early termination)
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: claude"));
        assert!(content.contains("max_iterations: 5"));
        assert!(content.contains("completion_promise: \"LOOP_COMPLETE\""));

        // Verify config struct
        assert_eq!(config.max_iterations, 5);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_completion_loop_complete_detected_passed() {
        let scenario = CompletionScenario::new();
        let result = mock_execution_result(); // Has LOOP_COMPLETE
        let assertion = scenario.loop_complete_detected(&result);
        assert!(assertion.passed, "Should pass when LOOP_COMPLETE detected");
    }

    #[test]
    fn test_completion_loop_complete_detected_failed() {
        let scenario = CompletionScenario::new();
        let mut result = mock_execution_result();
        result.termination_reason = Some("MAX_ITERATIONS".to_string());
        let assertion = scenario.loop_complete_detected(&result);
        assert!(
            !assertion.passed,
            "Should fail when termination reason is not LOOP_COMPLETE"
        );
    }

    #[test]
    fn test_completion_loop_complete_detected_failed_none() {
        let scenario = CompletionScenario::new();
        let mut result = mock_execution_result();
        result.termination_reason = None;
        let assertion = scenario.loop_complete_detected(&result);
        assert!(!assertion.passed, "Should fail when no termination reason");
    }

    #[test]
    fn test_completion_terminated_early_passed() {
        let scenario = CompletionScenario::new();
        let mut result = mock_execution_result();
        result.iterations = 2; // Less than max 5
        let assertion = scenario.terminated_early(&result);
        assert!(
            assertion.passed,
            "Should pass when terminated before max iterations"
        );
    }

    #[test]
    fn test_completion_terminated_early_failed_at_max() {
        let scenario = CompletionScenario::new();
        let mut result = mock_execution_result();
        result.iterations = 5; // Equals max
        let assertion = scenario.terminated_early(&result);
        assert!(!assertion.passed, "Should fail when ran all max iterations");
    }

    #[test]
    fn test_completion_terminated_early_failed_zero() {
        let scenario = CompletionScenario::new();
        let mut result = mock_execution_result();
        result.iterations = 0; // No iterations
        let assertion = scenario.terminated_early(&result);
        assert!(!assertion.passed, "Should fail when no iterations ran");
    }

    #[test]
    fn test_completion_description() {
        let scenario = CompletionScenario::new();
        assert!(scenario.description().contains("LOOP_COMPLETE"));
    }

    // ========== Integration Test (ignored by default) ==========

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_single_iter_full_run() {
        let workspace = test_workspace("single-iter-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = SingleIterScenario::new();
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
    async fn test_multi_iter_full_run() {
        let workspace = test_workspace("multi-iter-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MultiIterScenario::new();
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
    async fn test_completion_full_run() {
        let workspace = test_workspace("completion-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = CompletionScenario::new();
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
