//! Claude stream event types for parsing `--output-format stream-json` output.
//!
//! When invoked with `--output-format stream-json`, Claude emits newline-delimited
//! JSON events. This module provides typed Rust structures for deserializing
//! and processing these events.

use crate::stream_handler::SessionResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Events emitted by Claude's `--output-format stream-json`.
///
/// Each line of output is a JSON object with a `type` field that determines
/// the event variant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeStreamEvent {
    /// Session initialization - first event emitted.
    System {
        session_id: String,
        model: String,
        #[serde(default)]
        tools: Vec<serde_json::Value>,
    },

    /// Claude's response - contains text or tool invocations.
    Assistant { message: AssistantMessage },

    /// Tool results returned to Claude.
    User { message: UserMessage },

    /// Session complete - final event with stats.
    Result {
        duration_ms: u64,
        total_cost_usd: f64,
        num_turns: u32,
        is_error: bool,
        /// Per-model usage keyed by model id; carries each model's context window.
        #[serde(
            default,
            rename = "modelUsage",
            deserialize_with = "lenient_model_usage",
            skip_serializing_if = "BTreeMap::is_empty"
        )]
        model_usage: BTreeMap<String, ModelUsage>,
    },
}

/// One model's entry in a `result` line's `modelUsage` map.
///
/// Only the context window is read; token and cost totals come from the
/// per-turn `usage` and `total_cost_usd`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelUsage {
    /// The model's context window in tokens (`0` when not reported).
    #[serde(default, rename = "contextWindow")]
    pub context_window: u64,
}

/// Reads `modelUsage` without ever failing the `result` line it sits on.
///
/// The line also carries the session's cost and turns, so an unexpected shape
/// here (`null`, a non-object, a non-integer window) yields no window rather
/// than a parse error that would drop those too.
fn lenient_model_usage<'de, D>(deserializer: D) -> Result<BTreeMap<String, ModelUsage>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let Some(models) = value.as_object() else {
        return Ok(BTreeMap::new());
    };
    Ok(models
        .iter()
        .map(|(model, usage)| {
            let context_window = usage
                .get("contextWindow")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            (model.clone(), ModelUsage { context_window })
        })
        .collect())
}

/// Message content from Claude's assistant responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Message content from tool results (user turn).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserMessage {
    pub content: Vec<UserContentBlock>,
}

/// Content blocks in assistant messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text output from Claude.
    Text { text: String },
    /// Tool invocation by Claude.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Content blocks in user messages (tool results).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserContentBlock {
    /// Result from a tool invocation.
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Token usage statistics reported by Claude on each `Assistant` message.
///
/// Real `stream-json` payloads nest this under `message.usage`. Cache fields
/// may be absent on turns that do not use caching; `#[serde(default)]` makes
/// every field tolerant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// Session metrics accumulated from Claude `stream-json` events.
///
/// Claude's `usage` reports the **live** input footprint per turn (input +
/// cache_read + cache_creation), so summing across turns double-counts shared
/// context. Input and cache fields keep the per-turn peak; only
/// `output_tokens` is summed (new tokens produced per turn never overlap).
///
/// Shared by the PTY and headless CLI executors so both report the same
/// numbers for the same stream.
#[derive(Debug, Default)]
pub struct ClaudeSessionMetrics {
    peak_input: u64,
    total_output: u64,
    peak_cache_read: u64,
    peak_cache_write: u64,
    /// The session's model, from the `system` init line.
    model: Option<String>,
}

impl ClaudeSessionMetrics {
    /// Folds one event into the running totals.
    ///
    /// Returns the session result when `event` is the terminal `Result` line.
    /// [`SessionResult::context_window`] is the window the result line reports
    /// for the session's model, or `fallback_context_window` (the caller's
    /// configured ceiling) when the stream reports none.
    pub fn observe(
        &mut self,
        event: &ClaudeStreamEvent,
        fallback_context_window: u64,
    ) -> Option<SessionResult> {
        match event {
            ClaudeStreamEvent::Assistant { message } => {
                if let Some(usage) = &message.usage {
                    let turn_input = usage
                        .input_tokens
                        .saturating_add(usage.cache_creation_input_tokens)
                        .saturating_add(usage.cache_read_input_tokens);
                    self.peak_input = self.peak_input.max(turn_input);
                    self.total_output = self.total_output.saturating_add(usage.output_tokens);
                    self.peak_cache_read = self.peak_cache_read.max(usage.cache_read_input_tokens);
                    self.peak_cache_write =
                        self.peak_cache_write.max(usage.cache_creation_input_tokens);
                }
                None
            }
            ClaudeStreamEvent::Result {
                duration_ms,
                total_cost_usd,
                num_turns,
                is_error,
                model_usage,
            } => Some(SessionResult {
                duration_ms: *duration_ms,
                total_cost_usd: *total_cost_usd,
                num_turns: *num_turns,
                is_error: *is_error,
                input_tokens: self.peak_input,
                output_tokens: self.total_output,
                cache_read_tokens: self.peak_cache_read,
                cache_write_tokens: self.peak_cache_write,
                context_window: self
                    .reported_context_window(model_usage)
                    .unwrap_or(fallback_context_window),
            }),
            ClaudeStreamEvent::System { model, .. } => {
                self.model = Some(model.clone());
                None
            }
            ClaudeStreamEvent::User { .. } => None,
        }
    }

    /// The context window `modelUsage` reports for the session's model.
    ///
    /// Helper models (subagents, title generation) can appear alongside the
    /// session's own model, so the init line's model is preferred. Without a
    /// match, the largest reported window is used. `None` when no entry
    /// reports one.
    fn reported_context_window(&self, model_usage: &BTreeMap<String, ModelUsage>) -> Option<u64> {
        let window = |usage: &ModelUsage| Some(usage.context_window).filter(|&w| w > 0);
        self.model
            .as_ref()
            .and_then(|model| model_usage.get(model))
            .and_then(window)
            .or_else(|| model_usage.values().filter_map(window).max())
    }
}

/// Parses NDJSON lines from Claude's stream output.
pub struct ClaudeStreamParser;

impl ClaudeStreamParser {
    /// Parse a single line of NDJSON output.
    ///
    /// Returns `None` for empty lines or malformed JSON (logged at debug level).
    pub fn parse_line(line: &str) -> Option<ClaudeStreamEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        match serde_json::from_str::<ClaudeStreamEvent>(trimmed) {
            Ok(event) => Some(event),
            Err(e) => {
                tracing::debug!(
                    "Skipping malformed JSON line: {} (error: {})",
                    truncate(trimmed, 100),
                    e
                );
                None
            }
        }
    }
}

/// Truncates a string to a maximum length, adding "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find the last valid char boundary at or before max_len
        let boundary = s
            .char_indices()
            .take_while(|(i, _)| *i < max_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &s[..boundary])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_system_event() {
        let json = r#"{"type":"system","session_id":"abc123","model":"claude-opus","tools":[]}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();

        match event {
            ClaudeStreamEvent::System {
                session_id,
                model,
                tools,
            } => {
                assert_eq!(session_id, "abc123");
                assert_eq!(model, "claude-opus");
                assert_eq!(tools, [] as [serde_json::Value; 0]);
            }
            _ => panic!("Expected System event"),
        }
    }

    #[test]
    fn test_parse_assistant_text() {
        let json =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}]}}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();

        match event {
            ClaudeStreamEvent::Assistant { message, .. } => {
                assert_eq!(message.content.len(), 1);
                match &message.content[0] {
                    ContentBlock::Text { text } => assert_eq!(text, "Hello world"),
                    ContentBlock::ToolUse { .. } => panic!("Expected Text content"),
                }
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_assistant_tool_use() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"tool_1","name":"bash","input":{"command":"ls"}}]}}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();

        match event {
            ClaudeStreamEvent::Assistant { message, .. } => {
                assert_eq!(message.content.len(), 1);
                match &message.content[0] {
                    ContentBlock::ToolUse { id, name, input } => {
                        assert_eq!(id, "tool_1");
                        assert_eq!(name, "bash");
                        assert_eq!(input["command"], "ls");
                    }
                    ContentBlock::Text { .. } => panic!("Expected ToolUse content"),
                }
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_user_tool_result() {
        let json = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tool_1","content":"file.txt"}]}}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();

        match event {
            ClaudeStreamEvent::User { message } => {
                assert_eq!(message.content.len(), 1);
                match &message.content[0] {
                    UserContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => {
                        assert_eq!(tool_use_id, "tool_1");
                        assert_eq!(content, "file.txt");
                    }
                }
            }
            _ => panic!("Expected User event"),
        }
    }

    #[test]
    fn test_parse_assistant_message_usage_with_cache_fields() {
        // Real stream-json shape: usage lives inside message, alongside content.
        let json = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":123,"output_tokens":45,"cache_creation_input_tokens":10,"cache_read_input_tokens":88}}}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();

        match event {
            ClaudeStreamEvent::Assistant { message } => {
                let usage = message.usage.expect("usage should parse from message");
                assert_eq!(usage.input_tokens, 123);
                assert_eq!(usage.output_tokens, 45);
                assert_eq!(usage.cache_creation_input_tokens, 10);
                assert_eq!(usage.cache_read_input_tokens, 88);
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_assistant_message_usage_missing_cache_fields() {
        // Cache fields default to zero when absent (early sessions, non-cached turns).
        let json = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":7,"output_tokens":2}}}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();
        match event {
            ClaudeStreamEvent::Assistant { message } => {
                let usage = message.usage.expect("usage should parse");
                assert_eq!(usage.input_tokens, 7);
                assert_eq!(usage.output_tokens, 2);
                assert_eq!(usage.cache_creation_input_tokens, 0);
                assert_eq!(usage.cache_read_input_tokens, 0);
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_result_event() {
        let json = r#"{"type":"result","duration_ms":5000,"total_cost_usd":0.02,"num_turns":2,"is_error":false}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();

        match event {
            ClaudeStreamEvent::Result {
                duration_ms,
                total_cost_usd,
                num_turns,
                is_error,
                ..
            } => {
                assert_eq!(duration_ms, 5000);
                assert!((total_cost_usd - 0.02).abs() < f64::EPSILON);
                assert_eq!(num_turns, 2);
                assert!(!is_error);
            }
            _ => panic!("Expected Result event"),
        }
    }

    #[test]
    fn test_parse_empty_line() {
        assert!(ClaudeStreamParser::parse_line("").is_none());
        assert!(ClaudeStreamParser::parse_line("   ").is_none());
        assert!(ClaudeStreamParser::parse_line("\n").is_none());
    }

    #[test]
    fn test_parse_malformed_json() {
        assert!(ClaudeStreamParser::parse_line("{not valid json}").is_none());
        assert!(ClaudeStreamParser::parse_line("plain text").is_none());
        assert!(ClaudeStreamParser::parse_line("{\"type\":\"unknown\"}").is_none());
    }

    #[test]
    fn test_truncate_helper() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is a ...");
    }

    #[test]
    fn test_truncate_utf8_boundary() {
        // The arrow character → is 3 bytes (E2 86 92 in UTF-8)
        // This string: "hello→world" has bytes:
        //   h(0) e(1) l(2) l(3) o(4) →(5,6,7) w(8) o(9) r(10) l(11) d(12)
        // Truncating at byte 6 or 7 would land INSIDE the → character
        let s = "hello→world";

        // Truncate at max_len=6, which is inside the → character (bytes 5-7)
        // This should NOT panic - it should truncate to "hello" (before the →)
        let result = truncate(s, 6);

        // The result should be truncated at a valid UTF-8 boundary
        // Expected: "hello..." (truncated before the multi-byte char)
        assert!(result.ends_with("..."), "Should end with ellipsis");
        assert!(result.len() < s.len(), "Should be truncated");

        // Verify the result is valid UTF-8 (won't panic on iteration)
        for _ in result.chars() {}
    }

    #[test]
    fn test_truncate_utf8_emoji() {
        // Emoji like 🦀 is 4 bytes (F0 9F A6 80)
        // "hi🦀" = h(0) i(1) 🦀(2,3,4,5)
        // Truncating at byte 3, 4, or 5 would panic
        let s = "hi🦀bye";

        // Truncate at max_len=4, which is inside the 🦀 (bytes 2-5)
        let result = truncate(s, 4);

        // Should truncate to "hi..." (before the emoji)
        assert!(result.ends_with("..."));

        // Verify valid UTF-8
        for _ in result.chars() {}
    }

    fn turn(input: u64, output: u64, cache_write: u64, cache_read: u64) -> ClaudeStreamEvent {
        ClaudeStreamEvent::Assistant {
            message: AssistantMessage {
                content: vec![],
                usage: Some(Usage {
                    input_tokens: input,
                    output_tokens: output,
                    cache_creation_input_tokens: cache_write,
                    cache_read_input_tokens: cache_read,
                }),
            },
        }
    }

    fn result_event() -> ClaudeStreamEvent {
        ClaudeStreamEvent::Result {
            duration_ms: 1234,
            total_cost_usd: 0.5,
            num_turns: 3,
            is_error: false,
            model_usage: BTreeMap::new(),
        }
    }

    #[test]
    fn session_metrics_keep_input_and_cache_peaks_and_sum_output() {
        let mut metrics = ClaudeSessionMetrics::default();

        // Live footprint per turn: 1150, 4310, 211. Peaks must be retained
        // when a later turn is smaller; only output sums.
        assert!(metrics.observe(&turn(100, 20, 50, 1000), 0).is_none());
        assert_eq!(metrics.peak_input, 1150);
        assert!(metrics.observe(&turn(300, 40, 10, 4000), 0).is_none());
        assert_eq!(metrics.peak_input, 4310);
        assert!(metrics.observe(&turn(10, 5, 1, 200), 0).is_none());

        let session = metrics
            .observe(&result_event(), 200_000)
            .expect("Result event must yield SessionResult");
        assert_eq!(session.input_tokens, 4310);
        assert_eq!(session.output_tokens, 65);
        assert_eq!(session.cache_read_tokens, 4000);
        assert_eq!(session.cache_write_tokens, 50);
        assert_eq!(session.context_window, 200_000);
        assert_eq!(session.duration_ms, 1234);
        assert_eq!(session.num_turns, 3);
        assert!(!session.is_error);
        assert!((session.total_cost_usd - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn session_metrics_ignore_turns_without_usage_and_non_assistant_events() {
        let mut metrics = ClaudeSessionMetrics::default();
        let no_usage = ClaudeStreamEvent::Assistant {
            message: AssistantMessage {
                content: vec![ContentBlock::Text {
                    text: "hi".to_string(),
                }],
                usage: None,
            },
        };
        let system = ClaudeStreamEvent::System {
            session_id: "s".to_string(),
            model: "m".to_string(),
            tools: vec![],
        };

        assert!(metrics.observe(&no_usage, 0).is_none());
        assert!(metrics.observe(&system, 0).is_none());

        let session = metrics.observe(&result_event(), 0).expect("result");
        assert_eq!(session.input_tokens, 0);
        assert_eq!(session.output_tokens, 0);
        assert!((session.total_cost_usd - 0.5).abs() < f64::EPSILON);
    }

    fn parsed(line: &str) -> ClaudeStreamEvent {
        ClaudeStreamParser::parse_line(line).unwrap_or_else(|| panic!("line must parse: {line}"))
    }

    /// The session's context window, from a stream whose init line names
    /// `system_model` (if any) and whose result line carries `model_usage`.
    fn window_from_stream(system_model: Option<&str>, model_usage: &str, fallback: u64) -> u64 {
        let mut metrics = ClaudeSessionMetrics::default();
        if let Some(model) = system_model {
            let init = format!(
                r#"{{"type":"system","subtype":"init","session_id":"s","model":"{model}","tools":[]}}"#
            );
            assert!(metrics.observe(&parsed(&init), fallback).is_none());
        }
        let result = format!(
            r#"{{"type":"result","subtype":"success","is_error":false,"duration_ms":10,"num_turns":1,"total_cost_usd":0.1{model_usage}}}"#
        );
        metrics
            .observe(&parsed(&result), fallback)
            .expect("result line yields SessionResult")
            .context_window
    }

    #[test]
    fn session_metrics_take_context_window_from_result_model_usage() {
        let usage = r#","modelUsage":{"claude-opus-5-5":{"inputTokens":20,"outputTokens":610,"costUSD":0.1,"contextWindow":1000000}}"#;

        // The stream's figure wins over the caller's 200000 default, with or
        // without an init line naming the model.
        assert_eq!(
            window_from_stream(Some("claude-opus-5-5"), usage, 200_000),
            1_000_000
        );
        assert_eq!(window_from_stream(None, usage, 200_000), 1_000_000);
    }

    #[test]
    fn session_metrics_prefer_the_session_models_context_window() {
        // A helper model (here with the larger window) also shows up in
        // modelUsage; the session's own model, named on the init line, decides.
        let usage = r#","modelUsage":{"claude-haiku-5-5":{"costUSD":0.01,"contextWindow":1000000},"claude-sonnet-5-5":{"costUSD":0.09,"contextWindow":400000}}"#;
        assert_eq!(
            window_from_stream(Some("claude-sonnet-5-5"), usage, 123),
            400_000
        );

        // Init model absent from modelUsage: take the largest reported window.
        assert_eq!(
            window_from_stream(Some("claude-other"), usage, 123),
            1_000_000
        );
    }

    #[test]
    fn malformed_model_usage_does_not_cost_the_result_line() {
        // An unexpected modelUsage shape must not fail the whole result line,
        // which would drop its cost and turns along with the window.
        for usage in [
            r#","modelUsage":null"#,
            r#","modelUsage":[]"#,
            r#","modelUsage":{"m":null}"#,
            r#","modelUsage":{"m":{"contextWindow":"1000000"}}"#,
            r#","modelUsage":{"m":{"contextWindow":-1}}"#,
        ] {
            let line = format!(
                r#"{{"type":"result","is_error":false,"duration_ms":10,"num_turns":1,"total_cost_usd":0.25{usage}}}"#
            );
            let session = ClaudeSessionMetrics::default()
                .observe(&parsed(&line), 200_000)
                .expect("result line yields SessionResult");
            assert!(
                (session.total_cost_usd - 0.25).abs() < f64::EPSILON,
                "{usage}"
            );
            assert_eq!(session.context_window, 200_000, "{usage}");
        }
    }

    #[test]
    fn session_metrics_fall_back_when_the_stream_reports_no_context_window() {
        // No modelUsage, an empty one, or entries without contextWindow.
        assert_eq!(window_from_stream(Some("m"), "", 200_000), 200_000);
        assert_eq!(
            window_from_stream(Some("m"), r#","modelUsage":{}"#, 200_000),
            200_000
        );
        assert_eq!(
            window_from_stream(Some("m"), r#","modelUsage":{"m":{"costUSD":0.1}}"#, 0),
            0
        );
        assert_eq!(
            window_from_stream(Some("m"), r#","modelUsage":{"m":{"contextWindow":0}}"#, 7),
            7
        );
    }
}
