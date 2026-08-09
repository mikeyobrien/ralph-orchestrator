#[cfg(unix)]
mod pty_executor_integration {
    use ralph_adapters::{
        CliBackend, CliExecutor, OutputFormat, PromptMode, PtyConfig, PtyExecutor, SessionResult,
        StreamHandler, TerminationType,
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

        // Two turns with different costs. A text_delta makes this a realistic
        // clean session (TR7: a non-tool turn with zero assistant text would
        // otherwise be flagged as a protocol mismatch).
        let script = r#"printf '%s\n' \
'{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"working"}}' \
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
'{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"final answer"}}' \
'{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.01}},"stopReason":"stop"}}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        // Thinking is hidden outside TUI mode: only the assistant text is
        // surfaced to the handler, never the thinking delta.
        assert!(
            !handler.texts.iter().any(|t| t.contains("thinking text")),
            "thinking must be hidden without TUI: {:?}",
            handler.texts
        );
        assert!(handler.texts.iter().any(|t| t.contains("final answer")));
        // extracted_text (used for event parsing) holds assistant text only.
        assert_eq!(result.extracted_text, "final answer");
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
'{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"final answer"}}' \
'{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.01}},"stopReason":"stop"}}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        // In TUI mode the thinking delta IS surfaced to the handler ...
        assert!(
            handler.texts.iter().any(|t| t.contains("thinking text")),
            "thinking must be shown in TUI mode: {:?}",
            handler.texts
        );
        // ... but extracted_text (used for event parsing) holds assistant text
        // only, never thinking.
        assert_eq!(result.extracted_text, "final answer");
    }

    #[tokio::test]
    async fn run_observe_streaming_pi_protocol_mismatch_header_only() {
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

        // A successful Pi process that emits only header/unknown records must
        // NOT be a silent empty success — TR7 surfaces a protocol mismatch
        // (case 1: zero recognized assistant/tool/turn events).
        let script = r#"printf '%s\n' \
'{"type":"session","version":3}' \
'{"type":"agent_start"}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(
            !result.success,
            "header-only stream must not be a silent success"
        );
        assert_eq!(handler.completions.len(), 1);
        assert!(handler.completions[0].is_error);
        assert!(
            result
                .protocol_error
                .as_ref()
                .is_some_and(|pe| pe.contains("no usable")),
            "must surface a case-1 protocol mismatch: {:?}",
            result.protocol_error
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_pi_parity_no_tui_vs_pty() {
        // Step-2 Demo / AC3: the same fake Pi session produces identical
        // completion text and metrics through the no-TUI CliExecutor and the
        // PtyExecutor. PTY cols are widened so each fixture line fits one row
        // (80-column wrapping is exercised separately by realistic_long_lines).
        let ndjson = [
            r#"{"type":"session","version":3}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"done"}}"#,
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop","usage":{"input":5,"output":7,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.02}}}}"#,
        ];
        let script = format!(
            "printf '%s\\n' {}",
            ndjson
                .iter()
                .map(|l| format!("'{l}'"))
                .collect::<Vec<_>>()
                .join(" ")
        );

        // --- no-TUI: CliExecutor ---
        let cli_backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::PiStreamJson,
            env_vars: vec![],
        };
        let cli_executor = CliExecutor::new(cli_backend);
        let cli_result = cli_executor
            .execute_capture(&script)
            .await
            .expect("no-TUI execute");
        let cli_session = cli_result
            .session_result
            .expect("no-TUI Pi stream must produce a session result");

        // --- PTY: PtyExecutor ---
        let temp_dir = TempDir::new().expect("temp dir");
        let pty_backend = CliBackend {
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
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(pty_backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();
        let pty_result = executor
            .run_observe_streaming(&script, rx, &mut handler)
            .await
            .expect("PTY execute");

        // Identical completion text across modes.
        assert_eq!(
            cli_result.extracted_text.as_deref().unwrap_or(""),
            pty_result.extracted_text,
            "completion text must match across no-TUI and PTY"
        );
        // Identical fixture metrics across modes. The PTY result exposes
        // cost/tokens directly; num_turns comes from the on_complete SessionResult.
        let pty_session = handler
            .completions
            .first()
            .expect("PTY on_complete must fire for a Pi session");
        assert!(
            (cli_session.total_cost_usd - pty_result.total_cost_usd).abs() < 1e-10,
            "cost parity: {} vs {}",
            cli_session.total_cost_usd,
            pty_result.total_cost_usd
        );
        assert_eq!(cli_session.num_turns, pty_session.num_turns);
        assert_eq!(cli_session.input_tokens, pty_result.input_tokens);
        assert_eq!(cli_session.output_tokens, pty_result.output_tokens);
        assert_eq!(cli_session.cache_read_tokens, pty_result.cache_read_tokens);
        assert_eq!(
            cli_session.cache_write_tokens,
            pty_result.cache_write_tokens
        );
        assert!(
            cli_result.success && pty_result.success,
            "both modes must report success for a clean stream"
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_pi_swallowed_delta_recovers_via_turn_end_content() {
        // TR7a (PTY): when assistant text_delta events are an unrecognized shape
        // (routed to Other, accumulating no delta) but turn_end carries the
        // assistant text, the mandatory turn_end.content fallback recovers it,
        // surfaces it to the display handler, and keeps the session non-mismatch.
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
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        // toolcall_start is an unrecognized assistant subevent (-> Other), so no
        // delta text accumulates; recovery must come from turn_end.content.
        let script = r#"printf '%s\n' \
'{"type":"message_update","assistantMessageEvent":{"type":"toolcall_start","contentIndex":0}}' \
'{"type":"turn_end","message":{"content":[{"type":"text","text":"recovered answer"}],"stopReason":"stop","usage":{"input":1,"output":1,"cost":{"total":0.01}}}}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(
            result.success,
            "recovered text means a clean (non-mismatch) session"
        );
        // Recovered fallback text is surfaced to the display handler ...
        assert!(
            handler.texts.iter().any(|t| t.contains("recovered answer")),
            "recovered text must be shown on the display: {:?}",
            handler.texts
        );
        // ... and present in extracted_text for event parsing.
        assert_eq!(result.extracted_text, "recovered answer");
    }

    // =========================================================================
    // OMP (oh-my-pi) — same shared Pi-family processor, distinct OmpStreamJson
    // identity. Fixtures live under tests/fixtures/omp/ (see that README).
    // =========================================================================

    #[tokio::test]
    async fn run_observe_streaming_omp_stream_json_parses_events() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        // An OMP session: text + tool lifecycle (isError omitted → defaults false)
        // + a terminal turn_end with usage/cost. agent_end is ignored.
        let script = r#"printf '%s\n' \
'{"type":"session","version":3,"id":"omp-1","cwd":"/tmp/example"}' \
'{"type":"agent_start"}' \
'{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Hello from OMP"}}' \
'{"type":"tool_execution_start","toolCallId":"toolu_1","toolName":"bash","args":{"command":"echo omp"}}' \
'{"type":"tool_execution_end","toolCallId":"toolu_1","toolName":"bash","result":{"content":[{"type":"text","text":"omp\n"}]}}' \
'{"type":"turn_end","message":{"content":[{"type":"text","text":"Hello from OMP"}],"usage":{"input":100,"output":50,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.05}},"stopReason":"stop"}}' \
'{"type":"agent_end"}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success, "clean OMP stream should succeed");
        assert!(
            handler.texts.iter().any(|t| t.contains("Hello from OMP")),
            "Expected text delta, got: {:?}",
            handler.texts
        );
        assert_eq!(handler.tool_calls.len(), 1);
        assert_eq!(handler.tool_calls[0].0, "bash");
        assert_eq!(handler.tool_results.len(), 1);
        assert_eq!(handler.tool_results[0].1, "omp\n");
        // on_complete fires with accumulated cost; isError stays false even
        // though the tool end omitted isError (OMP optional → default false).
        assert_eq!(handler.completions.len(), 1);
        assert!(!handler.completions[0].is_error);
        assert!((handler.completions[0].total_cost_usd - 0.05).abs() < 1e-10);
        assert_eq!(handler.completions[0].num_turns, 1);
        // extracted_text (LOOP_COMPLETE source) holds the assistant text.
        assert!(
            result.extracted_text.contains("Hello from OMP"),
            "Expected extracted text, got: {:?}",
            result.extracted_text
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_omp_parity_no_tui_vs_pty() {
        // AC8 parity for OMP: the same fake OMP session produces identical
        // completion text and metrics through the no-TUI CliExecutor and the
        // PtyExecutor. PTY cols are widened so each fixture line fits one row.
        let ndjson = [
            r#"{"type":"session","version":3}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"done"}}"#,
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop","usage":{"input":5,"output":7,"cacheRead":3,"cacheWrite":2,"cost":{"total":0.02}}}}"#,
            r#"{"type":"agent_end"}"#,
        ];
        let script = format!(
            "printf '%s\\n' {}",
            ndjson
                .iter()
                .map(|l| format!("'{l}'"))
                .collect::<Vec<_>>()
                .join(" ")
        );

        // --- no-TUI: CliExecutor ---
        let cli_backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };
        let cli_executor = CliExecutor::new(cli_backend);
        let cli_result = cli_executor
            .execute_capture(&script)
            .await
            .expect("no-TUI execute");
        let cli_session = cli_result
            .session_result
            .expect("no-TUI OMP stream must produce a session result");

        // --- PTY: PtyExecutor ---
        let temp_dir = TempDir::new().expect("temp dir");
        let pty_backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(pty_backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();
        let pty_result = executor
            .run_observe_streaming(&script, rx, &mut handler)
            .await
            .expect("PTY execute");

        // Identical completion text across modes.
        assert_eq!(
            cli_result.extracted_text.as_deref().unwrap_or(""),
            pty_result.extracted_text,
            "OMP completion text must match across no-TUI and PTY"
        );
        let pty_session = handler
            .completions
            .first()
            .expect("PTY on_complete must fire for an OMP session");
        assert!(
            (cli_session.total_cost_usd - pty_result.total_cost_usd).abs() < 1e-10,
            "cost parity: {} vs {}",
            cli_session.total_cost_usd,
            pty_result.total_cost_usd
        );
        assert_eq!(cli_session.num_turns, pty_session.num_turns);
        assert_eq!(cli_session.input_tokens, pty_result.input_tokens);
        assert_eq!(cli_session.output_tokens, pty_result.output_tokens);
        assert_eq!(cli_session.cache_read_tokens, pty_result.cache_read_tokens);
        assert_eq!(
            cli_session.cache_write_tokens,
            pty_result.cache_write_tokens
        );
        assert!(
            cli_result.success && pty_result.success,
            "both modes must report success for a clean OMP stream"
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_omp_protocol_mismatch_header_only() {
        // OMP shares the case-1 protocol mismatch: a successful process that
        // emits only header/agent records must NOT be a silent empty success.
        // Drives the committed tests/fixtures/omp/no_usable_events.ndjson fixture
        // through the real PtyExecutor (the file is a real input, not dead).
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/omp/no_usable_events.ndjson");
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let script = format!("cat '{}'", fixture.display());
        let result = executor
            .run_observe_streaming(&script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(
            !result.success,
            "OMP header-only stream must not be a silent success"
        );
        assert_eq!(handler.completions.len(), 1);
        assert!(handler.completions[0].is_error);
        let pe = result
            .protocol_error
            .as_ref()
            .expect("must surface an OMP case-1 mismatch");
        assert!(pe.contains("no usable"), "case-1 wording: {pe}");
        // Design Q1 / TR9: OMP diagnostics must say OMP.
        assert!(
            pe.contains("OMP"),
            "OMP mismatch must be OMP-labelled: {pe}"
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_omp_fixture_malformed_lines_tolerated() {
        // Drives tests/fixtures/omp/malformed_mixed.ndjson: malformed lines are
        // skipped (counted), well-formed records still parse, and the recovered
        // assistant text surfaces (real fixture input, not a dead artifact).
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/omp/malformed_mixed.ndjson");
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        let script = format!("cat '{}'", fixture.display());
        let result = executor
            .run_observe_streaming(&script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(
            result.success,
            "a stream with valid records after malformed lines is a clean session"
        );
        assert!(
            result
                .extracted_text
                .contains("recovered after malformed line"),
            "valid record after malformed lines must be parsed: {:?}",
            result.extracted_text
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_omp_nonzero_exit_classified() {
        // TR8: a non-zero OMP process exit is classified as failure (success ==
        // false, exit code preserved) while the assistant text parsed before the
        // exit is still recovered. The failed process skips the mismatch check.
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        // Emit a valid text_delta + turn_end, then exit non-zero.
        let script = r#"printf '%s\n' \
'{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"partial OMP work"}}' \
'{"type":"turn_end","message":{"content":[],"stopReason":"stop","usage":{"input":1,"output":1,"cost":{"total":0.0}}}}'
exit 7"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(!result.success, "non-zero exit must be a failure");
        assert_eq!(result.exit_code, Some(7), "exit code is preserved");
        // Text parsed before the exit is still recovered.
        assert!(
            result.extracted_text.contains("partial OMP work"),
            "pre-exit assistant text must be recovered: {:?}",
            result.extracted_text
        );
        let session = handler
            .completions
            .first()
            .expect("on_complete must fire even on non-zero exit");
        assert!(session.is_error, "non-zero exit session is an error");
        assert!(
            result.protocol_error.is_none(),
            "failed process must skip the mismatch check"
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_omp_fixture_session_completes() {
        // Drives the committed tests/fixtures/omp/session.ndjson fixture through
        // the real PtyExecutor via a fake `cat`-style backend, proving the
        // fixture is a real input (not a dead artifact) and LOOP_COMPLETE is
        // detected from extracted assistant text — never from raw NDJSON.
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/omp/session.ndjson");
        let ndjson = std::fs::read_to_string(&fixture)
            .unwrap_or_else(|e| panic!("failed to read OMP fixture {}: {e}", fixture.display()));

        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::OmpStreamJson,
            env_vars: vec![],
        };
        let config = PtyConfig {
            interactive: false,
            idle_timeout_secs: 0,
            cols: 200,
            rows: 24,
            workspace_root: temp_dir.path().to_path_buf(),
        };
        let executor = PtyExecutor::new(backend, config);
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut handler = CapturingHandler::default();

        // `cat` the fixture so the fake omp emits the pinned OMP 17.2.10 stream.
        let script = format!("cat '{}'", fixture.display());
        let _ = ndjson; // fixture is exercised via the cat script above
        let result = executor
            .run_observe_streaming(&script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success, "OMP fixture session should succeed");
        // Completion marker detected from extracted assistant text only.
        assert!(
            result.extracted_text.contains("LOOP_COMPLETE"),
            "LOOP_COMPLETE must be recovered from extracted assistant text: {:?}",
            result.extracted_text
        );
        assert!(
            handler.tool_calls.iter().any(|(n, _, _)| n == "bash"),
            "fixture tool lifecycle must be parsed"
        );
        // Metrics from the terminal turn_end.
        let session = handler
            .completions
            .first()
            .expect("on_complete must fire for the OMP fixture");
        assert!(!session.is_error);
        assert_eq!(session.num_turns, 1);
        assert!((session.total_cost_usd - 0.01027).abs() < 1e-10);
    }

    /// Live test: run the actual Pi CLI through the PTY executor.
    /// Skip if `pi` is not installed. This test makes a real API call.
    #[tokio::test]
    #[ignore = "Requires pi CLI + API credentials; run with: cargo test -- --ignored pi_live"]
    async fn run_observe_streaming_pi_live_garbled_text_repro() {
        // Skip if pi is not installed
        if std::process::Command::new("pi")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: pi CLI not found");
            return;
        }

        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend::pi();
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
            .run_observe_streaming(
                "say exactly: 'hello world from pi test' and nothing else. do not use any tools.",
                rx,
                &mut handler,
            )
            .await
            .expect("run_observe_streaming");

        assert!(result.success, "Pi exited with failure");

        // Dump texts for debugging
        eprintln!("=== CAPTURED TEXTS ({} chunks) ===", handler.texts.len());
        for (i, t) in handler.texts.iter().enumerate() {
            eprintln!("  chunk[{}]: {:?}", i, t);
        }

        let all_text: String = handler.texts.iter().cloned().collect();
        eprintln!("=== JOINED TEXT ===\n{}", all_text);

        // The text should contain "hello world from pi test" without garbling
        assert!(
            all_text.contains("hello world from pi test"),
            "Expected text to contain 'hello world from pi test', got: {:?}",
            all_text
        );

        // Check for garbled output: text chunks should NOT have unexpected
        // line breaks in the middle of words
        let has_mid_word_break = handler.texts.windows(2).any(|pair| {
            let prev = &pair[0];
            let next = &pair[1];
            // If previous chunk doesn't end with whitespace/newline
            // and next chunk doesn't start with whitespace/newline
            // that's a suspicious break
            !prev.is_empty()
                && !next.is_empty()
                && !prev.ends_with(|c: char| c.is_whitespace())
                && !next.starts_with(|c: char| c.is_whitespace())
        });

        // This is informational — streaming naturally produces small chunks
        if has_mid_word_break {
            eprintln!("WARNING: Mid-word text breaks detected (may be normal for streaming)");
        }

        // Check extracted_text
        assert!(
            result.extracted_text.contains("hello world from pi test"),
            "Expected extracted_text to contain 'hello world from pi test', got: {:?}",
            result.extracted_text
        );
    }

    /// Live test: run Pi with a complex prompt that generates tool calls.
    #[tokio::test]
    #[ignore = "Requires pi CLI + API credentials"]
    async fn run_observe_streaming_pi_live_complex_prompt() {
        if std::process::Command::new("pi")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: pi CLI not found");
            return;
        }

        let temp_dir = TempDir::new().expect("temp dir");
        // Create a file for Pi to read
        std::fs::write(
            temp_dir.path().join("test.txt"),
            "Hello from test file\nLine 2\nLine 3\n",
        )
        .expect("write test file");

        let backend = CliBackend::pi();
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
            .run_observe_streaming(
                "Read test.txt and tell me how many lines it has. Include the exact count in your response like 'The file has N lines'.",
                rx,
                &mut handler,
            )
            .await
            .expect("run_observe_streaming");

        // Dump all events
        eprintln!("=== CAPTURED TEXTS ({} chunks) ===", handler.texts.len());
        for (i, t) in handler.texts.iter().enumerate() {
            eprintln!("  text[{}]: {:?}", i, t);
        }
        eprintln!("=== TOOL CALLS ({}) ===", handler.tool_calls.len());
        for (i, (name, id, _)) in handler.tool_calls.iter().enumerate() {
            eprintln!("  tool[{}]: {} ({})", i, name, id);
        }
        eprintln!("=== TOOL RESULTS ({}) ===", handler.tool_results.len());
        for (i, (id, output)) in handler.tool_results.iter().enumerate() {
            eprintln!(
                "  result[{}]: {} -> {:?}",
                i,
                id,
                &output[..output.len().min(100)]
            );
        }
        eprintln!("=== COMPLETIONS ({}) ===", handler.completions.len());
        for c in &handler.completions {
            eprintln!(
                "  cost={}, turns={}, error={}",
                c.total_cost_usd, c.num_turns, c.is_error
            );
        }

        let all_text: String = handler.texts.iter().cloned().collect();
        eprintln!("=== JOINED TEXT ===\n{}", all_text);
        eprintln!("=== EXTRACTED TEXT ===\n{}", result.extracted_text);

        assert!(result.success, "Pi exited with failure");

        // Should have at least one tool call (Read)
        assert!(
            !handler.tool_calls.is_empty(),
            "Expected at least one tool call"
        );
    }

    /// Live test: run Pi with a very long prompt (simulating Ralph's hat prompt)
    /// to check if prompt length causes garbled output.
    #[tokio::test]
    #[ignore = "Requires pi CLI + API credentials"]
    async fn run_observe_streaming_pi_live_long_prompt() {
        if std::process::Command::new("pi")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: pi CLI not found");
            return;
        }

        let temp_dir = TempDir::new().expect("temp dir");

        let backend = CliBackend::pi();
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

        // Build a long prompt (~5000 chars) similar to what Ralph generates
        let long_prompt = format!(
            "## SYSTEM INSTRUCTIONS\n\
            You are a software engineering assistant working on a Rust project.\n\
            {padding}\n\
            ## TASK\n\
            Write a numbered list of exactly 5 items about software testing best practices.\n\
            Each item should be one sentence.\n\
            Start your response with exactly 'Here are 5 testing practices:'\n\
            Do not use any tools.",
            padding = "This is padding text to make the prompt longer. ".repeat(80)
        );

        eprintln!("Prompt length: {} chars", long_prompt.len());

        let result = executor
            .run_observe_streaming(&long_prompt, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        eprintln!("=== CAPTURED TEXTS ({} chunks) ===", handler.texts.len());
        for (i, t) in handler.texts.iter().enumerate() {
            let repr = if t.len() > 80 {
                format!("{:?}...", &t[..80])
            } else {
                format!("{:?}", t)
            };
            eprintln!("  text[{}]: {}", i, repr);
        }

        let all_text: String = handler.texts.iter().cloned().collect();
        eprintln!("=== JOINED TEXT ===\n{}", all_text);

        assert!(result.success, "Pi exited with failure");

        // The text should be coherent
        assert!(
            all_text.contains("testing") || all_text.contains("test"),
            "Expected text about testing, got: {:?}",
            &all_text[..all_text.len().min(200)]
        );
    }

    /// Reproduces the Pi streaming issue: realistically long NDJSON lines
    /// (800+ chars each) output one-at-a-time with delays, simulating real streaming.
    #[tokio::test]
    async fn run_observe_streaming_pi_realistic_long_lines_streamed() {
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

        // Write NDJSON to a file and stream it line-by-line with delays (like real Pi)
        let ndjson_path = temp_dir.path().join("pi_output.jsonl");
        std::fs::write(
            &ndjson_path,
            // Each line here is 800+ chars, matching real Pi output
            concat!(
                r#"{"type":"session","version":3,"id":"test-session","timestamp":"2026-01-01T00:00:00Z","cwd":"/tmp"}"#, "\n",
                r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":1,"delta":"Plan is set.","partial":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053},"message":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053}}}"#, "\n",
                r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":1,"delta":"\nThree tasks created.","partial":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set.\nThree tasks created."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053},"message":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set.\nThree tasks created."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053}}}"#, "\n",
                r#"{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":100,"output":50,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.05}},"stopReason":"stop","provider":"kiro","model":"claude-sonnet-4-6"}}"#, "\n",
            ),
        )
        .expect("write ndjson");

        // Stream line-by-line with 10ms delays to simulate real Pi streaming
        let script = format!(
            "while IFS= read -r line; do printf '%s\\n' \"$line\"; sleep 0.01; done < {}",
            ndjson_path.display()
        );

        let result = executor
            .run_observe_streaming(&script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);

        let all_text: String = handler.texts.iter().cloned().collect();
        assert!(
            all_text.contains("Plan is set."),
            "Expected 'Plan is set.' in text, got texts: {:?}",
            handler.texts
        );
        assert!(
            all_text.contains("Three tasks created."),
            "Expected 'Three tasks created.' in text, got texts: {:?}",
            handler.texts
        );
    }

    /// Reproduces the Pi streaming issue where realistically long NDJSON lines
    /// (800+ chars each, matching real Pi output with partial/message fields)
    /// get corrupted when passing through the PTY at 80 columns.
    #[tokio::test]
    async fn run_observe_streaming_pi_realistic_long_lines() {
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

        // Write a script that outputs realistic Pi NDJSON lines (800+ chars each)
        // matching the real Pi output format with redundant partial/message fields.
        let script_path = temp_dir.path().join("pi_sim.sh");
        std::fs::write(
            &script_path,
            r#"#!/bin/sh
cat <<'NDJSON'
{"type":"session","version":3,"id":"test-session","timestamp":"2026-01-01T00:00:00Z","cwd":"/tmp"}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":1,"delta":"Plan is set.","partial":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053},"message":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053}}}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":1,"delta":"\nThree tasks created.","partial":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set.\nThree tasks created."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053},"message":{"role":"assistant","content":[{"type":"thinking","thinking":"The user wants me to create a detailed plan for reviewing changes."},{"type":"text","text":"Plan is set.\nThree tasks created."}],"api":"kiro-api","provider":"kiro","model":"claude-sonnet-4-6","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1772160820053}}}
{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":100,"output":50,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.05}},"stopReason":"stop","provider":"kiro","model":"claude-sonnet-4-6"}}
NDJSON
"#,
        )
        .expect("write script");
        std::fs::set_permissions(
            &script_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .expect("chmod");

        let result = executor
            .run_observe_streaming(script_path.to_str().unwrap(), rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);

        // The key assertion: text deltas should be received correctly
        let all_text: String = handler.texts.iter().cloned().collect();
        assert!(
            all_text.contains("Plan is set."),
            "Expected 'Plan is set.' in text, got: {:?}",
            handler.texts
        );
        assert!(
            all_text.contains("Three tasks created."),
            "Expected 'Three tasks created.' in text, got: {:?}",
            handler.texts
        );

        // extracted_text should also be correct for LOOP_COMPLETE detection
        assert!(
            result.extracted_text.contains("Plan is set."),
            "Expected extracted text to contain 'Plan is set.', got: {:?}",
            result.extracted_text
        );
    }

    #[tokio::test]
    async fn run_observe_streaming_copilot_stream_extracts_assistant_text() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::CopilotStreamJson,
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
'{"type":"assistant.turn_start","data":{"turnId":"0"}}' \
'{"type":"assistant.message","data":{"content":"Hello from Copilot"}}' \
'{"type":"result","exitCode":0}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert_eq!(handler.texts, vec!["Hello from Copilot".to_string()]);
        assert_eq!(result.extracted_text, "Hello from Copilot\n");
        assert_eq!(handler.completions.len(), 1);
        assert!(!handler.completions[0].is_error);
    }

    #[tokio::test]
    async fn run_observe_streaming_copilot_stream_reports_tool_events() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend = CliBackend {
            command: "sh".to_string(),
            args: vec!["-c".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::CopilotStreamJson,
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
'{"type":"assistant.turn_start","data":{"turnId":"0"}}' \
'{"type":"assistant.message_delta","data":{"messageId":"msg-1","deltaContent":"Checking parser"}}' \
'{"type":"assistant.message","data":{"messageId":"msg-1","content":"Checking parser","toolRequests":[{"toolCallId":"tool-1","name":"bash","arguments":{"command":"echo hi"},"type":"function"}]}}' \
'{"type":"tool.execution_start","data":{"toolCallId":"tool-1","toolName":"bash","arguments":{"command":"echo hi"}}}' \
'{"type":"tool.execution_complete","data":{"toolCallId":"tool-1","success":true,"result":{"content":"hi\n","detailedContent":"hi\n"}}}' \
'{"type":"assistant.message","data":{"messageId":"msg-2","content":"Done"}}' \
'{"type":"result","exitCode":0}'"#;

        let result = executor
            .run_observe_streaming(script, rx, &mut handler)
            .await
            .expect("run_observe_streaming");

        assert!(result.success);
        assert_eq!(
            handler.texts,
            vec![
                "Checking parser".to_string(),
                "\n".to_string(),
                "Done".to_string()
            ]
        );
        assert_eq!(handler.tool_calls.len(), 1);
        assert_eq!(handler.tool_calls[0].0, "bash");
        assert_eq!(handler.tool_calls[0].1, "tool-1");
        assert_eq!(handler.tool_calls[0].2["command"], "echo hi");
        assert_eq!(
            handler.tool_results,
            vec![("tool-1".to_string(), "hi\n".to_string())]
        );
        assert!(handler.errors.is_empty());
        assert_eq!(handler.completions.len(), 1);
        assert!(!handler.completions[0].is_error);
        assert_eq!(result.extracted_text, "Checking parser\nDone\n");
    }
}
