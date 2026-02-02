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
}
