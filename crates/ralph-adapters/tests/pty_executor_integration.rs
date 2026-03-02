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
}
