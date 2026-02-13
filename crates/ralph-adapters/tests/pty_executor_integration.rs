#[cfg(unix)]
mod pty_executor_integration {
    use ralph_adapters::{
        CliBackend, OutputFormat, PromptMode, PtyConfig, PtyExecutor, SessionResult, StreamHandler,
        TerminationType,
    };
    use tempfile::TempDir;

    #[derive(Default)]
    struct CapturingHandler {
        texts: Vec<String>,
        tool_calls: Vec<(String, String, serde_json::Value)>,
        tool_results: Vec<(String, String)>,
        errors: Vec<String>,
        completions: Vec<SessionResult>,
    }

    impl StreamHandler for CapturingHandler {
        fn on_text(&mut self, text: &str) {
            self.texts.push(text.to_string());
        }

        fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value) {
            self.tool_calls
                .push((name.to_string(), id.to_string(), input.clone()));
        }

        fn on_tool_result(&mut self, id: &str, output: &str) {
            self.tool_results.push((id.to_string(), output.to_string()));
        }

        fn on_error(&mut self, error: &str) {
            self.errors.push(error.to_string());
        }

        fn on_complete(&mut self, result: &SessionResult) {
            self.completions.push(result.clone());
        }
    }

    #[tokio::test]
    async fn run_observe_reports_nonzero_exit() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let result = executor
            .run_observe("exit 2", rx)
            .await
            .expect("run_observe");

        assert!(!result.success);
        assert_eq!(result.exit_code, Some(2));
        assert_eq!(result.termination, TerminationType::Natural);
    }

    #[tokio::test]
    async fn run_observe_streaming_ignores_invalid_json_lines() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::StreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let result = executor
            .run_observe_streaming("printf '%s\\n' 'not-json-line'", rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert!(result.output.contains("not-json-line"));
        assert!(handler.texts.is_empty());
        assert!(handler.completions.is_empty());
        assert!(result.extracted_text.is_empty());
    }

    #[tokio::test]
    async fn run_observe_streaming_reports_tool_calls_and_errors() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::StreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let script = r#"printf '%s\n' \
'{"type":"assistant","message":{"content":[{"type":"tool_use","id":"tool-1","name":"Read","input":{"path":"README.md"}}]}}' \
'{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tool-1","content":"done"}]}}' \
'{"type":"result","duration_ms":5,"total_cost_usd":0.0,"num_turns":1,"is_error":true}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert_eq!(handler.tool_calls.len(), 1);
        assert_eq!(handler.tool_results.len(), 1);
        assert_eq!(handler.errors.len(), 1);
        assert_eq!(handler.completions.len(), 1);
        assert!(handler.completions[0].is_error);
        assert!(result.extracted_text.is_empty());
    }

    #[tokio::test]
    async fn run_observe_streaming_pi_stream_json_parses_events() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::PiStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        // Simulate a Pi session with text, tool call, tool result, and turn_end
        let script = r#"printf '%s\n' \
'{"type":"session","version":3,"id":"test","timestamp":"2026-01-01T00:00:00Z","cwd":"/tmp"}' \
'{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Hello from Pi"}}' \
'{"type":"tool_execution_start","toolCallId":"toolu_1","toolName":"bash","args":{"command":"echo hi"}}' \
'{"type":"tool_execution_end","toolCallId":"toolu_1","toolName":"bash","result":{"content":[{"type":"text","text":"hi\n"}]},"isError":false}' \
'{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":100,"output":50,"cacheRead":0,"cacheWrite":0,"totalTokens":150,"cost":{"input":0.001,"output":0.002,"cacheRead":0,"cacheWrite":0,"total":0.05}},"stopReason":"stop"}}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        // Text delta should be captured
        assert!(
            handler.texts.iter().any(|t| t.contains("Hello from Pi")),
            "Expected text delta, got: {:?}",
            handler.texts
        );
        // Tool call should be captured
        assert_eq!(handler.tool_calls.len(), 1);
        assert_eq!(handler.tool_calls[0].0, "bash");
        assert_eq!(handler.tool_calls[0].1, "toolu_1");
        // Tool result should be captured
        assert_eq!(handler.tool_results.len(), 1);
        assert_eq!(handler.tool_results[0].1, "hi\n");
        // on_complete should be called with accumulated cost
        assert_eq!(handler.completions.len(), 1);
        assert!((handler.completions[0].total_cost_usd - 0.05).abs() < 1e-10);
        assert_eq!(handler.completions[0].num_turns, 1);
        assert!(!handler.completions[0].is_error);
        // extracted_text should contain the text for LOOP_COMPLETE detection
        assert!(
            result.extracted_text.contains("Hello from Pi"),
            "Expected extracted text, got: {:?}",
            result.extracted_text
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_pi_multi_turn_cost_accumulation() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::PiStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        // Two turns with different costs
        let script = r#"printf '%s\n' \
'{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":100,"output":50,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.05}},"stopReason":"toolUse"}}' \
'{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":200,"output":100,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.03}},"stopReason":"stop"}}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert_eq!(handler.completions.len(), 1);
        assert!((handler.completions[0].total_cost_usd - 0.08).abs() < 1e-10);
        assert_eq!(handler.completions[0].num_turns, 2);
    }

    #[tokio::test]
    async fn run_observe_streaming_pi_thinking_hidden_without_tui() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::PiStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let script = r#"printf '%s\n' \
'{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":"thinking text"}}' \
'{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.01}},"stopReason":"stop"}}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert!(handler.texts.is_empty());
        assert!(result.extracted_text.is_empty());
    }

    #[tokio::test]
    async fn run_observe_streaming_pi_thinking_shown_in_tui_mode() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::PiStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 80,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let mut executor = PtyExecutor::new(backend, config);
        executor.set_tui_mode(true);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let script = r#"printf '%s\n' \
'{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":"thinking text"}}' \
'{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.01}},"stopReason":"stop"}}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert_eq!(handler.texts, vec!["thinking text"]);
        // Thinking text should not be included in extracted_text (used for event parsing).
        assert!(result.extracted_text.is_empty());
    }
}
