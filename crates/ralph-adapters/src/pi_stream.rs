//! Pi stream event types for parsing `--mode json` NDJSON output.
//!
//! When invoked with `--mode json`, pi emits newline-delimited JSON events.
//! This module provides typed Rust structures for deserializing and processing
//! these events, plus a dispatch function for mapping them to `StreamHandler` calls.
//!
//! Only events that Ralph needs are modeled as typed variants. All other event
//! types are captured by `#[serde(other)]` and silently ignored, providing
//! forward compatibility with new pi event types.

use crate::stream_handler::StreamHandler;
use serde::{Deserialize, Serialize};

/// Events from pi's `--mode json` NDJSON output.
///
/// Only the events Ralph needs are modeled. All other event types
/// (session, agent_start, turn_start, message_start, message_end,
/// tool_execution_update, etc.) are captured by the `Other` variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiStreamEvent {
    /// Streaming text/thinking deltas and errors from assistant.
    MessageUpdate {
        #[serde(rename = "assistantMessageEvent")]
        assistant_message_event: PiAssistantEvent,
    },

    /// Tool begins execution.
    ToolExecutionStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        args: serde_json::Value,
    },

    /// Tool completes execution.
    ToolExecutionEnd {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        result: PiToolResult,
        #[serde(rename = "isError")]
        is_error: bool,
    },

    /// Turn completes — contains per-turn usage/cost.
    TurnEnd { message: Option<PiTurnMessage> },

    /// All other events (session, agent_start, turn_start, message_start,
    /// message_end, tool_execution_update, etc.)
    #[serde(other)]
    Other,
}

/// Assistant message event within a message_update.
///
/// Only text_delta, thinking_delta, and error are actionable.
/// All other sub-types are captured by `Other`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiAssistantEvent {
    /// Text content delta.
    TextDelta { delta: String },
    /// Extended thinking delta.
    ThinkingDelta { delta: String },
    /// Error during message generation.
    Error { reason: String },
    /// All other sub-types (text_start, text_end, thinking_start, thinking_end,
    /// toolcall_start, toolcall_delta, toolcall_end, done)
    #[serde(other)]
    Other,
}

/// Tool execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiToolResult {
    pub content: Vec<PiContentBlock>,
}

/// Content block within a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiContentBlock {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

/// Message in turn_end — contains usage data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiTurnMessage {
    #[serde(rename = "stopReason")]
    pub stop_reason: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub usage: Option<PiUsage>,
}

/// Token usage statistics from pi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiUsage {
    pub input: u64,
    pub output: u64,
    #[serde(rename = "cacheRead")]
    pub cache_read: u64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u64,
    pub cost: Option<PiCost>,
}

/// Cost breakdown from pi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiCost {
    pub total: f64,
}

/// Parses NDJSON lines from pi's stream output.
pub struct PiStreamParser;

impl PiStreamParser {
    /// Parse a single line of NDJSON output.
    ///
    /// Returns `None` for empty lines or malformed JSON (logged at debug level).
    pub fn parse_line(line: &str) -> Option<PiStreamEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        match serde_json::from_str::<PiStreamEvent>(trimmed) {
            Ok(event) => Some(event),
            Err(e) => {
                tracing::debug!(
                    "Skipping malformed pi JSON: {} (error: {})",
                    truncate(trimmed, 100),
                    e
                );
                None
            }
        }
    }
}

/// State accumulated across events for session summary.
pub struct PiSessionState {
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub stream_provider: Option<String>,
    pub stream_model: Option<String>,
}

impl PiSessionState {
    pub fn new() -> Self {
        Self {
            total_cost_usd: 0.0,
            num_turns: 0,
            stream_provider: None,
            stream_model: None,
        }
    }
}

impl Default for PiSessionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatch a pi stream event to the `StreamHandler`.
///
/// Accumulates cost/turn data in `state` for the final `on_complete()` call.
/// Appends text content to `extracted_text` for LOOP_COMPLETE detection.
pub fn dispatch_pi_stream_event<H: StreamHandler>(
    event: PiStreamEvent,
    handler: &mut H,
    extracted_text: &mut String,
    state: &mut PiSessionState,
    verbose: bool,
) {
    match event {
        PiStreamEvent::MessageUpdate {
            assistant_message_event,
        } => match assistant_message_event {
            PiAssistantEvent::TextDelta { delta } => {
                handler.on_text(&delta);
                extracted_text.push_str(&delta);
            }
            PiAssistantEvent::ThinkingDelta { delta } => {
                if verbose {
                    handler.on_text(&delta);
                }
            }
            PiAssistantEvent::Error { reason } => {
                handler.on_error(&reason);
            }
            PiAssistantEvent::Other => {}
        },
        PiStreamEvent::ToolExecutionStart {
            tool_name,
            tool_call_id,
            args,
        } => {
            handler.on_tool_call(&tool_name, &tool_call_id, &args);
        }
        PiStreamEvent::ToolExecutionEnd {
            tool_call_id,
            result,
            is_error,
            ..
        } => {
            let output = result
                .content
                .iter()
                .filter_map(|b| match b {
                    PiContentBlock::Text { text } => Some(text.as_str()),
                    PiContentBlock::Other => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if is_error {
                handler.on_error(&output);
            } else {
                handler.on_tool_result(&tool_call_id, &output);
            }
        }
        PiStreamEvent::TurnEnd { message } => {
            state.num_turns += 1;
            if let Some(msg) = &message {
                if let Some(provider) = &msg.provider
                    && !provider.is_empty()
                {
                    state.stream_provider = Some(provider.clone());
                }
                if let Some(model) = &msg.model
                    && !model.is_empty()
                {
                    state.stream_model = Some(model.clone());
                }
                if let Some(usage) = &msg.usage
                    && let Some(cost) = &usage.cost
                {
                    state.total_cost_usd += cost.total;
                }
            }
        }
        PiStreamEvent::Other => {}
    }
}

/// Truncates a string to a maximum length, adding "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
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
    use crate::SessionResult;
    use serde_json::json;

    // =========================================================================
    // PiStreamParser::parse_line tests
    // =========================================================================

    #[test]
    fn test_parse_text_delta() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Hello world"}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::MessageUpdate {
                assistant_message_event: PiAssistantEvent::TextDelta { delta },
            } => {
                assert_eq!(delta, "Hello world");
            }
            _ => panic!("Expected MessageUpdate with TextDelta, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_thinking_delta() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":"Let me think..."}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::MessageUpdate {
                assistant_message_event: PiAssistantEvent::ThinkingDelta { delta },
            } => {
                assert_eq!(delta, "Let me think...");
            }
            _ => panic!("Expected MessageUpdate with ThinkingDelta, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_error_event() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"error","reason":"aborted"}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::MessageUpdate {
                assistant_message_event: PiAssistantEvent::Error { reason },
            } => {
                assert_eq!(reason, "aborted");
            }
            _ => panic!("Expected MessageUpdate with Error, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_tool_execution_start() {
        let json = r#"{"type":"tool_execution_start","toolCallId":"toolu_123","toolName":"bash","args":{"command":"echo hello"}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                assert_eq!(tool_call_id, "toolu_123");
                assert_eq!(tool_name, "bash");
                assert_eq!(args["command"], "echo hello");
            }
            _ => panic!("Expected ToolExecutionStart, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_tool_execution_end() {
        let json = r#"{"type":"tool_execution_end","toolCallId":"toolu_123","toolName":"bash","result":{"content":[{"type":"text","text":"hello\n"}]},"isError":false}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
            } => {
                assert_eq!(tool_call_id, "toolu_123");
                assert_eq!(tool_name, "bash");
                assert!(!is_error);
                assert_eq!(result.content.len(), 1);
                match &result.content[0] {
                    PiContentBlock::Text { text } => assert_eq!(text, "hello\n"),
                    PiContentBlock::Other => panic!("Expected Text content block"),
                }
            }
            _ => panic!("Expected ToolExecutionEnd, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_tool_execution_end_error() {
        let json = r#"{"type":"tool_execution_end","toolCallId":"toolu_456","toolName":"Read","result":{"content":[{"type":"text","text":"file not found"}]},"isError":true}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::ToolExecutionEnd { is_error, .. } => {
                assert!(is_error);
            }
            _ => panic!("Expected ToolExecutionEnd, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_turn_end_with_usage() {
        let json = r#"{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":1,"output":14,"cacheRead":8932,"cacheWrite":70,"totalTokens":9017,"cost":{"input":0.000005,"output":0.00035,"cacheRead":0.00447,"cacheWrite":0.00044,"total":0.00526}},"stopReason":"stop"},"toolResults":[]}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::TurnEnd { message } => {
                let msg = message.unwrap();
                assert_eq!(msg.stop_reason, Some("stop".to_string()));
                let usage = msg.usage.unwrap();
                assert_eq!(usage.input, 1);
                assert_eq!(usage.output, 14);
                assert_eq!(usage.cache_read, 8932);
                let cost = usage.cost.unwrap();
                assert!((cost.total - 0.00526).abs() < 1e-10);
            }
            _ => panic!("Expected TurnEnd, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_turn_end_without_usage() {
        let json = r#"{"type":"turn_end","message":{"role":"assistant","content":[],"stopReason":"stop"}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();

        match event {
            PiStreamEvent::TurnEnd { message } => {
                let msg = message.unwrap();
                assert!(msg.usage.is_none());
            }
            _ => panic!("Expected TurnEnd, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_unknown_event_type() {
        // session, agent_start, turn_start, etc. should all parse as Other
        let json = r#"{"type":"session","version":3,"id":"uuid","timestamp":"2026-02-05T02:39:26.125Z","cwd":"/tmp"}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiStreamEvent::Other));

        let json = r#"{"type":"agent_start"}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiStreamEvent::Other));

        let json = r#"{"type":"turn_start"}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiStreamEvent::Other));

        let json = r#"{"type":"message_start","message":{"role":"user","content":[]}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiStreamEvent::Other));

        let json = r#"{"type":"message_end","message":{"role":"assistant","content":[]}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiStreamEvent::Other));
    }

    #[test]
    fn test_parse_unknown_assistant_event_type() {
        // toolcall_start, toolcall_delta, toolcall_end, text_start, text_end, done
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"toolcall_start","contentIndex":0}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        match event {
            PiStreamEvent::MessageUpdate {
                assistant_message_event: PiAssistantEvent::Other,
            } => {}
            _ => panic!("Expected MessageUpdate with Other assistant event"),
        }

        let json =
            r#"{"type":"message_update","assistantMessageEvent":{"type":"done","reason":"stop"}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        match event {
            PiStreamEvent::MessageUpdate {
                assistant_message_event: PiAssistantEvent::Other,
            } => {}
            _ => panic!("Expected MessageUpdate with Other assistant event"),
        }
    }

    #[test]
    fn test_parse_empty_line() {
        assert!(PiStreamParser::parse_line("").is_none());
        assert!(PiStreamParser::parse_line("   ").is_none());
        assert!(PiStreamParser::parse_line("\n").is_none());
    }

    #[test]
    fn test_parse_malformed_json() {
        assert!(PiStreamParser::parse_line("{not valid json}").is_none());
        assert!(PiStreamParser::parse_line("plain text").is_none());
    }

    #[test]
    fn test_parse_tool_execution_update_is_other() {
        let json = r#"{"type":"tool_execution_update","toolCallId":"toolu_123","toolName":"bash","args":{"command":"echo hello"},"partialResult":{"content":[{"type":"text","text":"hello\n"}]}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiStreamEvent::Other));
    }

    // =========================================================================
    // dispatch_pi_stream_event tests
    // =========================================================================

    /// Recording handler for testing dispatch behavior.
    #[derive(Default)]
    struct RecordingHandler {
        texts: Vec<String>,
        tool_calls: Vec<(String, String, serde_json::Value)>,
        tool_results: Vec<(String, String)>,
        errors: Vec<String>,
        completions: Vec<SessionResult>,
    }

    impl StreamHandler for RecordingHandler {
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

    #[test]
    fn test_dispatch_text_delta() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::MessageUpdate {
            assistant_message_event: PiAssistantEvent::TextDelta {
                delta: "Hello".to_string(),
            },
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert_eq!(handler.texts, vec!["Hello"]);
        assert_eq!(extracted, "Hello");
    }

    #[test]
    fn test_dispatch_thinking_delta_verbose() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::MessageUpdate {
            assistant_message_event: PiAssistantEvent::ThinkingDelta {
                delta: "thinking...".to_string(),
            },
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, true);
        assert_eq!(handler.texts, vec!["thinking..."]);
        // Thinking should NOT go into extracted_text (not part of output)
        assert!(extracted.is_empty());
    }

    #[test]
    fn test_dispatch_thinking_delta_not_verbose() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::MessageUpdate {
            assistant_message_event: PiAssistantEvent::ThinkingDelta {
                delta: "thinking...".to_string(),
            },
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);
        assert!(handler.texts.is_empty());
        assert!(extracted.is_empty());
    }

    #[test]
    fn test_dispatch_error() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::MessageUpdate {
            assistant_message_event: PiAssistantEvent::Error {
                reason: "aborted".to_string(),
            },
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);
        assert_eq!(handler.errors, vec!["aborted"]);
    }

    #[test]
    fn test_dispatch_tool_execution_start() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::ToolExecutionStart {
            tool_call_id: "toolu_123".to_string(),
            tool_name: "bash".to_string(),
            args: json!({"command": "echo hello"}),
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert_eq!(handler.tool_calls.len(), 1);
        assert_eq!(handler.tool_calls[0].0, "bash");
        assert_eq!(handler.tool_calls[0].1, "toolu_123");
        assert_eq!(handler.tool_calls[0].2["command"], "echo hello");
    }

    #[test]
    fn test_dispatch_tool_execution_end_success() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::ToolExecutionEnd {
            tool_call_id: "toolu_123".to_string(),
            tool_name: "bash".to_string(),
            result: PiToolResult {
                content: vec![PiContentBlock::Text {
                    text: "hello\n".to_string(),
                }],
            },
            is_error: false,
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert_eq!(handler.tool_results.len(), 1);
        assert_eq!(handler.tool_results[0].0, "toolu_123");
        assert_eq!(handler.tool_results[0].1, "hello\n");
        assert!(handler.errors.is_empty());
    }

    #[test]
    fn test_dispatch_tool_execution_end_error() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::ToolExecutionEnd {
            tool_call_id: "toolu_456".to_string(),
            tool_name: "Read".to_string(),
            result: PiToolResult {
                content: vec![PiContentBlock::Text {
                    text: "file not found".to_string(),
                }],
            },
            is_error: true,
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert!(handler.tool_results.is_empty());
        assert_eq!(handler.errors, vec!["file not found"]);
    }

    #[test]
    fn test_dispatch_turn_end_accumulates_cost() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        // Three turns with different costs
        for cost in [0.05, 0.03, 0.01] {
            let event = PiStreamEvent::TurnEnd {
                message: Some(PiTurnMessage {
                    stop_reason: Some("stop".to_string()),
                    provider: None,
                    model: None,
                    usage: Some(PiUsage {
                        input: 100,
                        output: 50,
                        cache_read: 0,
                        cache_write: 0,
                        cost: Some(PiCost { total: cost }),
                    }),
                }),
            };
            dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);
        }

        assert_eq!(state.num_turns, 3);
        assert!((state.total_cost_usd - 0.09).abs() < 1e-10);
    }

    #[test]
    fn test_dispatch_turn_end_missing_usage() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::TurnEnd {
            message: Some(PiTurnMessage {
                stop_reason: Some("stop".to_string()),
                provider: None,
                model: None,
                usage: None,
            }),
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert_eq!(state.num_turns, 1);
        assert!((state.total_cost_usd - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dispatch_turn_end_missing_message() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::TurnEnd { message: None };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert_eq!(state.num_turns, 1);
        assert!((state.total_cost_usd - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dispatch_other_is_noop() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        dispatch_pi_stream_event(
            PiStreamEvent::Other,
            &mut handler,
            &mut extracted,
            &mut state,
            false,
        );

        assert!(handler.texts.is_empty());
        assert!(handler.tool_calls.is_empty());
        assert!(handler.tool_results.is_empty());
        assert!(handler.errors.is_empty());
        assert!(handler.completions.is_empty());
        assert!(extracted.is_empty());
        assert_eq!(state.num_turns, 0);
    }

    #[test]
    fn test_dispatch_assistant_other_is_noop() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::MessageUpdate {
            assistant_message_event: PiAssistantEvent::Other,
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert!(handler.texts.is_empty());
        assert!(handler.errors.is_empty());
    }

    // =========================================================================
    // Real NDJSON line tests (from research samples)
    // =========================================================================

    #[test]
    fn test_parse_real_session_event() {
        let json = r#"{"type":"session","version":3,"id":"550e8400-e29b-41d4-a716-446655440000","timestamp":"2026-02-05T02:39:26.125Z","cwd":"/home/user/project"}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiStreamEvent::Other));
    }

    #[test]
    fn test_parse_real_tool_execution_start() {
        let json = r#"{"type":"tool_execution_start","toolCallId":"toolu_01BKzy4E5YAeFLdgwFKtNRqv","toolName":"bash","args":{"command":"echo hello"}}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        match event {
            PiStreamEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                assert_eq!(tool_call_id, "toolu_01BKzy4E5YAeFLdgwFKtNRqv");
                assert_eq!(tool_name, "bash");
                assert_eq!(args["command"], "echo hello");
            }
            _ => panic!("Expected ToolExecutionStart"),
        }
    }

    #[test]
    fn test_parse_real_turn_end() {
        let json = r#"{"type":"turn_end","message":{"role":"assistant","content":[{"type":"text","text":"Done."}],"api":"anthropic-messages","provider":"anthropic","model":"claude-opus-4-5","usage":{"input":1,"output":14,"cacheRead":8932,"cacheWrite":70,"totalTokens":9017,"cost":{"input":0.000005,"output":0.00035,"cacheRead":0.00447,"cacheWrite":0.00044,"total":0.00526}},"stopReason":"stop","timestamp":1770259166907},"toolResults":[]}"#;
        let event = PiStreamParser::parse_line(json).unwrap();
        match event {
            PiStreamEvent::TurnEnd { message } => {
                let msg = message.unwrap();
                assert_eq!(msg.stop_reason, Some("stop".to_string()));
                assert_eq!(msg.provider, Some("anthropic".to_string()));
                assert_eq!(msg.model, Some("claude-opus-4-5".to_string()));
                let usage = msg.usage.unwrap();
                let cost = usage.cost.unwrap();
                assert!((cost.total - 0.00526).abs() < 1e-10);
            }
            _ => panic!("Expected TurnEnd"),
        }
    }

    #[test]
    fn test_dispatch_turn_end_captures_stream_identity() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::TurnEnd {
            message: Some(PiTurnMessage {
                stop_reason: Some("stop".to_string()),
                provider: Some("anthropic".to_string()),
                model: Some("claude-sonnet-4".to_string()),
                usage: None,
            }),
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert_eq!(state.stream_provider, Some("anthropic".to_string()));
        assert_eq!(state.stream_model, Some("claude-sonnet-4".to_string()));
    }

    #[test]
    fn test_tool_result_multiple_content_blocks() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = PiSessionState::new();

        let event = PiStreamEvent::ToolExecutionEnd {
            tool_call_id: "toolu_789".to_string(),
            tool_name: "Read".to_string(),
            result: PiToolResult {
                content: vec![
                    PiContentBlock::Text {
                        text: "line 1".to_string(),
                    },
                    PiContentBlock::Text {
                        text: "line 2".to_string(),
                    },
                ],
            },
            is_error: false,
        };

        dispatch_pi_stream_event(event, &mut handler, &mut extracted, &mut state, false);

        assert_eq!(handler.tool_results[0].1, "line 1\nline 2");
    }
}
