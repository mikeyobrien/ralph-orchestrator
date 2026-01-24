//! Tier 1: Connectivity test scenarios.
//!
//! These scenarios verify basic connectivity to AI backends:
//! - Backend availability
//! - Authentication validation
//! - Basic prompt/response roundtrip
//!
//! These are the most fundamental tests and should pass before running
//! more complex scenarios.

use super::{Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::TestResult;
use async_trait::async_trait;
use std::path::Path;

/// Test scenario that verifies basic connectivity to any backend.
///
/// This scenario is backend-agnostic and will work with Claude, Kiro, or OpenCode.
/// It configures itself at setup time based on the target backend.
///
/// The scenario:
/// - Sends a simple prompt asking for "PING" response
/// - Verifies the backend responds
/// - Validates the response contains expected content
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{ConnectivityScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = ConnectivityScenario::new();
/// assert_eq!(scenario.tier(), "Tier 1: Connectivity");
/// assert!(scenario.supported_backends().contains(&Backend::Claude));
/// ```
pub struct ConnectivityScenario {
    id: String,
    description: String,
    tier: String,
}

impl ConnectivityScenario {
    /// Creates a new connectivity scenario.
    pub fn new() -> Self {
        Self {
            id: "connect".to_string(),
            description: "Verifies basic backend connectivity and authentication".to_string(),
            tier: "Tier 1: Connectivity".to_string(),
        }
    }
}

impl Default for ConnectivityScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for ConnectivityScenario {
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

        // Create backend-specific ralph.yml
        let config_content = format!(
            r#"# Connectivity test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
  completion_promise: "PONG"
"#,
            backend,
            backend.as_config_str()
        );

        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Simple connectivity test prompt
        let prompt = r#"You are testing connectivity. Your ONLY task is to respond with the exact word "PONG" (nothing else)."#;

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

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Build assertions
        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.response_contains_pong(&execution),
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

impl ConnectivityScenario {
    /// Asserts that the response contains "PONG".
    fn response_contains_pong(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let contains_pong = result.stdout.to_uppercase().contains("PONG");

        super::AssertionBuilder::new("Response contains PONG")
            .expected("Output contains 'PONG'")
            .actual(if contains_pong {
                "Found PONG in response".to_string()
            } else {
                format!("PONG not found. Output: {}", truncate(&result.stdout, 100))
            })
            .build()
            .with_passed(contains_pong)
    }
}

/// Extension trait for with_passed.
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
    use std::time::Duration;

    #[test]
    fn test_truncate_does_not_panic_on_multibyte_chars() {
        let s = format!("{}✅{}", "x".repeat(99), "y".repeat(10));
        let out = truncate(&s, 100);
        for _ in out.chars() {}
    }

    fn test_workspace(test_name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-connectivity-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    fn cleanup_workspace(path: &std::path::PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    fn mock_execution_result(has_pong: bool) -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: Some(0),
            stdout: if has_pong {
                "PONG".to_string()
            } else {
                "Something else".to_string()
            },
            stderr: String::new(),
            duration: Duration::from_secs(5),
            scratchpad: None,
            events: vec![],
            iterations: 1,
            termination_reason: Some("PONG".to_string()),
            timed_out: false,
        }
    }

    #[test]
    fn test_connectivity_scenario_new() {
        let scenario = ConnectivityScenario::new();
        assert_eq!(scenario.id(), "connect");
        assert_eq!(scenario.tier(), "Tier 1: Connectivity");
    }

    #[test]
    fn test_connectivity_scenario_default() {
        let scenario = ConnectivityScenario::default();
        assert_eq!(scenario.id(), "connect");
    }

    #[test]
    fn test_connectivity_supports_all_backends() {
        let scenario = ConnectivityScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_connectivity_setup_claude() {
        let workspace = test_workspace("setup-claude");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ConnectivityScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: claude"));

        assert_eq!(config.timeout, Backend::Claude.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_connectivity_setup_kiro() {
        let workspace = test_workspace("setup-kiro");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ConnectivityScenario::new();
        let config = scenario.setup(&workspace, Backend::Kiro).unwrap();

        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: kiro"));

        assert_eq!(config.timeout, Backend::Kiro.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_connectivity_setup_opencode() {
        let workspace = test_workspace("setup-opencode");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ConnectivityScenario::new();
        let config = scenario.setup(&workspace, Backend::OpenCode).unwrap();

        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: opencode"));

        assert_eq!(config.timeout, Backend::OpenCode.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_connectivity_pong_assertion_passed() {
        let scenario = ConnectivityScenario::new();
        let result = mock_execution_result(true);
        let assertion = scenario.response_contains_pong(&result);
        assert!(assertion.passed, "Should pass when PONG is in output");
    }

    #[test]
    fn test_connectivity_pong_assertion_failed() {
        let scenario = ConnectivityScenario::new();
        let result = mock_execution_result(false);
        let assertion = scenario.response_contains_pong(&result);
        assert!(!assertion.passed, "Should fail when PONG is not in output");
    }

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_connectivity_full_run_claude() {
        let workspace = test_workspace("full-claude");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ConnectivityScenario::new();
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
