//! Copilot stream event types for parsing `--output-format json` output.
//!
//! Copilot emits JSONL in prompt mode. This module provides lightweight parsing
//! helpers for extracting assistant text from those events and dispatching
//! structured tool events to Ralph's stream handlers.
//!
//! Ralph intentionally handles only the subset of Copilot prompt-mode events it
//! needs today. Additional SDK-documented session events currently map to
//! [`CopilotStreamEvent::Other`].

use std::collections::HashSet;

use crate::stream_handler::{SessionResult, StreamHandler};
use serde_json::Value;

/// Tool request embedded in an assistant message.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotToolRequest {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: Value,
}

/// Assistant message payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotAssistantMessage {
    pub message_id: Option<String>,
    pub content: Value,
    pub tool_requests: Vec<CopilotToolRequest>,
}

/// Assistant message delta payload emitted by Copilot while streaming a reply.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotAssistantMessageDelta {
    pub message_id: Option<String>,
    pub delta_content: String,
}

/// Assistant reasoning payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotAssistantReasoning {
    pub reasoning_id: Option<String>,
    pub content: String,
}

/// Assistant reasoning delta payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotAssistantReasoningDelta {
    pub reasoning_id: Option<String>,
    pub delta_content: String,
}

/// Turn boundary payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotTurnBoundary {
    pub turn_id: Option<String>,
}

/// Tool execution start payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotToolExecutionStart {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
}

/// Tool execution partial result payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotToolExecutionPartialResult {
    pub tool_call_id: String,
    pub partial_output: String,
}

/// Tool execution output payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotToolExecutionOutput {
    pub content: Value,
    pub detailed_content: Option<String>,
}

/// Tool execution error payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotToolExecutionError {
    pub message: String,
    pub code: Option<String>,
}

/// Tool execution completion payload emitted by Copilot.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotToolExecutionComplete {
    pub tool_call_id: String,
    pub success: bool,
    pub result: Option<CopilotToolExecutionOutput>,
    pub error: Option<CopilotToolExecutionError>,
}

impl CopilotToolExecutionComplete {
    fn output_text(&self) -> Option<String> {
        self.result.as_ref().and_then(|result| {
            result
                .detailed_content
                .clone()
                .or_else(|| extract_content_text(&result.content))
        })
    }

    fn error_text(&self) -> Option<String> {
        self.error
            .as_ref()
            .map(|error| error.message.clone())
            .or_else(|| self.output_text())
    }
}

/// Prompt-mode completion summary emitted by the Copilot CLI.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotResult {
    pub exit_code: Option<i32>,
    pub session_duration_ms: Option<u64>,
    pub total_api_duration_ms: Option<u64>,
}

/// Events emitted by Copilot's `--output-format json`.
#[derive(Debug, Clone, PartialEq)]
pub enum CopilotStreamEvent {
    /// Assistant message content.
    AssistantMessage { data: CopilotAssistantMessage },
    /// Incremental assistant message content.
    AssistantMessageDelta { data: CopilotAssistantMessageDelta },
    /// Full reasoning content.
    AssistantReasoning { data: CopilotAssistantReasoning },
    /// Incremental reasoning content.
    AssistantReasoningDelta {
        data: CopilotAssistantReasoningDelta,
    },
    /// Assistant turn start.
    AssistantTurnStart { data: CopilotTurnBoundary },
    /// Assistant turn end.
    AssistantTurnEnd { data: CopilotTurnBoundary },
    /// Tool begins execution.
    ToolExecutionStart { data: CopilotToolExecutionStart },
    /// Tool emits a partial result update.
    ToolExecutionPartialResult {
        data: CopilotToolExecutionPartialResult,
    },
    /// Tool completes execution.
    ToolExecutionComplete { data: CopilotToolExecutionComplete },
    /// Session completes.
    Result { data: CopilotResult },
    /// Any other event type that Ralph currently ignores.
    Other,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CopilotLiveChunk {
    pub text: String,
    pub append_newline: bool,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct CopilotStreamState {
    streamed_message_ids: HashSet<String>,
    completed_turns: u32,
}

impl CopilotStreamState {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

/// Parses JSONL lines from Copilot's prompt-mode output.
pub struct CopilotStreamParser;

impl CopilotStreamParser {
    /// Parse a single line of JSONL output.
    ///
    /// Returns `None` for empty lines or malformed JSON.
    pub fn parse_line(line: &str) -> Option<CopilotStreamEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let value = match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => value,
            Err(e) => {
                tracing::debug!(
                    "Skipping malformed JSON line: {} (error: {})",
                    truncate(trimmed, 100),
                    e
                );
                return None;
            }
        };

        match value.get("type").and_then(Value::as_str) {
            Some("assistant.message") => Some(CopilotStreamEvent::AssistantMessage {
                data: parse_assistant_message(&value),
            }),
            Some("assistant.message_delta") => Some(CopilotStreamEvent::AssistantMessageDelta {
                data: CopilotAssistantMessageDelta {
                    message_id: data_str(&value, "messageId"),
                    delta_content: data_str(&value, "deltaContent").unwrap_or_default(),
                },
            }),
            Some("assistant.reasoning") => Some(CopilotStreamEvent::AssistantReasoning {
                data: CopilotAssistantReasoning {
                    reasoning_id: data_str(&value, "reasoningId"),
                    content: data_str(&value, "content").unwrap_or_default(),
                },
            }),
            Some("assistant.reasoning_delta") => {
                Some(CopilotStreamEvent::AssistantReasoningDelta {
                    data: CopilotAssistantReasoningDelta {
                        reasoning_id: data_str(&value, "reasoningId"),
                        delta_content: data_str(&value, "deltaContent").unwrap_or_default(),
                    },
                })
            }
            Some("assistant.turn_start") => Some(CopilotStreamEvent::AssistantTurnStart {
                data: CopilotTurnBoundary {
                    turn_id: data_str(&value, "turnId"),
                },
            }),
            Some("assistant.turn_end") => Some(CopilotStreamEvent::AssistantTurnEnd {
                data: CopilotTurnBoundary {
                    turn_id: data_str(&value, "turnId"),
                },
            }),
            Some("tool.execution_start") => parse_tool_execution_start(&value)
                .map(|data| CopilotStreamEvent::ToolExecutionStart { data })
                .or(Some(CopilotStreamEvent::Other)),
            Some("tool.execution_partial_result") => parse_tool_execution_partial_result(&value)
                .map(|data| CopilotStreamEvent::ToolExecutionPartialResult { data })
                .or(Some(CopilotStreamEvent::Other)),
            Some("tool.execution_complete") => parse_tool_execution_complete(&value)
                .map(|data| CopilotStreamEvent::ToolExecutionComplete { data })
                .or(Some(CopilotStreamEvent::Other)),
            Some("result") => Some(CopilotStreamEvent::Result {
                data: parse_result(&value),
            }),
            Some(_) => Some(CopilotStreamEvent::Other),
            None => None,
        }
    }

    /// Extract assistant text from a single Copilot JSONL line.
    pub fn extract_text(line: &str) -> Option<String> {
        match Self::parse_line(line)? {
            CopilotStreamEvent::AssistantMessage { data } => extract_content_text(&data.content),
            CopilotStreamEvent::AssistantMessageDelta { .. }
            | CopilotStreamEvent::AssistantReasoning { .. }
            | CopilotStreamEvent::AssistantReasoningDelta { .. }
            | CopilotStreamEvent::AssistantTurnStart { .. }
            | CopilotStreamEvent::AssistantTurnEnd { .. }
            | CopilotStreamEvent::ToolExecutionStart { .. }
            | CopilotStreamEvent::ToolExecutionPartialResult { .. }
            | CopilotStreamEvent::ToolExecutionComplete { .. }
            | CopilotStreamEvent::Result { .. }
            | CopilotStreamEvent::Other => None,
        }
    }

    /// Extract text for live rendering, using deltas when available and
    /// suppressing duplicate full-message replays for message IDs already
    /// streamed incrementally.
    #[cfg(test)]
    pub(crate) fn extract_live_chunk(
        line: &str,
        state: &mut CopilotStreamState,
    ) -> Option<CopilotLiveChunk> {
        match Self::parse_line(line)? {
            CopilotStreamEvent::AssistantMessageDelta { data } => {
                if let Some(message_id) = data.message_id {
                    state.streamed_message_ids.insert(message_id);
                }

                if data.delta_content.is_empty() {
                    None
                } else {
                    Some(CopilotLiveChunk {
                        text: data.delta_content,
                        append_newline: false,
                    })
                }
            }
            CopilotStreamEvent::AssistantMessage { data } => {
                if should_suppress_full_message(data.message_id.as_deref(), state) {
                    return Some(CopilotLiveChunk {
                        text: String::new(),
                        append_newline: true,
                    });
                }

                extract_content_text(&data.content).map(|text| CopilotLiveChunk {
                    text,
                    append_newline: true,
                })
            }
            CopilotStreamEvent::AssistantReasoning { .. }
            | CopilotStreamEvent::AssistantReasoningDelta { .. }
            | CopilotStreamEvent::AssistantTurnStart { .. }
            | CopilotStreamEvent::AssistantTurnEnd { .. }
            | CopilotStreamEvent::ToolExecutionStart { .. }
            | CopilotStreamEvent::ToolExecutionPartialResult { .. }
            | CopilotStreamEvent::ToolExecutionComplete { .. }
            | CopilotStreamEvent::Result { .. }
            | CopilotStreamEvent::Other => None,
        }
    }

    /// Extract assistant text from a full Copilot JSONL payload.
    pub fn extract_all_text(raw_output: &str) -> String {
        let mut extracted = String::new();

        for line in raw_output.lines() {
            let Some(text) = Self::extract_text(line) else {
                continue;
            };
            Self::append_text_chunk(&mut extracted, &text);
        }

        extracted
    }

    /// Appends text while preserving message boundaries for downstream parsing.
    pub fn append_text_chunk(output: &mut String, chunk: &str) {
        output.push_str(chunk);
        if !chunk.ends_with('\n') {
            output.push('\n');
        }
    }
}

pub(crate) fn dispatch_copilot_stream_event<H: StreamHandler>(
    event: CopilotStreamEvent,
    handler: &mut H,
    extracted_text: &mut String,
    state: &mut CopilotStreamState,
) -> Option<SessionResult> {
    match event {
        CopilotStreamEvent::AssistantMessageDelta { data } => {
            if let Some(message_id) = data.message_id {
                state.streamed_message_ids.insert(message_id);
            }

            if !data.delta_content.is_empty() {
                handler.on_text(&data.delta_content);
            }
            None
        }
        CopilotStreamEvent::AssistantMessage { data } => {
            let message_text = extract_content_text(&data.content);

            if should_suppress_full_message(data.message_id.as_deref(), state) {
                handler.on_text("\n");
            } else if let Some(text) = message_text.as_deref() {
                handler.on_text(text);
            }

            if let Some(text) = message_text {
                CopilotStreamParser::append_text_chunk(extracted_text, &text);
            }
            None
        }
        CopilotStreamEvent::AssistantReasoning { .. }
        | CopilotStreamEvent::AssistantReasoningDelta { .. }
        | CopilotStreamEvent::AssistantTurnStart { .. }
        | CopilotStreamEvent::ToolExecutionPartialResult { .. }
        | CopilotStreamEvent::Other => None,
        CopilotStreamEvent::AssistantTurnEnd { .. } => {
            state.completed_turns += 1;
            None
        }
        CopilotStreamEvent::ToolExecutionStart { data } => {
            handler.on_tool_call(&data.tool_name, &data.tool_call_id, &data.arguments);
            None
        }
        CopilotStreamEvent::ToolExecutionComplete { data } => {
            if data.success {
                handler.on_tool_result(&data.tool_call_id, &data.output_text().unwrap_or_default());
            } else {
                handler.on_error(
                    &data
                        .error_text()
                        .unwrap_or_else(|| format!("Tool execution failed: {}", data.tool_call_id)),
                );
            }
            None
        }
        CopilotStreamEvent::Result { data } => {
            let exit_code = data.exit_code.unwrap_or_default();
            let session_result = SessionResult {
                duration_ms: data
                    .session_duration_ms
                    .or(data.total_api_duration_ms)
                    .unwrap_or_default(),
                total_cost_usd: 0.0,
                num_turns: state.completed_turns,
                is_error: exit_code != 0,
                ..Default::default()
            };
            if session_result.is_error {
                handler.on_error(&format!("Session ended with exit code {exit_code}"));
            }
            handler.on_complete(&session_result);
            Some(session_result)
        }
    }
}

fn parse_assistant_message(value: &Value) -> CopilotAssistantMessage {
    let tool_requests = value
        .get("data")
        .and_then(|data| data.get("toolRequests"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(parse_tool_request)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    CopilotAssistantMessage {
        message_id: data_str(value, "messageId"),
        content: data_value(value, "content").cloned().unwrap_or(Value::Null),
        tool_requests,
    }
}

fn parse_tool_request(value: &Value) -> Option<CopilotToolRequest> {
    Some(CopilotToolRequest {
        tool_call_id: value.get("toolCallId").and_then(Value::as_str)?.to_string(),
        name: value.get("name").and_then(Value::as_str)?.to_string(),
        arguments: value.get("arguments").cloned().unwrap_or(Value::Null),
    })
}

fn parse_tool_execution_start(value: &Value) -> Option<CopilotToolExecutionStart> {
    Some(CopilotToolExecutionStart {
        tool_call_id: data_str(value, "toolCallId")?,
        tool_name: data_str(value, "toolName")?,
        arguments: data_value(value, "arguments")
            .cloned()
            .unwrap_or(Value::Null),
    })
}

fn parse_tool_execution_partial_result(value: &Value) -> Option<CopilotToolExecutionPartialResult> {
    Some(CopilotToolExecutionPartialResult {
        tool_call_id: data_str(value, "toolCallId")?,
        partial_output: data_str(value, "partialOutput").unwrap_or_default(),
    })
}

fn parse_tool_execution_complete(value: &Value) -> Option<CopilotToolExecutionComplete> {
    Some(CopilotToolExecutionComplete {
        tool_call_id: data_str(value, "toolCallId")?,
        success: data_bool(value, "success").unwrap_or(false),
        result: data_value(value, "result").map(parse_tool_execution_output),
        error: data_value(value, "error").and_then(parse_tool_execution_error),
    })
}

fn parse_tool_execution_output(value: &Value) -> CopilotToolExecutionOutput {
    CopilotToolExecutionOutput {
        content: value.get("content").cloned().unwrap_or(Value::Null),
        detailed_content: value
            .get("detailedContent")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn parse_tool_execution_error(value: &Value) -> Option<CopilotToolExecutionError> {
    Some(CopilotToolExecutionError {
        message: value.get("message").and_then(Value::as_str)?.to_string(),
        code: value
            .get("code")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn parse_result(value: &Value) -> CopilotResult {
    let usage = value.get("usage");
    CopilotResult {
        exit_code: value
            .get("exitCode")
            .and_then(Value::as_i64)
            .and_then(|code| i32::try_from(code).ok()),
        session_duration_ms: usage
            .and_then(|usage| usage.get("sessionDurationMs"))
            .and_then(Value::as_u64),
        total_api_duration_ms: usage
            .and_then(|usage| usage.get("totalApiDurationMs"))
            .and_then(Value::as_u64),
    }
}

fn data_value<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get("data").and_then(|data| data.get(key))
}

fn data_str(value: &Value, key: &str) -> Option<String> {
    data_value(value, key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn data_bool(value: &Value, key: &str) -> Option<bool> {
    data_value(value, key).and_then(Value::as_bool)
}

fn should_suppress_full_message(message_id: Option<&str>, state: &CopilotStreamState) -> bool {
    message_id.is_some_and(|message_id| state.streamed_message_ids.contains(message_id))
}

fn extract_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let mut combined = String::new();
            for item in items {
                let text = match item {
                    Value::String(text) => Some(text.clone()),
                    Value::Object(map) => map
                        .get("text")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                    _ => None,
                };
                if let Some(text) = text {
                    combined.push_str(&text);
                }
            }

            if combined.is_empty() {
                None
            } else {
                Some(combined)
            }
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

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
    use super::{
        CopilotLiveChunk, CopilotStreamEvent, CopilotStreamParser, CopilotStreamState,
        dispatch_copilot_stream_event,
    };
    use crate::stream_handler::{SessionResult, StreamHandler};
    use serde_json::{Value, json};

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
    fn test_parse_assistant_message_content() {
        let line = r#"{"type":"assistant.message","data":{"messageId":"msg-1","content":"hello world","toolRequests":[]}}"#;
        let event = CopilotStreamParser::parse_line(line).unwrap();

        match event {
            CopilotStreamEvent::AssistantMessage { data } => {
                assert_eq!(data.message_id.as_deref(), Some("msg-1"));
                assert_eq!(data.content, Value::String("hello world".to_string()));
                assert!(data.tool_requests.is_empty());
            }
            _ => panic!("Expected AssistantMessage event"),
        }
    }

    #[test]
    fn test_parse_assistant_message_delta() {
        let line = r#"{"type":"assistant.message_delta","data":{"messageId":"msg-1","deltaContent":"hello"}}"#;
        let event = CopilotStreamParser::parse_line(line).unwrap();

        match event {
            CopilotStreamEvent::AssistantMessageDelta { data } => {
                assert_eq!(data.message_id.as_deref(), Some("msg-1"));
                assert_eq!(data.delta_content, "hello");
            }
            _ => panic!("Expected AssistantMessageDelta event"),
        }
    }

    #[test]
    fn test_parse_assistant_message_with_tool_requests() {
        let line = r#"{"type":"assistant.message","data":{"messageId":"msg-1","content":"Let me inspect that.","toolRequests":[{"toolCallId":"tool-1","name":"bash","arguments":{"command":"echo hi"},"type":"function"}]}}"#;
        let event = CopilotStreamParser::parse_line(line).unwrap();

        match event {
            CopilotStreamEvent::AssistantMessage { data } => {
                assert_eq!(data.message_id.as_deref(), Some("msg-1"));
                assert_eq!(
                    data.content,
                    Value::String("Let me inspect that.".to_string())
                );
                assert_eq!(data.tool_requests.len(), 1);
                assert_eq!(data.tool_requests[0].tool_call_id, "tool-1");
                assert_eq!(data.tool_requests[0].name, "bash");
                assert_eq!(
                    data.tool_requests[0].arguments,
                    json!({"command": "echo hi"})
                );
            }
            _ => panic!("Expected AssistantMessage event"),
        }
    }

    #[test]
    fn test_parse_assistant_reasoning_delta() {
        let line = r#"{"type":"assistant.reasoning_delta","data":{"reasoningId":"reason-1","deltaContent":"Thinking..."}}"#;
        let event = CopilotStreamParser::parse_line(line).unwrap();

        match event {
            CopilotStreamEvent::AssistantReasoningDelta { data } => {
                assert_eq!(data.reasoning_id.as_deref(), Some("reason-1"));
                assert_eq!(data.delta_content, "Thinking...");
            }
            _ => panic!("Expected AssistantReasoningDelta event"),
        }
    }

    #[test]
    fn test_parse_tool_execution_start() {
        let line = r#"{"type":"tool.execution_start","data":{"toolCallId":"tool-1","toolName":"bash","arguments":{"command":"echo hi"}}}"#;
        let event = CopilotStreamParser::parse_line(line).unwrap();

        match event {
            CopilotStreamEvent::ToolExecutionStart { data } => {
                assert_eq!(data.tool_call_id, "tool-1");
                assert_eq!(data.tool_name, "bash");
                assert_eq!(data.arguments, json!({"command": "echo hi"}));
            }
            _ => panic!("Expected ToolExecutionStart event"),
        }
    }

    #[test]
    fn test_parse_tool_execution_complete_success() {
        let line = r#"{"type":"tool.execution_complete","data":{"toolCallId":"tool-1","success":true,"result":{"content":"hi\n","detailedContent":"hi\n"}}}"#;
        let event = CopilotStreamParser::parse_line(line).unwrap();

        match event {
            CopilotStreamEvent::ToolExecutionComplete { data } => {
                assert_eq!(data.tool_call_id, "tool-1");
                assert!(data.success);
                assert_eq!(
                    data.result.and_then(|result| result.detailed_content),
                    Some("hi\n".to_string())
                );
            }
            _ => panic!("Expected ToolExecutionComplete event"),
        }
    }

    #[test]
    fn test_parse_result_event() {
        let line = r#"{"type":"result","exitCode":0,"usage":{"totalApiDurationMs":12,"sessionDurationMs":34}}"#;
        let event = CopilotStreamParser::parse_line(line).unwrap();

        match event {
            CopilotStreamEvent::Result { data } => {
                assert_eq!(data.exit_code, Some(0));
                assert_eq!(data.total_api_duration_ms, Some(12));
                assert_eq!(data.session_duration_ms, Some(34));
            }
            _ => panic!("Expected Result event"),
        }
    }

    #[test]
    fn test_extract_text_ignores_non_assistant_lines() {
        let line = r#"{"type":"assistant.turn_start","data":{"turnId":"0"}}"#;
        assert_eq!(CopilotStreamParser::extract_text(line), None);
    }

    #[test]
    fn test_extract_live_chunk_streams_deltas_without_duplication() {
        let mut state = CopilotStreamState::new();
        let delta = r#"{"type":"assistant.message_delta","data":{"messageId":"msg-1","deltaContent":"Hello"}}"#;
        let message =
            r#"{"type":"assistant.message","data":{"messageId":"msg-1","content":"Hello"}}"#;

        assert_eq!(
            CopilotStreamParser::extract_live_chunk(delta, &mut state),
            Some(CopilotLiveChunk {
                text: "Hello".to_string(),
                append_newline: false,
            })
        );
        assert_eq!(
            CopilotStreamParser::extract_live_chunk(message, &mut state),
            Some(CopilotLiveChunk {
                text: String::new(),
                append_newline: true,
            })
        );
    }

    #[test]
    fn test_extract_all_text_aggregates_text_from_jsonl() {
        let raw = concat!(
            "{\"type\":\"assistant.turn_start\",\"data\":{\"turnId\":\"0\"}}\n",
            "{\"type\":\"assistant.message_delta\",\"data\":{\"messageId\":\"msg-1\",\"deltaContent\":\"ignored\"}}\n",
            "{\"type\":\"assistant.message\",\"data\":{\"content\":\"First line\"}}\n",
            "{\"type\":\"assistant.message\",\"data\":{\"content\":\"LOOP_COMPLETE\"}}\n",
            "{\"type\":\"result\",\"exitCode\":0}\n"
        );

        assert_eq!(
            CopilotStreamParser::extract_all_text(raw),
            "First line\nLOOP_COMPLETE\n"
        );
    }

    #[test]
    fn test_sdk_events_outside_supported_subset_parse_as_other() {
        let intent = r#"{"type":"assistant.intent","data":{"intent":"Reviewing parser changes"}}"#;
        let idle = r#"{"type":"session.idle","data":{"backgroundTasks":{}}}"#;

        assert_eq!(
            CopilotStreamParser::parse_line(intent),
            Some(CopilotStreamEvent::Other)
        );
        assert_eq!(
            CopilotStreamParser::parse_line(idle),
            Some(CopilotStreamEvent::Other)
        );
    }

    #[test]
    fn test_dispatch_tool_execution_events_routes_handler_callbacks() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = CopilotStreamState::new();

        let start = CopilotStreamParser::parse_line(
            r#"{"type":"tool.execution_start","data":{"toolCallId":"tool-1","toolName":"bash","arguments":{"command":"echo hi"}}}"#,
        )
        .unwrap();
        dispatch_copilot_stream_event(start, &mut handler, &mut extracted, &mut state);

        let complete = CopilotStreamParser::parse_line(
            r#"{"type":"tool.execution_complete","data":{"toolCallId":"tool-1","success":true,"result":{"content":"hi\n","detailedContent":"hi\n"}}}"#,
        )
        .unwrap();
        dispatch_copilot_stream_event(complete, &mut handler, &mut extracted, &mut state);

        assert_eq!(
            handler.tool_calls,
            vec![(
                "bash".to_string(),
                "tool-1".to_string(),
                json!({"command": "echo hi"}),
            )]
        );
        assert_eq!(
            handler.tool_results,
            vec![("tool-1".to_string(), "hi\n".to_string())]
        );
        assert!(handler.errors.is_empty());
        assert!(extracted.is_empty());
    }

    #[test]
    fn test_dispatch_suppressed_full_message_still_records_extracted_text() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = CopilotStreamState::new();
        state.streamed_message_ids.insert("msg-1".to_string());

        let message = CopilotStreamParser::parse_line(
            r#"{"type":"assistant.message","data":{"messageId":"msg-1","content":"Checking parser"}}"#,
        )
        .unwrap();
        dispatch_copilot_stream_event(message, &mut handler, &mut extracted, &mut state);

        assert_eq!(handler.texts, vec!["\n".to_string()]);
        assert_eq!(extracted, "Checking parser\n");
    }

    #[test]
    fn test_dispatch_tool_execution_complete_error_routes_handler_error() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = CopilotStreamState::new();

        let complete = CopilotStreamParser::parse_line(
            r#"{"type":"tool.execution_complete","data":{"toolCallId":"tool-1","success":false,"error":{"message":"rg: unrecognized file type: rs","code":"failure"}}}"#,
        )
        .unwrap();
        dispatch_copilot_stream_event(complete, &mut handler, &mut extracted, &mut state);

        assert!(handler.tool_results.is_empty());
        assert_eq!(
            handler.errors,
            vec!["rg: unrecognized file type: rs".to_string()]
        );
        assert!(extracted.is_empty());
    }

    #[test]
    fn test_dispatch_result_routes_completion() {
        let mut handler = RecordingHandler::default();
        let mut extracted = String::new();
        let mut state = CopilotStreamState::new();

        let turn_end = CopilotStreamParser::parse_line(
            r#"{"type":"assistant.turn_end","data":{"turnId":"0"}}"#,
        )
        .unwrap();
        dispatch_copilot_stream_event(turn_end, &mut handler, &mut extracted, &mut state);

        let result = CopilotStreamParser::parse_line(
            r#"{"type":"result","exitCode":0,"usage":{"sessionDurationMs":34}}"#,
        )
        .unwrap();
        let session_result =
            dispatch_copilot_stream_event(result, &mut handler, &mut extracted, &mut state)
                .expect("session result");

        assert_eq!(session_result.duration_ms, 34);
        assert_eq!(session_result.num_turns, 1);
        assert!(!session_result.is_error);
        assert_eq!(handler.completions.len(), 1);
        assert_eq!(handler.completions[0].duration_ms, 34);
        assert_eq!(handler.completions[0].num_turns, 1);
    }
}
