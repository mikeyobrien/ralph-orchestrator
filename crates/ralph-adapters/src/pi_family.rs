//! Pi-family stream event types for parsing `--mode json` NDJSON output.
//!
//! When invoked with `--mode json`, Pi-family backends (today: Pi; later: OMP)
//! emit newline-delimited JSON events. This module provides typed Rust structures
//! for deserializing and processing these events, plus a dispatch function for
//! mapping them to `StreamHandler` calls. Pi and OMP keep distinct
//! `OutputFormat` identities (`PiStreamJson` /, later, `OmpStreamJson`) but share
//! this one tolerant processor.
//!
//! Only events that Ralph needs are modeled as typed variants. All other event
//! types are captured by `#[serde(other)]` and silently ignored, providing
//! forward compatibility with new Pi-family event types.

use crate::stream_handler::{SessionResult, StreamHandler};
use serde::{Deserialize, Deserializer, Serialize};
use std::time::Duration;

/// Events from a Pi-family `--mode json` NDJSON output.
///
/// Only the events Ralph needs are modeled. All other event types
/// (session, agent_start, turn_start, message_start, message_end,
/// tool_execution_update, etc.) are captured by the `Other` variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiFamilyEvent {
    /// Streaming text/thinking deltas and errors from assistant.
    MessageUpdate {
        #[serde(rename = "assistantMessageEvent")]
        assistant_message_event: PiFamilyAssistantEvent,
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
        result: PiFamilyToolResult,
        /// OMP's `isError` is optional; Pi's is required. Absent defaults to
        /// `false` so a missing flag never turns a success into an error.
        #[serde(rename = "isError", default)]
        is_error: bool,
    },

    /// Turn completes — contains per-turn usage/cost and the assistant content
    /// used by the mandatory final-text fallback.
    TurnEnd {
        message: Option<PiFamilyTurnMessage>,
    },

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
pub enum PiFamilyAssistantEvent {
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
///
/// `content` is tolerant of unfamiliar JSON shapes: a non-array value or a
/// partially-malformed array yields an empty/partial vector rather than
/// rejecting the whole `ToolExecutionEnd` record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiFamilyToolResult {
    #[serde(default, deserialize_with = "deserialize_content_blocks")]
    pub content: Vec<PiFamilyContentBlock>,
}

/// Content block within a tool result or an assistant turn message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiFamilyContentBlock {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

/// Message in turn_end — contains usage data and the assistant content blocks
/// used by the mandatory final-text fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiFamilyTurnMessage {
    #[serde(rename = "stopReason")]
    pub stop_reason: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub usage: Option<PiFamilyUsage>,
    /// Assistant content blocks. Real Pi/OMP turn_end fixtures carry this; it is
    /// the source of the mandatory final assistant text fallback. Optional
    /// because some records omit it.
    #[serde(default)]
    pub content: Option<Vec<PiFamilyContentBlock>>,
}

/// Token usage statistics from a Pi-family backend.
///
/// Each counter defaults independently to zero so a partial usage record never
/// rejects the whole turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiFamilyUsage {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(rename = "cacheRead", default)]
    pub cache_read: u64,
    #[serde(rename = "cacheWrite", default)]
    pub cache_write: u64,
    pub cost: Option<PiFamilyCost>,
}

/// Cost breakdown from a Pi-family backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiFamilyCost {
    pub total: f64,
}

/// Parses NDJSON lines from a Pi-family backend's stream output.
pub struct PiFamilyStreamParser;

impl PiFamilyStreamParser {
    /// Parse a single line of NDJSON output.
    ///
    /// Returns `None` for empty lines or malformed JSON (logged at debug level).
    pub fn parse_line(line: &str) -> Option<PiFamilyEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        match serde_json::from_str::<PiFamilyEvent>(trimmed) {
            Ok(event) => Some(event),
            Err(e) => {
                tracing::debug!(
                    "Skipping malformed Pi-family JSON: {} (error: {})",
                    truncate(trimmed, 100),
                    e
                );
                None
            }
        }
    }
}

/// Accumulating processor for a Pi-family `--mode json` NDJSON stream.
///
/// Owns line parsing, blank/malformed-line tolerance, the incremental extracted
/// assistant text, recognized/actionable/malformed counts, terminal stream-error
/// state, the final turn's stop reason + content (for the mandatory final-text
/// fallback), and provider/model + usage/cost accumulation. Callers feed lines
/// via [`Self::process_line`] and finish with [`Self::finalize`].
pub struct PiFamilySessionState {
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub stream_provider: Option<String>,
    pub stream_model: Option<String>,
    /// Accumulated input tokens across all turns.
    pub input_tokens: u64,
    /// Peak per-turn input footprint (`usage.input + usage.cache_read`) across
    /// all turns. Used as the reported `SessionResult.input_tokens` so the
    /// figure reflects the largest live-context turn rather than a cumulative
    /// sum. `input_tokens` (above) stays cumulative for diagnostics.
    pub peak_input_tokens: u64,
    /// Accumulated output tokens across all turns.
    pub output_tokens: u64,
    /// Accumulated cache-read tokens across all turns.
    pub cache_read_tokens: u64,
    /// Accumulated cache-write tokens across all turns.
    pub cache_write_tokens: u64,
    /// Incremental assistant text extracted from `text_delta` events. The
    /// mandatory final-text fallback is appended here at finalization only.
    pub extracted_text: String,
    /// Terminal stream-error state: set when an assistant `error` subevent fires.
    pub stream_error: bool,
    /// Count of recognized (non-`Other`) typed events — the "usable signal"
    /// used by the protocol-mismatch check.
    pub recognized_events: u32,
    /// Count of non-blank lines that failed to deserialize (skipped, logged).
    pub malformed_lines: u32,
    /// Stop reason of the final `turn_end` (e.g. `stop`, `tool_use`).
    pub last_stop_reason: Option<String>,
    /// Content blocks of the final `turn_end` assistant message — the source of
    /// the mandatory final-text fallback when the delta accumulator is empty.
    pub last_turn_content: Option<Vec<PiFamilyContentBlock>>,
    /// Presentation label for diagnostics — `"Pi"` or `"OMP"`. Defaults to
    /// `"Pi-family"` (e.g. in unit tests that drive the processor directly) so
    /// the protocol-mismatch message always names a backend; executors set the
    /// concrete flavor from `OutputFormat` (design Q1 / TR9: OMP diagnostics say
    /// OMP even though parsing is shared).
    pub flavor_label: &'static str,
}

impl PiFamilySessionState {
    pub fn new() -> Self {
        Self {
            total_cost_usd: 0.0,
            num_turns: 0,
            stream_provider: None,
            stream_model: None,
            input_tokens: 0,
            peak_input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            extracted_text: String::new(),
            stream_error: false,
            recognized_events: 0,
            malformed_lines: 0,
            last_stop_reason: None,
            last_turn_content: None,
            flavor_label: "Pi-family",
        }
    }

    /// Returns the assistant text accumulated so far (no fallback applied until
    /// [`Self::finalize`]).
    pub fn extracted_text(&self) -> &str {
        &self.extracted_text
    }

    /// Parse one complete NDJSON line, update counts, and dispatch any typed
    /// event. Owns blank/malformed-line tolerance: blank lines are skipped
    /// silently; malformed lines are counted and (via [`PiFamilyStreamParser`])
    /// logged at debug with a bounded preview. `Other` events are dispatched as
    /// no-ops and do not count as recognized.
    pub fn process_line<H: StreamHandler>(&mut self, line: &str, handler: &mut H, verbose: bool) {
        if line.trim().is_empty() {
            return;
        }
        match PiFamilyStreamParser::parse_line(line) {
            Some(event) => {
                if !matches!(event, PiFamilyEvent::Other) {
                    self.recognized_events += 1;
                }
                dispatch_pi_family_event(event, handler, self, verbose);
            }
            None => self.malformed_lines += 1,
        }
    }

    /// Finalize the session: apply the mandatory final assistant-text fallback,
    /// run the two-case protocol-mismatch check, and construct the `SessionResult`
    /// from duration, process status, and accumulated stream state.
    ///
    /// `process_success` is the raw child exit success. A protocol mismatch on a
    /// successful process flips `is_error` to true and surfaces a separate
    /// actionable message; the caller preserves the exit code and reports the
    /// execution as unsuccessful. A failed process never reports a mismatch
    /// (the non-zero exit already explains the failure).
    pub fn finalize(&mut self, process_success: bool, duration: Duration) -> PiFamilyFinalSummary {
        // Mandatory final assistant-text fallback: ONLY when the delta
        // accumulator is empty, source the text from the final
        // turn_end.message.content[].text blocks. Applied at finalization (not
        // per-turn) so recovered text is never duplicated.
        if self.extracted_text.trim().is_empty()
            && let Some(blocks) = &self.last_turn_content
        {
            let recovered: String = blocks
                .iter()
                .filter_map(|b| match b {
                    PiFamilyContentBlock::Text { text } => Some(text.as_str()),
                    PiFamilyContentBlock::Other => None,
                })
                .collect::<Vec<_>>()
                .join("");
            self.extracted_text = recovered;
        }

        let has_text = !self.extracted_text.trim().is_empty();
        let is_tool_only_turn = self.last_stop_reason.as_deref() == Some("tool_use");

        // Two-case protocol mismatch, checked only on a successful process:
        // (1) no usable assistant/tool/turn event (header/garbage-only stream);
        // (2) usable events but zero recoverable assistant text (no deltas and
        //     empty turn_end content) on a non-tool-only turn. A tool-only turn
        //     (stop reason = tool use) with no text is NOT a mismatch.
        let protocol_error = if process_success && self.recognized_events == 0 {
            Some(format!(
                "{} protocol mismatch: no usable assistant/tool/turn events \
                 recognized (recognized={}, malformed={}); the stream produced only \
                 header/unknown records.",
                self.flavor_label, self.recognized_events, self.malformed_lines
            ))
        } else if process_success && !has_text && !is_tool_only_turn {
            Some(format!(
                "{} protocol mismatch: usable events were seen but zero \
                 assistant text was recovered (no deltas and empty turn_end content) \
                 on a non-tool-only turn.",
                self.flavor_label
            ))
        } else {
            None
        };

        let is_error = !process_success || self.stream_error || protocol_error.is_some();

        let session_result = SessionResult {
            duration_ms: duration.as_millis() as u64,
            total_cost_usd: self.total_cost_usd,
            num_turns: self.num_turns,
            is_error,
            input_tokens: self.peak_input_tokens,
            output_tokens: self.output_tokens,
            cache_read_tokens: self.cache_read_tokens,
            cache_write_tokens: self.cache_write_tokens,
            context_window: 0,
        };

        PiFamilyFinalSummary {
            extracted_text: self.extracted_text.clone(),
            session_result,
            protocol_error,
        }
    }
}

impl Default for PiFamilySessionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Finalized view of a Pi-family stream session.
///
/// Produced by [`PiFamilySessionState::finalize`]. Carries the extracted
/// assistant text (with the mandatory final-text fallback applied), the
/// constructed `SessionResult`, and any protocol-mismatch error.
#[derive(Debug, Clone)]
pub struct PiFamilyFinalSummary {
    /// Assistant text extracted from deltas, with the mandatory
    /// `turn_end.message.content` fallback applied when deltas were empty.
    pub extracted_text: String,
    /// Constructed session result (metrics + merged `is_error`).
    pub session_result: SessionResult,
    /// Protocol-mismatch error surfaced on a successful process that produced
    /// no usable signal (case 1) or no recoverable assistant text on a
    /// non-tool-only turn (case 2). `None` on a clean stream or a failed process.
    pub protocol_error: Option<String>,
}

/// Dispatch a Pi-family stream event to the `StreamHandler`.
///
/// Accumulates cost/turn/identity data and the incremental assistant text in
/// `state` (the processor owns `extracted_text`). The mandatory final-text
/// fallback and protocol-mismatch check run later, at
/// [`PiFamilySessionState::finalize`].
pub fn dispatch_pi_family_event<H: StreamHandler>(
    event: PiFamilyEvent,
    handler: &mut H,
    state: &mut PiFamilySessionState,
    verbose: bool,
) {
    match event {
        PiFamilyEvent::MessageUpdate {
            assistant_message_event,
        } => match assistant_message_event {
            PiFamilyAssistantEvent::TextDelta { delta } => {
                handler.on_text(&delta);
                state.extracted_text.push_str(&delta);
            }
            PiFamilyAssistantEvent::ThinkingDelta { delta } => {
                if verbose {
                    handler.on_text(&delta);
                }
            }
            PiFamilyAssistantEvent::Error { reason } => {
                state.stream_error = true;
                handler.on_error(&reason);
            }
            PiFamilyAssistantEvent::Other => {}
        },
        PiFamilyEvent::ToolExecutionStart {
            tool_name,
            tool_call_id,
            args,
        } => {
            handler.on_tool_call(&tool_name, &tool_call_id, &args);
        }
        PiFamilyEvent::ToolExecutionEnd {
            tool_call_id,
            result,
            is_error,
            ..
        } => {
            let output = result
                .content
                .iter()
                .filter_map(|b| match b {
                    PiFamilyContentBlock::Text { text } => Some(text.as_str()),
                    PiFamilyContentBlock::Other => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if is_error {
                handler.on_error(&output);
            } else {
                handler.on_tool_result(&tool_call_id, &output);
            }
        }
        PiFamilyEvent::TurnEnd { message } => {
            state.num_turns += 1;
            if let Some(msg) = &message {
                state.last_stop_reason = msg.stop_reason.clone();
                state.last_turn_content = msg.content.clone();
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
                if let Some(usage) = &msg.usage {
                    if let Some(cost) = &usage.cost {
                        state.total_cost_usd += cost.total;
                    }
                    state.input_tokens += usage.input;
                    state.peak_input_tokens =
                        state.peak_input_tokens.max(usage.input + usage.cache_read);
                    state.output_tokens += usage.output;
                    state.cache_read_tokens += usage.cache_read;
                    state.cache_write_tokens += usage.cache_write;
                }
            }
        }
        PiFamilyEvent::Other => {}
    }
}

/// Tolerantly deserialize a content-block array.
///
/// A missing field, `null`, a non-array value, or an array containing partially
/// malformed blocks never rejects the enclosing record: well-formed text blocks
/// are kept and everything else is dropped. This preserves tool/turn lifecycle
/// even when the result body uses an unfamiliar JSON shape.
fn deserialize_content_blocks<'de, D>(
    deserializer: D,
) -> Result<Vec<PiFamilyContentBlock>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| serde_json::from_value::<PiFamilyContentBlock>(item).ok())
            .collect(),
        _ => Vec::new(),
    })
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
    // PiFamilyStreamParser::parse_line tests
    // =========================================================================

    #[test]
    fn test_parse_text_delta() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Hello world"}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::MessageUpdate {
                assistant_message_event: PiFamilyAssistantEvent::TextDelta { delta },
            } => {
                assert_eq!(delta, "Hello world");
            }
            _ => panic!("Expected MessageUpdate with TextDelta, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_thinking_delta() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":"Let me think..."}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::MessageUpdate {
                assistant_message_event: PiFamilyAssistantEvent::ThinkingDelta { delta },
            } => {
                assert_eq!(delta, "Let me think...");
            }
            _ => panic!("Expected MessageUpdate with ThinkingDelta, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_error_event() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"error","reason":"aborted"}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::MessageUpdate {
                assistant_message_event: PiFamilyAssistantEvent::Error { reason },
            } => {
                assert_eq!(reason, "aborted");
            }
            _ => panic!("Expected MessageUpdate with Error, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_tool_execution_start() {
        let json = r#"{"type":"tool_execution_start","toolCallId":"toolu_123","toolName":"bash","args":{"command":"echo hello"}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::ToolExecutionStart {
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
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::ToolExecutionEnd {
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
                    PiFamilyContentBlock::Text { text } => assert_eq!(text, "hello\n"),
                    PiFamilyContentBlock::Other => panic!("Expected Text content block"),
                }
            }
            _ => panic!("Expected ToolExecutionEnd, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_tool_execution_end_error() {
        let json = r#"{"type":"tool_execution_end","toolCallId":"toolu_456","toolName":"Read","result":{"content":[{"type":"text","text":"file not found"}]},"isError":true}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::ToolExecutionEnd { is_error, .. } => {
                assert!(is_error);
            }
            _ => panic!("Expected ToolExecutionEnd, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_turn_end_with_usage() {
        let json = r#"{"type":"turn_end","message":{"role":"assistant","content":[],"usage":{"input":1,"output":14,"cacheRead":8932,"cacheWrite":70,"totalTokens":9017,"cost":{"input":0.000005,"output":0.00035,"cacheRead":0.00447,"cacheWrite":0.00044,"total":0.00526}},"stopReason":"stop"},"toolResults":[]}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::TurnEnd { message } => {
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
        let event = PiFamilyStreamParser::parse_line(json).unwrap();

        match event {
            PiFamilyEvent::TurnEnd { message } => {
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
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiFamilyEvent::Other));

        let json = r#"{"type":"agent_start"}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiFamilyEvent::Other));

        let json = r#"{"type":"turn_start"}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiFamilyEvent::Other));

        let json = r#"{"type":"message_start","message":{"role":"user","content":[]}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiFamilyEvent::Other));

        let json = r#"{"type":"message_end","message":{"role":"assistant","content":[]}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiFamilyEvent::Other));
    }

    #[test]
    fn test_parse_unknown_assistant_event_type() {
        // toolcall_start, toolcall_delta, toolcall_end, text_start, text_end, done
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"toolcall_start","contentIndex":0}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        match event {
            PiFamilyEvent::MessageUpdate {
                assistant_message_event: PiFamilyAssistantEvent::Other,
            } => {}
            _ => panic!("Expected MessageUpdate with Other assistant event"),
        }

        let json =
            r#"{"type":"message_update","assistantMessageEvent":{"type":"done","reason":"stop"}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        match event {
            PiFamilyEvent::MessageUpdate {
                assistant_message_event: PiFamilyAssistantEvent::Other,
            } => {}
            _ => panic!("Expected MessageUpdate with Other assistant event"),
        }
    }

    #[test]
    fn test_parse_empty_line() {
        assert!(PiFamilyStreamParser::parse_line("").is_none());
        assert!(PiFamilyStreamParser::parse_line("   ").is_none());
        assert!(PiFamilyStreamParser::parse_line("\n").is_none());
    }

    #[test]
    fn test_parse_malformed_json() {
        assert!(PiFamilyStreamParser::parse_line("{not valid json}").is_none());
        assert!(PiFamilyStreamParser::parse_line("plain text").is_none());
    }

    #[test]
    fn test_parse_tool_execution_update_is_other() {
        let json = r#"{"type":"tool_execution_update","toolCallId":"toolu_123","toolName":"bash","args":{"command":"echo hello"},"partialResult":{"content":[{"type":"text","text":"hello\n"}]}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiFamilyEvent::Other));
    }

    // =========================================================================
    // dispatch_pi_family_event tests
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
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::MessageUpdate {
            assistant_message_event: PiFamilyAssistantEvent::TextDelta {
                delta: "Hello".to_string(),
            },
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert_eq!(handler.texts, vec!["Hello"]);
        assert_eq!(state.extracted_text, "Hello");
    }

    #[test]
    fn test_dispatch_thinking_delta_verbose() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::MessageUpdate {
            assistant_message_event: PiFamilyAssistantEvent::ThinkingDelta {
                delta: "thinking...".to_string(),
            },
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, true);
        assert_eq!(handler.texts, vec!["thinking..."]);
        // Thinking should NOT go into extracted_text (not part of output)
        assert!(state.extracted_text.is_empty());
    }

    #[test]
    fn test_dispatch_thinking_delta_not_verbose() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::MessageUpdate {
            assistant_message_event: PiFamilyAssistantEvent::ThinkingDelta {
                delta: "thinking...".to_string(),
            },
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);
        assert!(handler.texts.is_empty());
        assert!(state.extracted_text.is_empty());
    }

    #[test]
    fn test_dispatch_error() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::MessageUpdate {
            assistant_message_event: PiFamilyAssistantEvent::Error {
                reason: "aborted".to_string(),
            },
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);
        assert_eq!(handler.errors, vec!["aborted"]);
    }

    #[test]
    fn test_dispatch_tool_execution_start() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::ToolExecutionStart {
            tool_call_id: "toolu_123".to_string(),
            tool_name: "bash".to_string(),
            args: json!({"command": "echo hello"}),
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert_eq!(handler.tool_calls.len(), 1);
        assert_eq!(handler.tool_calls[0].0, "bash");
        assert_eq!(handler.tool_calls[0].1, "toolu_123");
        assert_eq!(handler.tool_calls[0].2["command"], "echo hello");
    }

    #[test]
    fn test_dispatch_tool_execution_end_success() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::ToolExecutionEnd {
            tool_call_id: "toolu_123".to_string(),
            tool_name: "bash".to_string(),
            result: PiFamilyToolResult {
                content: vec![PiFamilyContentBlock::Text {
                    text: "hello\n".to_string(),
                }],
            },
            is_error: false,
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert_eq!(handler.tool_results.len(), 1);
        assert_eq!(handler.tool_results[0].0, "toolu_123");
        assert_eq!(handler.tool_results[0].1, "hello\n");
        assert!(handler.errors.is_empty());
    }

    #[test]
    fn test_dispatch_tool_execution_end_error() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::ToolExecutionEnd {
            tool_call_id: "toolu_456".to_string(),
            tool_name: "Read".to_string(),
            result: PiFamilyToolResult {
                content: vec![PiFamilyContentBlock::Text {
                    text: "file not found".to_string(),
                }],
            },
            is_error: true,
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert!(handler.tool_results.is_empty());
        assert_eq!(handler.errors, vec!["file not found"]);
    }

    #[test]
    fn test_dispatch_turn_end_accumulates_cost() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        // Three turns with different costs
        for cost in [0.05, 0.03, 0.01] {
            let event = PiFamilyEvent::TurnEnd {
                message: Some(PiFamilyTurnMessage {
                    stop_reason: Some("stop".to_string()),
                    provider: None,
                    model: None,
                    usage: Some(PiFamilyUsage {
                        input: 100,
                        output: 50,
                        cache_read: 0,
                        cache_write: 0,
                        cost: Some(PiFamilyCost { total: cost }),
                    }),
                    content: None,
                }),
            };
            dispatch_pi_family_event(event, &mut handler, &mut state, false);
        }

        assert_eq!(state.num_turns, 3);
        assert!((state.total_cost_usd - 0.09).abs() < 1e-10);
    }

    #[test]
    fn test_dispatch_turn_end_missing_usage() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::TurnEnd {
            message: Some(PiFamilyTurnMessage {
                stop_reason: Some("stop".to_string()),
                provider: None,
                model: None,
                usage: None,
                content: None,
            }),
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert_eq!(state.num_turns, 1);
        assert!((state.total_cost_usd - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dispatch_turn_end_missing_message() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::TurnEnd { message: None };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert_eq!(state.num_turns, 1);
        assert!((state.total_cost_usd - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dispatch_other_is_noop() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        dispatch_pi_family_event(PiFamilyEvent::Other, &mut handler, &mut state, false);

        assert!(handler.texts.is_empty());
        assert!(handler.tool_calls.is_empty());
        assert!(handler.tool_results.is_empty());
        assert!(handler.errors.is_empty());
        assert!(handler.completions.is_empty());
        assert!(state.extracted_text.is_empty());
        assert_eq!(state.num_turns, 0);
    }

    #[test]
    fn test_dispatch_assistant_other_is_noop() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::MessageUpdate {
            assistant_message_event: PiFamilyAssistantEvent::Other,
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert!(handler.texts.is_empty());
        assert!(handler.errors.is_empty());
    }

    // =========================================================================
    // Real NDJSON line tests (from research samples)
    // =========================================================================

    #[test]
    fn test_parse_real_session_event() {
        let json = r#"{"type":"session","version":3,"id":"550e8400-e29b-41d4-a716-446655440000","timestamp":"2026-02-05T02:39:26.125Z","cwd":"/home/user/project"}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        assert!(matches!(event, PiFamilyEvent::Other));
    }

    #[test]
    fn test_parse_real_tool_execution_start() {
        let json = r#"{"type":"tool_execution_start","toolCallId":"toolu_01BKzy4E5YAeFLdgwFKtNRqv","toolName":"bash","args":{"command":"echo hello"}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        match event {
            PiFamilyEvent::ToolExecutionStart {
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
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        match event {
            PiFamilyEvent::TurnEnd { message } => {
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
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::TurnEnd {
            message: Some(PiFamilyTurnMessage {
                stop_reason: Some("stop".to_string()),
                provider: Some("anthropic".to_string()),
                model: Some("claude-sonnet-4".to_string()),
                usage: None,
                content: None,
            }),
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert_eq!(state.stream_provider, Some("anthropic".to_string()));
        assert_eq!(state.stream_model, Some("claude-sonnet-4".to_string()));
    }

    #[test]
    fn test_tool_result_multiple_content_blocks() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();

        let event = PiFamilyEvent::ToolExecutionEnd {
            tool_call_id: "toolu_789".to_string(),
            tool_name: "Read".to_string(),
            result: PiFamilyToolResult {
                content: vec![
                    PiFamilyContentBlock::Text {
                        text: "line 1".to_string(),
                    },
                    PiFamilyContentBlock::Text {
                        text: "line 2".to_string(),
                    },
                ],
            },
            is_error: false,
        };

        dispatch_pi_family_event(event, &mut handler, &mut state, false);

        assert_eq!(handler.tool_results[0].1, "line 1\nline 2");
    }

    // =========================================================================
    // Schema tolerance (OMP-family readiness) — Step 2 hardening
    // =========================================================================

    #[test]
    fn test_parse_turn_end_carries_content_blocks() {
        // Real Pi/OMP turn_end fixtures carry message.content[]; before this
        // step the field was absent on the struct and serde silently dropped it.
        // It must be modeled so the mandatory final-text fallback has a source.
        let json = r#"{"type":"turn_end","message":{"role":"assistant","content":[{"type":"text","text":"Done."}],"stopReason":"stop"}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        let PiFamilyEvent::TurnEnd { message } = event else {
            panic!("expected TurnEnd, got {event:?}");
        };
        let content = message.unwrap().content.expect("content must be modeled");
        match &content[0] {
            PiFamilyContentBlock::Text { text } => assert_eq!(text, "Done."),
            PiFamilyContentBlock::Other => panic!("expected Text content block"),
        }
    }

    #[test]
    fn test_parse_tool_end_missing_is_error_defaults_false() {
        // OMP's isError is optional. Absent must default to false (a missing flag
        // must never turn a success into an error or reject the record).
        let json = r#"{"type":"tool_execution_end","toolCallId":"t1","toolName":"bash","result":{"content":[{"type":"text","text":"ok"}]}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        match event {
            PiFamilyEvent::ToolExecutionEnd { is_error, .. } => assert!(!is_error),
            _ => panic!("expected ToolExecutionEnd, got {event:?}"),
        }
    }

    #[test]
    fn test_parse_usage_missing_fields_default_zero() {
        // Each token/cache counter must default independently; one present
        // counter must not reject the whole usage record.
        let json = r#"{"type":"turn_end","message":{"content":[],"usage":{"input":7},"stopReason":"stop"}}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        let PiFamilyEvent::TurnEnd { message } = event else {
            panic!("expected TurnEnd, got {event:?}");
        };
        let usage = message.unwrap().usage.unwrap();
        assert_eq!(usage.input, 7);
        assert_eq!(usage.output, 0);
        assert_eq!(usage.cache_read, 0);
        assert_eq!(usage.cache_write, 0);
    }

    #[test]
    fn test_parse_tool_result_non_array_content_not_rejected() {
        // An unfamiliar result.content shape must not reject the whole
        // ToolExecutionEnd — the tool lifecycle is preserved, body omitted.
        let json = r#"{"type":"tool_execution_end","toolCallId":"t1","toolName":"bash","result":{"content":"not an array"},"isError":false}"#;
        let Some(event) = PiFamilyStreamParser::parse_line(json) else {
            panic!("ToolExecutionEnd must survive non-array content");
        };
        match event {
            PiFamilyEvent::ToolExecutionEnd {
                is_error, result, ..
            } => {
                assert!(!is_error);
                assert!(result.content.is_empty(), "non-displayable body -> empty");
            }
            _ => panic!("expected ToolExecutionEnd, got {event:?}"),
        }
    }

    #[test]
    fn test_parse_tool_result_array_with_bad_block_keeps_good() {
        // A single malformed content block must not discard a good sibling nor
        // reject the record.
        let json = r#"{"type":"tool_execution_end","toolCallId":"t1","toolName":"bash","result":{"content":[{"type":"text","text":"keep"},{"type":"text"}]},"isError":false}"#;
        let event = PiFamilyStreamParser::parse_line(json).unwrap();
        match event {
            PiFamilyEvent::ToolExecutionEnd { result, .. } => {
                let texts: Vec<&str> = result
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        PiFamilyContentBlock::Text { text } => Some(text.as_str()),
                        PiFamilyContentBlock::Other => None,
                    })
                    .collect();
                assert_eq!(texts, vec!["keep"]);
            }
            _ => panic!("expected ToolExecutionEnd, got {event:?}"),
        }
    }

    // =========================================================================
    // finalize() — protocol mismatch + final-text fallback (TR4 / TR7)
    // =========================================================================

    #[test]
    fn test_finalize_clean_stream_no_mismatch() {
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        state.process_line(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"hello"}}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop","usage":{"input":1,"output":2}}}"#,
            &mut handler,
            false,
        );
        let summary = state.finalize(true, Duration::from_millis(500));
        assert_eq!(summary.extracted_text, "hello");
        assert!(summary.protocol_error.is_none());
        assert!(!summary.session_result.is_error);
        assert_eq!(summary.session_result.duration_ms, 500);
        assert_eq!(summary.session_result.input_tokens, 1);
        assert_eq!(summary.session_result.output_tokens, 2);
        assert_eq!(summary.session_result.num_turns, 1);
    }

    #[test]
    fn test_finalize_mismatch_no_recognized_events() {
        // Case 1: successful process that produced only header/unknown records.
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        state.process_line(r#"{"type":"session","version":3}"#, &mut handler, false);
        state.process_line(r#"{"type":"agent_start"}"#, &mut handler, false);
        let summary = state.finalize(true, Duration::from_millis(100));
        let pe = summary.protocol_error.expect("case-1 mismatch");
        assert!(pe.contains("no usable"), "case-1 wording: {pe}");
        assert!(summary.session_result.is_error);
        assert!(summary.extracted_text.is_empty());
    }

    #[test]
    fn test_finalize_mismatch_no_text_non_tool_turn() {
        // Case 2: recognized events but zero recoverable assistant text on a
        // non-tool-only turn (stop reason "stop", empty turn_end content).
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        state.process_line(
            r#"{"type":"tool_execution_end","toolCallId":"t1","toolName":"bash","result":{"content":[{"type":"text","text":"ok"}]},"isError":false}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop"}}"#,
            &mut handler,
            false,
        );
        let summary = state.finalize(true, Duration::from_millis(100));
        let pe = summary.protocol_error.expect("case-2 mismatch");
        assert!(pe.contains("zero assistant text"), "case-2 wording: {pe}");
        assert!(summary.session_result.is_error);
    }

    #[test]
    fn test_finalize_tool_only_turn_no_text_is_not_mismatch() {
        // A tool-only turn (stop reason = tool use) with no assistant text is
        // NOT a mismatch — the model legitimately yielded to call a tool.
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        state.process_line(
            r#"{"type":"tool_execution_end","toolCallId":"t1","toolName":"bash","result":{"content":[{"type":"text","text":"ok"}]},"isError":false}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"tool_use"}}"#,
            &mut handler,
            false,
        );
        let summary = state.finalize(true, Duration::from_millis(100));
        assert!(
            summary.protocol_error.is_none(),
            "tool-only turn must not mismatch"
        );
        assert!(!summary.session_result.is_error);
    }

    #[test]
    fn test_finalize_swallowed_delta_turn_end_content_fallback() {
        // Swallowed-delta recovery (7a): the assistant text_delta events fail to
        // deserialize (routed to Other), but turn_end carries non-empty
        // message.content text — the mandatory final-text fallback recovers it.
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        // Unknown assistant subevent (serde(other)) — accumulates no delta text.
        state.process_line(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"toolcall_start","contentIndex":0}}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[{"type":"text","text":"recovered final text"}],"stopReason":"stop","usage":{"input":3,"output":4}}}"#,
            &mut handler,
            false,
        );
        let summary = state.finalize(true, Duration::from_millis(100));
        assert_eq!(summary.extracted_text, "recovered final text");
        assert!(
            summary.protocol_error.is_none(),
            "recovered text means no mismatch"
        );
        assert!(!summary.session_result.is_error);
        assert_eq!(summary.session_result.input_tokens, 3);
        assert_eq!(summary.session_result.output_tokens, 4);
    }

    #[test]
    fn test_finalize_failed_process_skips_mismatch_check() {
        // A failed process (non-zero exit) never reports a mismatch — the
        // non-zero exit already explains the failure.
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        state.process_line(r#"{"type":"session","version":3}"#, &mut handler, false);
        let summary = state.finalize(false, Duration::from_millis(100));
        assert!(
            summary.protocol_error.is_none(),
            "failed process skips the mismatch check"
        );
        assert!(
            summary.session_result.is_error,
            "failed process is still an error"
        );
    }

    #[test]
    fn test_finalize_stream_error_flips_is_error_without_mismatch() {
        // An assistant error subevent sets stream_error; on a successful process
        // with text, is_error flips true but no protocol mismatch is reported.
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        state.process_line(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"partial"}}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"error","reason":"aborted"}}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop"}}"#,
            &mut handler,
            false,
        );
        let summary = state.finalize(true, Duration::from_millis(100));
        assert_eq!(summary.extracted_text, "partial");
        assert!(summary.protocol_error.is_none());
        assert!(
            summary.session_result.is_error,
            "stream_error flips is_error"
        );
    }

    // =========================================================================
    // OMP — same shared processor, OmpStreamJson identity (Step 3)
    // =========================================================================

    #[test]
    fn test_process_omp_session_lines_through_shared_processor() {
        // OMP selects the same shared processor as Pi. Exercises the OMP-specific
        // schema quirks: `isError` omitted on the tool end (defaults false, never
        // an error), `agent_end` ignored (Other), and a terminal turn_end
        // carrying usage/cost + the final-text fallback source.
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        state.process_line(r#"{"type":"session","version":3}"#, &mut handler, false);
        state.process_line(r#"{"type":"agent_start"}"#, &mut handler, false);
        state.process_line(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"OMP says "}}"#,
            &mut handler,
            false,
        );
        // Tool end with NO isError field — OMP-optional, must default to false.
        state.process_line(
            r#"{"type":"tool_execution_end","toolCallId":"t1","toolName":"bash","result":{"content":[{"type":"text","text":"ok"}]}}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[{"type":"text","text":"OMP says done"}],"usage":{"input":10,"output":5,"cacheRead":2,"cacheWrite":1,"cost":{"total":0.03}},"stopReason":"stop"}}"#,
            &mut handler,
            false,
        );
        // agent_end is an OMP record Ralph ignores (Other) — must not affect state.
        state.process_line(r#"{"type":"agent_end"}"#, &mut handler, false);

        let summary = state.finalize(true, Duration::from_millis(250));
        // Delta accumulator wins over the turn_end fallback when both are present.
        assert_eq!(summary.extracted_text, "OMP says ");
        assert!(summary.protocol_error.is_none());
        let session = summary.session_result;
        assert!(!session.is_error, "OMP isError defaults false");
        assert_eq!(session.num_turns, 1);
        assert_eq!(session.input_tokens, 12); // peak: input(10) + cacheRead(2)
        assert_eq!(session.output_tokens, 5);
        assert_eq!(session.cache_read_tokens, 2);
        assert_eq!(session.cache_write_tokens, 1);
        assert!((session.total_cost_usd - 0.03).abs() < 1e-10);
        assert_eq!(session.duration_ms, 250);
        // The tool end (isError omitted) was dispatched as a success result.
        assert_eq!(handler.tool_results.len(), 1);
        assert!(handler.errors.is_empty());
    }

    #[test]
    fn test_finalize_reports_peak_input_tokens_not_cumulative() {
        // SessionResult.input_tokens mirrors the peak per-turn live-context
        // footprint (input + cacheRead), not the cumulative sum, so a smaller
        // later turn never lowers the reported figure and a larger one raises it.
        let mut handler = RecordingHandler::default();
        let mut state = PiFamilySessionState::new();
        // Turn 1 — input=100, cacheRead=1000 → live context 1100.
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop","usage":{"input":100,"output":50,"cacheRead":1000,"cacheWrite":20}}}"#,
            &mut handler,
            false,
        );
        // Turn 2 — input=500, cacheRead=4000 → live context 4500 (new peak).
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop","usage":{"input":500,"output":60,"cacheRead":4000,"cacheWrite":30}}}"#,
            &mut handler,
            false,
        );
        // Turn 3 — smaller again: peak retained, not replaced or summed.
        state.process_line(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"x"}}"#,
            &mut handler,
            false,
        );
        state.process_line(
            r#"{"type":"turn_end","message":{"content":[],"stopReason":"stop","usage":{"input":10,"output":5,"cacheRead":80,"cacheWrite":0}}}"#,
            &mut handler,
            false,
        );
        let session = state
            .finalize(true, Duration::from_millis(0))
            .session_result;
        assert_eq!(
            session.input_tokens, 4500,
            "reported input = peak live context"
        );
        // Cumulative counters (state) are unaffected by the peak mapping.
    }
}
