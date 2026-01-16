//! Claude stream event types for parsing `--output-format stream-json` output.
//!
//! When invoked with `--output-format stream-json`, Claude emits newline-delimited
//! JSON events. This module provides typed Rust structures for deserializing
//! and processing these events.

use serde::{Deserialize, Serialize};

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
    Assistant {
        message: AssistantMessage,
        #[serde(default)]
        usage: Option<Usage>,
    },

    /// Tool results returned to Claude.
    User { message: UserMessage },

    /// Session complete - final event with stats.
    Result {
        duration_ms: u64,
        total_cost_usd: f64,
        num_turns: u32,
        is_error: bool,
    },
}

/// Message content from Claude's assistant responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
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

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
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
        format!("{}...", &s[..max_len])
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
                assert!(tools.is_empty());
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
    fn test_parse_result_event() {
        let json = r#"{"type":"result","duration_ms":5000,"total_cost_usd":0.02,"num_turns":2,"is_error":false}"#;
        let event = ClaudeStreamParser::parse_line(json).unwrap();

        match event {
            ClaudeStreamEvent::Result {
                duration_ms,
                total_cost_usd,
                num_turns,
                is_error,
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
}
