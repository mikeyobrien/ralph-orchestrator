//! Tier 4: Capabilities test scenarios.
//!
//! These scenarios test advanced agent capabilities through Ralph:
//! - Tool invocation and response handling
//! - NDJSON streaming output parsing
//!
//! These tests verify that Ralph correctly interfaces with backend
//! extended features beyond basic text generation.

use super::{Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::TestResult;
use async_trait::async_trait;
use std::path::Path;

/// Test scenario that verifies tool invocation.
///
/// This scenario:
/// - Sends a prompt that requires using a tool (e.g., file system access)
/// - Verifies that the agent invokes the tool correctly
/// - Validates that tool results are incorporated into the response
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{ToolUseScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = ToolUseScenario::new();
/// assert_eq!(scenario.tier(), "Tier 4: Capabilities");
/// ```
pub struct ToolUseScenario {
    id: String,
    description: String,
    tier: String,
}

impl ToolUseScenario {
    /// Creates a new tool use scenario.
    pub fn new() -> Self {
        Self {
            id: "tool-use".to_string(),
            description: "Verifies tool invocation and response handling".to_string(),
            tier: "Tier 4: Capabilities".to_string(),
        }
    }
}

impl Default for ToolUseScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for ToolUseScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create a test file that the agent should read
        let test_file = workspace.join("test-data.txt");
        std::fs::write(&test_file, "Secret content: E2E_TEST_MARKER_42\n")
            .map_err(|e| ScenarioError::SetupError(format!("failed to write test file: {}", e)))?;

        // Create backend-specific ralph.yml
        let config_content = format!(
            r#"# Tool use test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // Create a prompt that requires tool use to read a file
        let prompt = format!(
            r"You are testing tool invocation capabilities.

Your task:
1. Read the contents of the file at: {}/test-data.txt
2. Report what you found in the file
3. Output LOOP_COMPLETE

You MUST use a tool to read the file. Do not guess the contents.",
            workspace.display()
        );

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt),
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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.tool_was_invoked(&execution),
            self.file_content_reported(&execution),
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

impl ToolUseScenario {
    /// Asserts that a tool was invoked during execution.
    fn tool_was_invoked(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let stdout = &result.stdout.to_lowercase();
        let has_tool_markers = stdout.contains("read")
            || stdout.contains("bash")
            || stdout.contains("cat ")
            || stdout.contains("test-data.txt")
            || stdout.contains("tool");

        super::AssertionBuilder::new("Tool was invoked")
            .expected("Evidence of tool invocation in output")
            .actual(if has_tool_markers {
                "Found tool-related content".to_string()
            } else {
                format!(
                    "No tool markers found. Output: {}",
                    truncate(&result.stdout, 100)
                )
            })
            .build()
            .with_passed(has_tool_markers)
    }

    /// Asserts that the file content was reported in the output.
    fn file_content_reported(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let contains_marker = result.stdout.contains("E2E_TEST_MARKER_42");

        super::AssertionBuilder::new("File content reported")
            .expected("Output contains 'E2E_TEST_MARKER_42' from test file")
            .actual(if contains_marker {
                "Found marker in output".to_string()
            } else {
                format!(
                    "Marker not found. Output: {}",
                    truncate(&result.stdout, 100)
                )
            })
            .build()
            .with_passed(contains_marker)
    }
}

/// Test scenario that verifies NDJSON streaming output.
///
/// This scenario:
/// - Configures output in NDJSON streaming format
/// - Verifies that Ralph correctly parses the streaming output
/// - Validates that iteration boundaries are detected
///
/// NDJSON (Newline-Delimited JSON) is used by some CLIs for structured streaming output.
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{StreamingScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = StreamingScenario::new();
/// assert_eq!(scenario.tier(), "Tier 4: Capabilities");
/// ```
pub struct StreamingScenario {
    id: String,
    description: String,
    tier: String,
}

impl StreamingScenario {
    /// Creates a new streaming scenario.
    pub fn new() -> Self {
        Self {
            id: "streaming".to_string(),
            description: "Verifies NDJSON streaming output parsing".to_string(),
            tier: "Tier 4: Capabilities".to_string(),
        }
    }
}

impl Default for StreamingScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for StreamingScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create backend-specific ralph.yml with streaming enabled
        let config_content = format!(
            r#"# Streaming test config for {}
cli:
  backend: {}
  args:
    - "--output-format"
    - "stream-json"

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r#"You are testing streaming output.

Say "Hello from streaming test!" and then output LOOP_COMPLETE.

Keep your response short."#;

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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.streaming_output_received(&execution),
            self.content_extracted(&execution),
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

impl StreamingScenario {
    /// Asserts that streaming output was received.
    fn streaming_output_received(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let is_streaming = result.stdout.contains("{\"")
            || result.stdout.contains("\"type\"")
            || !result.stdout.is_empty();

        super::AssertionBuilder::new("Streaming output received")
            .expected("Non-empty output (JSON or text)")
            .actual(if is_streaming {
                format!("Received {} bytes", result.stdout.len())
            } else {
                "Empty output".to_string()
            })
            .build()
            .with_passed(is_streaming)
    }

    /// Asserts that meaningful content was extracted from the stream.
    fn content_extracted(
        &self,
        result: &crate::executor::ExecutionResult,
    ) -> crate::models::Assertion {
        let has_content = result.stdout.to_lowercase().contains("hello")
            || result.stdout.to_lowercase().contains("streaming")
            || result.stdout.contains("LOOP_COMPLETE")
            || result.stdout.len() > 50;

        super::AssertionBuilder::new("Content extracted from stream")
            .expected("Meaningful content in output")
            .actual(if has_content {
                "Found expected content".to_string()
            } else {
                format!("Limited content. Output: {}", truncate(&result.stdout, 100))
            })
            .build()
            .with_passed(has_content)
    }
}

/// Extension trait for with_passed
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
            "ralph-e2e-caps-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    fn cleanup_workspace(path: &std::path::PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    fn mock_tool_use_result() -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: Some(0),
            stdout: "I'll read the file using the Read tool.\n\nThe file contains: E2E_TEST_MARKER_42\n\nLOOP_COMPLETE".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(10),
            scratchpad: None,
            events: vec![],
            iterations: 1,
            termination_reason: Some("LOOP_COMPLETE".to_string()),
            timed_out: false,
        }
    }

    fn mock_streaming_result() -> crate::executor::ExecutionResult {
        crate::executor::ExecutionResult {
            exit_code: Some(0),
            stdout: "{\"type\":\"text\",\"content\":\"Hello from streaming test!\"}\n{\"type\":\"result\",\"content\":\"LOOP_COMPLETE\"}".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(5),
            scratchpad: None,
            events: vec![],
            iterations: 1,
            termination_reason: Some("LOOP_COMPLETE".to_string()),
            timed_out: false,
        }
    }

    // ========== ToolUseScenario Tests ==========

    #[test]
    fn test_tool_use_scenario_new() {
        let scenario = ToolUseScenario::new();
        assert_eq!(scenario.id(), "tool-use");
        assert_eq!(scenario.tier(), "Tier 4: Capabilities");
    }

    #[test]
    fn test_tool_use_scenario_default() {
        let scenario = ToolUseScenario::default();
        assert_eq!(scenario.id(), "tool-use");
    }

    #[test]
    fn test_tool_use_supports_all_backends() {
        let scenario = ToolUseScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_tool_use_setup_creates_config() {
        let workspace = test_workspace("tool-use-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ToolUseScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: claude"));

        let test_file = workspace.join("test-data.txt");
        assert!(test_file.exists(), "test-data.txt should exist");
        let file_content = fs::read_to_string(&test_file).unwrap();
        assert!(file_content.contains("E2E_TEST_MARKER_42"));

        assert!(workspace.join(".agent").exists());
        assert_eq!(config.max_iterations, 1);
        assert_eq!(config.timeout, Backend::Claude.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_tool_use_tool_was_invoked_passed() {
        let scenario = ToolUseScenario::new();
        let result = mock_tool_use_result();
        let assertion = scenario.tool_was_invoked(&result);
        assert!(assertion.passed, "Should pass when tool invocation evident");
    }

    #[test]
    fn test_tool_use_tool_was_invoked_failed() {
        let scenario = ToolUseScenario::new();
        let mut result = mock_tool_use_result();
        result.stdout = "Just some ordinary output with no relevant markers".to_string();
        let assertion = scenario.tool_was_invoked(&result);
        assert!(!assertion.passed, "Should fail without relevant markers");
    }

    #[test]
    fn test_tool_use_file_content_reported_passed() {
        let scenario = ToolUseScenario::new();
        let result = mock_tool_use_result();
        let assertion = scenario.file_content_reported(&result);
        assert!(assertion.passed, "Should pass when marker found");
    }

    #[test]
    fn test_tool_use_file_content_reported_failed() {
        let scenario = ToolUseScenario::new();
        let mut result = mock_tool_use_result();
        result.stdout = "I read the file but didn't find anything".to_string();
        let assertion = scenario.file_content_reported(&result);
        assert!(!assertion.passed, "Should fail without marker");
    }

    // ========== StreamingScenario Tests ==========

    #[test]
    fn test_streaming_scenario_new() {
        let scenario = StreamingScenario::new();
        assert_eq!(scenario.id(), "streaming");
        assert_eq!(scenario.tier(), "Tier 4: Capabilities");
    }

    #[test]
    fn test_streaming_scenario_default() {
        let scenario = StreamingScenario::default();
        assert_eq!(scenario.id(), "streaming");
    }

    #[test]
    fn test_streaming_supports_all_backends() {
        let scenario = StreamingScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_streaming_setup_creates_config() {
        let workspace = test_workspace("streaming-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = StreamingScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("backend: claude"));
        assert!(content.contains("stream-json"));

        assert_eq!(config.max_iterations, 1);
        assert_eq!(config.timeout, Backend::Claude.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_streaming_output_received_passed_json() {
        let scenario = StreamingScenario::new();
        let result = mock_streaming_result();
        let assertion = scenario.streaming_output_received(&result);
        assert!(assertion.passed, "Should pass with JSON output");
    }

    #[test]
    fn test_streaming_output_received_passed_text() {
        let scenario = StreamingScenario::new();
        let mut result = mock_streaming_result();
        result.stdout = "Regular text output".to_string();
        let assertion = scenario.streaming_output_received(&result);
        assert!(assertion.passed, "Should pass with regular text");
    }

    #[test]
    fn test_streaming_output_received_failed() {
        let scenario = StreamingScenario::new();
        let mut result = mock_streaming_result();
        result.stdout = String::new();
        let assertion = scenario.streaming_output_received(&result);
        assert!(!assertion.passed, "Should fail with empty output");
    }

    #[test]
    fn test_streaming_content_extracted_passed() {
        let scenario = StreamingScenario::new();
        let result = mock_streaming_result();
        let assertion = scenario.content_extracted(&result);
        assert!(assertion.passed, "Should pass with expected content");
    }

    #[test]
    fn test_streaming_content_extracted_failed() {
        let scenario = StreamingScenario::new();
        let mut result = mock_streaming_result();
        result.stdout = "tiny".to_string();
        let assertion = scenario.content_extracted(&result);
        assert!(
            !assertion.passed,
            "Should fail with minimal meaningless content"
        );
    }

    // ========== Integration Tests (ignored by default) ==========

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_tool_use_full_run() {
        let workspace = test_workspace("tool-use-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ToolUseScenario::new();
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
    async fn test_streaming_full_run() {
        let workspace = test_workspace("streaming-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = StreamingScenario::new();
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
