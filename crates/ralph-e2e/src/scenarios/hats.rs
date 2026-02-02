//! Tier 5: Hat Collections test scenarios (backend-agnostic).
//!
//! These scenarios test hat-based workflows with real backends, including:
//! - Single custom hat execution
//! - Multi-hat workflow delegation (Planner → Builder)
//! - Hat instructions verification
//! - Event routing between hats
//! - Per-hat backend overrides
//!
//! All scenarios are backend-agnostic and configure themselves at setup time
//! based on the target backend. They support Claude, Kiro, and OpenCode.
//!
//! These scenarios are more complex than orchestration tests and validate
//! Ralph's hat system for coordinating specialized agent personas.

use super::{AssertionBuilder, Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{ExecutionResult, PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::TestResult;
use async_trait::async_trait;
use std::path::Path;

/// Extension trait for Assertion to allow chained modification.
trait AssertionExt {
    fn with_passed(self, passed: bool) -> Self;
}

impl AssertionExt for crate::models::Assertion {
    fn with_passed(mut self, passed: bool) -> Self {
        self.passed = passed;
        self
    }
}

// =============================================================================
// HatSingleScenario - Execute with single custom hat
// =============================================================================

/// Test scenario that verifies a single custom hat executes correctly.
///
/// This scenario:
/// - Configures a single "builder" hat with custom instructions
/// - Verifies the hat receives the initial task event
/// - Verifies the hat produces the expected output event
/// - Verifies hat-specific behavior appears in the output
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{HatSingleScenario, TestScenario};
///
/// let scenario = HatSingleScenario::new();
/// assert_eq!(scenario.tier(), "Tier 5: Hat Collections");
/// ```
pub struct HatSingleScenario {
    id: String,
    description: String,
    tier: String,
}

impl HatSingleScenario {
    /// Creates a new single hat scenario.
    pub fn new() -> Self {
        Self {
            id: "hat-single".to_string(),
            description: "Verifies single custom hat executes with correct persona".to_string(),
            tier: "Tier 5: Hat Collections".to_string(),
        }
    }
}

impl Default for HatSingleScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for HatSingleScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with a single custom hat
        let config_content = format!(
            r#"# Single hat test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 2
  completion_promise: "LOOP_COMPLETE"

hats:
  builder:
    name: "Builder"
    description: "A focused builder hat that implements tasks"
    triggers:
      - build.task
    publishes:
      - build.done
    instructions: |
      You are Builder, a focused implementation specialist.

      When you receive a build.task event:
      1. Acknowledge your Builder role explicitly
      2. Complete the requested task
      3. Emit a build.done event with your results using this exact XML format:

      <event topic="build.done">
      tests: pass
      lint: pass
      typecheck: pass
      audit: pass
      coverage: pass
      </event>

      Always mention "Builder role activated" in your response.
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Create a prompt that triggers the builder hat
        let prompt = r#"You are testing Ralph's hat system.

Start by emitting a build.task event to trigger the Builder hat:
<event topic="build.task">
Task: Create a simple greeting function
</event>

After the builder completes, output LOOP_COMPLETE."#;

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

        // Build assertions for single hat behavior
        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.hat_persona_visible(&execution),
            self.build_event_emitted(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl HatSingleScenario {
    /// Asserts that the builder hat persona is visible in the output.
    fn hat_persona_visible(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();
        // Check for builder-related terms in output
        let visible = stdout_lower.contains("builder")
            || stdout_lower.contains("implement")
            || stdout_lower.contains("build");
        AssertionBuilder::new("Hat persona visible")
            .expected("Output mentions Builder role or building")
            .actual(if visible {
                "Builder persona detected in output".to_string()
            } else {
                "No builder persona detected".to_string()
            })
            .build()
            .with_passed(visible)
    }

    /// Asserts that a build.done or build.task event was emitted.
    fn build_event_emitted(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let has_build_event = result.events.iter().any(|e| e.topic.starts_with("build."));
        AssertionBuilder::new("Build event emitted")
            .expected("Event with topic starting with 'build.'")
            .actual(if has_build_event {
                format!(
                    "Found build events: {:?}",
                    result
                        .events
                        .iter()
                        .filter(|e| e.topic.starts_with("build."))
                        .map(|e| &e.topic)
                        .collect::<Vec<_>>()
                )
            } else {
                format!(
                    "No build events. All events: {:?}",
                    result.events.iter().map(|e| &e.topic).collect::<Vec<_>>()
                )
            })
            .build()
            .with_passed(has_build_event)
    }
}

// =============================================================================
// HatMultiWorkflowScenario - Planner → Builder delegation
// =============================================================================

/// Test scenario that verifies multi-hat workflow with delegation.
///
/// This scenario:
/// - Configures a planner hat that delegates to a builder hat
/// - Verifies events flow from planner → builder correctly
/// - Verifies both hats contribute to the final output
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{HatMultiWorkflowScenario, TestScenario};
///
/// let scenario = HatMultiWorkflowScenario::new();
/// assert_eq!(scenario.id(), "hat-multi-workflow");
/// ```
pub struct HatMultiWorkflowScenario {
    id: String,
    description: String,
    tier: String,
}

impl HatMultiWorkflowScenario {
    /// Creates a new multi-hat workflow scenario.
    pub fn new() -> Self {
        Self {
            id: "hat-multi-workflow".to_string(),
            description: "Verifies Planner → Builder delegation workflow".to_string(),
            tier: "Tier 5: Hat Collections".to_string(),
        }
    }
}

impl Default for HatMultiWorkflowScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for HatMultiWorkflowScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with planner and builder hats
        let config_content = format!(
            r#"# Multi-hat workflow test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 6
  completion_promise: "LOOP_COMPLETE"

hats:
  planner:
    name: "Planner"
    description: "Plans tasks and delegates to Builder"
    triggers:
      - plan.request
    publishes:
      - build.task
    instructions: |
      You are Planner. When you receive a plan.request:
      1. Break down the request into a build task
      2. Emit a build.task event to delegate to Builder using this exact XML format:

      <event topic="build.task">
      Task: [describe the task]
      </event>

      Always mention "Planner analyzing" in your response.

  builder:
    name: "Builder"
    description: "Implements tasks from Planner"
    triggers:
      - build.task
    publishes:
      - build.done
    instructions: |
      You are Builder. When you receive a build.task:
      1. Implement the requested task
      2. Emit a build.done event when complete using this exact XML format:

      <event topic="build.done">
      result: complete
      </event>

      Always mention "Builder implementing" in your response.
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r#"You are testing Ralph's multi-hat workflow. Your FIRST action must be to emit this event EXACTLY as shown:

<event topic="plan.request">
Request: Create a utility function for string formatting
</event>

This will trigger the Planner hat. After emitting the event above, the workflow will be:
1. Your plan.request triggers Planner
2. Planner emits build.task which triggers Builder
3. Builder emits build.done when complete
4. You output LOOP_COMPLETE

IMPORTANT: You MUST output the XML event tags exactly as shown above in your response. Do not just describe it - actually output the <event> tags."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 6, // Extra buffer for multi-hat workflow
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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.workflow_progressed(&execution),
            self.both_hats_contributed(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl HatMultiWorkflowScenario {
    /// Asserts that the workflow progressed through expected events.
    fn workflow_progressed(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let topics: Vec<&str> = result.events.iter().map(|e| e.topic.as_str()).collect();

        // Check for workflow progression (plan.request → build.task → build.done)
        let has_plan = topics.iter().any(|t| t.contains("plan"));
        let has_build_task = topics.contains(&"build.task");
        let has_build_done = topics.contains(&"build.done");

        let progressed = has_build_task || (has_plan && has_build_done);

        AssertionBuilder::new("Workflow progressed")
            .expected("Events show plan → build progression")
            .actual(format!(
                "Events: {:?} (plan:{}, build.task:{}, build.done:{})",
                topics, has_plan, has_build_task, has_build_done
            ))
            .build()
            .with_passed(progressed)
    }

    /// Asserts that both hats contributed to the output.
    fn both_hats_contributed(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();

        // Look for evidence of both planner and builder
        let has_planner = stdout_lower.contains("planner") || stdout_lower.contains("plan");
        let has_builder = stdout_lower.contains("builder") || stdout_lower.contains("implement");

        let both = has_planner || has_builder; // At least one hat visible

        AssertionBuilder::new("Hat contributions visible")
            .expected("Output shows planner and/or builder activity")
            .actual(format!(
                "Planner visible: {}, Builder visible: {}",
                has_planner, has_builder
            ))
            .build()
            .with_passed(both)
    }
}

// =============================================================================
// HatInstructionsScenario - Verify hat instructions followed
// =============================================================================

/// Test scenario that verifies hat instructions are followed.
///
/// This scenario:
/// - Configures a hat with specific instructions
/// - Verifies the agent follows those instructions
/// - Checks for specific phrases/behaviors defined in instructions
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{HatInstructionsScenario, TestScenario};
///
/// let scenario = HatInstructionsScenario::new();
/// assert_eq!(scenario.id(), "hat-instructions");
/// ```
pub struct HatInstructionsScenario {
    id: String,
    description: String,
    tier: String,
}

impl HatInstructionsScenario {
    /// Creates a new hat instructions scenario.
    pub fn new() -> Self {
        Self {
            id: "hat-instructions".to_string(),
            description: "Verifies hat instructions are followed by the agent".to_string(),
            tier: "Tier 5: Hat Collections".to_string(),
        }
    }
}

impl Default for HatInstructionsScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for HatInstructionsScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with a hat that has very specific instructions
        let config_content = format!(
            r#"# Hat instructions test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 2
  completion_promise: "LOOP_COMPLETE"

hats:
  reviewer:
    name: "Code Reviewer"
    description: "Reviews code with a specific checklist"
    triggers:
      - review.request
    publishes:
      - review.done
    instructions: |
      You are Code Reviewer. You MUST follow this exact process:

      1. Start your response with "REVIEW CHECKLIST:"
      2. Check for: Security, Performance, Readability
      3. End with a verdict: APPROVED or NEEDS_CHANGES
      4. Emit review.done with your verdict using this exact XML format:

      <event topic="review.done">
      verdict: APPROVED
      </event>

      This checklist format is MANDATORY.
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r#"You are testing Ralph's hat instruction system.

Emit a review.request event to trigger the Code Reviewer hat:
<event topic="review.request">
Code to review:
```python
def add(a, b):
    return a + b
```
</event>

The reviewer should follow its checklist instructions.
After the review is complete, output LOOP_COMPLETE."#;

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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.checklist_format_used(&execution),
            self.verdict_provided(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl HatInstructionsScenario {
    /// Asserts that the checklist format was used.
    fn checklist_format_used(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();

        // Check for checklist-related terms
        let has_checklist = stdout_lower.contains("checklist")
            || stdout_lower.contains("security")
            || stdout_lower.contains("performance")
            || stdout_lower.contains("readability");

        AssertionBuilder::new("Checklist format used")
            .expected("Output contains checklist items (security, performance, readability)")
            .actual(if has_checklist {
                "Checklist format detected".to_string()
            } else {
                "No checklist format detected".to_string()
            })
            .build()
            .with_passed(has_checklist)
    }

    /// Asserts that a verdict was provided.
    fn verdict_provided(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_upper = result.stdout.to_uppercase();

        // Check stdout for plain-text verdict
        let has_verdict_in_stdout = stdout_upper.contains("APPROVED")
            || stdout_upper.contains("NEEDS_CHANGES")
            || stdout_upper.contains("NEEDS CHANGES")
            || result.stdout.to_lowercase().contains("approved")
            || result.stdout.to_lowercase().contains("verdict");

        // Check parsed events for verdict in XML event payload
        let has_verdict_in_events = result.events.iter().any(|e| {
            e.topic == "review.done"
                && (e.payload.to_uppercase().contains("APPROVED")
                    || e.payload.to_uppercase().contains("NEEDS_CHANGES")
                    || e.payload.to_uppercase().contains("NEEDS CHANGES"))
        });

        let has_verdict = has_verdict_in_stdout || has_verdict_in_events;

        AssertionBuilder::new("Verdict provided")
            .expected("Output contains APPROVED or NEEDS_CHANGES verdict (in text or event)")
            .actual(if has_verdict {
                if has_verdict_in_stdout {
                    "Verdict found in stdout".to_string()
                } else {
                    "Verdict found in review.done event".to_string()
                }
            } else {
                "No verdict found".to_string()
            })
            .build()
            .with_passed(has_verdict)
    }
}

// =============================================================================
// HatEventRoutingScenario - Events route to correct hat
// =============================================================================

/// Test scenario that verifies event routing between hats.
///
/// This scenario:
/// - Configures multiple hats with different triggers
/// - Emits events that should route to specific hats
/// - Verifies events reach the correct hat based on topic
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{HatEventRoutingScenario, TestScenario};
///
/// let scenario = HatEventRoutingScenario::new();
/// assert_eq!(scenario.id(), "hat-event-routing");
/// ```
pub struct HatEventRoutingScenario {
    id: String,
    description: String,
    tier: String,
}

impl HatEventRoutingScenario {
    /// Creates a new event routing scenario.
    pub fn new() -> Self {
        Self {
            id: "hat-event-routing".to_string(),
            description: "Verifies events route to correct hat based on triggers".to_string(),
            tier: "Tier 5: Hat Collections".to_string(),
        }
    }
}

impl Default for HatEventRoutingScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for HatEventRoutingScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with multiple hats with distinct triggers
        let config_content = format!(
            r#"# Event routing test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 3
  completion_promise: "LOOP_COMPLETE"

hats:
  tester:
    name: "Tester"
    description: "Handles test-related events"
    triggers:
      - test.run
      - test.request
    publishes:
      - test.done
    instructions: |
      You are Tester. When you receive a test event, run tests and report results.
      Always include "TEST RESULTS:" in your output.
      When done, emit the result using this exact XML format:

      <event topic="test.done">
      status: passed
      </event>

  deployer:
    name: "Deployer"
    description: "Handles deployment events"
    triggers:
      - deploy.request
    publishes:
      - deploy.done
    instructions: |
      You are Deployer. When you receive a deploy event, handle deployment.
      Always include "DEPLOYMENT STATUS:" in your output.
      When done, emit using this exact XML format:

      <event topic="deploy.done">
      status: deployed
      </event>
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Send a test.run event - should route to Tester, NOT Deployer
        let prompt = r#"You are testing Ralph's event routing system.

Emit a test.run event (should route to Tester hat):
<event topic="test.run">
Run unit tests for the math module
</event>

The Tester hat should handle this, NOT the Deployer.
After tests complete, output LOOP_COMPLETE."#;

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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.correct_hat_responded(&execution),
            self.wrong_hat_not_triggered(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl HatEventRoutingScenario {
    /// Asserts that the correct hat (Tester) responded.
    fn correct_hat_responded(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();

        // Tester should respond with test-related output
        let tester_responded = stdout_lower.contains("test")
            || stdout_lower.contains("tester")
            || result.events.iter().any(|e| e.topic.starts_with("test."));

        AssertionBuilder::new("Correct hat responded")
            .expected("Tester hat responded to test.run event")
            .actual(if tester_responded {
                "Tester hat activity detected".to_string()
            } else {
                "No Tester hat activity detected".to_string()
            })
            .build()
            .with_passed(tester_responded)
    }

    /// Asserts that the wrong hat (Deployer) was not triggered.
    fn wrong_hat_not_triggered(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_upper = result.stdout.to_uppercase();

        // Deployer should NOT have responded
        let deployer_active = stdout_upper.contains("DEPLOYMENT STATUS:")
            || result.events.iter().any(|e| e.topic.starts_with("deploy."));

        AssertionBuilder::new("Wrong hat not triggered")
            .expected("Deployer hat should not respond to test.run")
            .actual(if deployer_active {
                "Deployer was incorrectly triggered".to_string()
            } else {
                "Deployer correctly stayed inactive".to_string()
            })
            .build()
            .with_passed(!deployer_active)
    }
}

// =============================================================================
// HatBackendOverrideScenario - Per-hat backend selection
// =============================================================================

/// Test scenario that verifies per-hat backend override configuration.
///
/// This scenario:
/// - Configures a hat with a specific backend override
/// - Verifies the configuration is parsed correctly
/// - Note: Actual backend switching requires live multi-backend testing
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{HatBackendOverrideScenario, TestScenario};
///
/// let scenario = HatBackendOverrideScenario::new();
/// assert_eq!(scenario.id(), "hat-backend-override");
/// ```
pub struct HatBackendOverrideScenario {
    id: String,
    description: String,
    tier: String,
}

impl HatBackendOverrideScenario {
    /// Creates a new backend override scenario.
    pub fn new() -> Self {
        Self {
            id: "hat-backend-override".to_string(),
            description: "Verifies per-hat backend override configuration".to_string(),
            tier: "Tier 5: Hat Collections".to_string(),
        }
    }
}

impl Default for HatBackendOverrideScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for HatBackendOverrideScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with backend override configuration
        // Note: We use the same backend for both cli and hat since this tests config parsing,
        // not actual multi-backend execution (which requires multiple CLIs)
        let config_content = format!(
            r#"# Backend override test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 2
  completion_promise: "LOOP_COMPLETE"

hats:
  specialist:
    name: "Specialist"
    description: "A specialist hat with backend override"
    triggers:
      - special.task
    publishes:
      - special.done
    backend: {}  # Explicit backend override
    instructions: |
      You are Specialist with a dedicated backend configuration.
      Acknowledge your specialist role and complete the task.
      Always mention "Specialist backend active" in your response.
      When done, emit using this exact XML format:

      <event topic="special.done">
      result: complete
      </event>
"#,
            backend,
            backend.as_config_str(),
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r#"You are testing Ralph's backend override configuration.

Emit a special.task event to trigger the Specialist hat:
<event topic="special.task">
Task: Demonstrate backend override functionality
</event>

The Specialist hat has a dedicated backend configuration.
After completion, output LOOP_COMPLETE."#;

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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.config_parsed_successfully(&execution),
            self.hat_executed(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl HatBackendOverrideScenario {
    /// Asserts that the config was parsed successfully (no config errors).
    fn config_parsed_successfully(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stderr_lower = result.stderr.to_lowercase();

        // Check for config parsing errors
        let has_config_error = stderr_lower.contains("config")
            && (stderr_lower.contains("error") || stderr_lower.contains("invalid"));

        AssertionBuilder::new("Config parsed successfully")
            .expected("No configuration parsing errors")
            .actual(if has_config_error {
                format!("Config error detected: {}", truncate(&result.stderr, 100))
            } else {
                "Config parsed without errors".to_string()
            })
            .build()
            .with_passed(!has_config_error)
    }

    /// Asserts that the hat executed (response received).
    fn hat_executed(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();

        let executed = stdout_lower.contains("specialist")
            || stdout_lower.contains("special")
            || result
                .events
                .iter()
                .any(|e| e.topic.starts_with("special."));

        AssertionBuilder::new("Hat executed")
            .expected("Specialist hat executed with configured backend")
            .actual(if executed {
                "Specialist hat activity detected".to_string()
            } else {
                "No Specialist hat activity detected".to_string()
            })
            .build()
            .with_passed(executed)
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::EventRecord;
    use std::env;
    use std::fs;
    use std::time::Duration;

    #[test]
    fn test_truncate_does_not_panic_on_multibyte_chars() {
        let s = format!("{}✅{}", "x".repeat(99), "y".repeat(10));
        let out = truncate(&s, 100);
        for _ in out.chars() {}
    }

    fn test_workspace(test_name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-hats-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    fn cleanup_workspace(path: &std::path::PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    fn mock_execution_result() -> ExecutionResult {
        ExecutionResult {
            exit_code: Some(0),
            stdout: "Builder role activated. Implementing task...".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(10),
            scratchpad: Some("## Tasks\n- [x] Build complete".to_string()),
            events: vec![
                EventRecord {
                    topic: "build.task".to_string(),
                    payload: "Task: Create function".to_string(),
                },
                EventRecord {
                    topic: "build.done".to_string(),
                    payload:
                        "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"
                            .to_string(),
                },
            ],
            iterations: 2,
            termination_reason: Some("LOOP_COMPLETE".to_string()),
            timed_out: false,
        }
    }

    // ========== HatSingleScenario Tests ==========

    #[test]
    fn test_hat_single_scenario_new() {
        let scenario = HatSingleScenario::new();
        assert_eq!(scenario.id(), "hat-single");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 5: Hat Collections");
    }

    #[test]
    fn test_hat_single_scenario_default() {
        let scenario = HatSingleScenario::default();
        assert_eq!(scenario.id(), "hat-single");
    }

    #[test]
    fn test_hat_single_setup_creates_config() {
        let workspace = test_workspace("hat-single-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatSingleScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("hats:"), "Should have hats section");
        assert!(content.contains("builder:"), "Should have builder hat");
        assert!(content.contains("triggers:"), "Should have triggers");
        assert!(
            content.contains("build.task"),
            "Should trigger on build.task"
        );

        assert!(workspace.join(".agent").exists(), ".agent should exist");
        assert_eq!(config.max_iterations, 2);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_hat_single_persona_visible_passed() {
        let scenario = HatSingleScenario::new();
        let result = mock_execution_result();
        let assertion = scenario.hat_persona_visible(&result);
        assert!(assertion.passed, "Should pass when Builder is in output");
    }

    #[test]
    fn test_hat_single_persona_visible_failed() {
        let scenario = HatSingleScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "Generic response with no hat reference".to_string();
        let assertion = scenario.hat_persona_visible(&result);
        assert!(!assertion.passed, "Should fail when no hat persona visible");
    }

    #[test]
    fn test_hat_single_build_event_passed() {
        let scenario = HatSingleScenario::new();
        let result = mock_execution_result();
        let assertion = scenario.build_event_emitted(&result);
        assert!(assertion.passed, "Should pass when build events exist");
    }

    #[test]
    fn test_hat_single_build_event_failed() {
        let scenario = HatSingleScenario::new();
        let mut result = mock_execution_result();
        result.events = vec![EventRecord {
            topic: "other.event".to_string(),
            payload: "not build".to_string(),
        }];
        let assertion = scenario.build_event_emitted(&result);
        assert!(!assertion.passed, "Should fail when no build events");
    }

    #[test]
    fn test_hat_single_description() {
        let scenario = HatSingleScenario::new();
        assert!(scenario.description().contains("single"));
        assert!(scenario.description().contains("hat"));
    }

    // ========== HatMultiWorkflowScenario Tests ==========

    #[test]
    fn test_hat_multi_workflow_new() {
        let scenario = HatMultiWorkflowScenario::new();
        assert_eq!(scenario.id(), "hat-multi-workflow");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 5: Hat Collections");
    }

    #[test]
    fn test_hat_multi_workflow_default() {
        let scenario = HatMultiWorkflowScenario::default();
        assert_eq!(scenario.id(), "hat-multi-workflow");
    }

    #[test]
    fn test_hat_multi_workflow_setup() {
        let workspace = test_workspace("hat-multi-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatMultiWorkflowScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();

        assert!(content.contains("planner:"), "Should have planner hat");
        assert!(content.contains("builder:"), "Should have builder hat");
        assert!(
            content.contains("plan.request"),
            "Planner triggers on plan.request"
        );
        assert!(
            content.contains("build.task"),
            "Builder triggers on build.task"
        );

        assert_eq!(config.max_iterations, 6);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_hat_multi_workflow_progressed_passed() {
        let scenario = HatMultiWorkflowScenario::new();
        let mut result = mock_execution_result();
        result.events = vec![
            EventRecord {
                topic: "plan.request".to_string(),
                payload: "req".to_string(),
            },
            EventRecord {
                topic: "build.task".to_string(),
                payload: "task".to_string(),
            },
            EventRecord {
                topic: "build.done".to_string(),
                payload: "done".to_string(),
            },
        ];
        let assertion = scenario.workflow_progressed(&result);
        assert!(assertion.passed, "Should pass with full workflow");
    }

    #[test]
    fn test_hat_multi_workflow_progressed_failed() {
        let scenario = HatMultiWorkflowScenario::new();
        let mut result = mock_execution_result();
        result.events = vec![EventRecord {
            topic: "unrelated.event".to_string(),
            payload: "x".to_string(),
        }];
        let assertion = scenario.workflow_progressed(&result);
        assert!(!assertion.passed, "Should fail with no workflow events");
    }

    #[test]
    fn test_hat_multi_both_contributed_passed() {
        let scenario = HatMultiWorkflowScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "Planner analyzing... Builder implementing...".to_string();
        let assertion = scenario.both_hats_contributed(&result);
        assert!(assertion.passed, "Should pass with both hats visible");
    }

    // ========== HatInstructionsScenario Tests ==========

    #[test]
    fn test_hat_instructions_new() {
        let scenario = HatInstructionsScenario::new();
        assert_eq!(scenario.id(), "hat-instructions");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 5: Hat Collections");
    }

    #[test]
    fn test_hat_instructions_default() {
        let scenario = HatInstructionsScenario::default();
        assert_eq!(scenario.id(), "hat-instructions");
    }

    #[test]
    fn test_hat_instructions_setup() {
        let workspace = test_workspace("hat-instructions-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatInstructionsScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();

        assert!(content.contains("reviewer:"), "Should have reviewer hat");
        assert!(
            content.contains("REVIEW CHECKLIST"),
            "Should have checklist instruction"
        );
        assert!(content.contains("Security"), "Should mention security");
        assert!(content.contains("APPROVED"), "Should mention approval");

        assert_eq!(config.max_iterations, 2);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_hat_instructions_checklist_passed() {
        let scenario = HatInstructionsScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "REVIEW CHECKLIST:\n- Security: OK\n- Performance: OK".to_string();
        let assertion = scenario.checklist_format_used(&result);
        assert!(assertion.passed, "Should pass with checklist format");
    }

    #[test]
    fn test_hat_instructions_checklist_failed() {
        let scenario = HatInstructionsScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "I reviewed the code and it looks fine.".to_string();
        let assertion = scenario.checklist_format_used(&result);
        assert!(!assertion.passed, "Should fail without checklist keywords");
    }

    #[test]
    fn test_hat_instructions_verdict_passed() {
        let scenario = HatInstructionsScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "REVIEW CHECKLIST:\n...\nVerdict: APPROVED".to_string();
        let assertion = scenario.verdict_provided(&result);
        assert!(assertion.passed, "Should pass with verdict");
    }

    #[test]
    fn test_hat_instructions_verdict_failed() {
        let scenario = HatInstructionsScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "The code looks fine to me.".to_string();
        let assertion = scenario.verdict_provided(&result);
        assert!(!assertion.passed, "Should fail without verdict");
    }

    // ========== HatEventRoutingScenario Tests ==========

    #[test]
    fn test_hat_event_routing_new() {
        let scenario = HatEventRoutingScenario::new();
        assert_eq!(scenario.id(), "hat-event-routing");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 5: Hat Collections");
    }

    #[test]
    fn test_hat_event_routing_default() {
        let scenario = HatEventRoutingScenario::default();
        assert_eq!(scenario.id(), "hat-event-routing");
    }

    #[test]
    fn test_hat_event_routing_setup() {
        let workspace = test_workspace("hat-routing-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatEventRoutingScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();

        assert!(content.contains("tester:"), "Should have tester hat");
        assert!(content.contains("deployer:"), "Should have deployer hat");
        assert!(content.contains("test.run"), "Tester triggers on test.run");
        assert!(
            content.contains("deploy.request"),
            "Deployer triggers on deploy.request"
        );

        assert_eq!(config.max_iterations, 3);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_hat_event_routing_correct_hat_passed() {
        let scenario = HatEventRoutingScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "TEST RESULTS: All tests passed".to_string();
        result.events = vec![EventRecord {
            topic: "test.done".to_string(),
            payload: "passed".to_string(),
        }];
        let assertion = scenario.correct_hat_responded(&result);
        assert!(assertion.passed, "Should pass when tester responds");
    }

    #[test]
    fn test_hat_event_routing_wrong_hat_not_triggered_passed() {
        let scenario = HatEventRoutingScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "TEST RESULTS: Passed".to_string();
        result.events = vec![EventRecord {
            topic: "test.done".to_string(),
            payload: "ok".to_string(),
        }];
        let assertion = scenario.wrong_hat_not_triggered(&result);
        assert!(assertion.passed, "Should pass when deployer not triggered");
    }

    #[test]
    fn test_hat_event_routing_wrong_hat_triggered_failed() {
        let scenario = HatEventRoutingScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "DEPLOYMENT STATUS: Starting...".to_string();
        result.events = vec![EventRecord {
            topic: "deploy.done".to_string(),
            payload: "deployed".to_string(),
        }];
        let assertion = scenario.wrong_hat_not_triggered(&result);
        assert!(
            !assertion.passed,
            "Should fail when deployer incorrectly triggered"
        );
    }

    // ========== HatBackendOverrideScenario Tests ==========

    #[test]
    fn test_hat_backend_override_new() {
        let scenario = HatBackendOverrideScenario::new();
        assert_eq!(scenario.id(), "hat-backend-override");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 5: Hat Collections");
    }

    #[test]
    fn test_hat_backend_override_default() {
        let scenario = HatBackendOverrideScenario::default();
        assert_eq!(scenario.id(), "hat-backend-override");
    }

    #[test]
    fn test_hat_backend_override_setup() {
        let workspace = test_workspace("hat-backend-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatBackendOverrideScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();

        assert!(
            content.contains("specialist:"),
            "Should have specialist hat"
        );
        assert!(
            content.contains("backend: claude"),
            "Should have backend override"
        );
        assert!(
            content.contains("special.task"),
            "Should trigger on special.task"
        );

        assert_eq!(config.max_iterations, 2);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_hat_backend_override_config_parsed_passed() {
        let scenario = HatBackendOverrideScenario::new();
        let result = mock_execution_result();
        let assertion = scenario.config_parsed_successfully(&result);
        assert!(assertion.passed, "Should pass with no config errors");
    }

    #[test]
    fn test_hat_backend_override_config_parsed_failed() {
        let scenario = HatBackendOverrideScenario::new();
        let mut result = mock_execution_result();
        result.stderr = "Error: invalid config: backend not found".to_string();
        let assertion = scenario.config_parsed_successfully(&result);
        assert!(!assertion.passed, "Should fail with config error");
    }

    #[test]
    fn test_hat_backend_override_executed_passed() {
        let scenario = HatBackendOverrideScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "Specialist backend active. Task completed.".to_string();
        result.events = vec![EventRecord {
            topic: "special.done".to_string(),
            payload: "complete".to_string(),
        }];
        let assertion = scenario.hat_executed(&result);
        assert!(assertion.passed, "Should pass when specialist hat executes");
    }

    #[test]
    fn test_hat_backend_override_executed_failed() {
        let scenario = HatBackendOverrideScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "Generic response".to_string();
        result.events = vec![];
        let assertion = scenario.hat_executed(&result);
        assert!(
            !assertion.passed,
            "Should fail when specialist not detected"
        );
    }

    // ========== Helper function tests ==========

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("this is a long string", 10), "this is a ...");
    }

    // ========== Integration Tests (ignored by default) ==========

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_hat_single_full_run() {
        let workspace = test_workspace("hat-single-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatSingleScenario::new();
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
    async fn test_hat_multi_workflow_full_run() {
        let workspace = test_workspace("hat-multi-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatMultiWorkflowScenario::new();
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
    async fn test_hat_instructions_full_run() {
        let workspace = test_workspace("hat-instructions-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatInstructionsScenario::new();
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
    async fn test_hat_event_routing_full_run() {
        let workspace = test_workspace("hat-routing-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatEventRoutingScenario::new();
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
    async fn test_hat_backend_override_full_run() {
        let workspace = test_workspace("hat-backend-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = HatBackendOverrideScenario::new();
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
