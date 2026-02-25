//! Integration tests: validates ACP executor can launch kiro-cli and execute prompts.
//!
//! Requires `kiro-cli` on PATH. Skipped automatically if not available.
//! Run with: cargo test -p ralph-adapters --test acp_executor_integration -- --ignored

use ralph_adapters::{
    AcpExecutor, CliBackend, OutputFormat, PromptMode, SessionResult, StreamHandler,
};
use tempfile::TempDir;

fn kiro_available() -> bool {
    std::process::Command::new("kiro-cli")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[derive(Default, Debug)]
struct CapturingHandler {
    texts: Vec<String>,
    tool_calls: Vec<(String, String)>,
    tool_results: Vec<(String, String)>,
    errors: Vec<String>,
    completed: bool,
}

impl StreamHandler for CapturingHandler {
    fn on_text(&mut self, text: &str) {
        self.texts.push(text.to_string());
    }
    fn on_tool_call(&mut self, name: &str, id: &str, _input: &serde_json::Value) {
        self.tool_calls.push((name.to_string(), id.to_string()));
    }
    fn on_tool_result(&mut self, id: &str, output: &str) {
        self.tool_results.push((id.to_string(), output.to_string()));
    }
    fn on_error(&mut self, error: &str) {
        self.errors.push(error.to_string());
    }
    fn on_complete(&mut self, _result: &SessionResult) {
        self.completed = true;
    }
}

/// Basic smoke test: launch kiro via ACP, read a file, get a response.
#[tokio::test]
#[ignore = "requires live kiro-cli"]
async fn acp_launches_kiro_and_gets_response() {
    if !kiro_available() {
        eprintln!("SKIP: kiro-cli not on PATH");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    std::fs::write(temp_dir.path().join("hello.txt"), "world").unwrap();

    let backend = CliBackend::kiro_acp();
    let executor = AcpExecutor::new(backend, temp_dir.path().to_path_buf());
    let mut handler = CapturingHandler::default();

    let result = executor
        .execute(
            "Read the file hello.txt and tell me its contents. Be brief.",
            &mut handler,
        )
        .await
        .expect("ACP execute should not error");

    assert!(result.success, "ACP execution should succeed");
    assert!(!result.output.is_empty(), "Should produce output");
    assert!(handler.completed, "on_complete should be called");
    assert!(
        result.extracted_text.to_lowercase().contains("world"),
        "Output should contain file contents 'world', got: {}",
        result.extracted_text
    );
}

/// Verify the ACP session operates in the specified workspace directory.
#[tokio::test]
#[ignore = "requires live kiro-cli"]
async fn acp_operates_in_specified_workspace_root() {
    if !kiro_available() {
        eprintln!("SKIP: kiro-cli not on PATH");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let marker = format!("unique-marker-{}", std::process::id());
    std::fs::write(temp_dir.path().join("marker.txt"), &marker).unwrap();

    let backend = CliBackend::kiro_acp();
    let executor = AcpExecutor::new(backend, temp_dir.path().to_path_buf());
    let mut handler = CapturingHandler::default();

    let result = executor
        .execute(
            "List the files in the current directory. Just output the filenames, nothing else.",
            &mut handler,
        )
        .await
        .expect("ACP execute should not error");

    assert!(result.success);
    assert!(
        result.extracted_text.contains("marker.txt"),
        "Kiro should see marker.txt in workspace root, got: {}",
        result.extracted_text
    );
}

/// Verify tool calls and tool results flow through the StreamHandler.
#[tokio::test]
#[ignore = "requires live kiro-cli"]
async fn acp_streams_tool_calls_and_results() {
    if !kiro_available() {
        eprintln!("SKIP: kiro-cli not on PATH");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    std::fs::write(temp_dir.path().join("data.txt"), "test-content-42").unwrap();

    let backend = CliBackend::kiro_acp();
    let executor = AcpExecutor::new(backend, temp_dir.path().to_path_buf());
    let mut handler = CapturingHandler::default();

    let result = executor
        .execute(
            "Read the file data.txt and tell me what it says.",
            &mut handler,
        )
        .await
        .expect("ACP execute should not error");

    assert!(result.success);

    // LLM text should stream through handler
    assert!(
        !handler.texts.is_empty(),
        "handler.texts should capture streamed LLM text"
    );

    // Kiro should have made at least one tool call to read the file
    assert!(
        !handler.tool_calls.is_empty(),
        "handler.tool_calls should capture tool invocations, got none"
    );

    // Tool results should flow back
    assert!(
        !handler.tool_results.is_empty(),
        "handler.tool_results should capture tool outputs, got none"
    );

    // The tool result should contain the file content
    let all_results: String = handler
        .tool_results
        .iter()
        .map(|(_, o)| o.as_str())
        .collect();
    assert!(
        all_results.contains("test-content-42"),
        "Tool result should contain file content, got: {}",
        all_results
    );
}

/// Verify tool trust: without --trust-all-tools, our RalphAcpClient auto-approves
/// permission requests so tools still execute successfully.
#[tokio::test]
#[ignore = "requires live kiro-cli"]
async fn acp_auto_approves_tool_permissions_without_trust_flag() {
    if !kiro_available() {
        eprintln!("SKIP: kiro-cli not on PATH");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    std::fs::write(temp_dir.path().join("secret.txt"), "permission-granted").unwrap();

    // Build kiro-acp backend WITHOUT --trust-all-tools
    let backend = CliBackend {
        command: "kiro-cli".to_string(),
        args: vec!["acp".to_string()],
        prompt_mode: PromptMode::Stdin,
        prompt_flag: None,
        output_format: OutputFormat::Acp,
        env_vars: vec![],
    };

    let executor = AcpExecutor::new(backend, temp_dir.path().to_path_buf());
    let mut handler = CapturingHandler::default();

    let result = executor
        .execute(
            "Read the file secret.txt and tell me its contents. Be brief.",
            &mut handler,
        )
        .await
        .expect("ACP execute should not error");

    // Our request_permission impl auto-approves, so this should still work
    assert!(
        result.success,
        "Should succeed with auto-approved permissions"
    );
    assert!(
        result
            .extracted_text
            .to_lowercase()
            .contains("permission-granted"),
        "Tool should execute after auto-approval, got: {}",
        result.extracted_text
    );
    assert!(
        !handler.tool_calls.is_empty(),
        "Should have tool calls even without --trust-all-tools"
    );
}

/// Verify kiro_acp_with_options passes the --agent flag correctly.
#[test]
fn kiro_acp_with_agent_sets_args() {
    let backend = CliBackend::kiro_acp_with_options(Some("my-custom-agent"), None);
    assert_eq!(backend.command, "kiro-cli");
    assert!(backend.args.contains(&"--agent".to_string()));
    assert!(backend.args.contains(&"my-custom-agent".to_string()));
    assert!(!backend.args.contains(&"--model".to_string()));
}

#[test]
fn kiro_acp_without_agent_has_no_agent_flag() {
    let backend = CliBackend::kiro_acp();
    assert!(!backend.args.contains(&"--agent".to_string()));
    assert!(!backend.args.contains(&"--model".to_string()));
}

/// Verify kiro_acp_with_options passes the --model flag correctly.
#[test]
fn kiro_acp_with_model_sets_args() {
    let backend = CliBackend::kiro_acp_with_options(None, Some("claude-sonnet-4"));
    assert!(backend.args.contains(&"--model".to_string()));
    assert!(backend.args.contains(&"claude-sonnet-4".to_string()));
    assert!(!backend.args.contains(&"--agent".to_string()));
}

#[test]
fn kiro_acp_with_agent_and_model_sets_both() {
    let backend = CliBackend::kiro_acp_with_options(Some("my-agent"), Some("my-model"));
    assert!(backend.args.contains(&"--agent".to_string()));
    assert!(backend.args.contains(&"my-agent".to_string()));
    assert!(backend.args.contains(&"--model".to_string()));
    assert!(backend.args.contains(&"my-model".to_string()));
}

/// Verify that a non-existent command fails with an error.
#[tokio::test]
#[ignore = "requires live kiro-cli"]
async fn acp_nonexistent_command_returns_error() {
    let temp_dir = TempDir::new().unwrap();
    let mut backend = CliBackend::kiro_acp();
    backend.command = "nonexistent-binary-that-does-not-exist".to_string();

    let executor = AcpExecutor::new(backend, temp_dir.path().to_path_buf());
    let mut handler = CapturingHandler::default();

    let result = executor.execute("hello", &mut handler).await;
    assert!(result.is_err(), "Should return Err for missing binary");
}
