//! JSON-RPC protocol types for Ralph's stdin/stdout communication.
//!
//! This module defines the wire format for Ralph's JSON-lines protocol,
//! enabling IPC between the orchestration loop and frontends (TUI, IDE
//! integrations, custom UIs). The protocol follows pi's `--mode rpc`
//! conventions but is tailored for Ralph's multi-hat, iteration-based model.
//!
//! ## Protocol Overview
//!
//! - **Transport**: Newline-delimited JSON over stdin (commands) and stdout (events)
//! - **Commands**: Sent to Ralph via stdin to control the loop
//! - **Events**: Emitted by Ralph via stdout to report state changes
//! - **Responses**: Command replies with success/failure and optional data

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// Commands (stdin → Ralph)
// ============================================================================

/// Commands sent to Ralph via stdin.
///
/// Each command is a single JSON line. Commands with an `id` field receive
/// correlated responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcCommand {
    /// Start the loop with a prompt (must be sent before loop starts).
    Prompt {
        #[serde(default)]
        id: Option<String>,
        /// The prompt text to execute.
        prompt: String,
        /// Optional backend override.
        #[serde(default)]
        backend: Option<String>,
        /// Optional max iterations override.
        #[serde(default)]
        max_iterations: Option<u32>,
    },

    /// Inject guidance that affects the current or next iteration.
    /// Equivalent to the TUI's guidance input.
    Guidance {
        #[serde(default)]
        id: Option<String>,
        /// The guidance message to inject.
        message: String,
    },

    /// Steer the agent immediately during the current iteration.
    /// The guidance is injected into the running context.
    Steer {
        #[serde(default)]
        id: Option<String>,
        /// The steering message.
        message: String,
    },

    /// Queue a follow-up message for the next iteration.
    FollowUp {
        #[serde(default)]
        id: Option<String>,
        /// The follow-up message.
        message: String,
    },

    /// Request immediate termination of the loop.
    Abort {
        #[serde(default)]
        id: Option<String>,
        /// Optional reason for the abort.
        #[serde(default)]
        reason: Option<String>,
    },

    /// Request the current loop state snapshot.
    GetState {
        #[serde(default)]
        id: Option<String>,
    },

    /// Request iteration history and metadata.
    GetIterations {
        #[serde(default)]
        id: Option<String>,
        /// If true, include full iteration content. Default: false (metadata only).
        #[serde(default)]
        include_content: bool,
    },

    /// Force a hat change for the next iteration.
    SetHat {
        #[serde(default)]
        id: Option<String>,
        /// The hat ID to switch to.
        hat: String,
    },

    /// Response to an extension UI prompt (future support).
    ExtensionUiResponse {
        #[serde(default)]
        id: Option<String>,
        /// The extension request ID being responded to.
        request_id: String,
        /// The user's response data.
        response: Value,
    },
}

impl RpcCommand {
    /// Returns the command's correlation ID if present.
    pub fn id(&self) -> Option<&str> {
        match self {
            RpcCommand::Prompt { id, .. } => id.as_deref(),
            RpcCommand::Guidance { id, .. } => id.as_deref(),
            RpcCommand::Steer { id, .. } => id.as_deref(),
            RpcCommand::FollowUp { id, .. } => id.as_deref(),
            RpcCommand::Abort { id, .. } => id.as_deref(),
            RpcCommand::GetState { id } => id.as_deref(),
            RpcCommand::GetIterations { id, .. } => id.as_deref(),
            RpcCommand::SetHat { id, .. } => id.as_deref(),
            RpcCommand::ExtensionUiResponse { id, .. } => id.as_deref(),
        }
    }

    /// Returns the command type name (for response correlation).
    pub fn command_type(&self) -> &'static str {
        match self {
            RpcCommand::Prompt { .. } => "prompt",
            RpcCommand::Guidance { .. } => "guidance",
            RpcCommand::Steer { .. } => "steer",
            RpcCommand::FollowUp { .. } => "follow_up",
            RpcCommand::Abort { .. } => "abort",
            RpcCommand::GetState { .. } => "get_state",
            RpcCommand::GetIterations { .. } => "get_iterations",
            RpcCommand::SetHat { .. } => "set_hat",
            RpcCommand::ExtensionUiResponse { .. } => "extension_ui_response",
        }
    }
}

// ============================================================================
// Events (Ralph → stdout)
// ============================================================================

/// Events emitted by Ralph via stdout.
///
/// Each event is a single JSON line. Events are emitted in real-time as the
/// orchestration loop progresses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcEvent {
    /// The orchestration loop has started.
    LoopStarted {
        /// The prompt being executed.
        prompt: String,
        /// Maximum iterations configured.
        max_iterations: Option<u32>,
        /// Backend being used.
        backend: String,
        /// Unix timestamp (milliseconds) when the loop started.
        started_at: u64,
    },

    /// A new iteration is beginning.
    IterationStart {
        /// Iteration number (1-indexed).
        iteration: u32,
        /// Maximum iterations configured.
        max_iterations: Option<u32>,
        /// The hat being worn for this iteration.
        hat: String,
        /// Hat display name (with emoji).
        hat_display: String,
        /// Backend being used.
        backend: String,
        /// Unix timestamp (milliseconds).
        started_at: u64,
    },

    /// An iteration has completed.
    IterationEnd {
        /// Iteration number.
        iteration: u32,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Estimated cost in USD.
        cost_usd: f64,
        /// Input tokens used.
        input_tokens: u64,
        /// Output tokens used.
        output_tokens: u64,
        /// Cache read tokens.
        cache_read_tokens: u64,
        /// Cache write tokens.
        cache_write_tokens: u64,
        /// Whether this iteration triggered LOOP_COMPLETE.
        loop_complete_triggered: bool,
    },

    /// Streaming text delta from the agent.
    TextDelta {
        /// Iteration number.
        iteration: u32,
        /// The text chunk.
        delta: String,
    },

    /// A tool invocation is starting.
    ToolCallStart {
        /// Iteration number.
        iteration: u32,
        /// Tool name (e.g., "Bash", "Read", "Grep").
        tool_name: String,
        /// Unique tool call ID.
        tool_call_id: String,
        /// Tool input parameters.
        input: Value,
    },

    /// A tool invocation has completed.
    ToolCallEnd {
        /// Iteration number.
        iteration: u32,
        /// Tool call ID (matches ToolCallStart).
        tool_call_id: String,
        /// Tool output (may be truncated for large outputs).
        output: String,
        /// Whether this was an error result.
        is_error: bool,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    /// An error occurred during execution.
    Error {
        /// Iteration number (0 if loop-level error).
        iteration: u32,
        /// Error code (e.g., "TIMEOUT", "API_ERROR", "PARSE_ERROR").
        code: String,
        /// Human-readable error message.
        message: String,
        /// Whether the error is recoverable.
        recoverable: bool,
    },

    /// The hat has changed.
    HatChanged {
        /// Iteration number where the change takes effect.
        iteration: u32,
        /// Previous hat ID.
        from_hat: String,
        /// New hat ID.
        to_hat: String,
        /// New hat display name.
        to_hat_display: String,
        /// Reason for the change.
        reason: String,
    },

    /// Task status has changed.
    TaskStatusChanged {
        /// Task ID.
        task_id: String,
        /// Previous status.
        from_status: String,
        /// New status.
        to_status: String,
        /// Task title.
        title: String,
    },

    /// Current task counts have been updated.
    TaskCountsUpdated {
        /// Total number of tasks.
        total: usize,
        /// Number of open tasks.
        open: usize,
        /// Number of closed tasks.
        closed: usize,
        /// Number of ready (unblocked) tasks.
        ready: usize,
    },

    /// Acknowledgment that guidance was received.
    GuidanceAck {
        /// The guidance message that was received.
        message: String,
        /// Whether the guidance will be applied to the current or next iteration.
        applies_to: GuidanceTarget,
    },

    /// The orchestration loop has terminated.
    LoopTerminated {
        /// Reason for termination.
        reason: TerminationReason,
        /// Total iterations completed.
        total_iterations: u32,
        /// Total duration in milliseconds.
        duration_ms: u64,
        /// Total estimated cost in USD.
        total_cost_usd: f64,
        /// Unix timestamp (milliseconds).
        terminated_at: u64,
    },

    /// Response to a command.
    Response {
        /// The command type this responds to.
        command: String,
        /// Correlation ID from the command (if provided).
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Whether the command succeeded.
        success: bool,
        /// Response data (command-specific).
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
        /// Error message if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// A generic orchestration event from the EventBus.
    /// Maps ralph_proto::Event topics to RPC for observability.
    OrchestrationEvent {
        /// Event topic (e.g., "build.task", "build.done", "loop.terminate").
        topic: String,
        /// Event payload.
        payload: String,
        /// Source hat ID (if any).
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        /// Target hat ID (if any).
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },

    /// A wave of parallel workers has started.
    WaveStarted {
        /// Hat display name.
        hat_name: String,
        /// Number of parallel workers.
        worker_count: u32,
        /// Timeout per worker in seconds.
        timeout_secs: u64,
    },

    /// A wave worker has completed.
    WaveWorkerDone {
        /// Zero-based worker index.
        index: u32,
        /// Total workers in this wave.
        total: u32,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Whether the worker succeeded.
        success: bool,
        /// Preview of the worker's payload/role.
        payload_preview: String,
    },

    /// Streaming text delta from a wave worker.
    WaveWorkerTextDelta {
        /// Zero-based worker index.
        worker_index: u32,
        /// The text chunk.
        delta: String,
    },

    /// All wave workers have completed.
    WaveCompleted {
        /// Number of successful workers.
        succeeded: usize,
        /// Number of failed workers.
        failed: usize,
        /// Total duration in milliseconds.
        duration_ms: u64,
    },
}

impl RpcEvent {
    /// Creates a successful response event.
    pub fn success_response(command: &str, id: Option<String>, data: Option<Value>) -> Self {
        RpcEvent::Response {
            command: command.to_string(),
            id,
            success: true,
            data,
            error: None,
        }
    }

    /// Creates a failed response event.
    pub fn error_response(command: &str, id: Option<String>, error: impl Into<String>) -> Self {
        RpcEvent::Response {
            command: command.to_string(),
            id,
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

// ============================================================================
// Supporting types
// ============================================================================

/// Target for guidance application.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceTarget {
    /// Guidance will be applied to the current iteration (steer).
    Current,
    /// Guidance will be applied to the next iteration (follow-up).
    Next,
}

/// Reason for loop termination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    /// Loop completed successfully (LOOP_COMPLETE detected).
    Completed,
    /// Maximum iterations reached.
    MaxIterations,
    /// User requested abort.
    Interrupted,
    /// An unrecoverable error occurred.
    Error,
    /// All tasks completed.
    AllTasksClosed,
    /// Backpressure blocked too many times.
    BackpressureLimit,
}

/// Snapshot of loop state (returned by get_state command).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcState {
    /// Current iteration number.
    pub iteration: u32,
    /// Maximum iterations configured.
    pub max_iterations: Option<u32>,
    /// Current hat ID.
    pub hat: String,
    /// Current hat display name.
    pub hat_display: String,
    /// Backend being used.
    pub backend: String,
    /// Whether the loop has completed.
    pub completed: bool,
    /// Loop start time (Unix ms).
    pub started_at: u64,
    /// Current iteration start time (Unix ms).
    pub iteration_started_at: Option<u64>,
    /// Task counts.
    pub task_counts: RpcTaskCounts,
    /// Active task (if any).
    pub active_task: Option<RpcTaskSummary>,
    /// Total cost so far.
    pub total_cost_usd: f64,
}

/// Task counts for RPC state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RpcTaskCounts {
    pub total: usize,
    pub open: usize,
    pub closed: usize,
    pub ready: usize,
}

/// Task summary for RPC state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcTaskSummary {
    pub id: String,
    pub title: String,
    pub status: String,
}

/// Iteration metadata (returned by get_iterations command).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcIterationInfo {
    /// Iteration number.
    pub iteration: u32,
    /// Hat used for this iteration.
    pub hat: String,
    /// Backend used.
    pub backend: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Cost in USD.
    pub cost_usd: f64,
    /// Whether LOOP_COMPLETE was triggered.
    pub loop_complete_triggered: bool,
    /// Full content (only if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ============================================================================
// Parsing and serialization helpers
// ============================================================================

/// Parses a JSON line into an RpcCommand.
///
/// Returns an error with a descriptive message if parsing fails.
pub fn parse_command(line: &str) -> Result<RpcCommand, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("empty line".to_string());
    }
    serde_json::from_str(trimmed).map_err(|e| format!("JSON parse error: {e}"))
}

/// Serializes an RpcEvent to a JSON line (no trailing newline).
pub fn emit_event(event: &RpcEvent) -> String {
    // Unwrap is safe: RpcEvent is always serializable
    serde_json::to_string(event).expect("RpcEvent serialization failed")
}

/// Serializes an RpcEvent to a JSON line with trailing newline.
pub fn emit_event_line(event: &RpcEvent) -> String {
    let mut line = emit_event(event);
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // Command round-trip tests
    // =========================================================================

    #[test]
    fn test_prompt_command_roundtrip() {
        let cmd = RpcCommand::Prompt {
            id: Some("req-1".to_string()),
            prompt: "implement feature X".to_string(),
            backend: Some("claude".to_string()),
            max_iterations: Some(5),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_guidance_command_roundtrip() {
        let cmd = RpcCommand::Guidance {
            id: None,
            message: "focus on tests".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_steer_command_roundtrip() {
        let cmd = RpcCommand::Steer {
            id: Some("steer-1".to_string()),
            message: "use async instead".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_follow_up_command_roundtrip() {
        let cmd = RpcCommand::FollowUp {
            id: None,
            message: "now run the tests".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_abort_command_roundtrip() {
        let cmd = RpcCommand::Abort {
            id: Some("abort-1".to_string()),
            reason: Some("user cancelled".to_string()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_get_state_command_roundtrip() {
        let cmd = RpcCommand::GetState {
            id: Some("state-1".to_string()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_get_iterations_command_roundtrip() {
        let cmd = RpcCommand::GetIterations {
            id: Some("iters-1".to_string()),
            include_content: true,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_set_hat_command_roundtrip() {
        let cmd = RpcCommand::SetHat {
            id: None,
            hat: "confessor".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_extension_ui_response_command_roundtrip() {
        let cmd = RpcCommand::ExtensionUiResponse {
            id: Some("ext-1".to_string()),
            request_id: "ui-req-123".to_string(),
            response: json!({"selected": "option-a"}),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: RpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    // =========================================================================
    // Event round-trip tests
    // =========================================================================

    #[test]
    fn test_loop_started_event_roundtrip() {
        let event = RpcEvent::LoopStarted {
            prompt: "test prompt".to_string(),
            max_iterations: Some(10),
            backend: "claude".to_string(),
            started_at: 1_700_000_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_iteration_start_event_roundtrip() {
        let event = RpcEvent::IterationStart {
            iteration: 3,
            max_iterations: Some(10),
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            started_at: 1_700_000_001_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_iteration_end_event_roundtrip() {
        let event = RpcEvent::IterationEnd {
            iteration: 3,
            duration_ms: 5432,
            cost_usd: 0.0054,
            input_tokens: 8000,
            output_tokens: 500,
            cache_read_tokens: 7500,
            cache_write_tokens: 100,
            loop_complete_triggered: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_text_delta_event_roundtrip() {
        let event = RpcEvent::TextDelta {
            iteration: 2,
            delta: "Hello, world!".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_tool_call_start_event_roundtrip() {
        let event = RpcEvent::ToolCallStart {
            iteration: 1,
            tool_name: "Bash".to_string(),
            tool_call_id: "toolu_123".to_string(),
            input: json!({"command": "ls -la"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_tool_call_end_event_roundtrip() {
        let event = RpcEvent::ToolCallEnd {
            iteration: 1,
            tool_call_id: "toolu_123".to_string(),
            output: "file1.rs\nfile2.rs".to_string(),
            is_error: false,
            duration_ms: 150,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_error_event_roundtrip() {
        let event = RpcEvent::Error {
            iteration: 2,
            code: "TIMEOUT".to_string(),
            message: "API request timed out".to_string(),
            recoverable: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_hat_changed_event_roundtrip() {
        let event = RpcEvent::HatChanged {
            iteration: 4,
            from_hat: "builder".to_string(),
            to_hat: "confessor".to_string(),
            to_hat_display: "🙏Confessor".to_string(),
            reason: "build.done received".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_task_status_changed_event_roundtrip() {
        let event = RpcEvent::TaskStatusChanged {
            task_id: "task-123".to_string(),
            from_status: "open".to_string(),
            to_status: "closed".to_string(),
            title: "Implement feature X".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_task_counts_updated_event_roundtrip() {
        let event = RpcEvent::TaskCountsUpdated {
            total: 10,
            open: 3,
            closed: 7,
            ready: 2,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_guidance_ack_event_roundtrip() {
        let event = RpcEvent::GuidanceAck {
            message: "focus on tests".to_string(),
            applies_to: GuidanceTarget::Next,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_loop_terminated_event_roundtrip() {
        let event = RpcEvent::LoopTerminated {
            reason: TerminationReason::Completed,
            total_iterations: 5,
            duration_ms: 120_000,
            total_cost_usd: 0.25,
            terminated_at: 1_700_000_120_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_response_event_success_roundtrip() {
        let event = RpcEvent::Response {
            command: "get_state".to_string(),
            id: Some("req-42".to_string()),
            success: true,
            data: Some(json!({"iteration": 3})),
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_response_event_error_roundtrip() {
        let event = RpcEvent::Response {
            command: "prompt".to_string(),
            id: Some("req-43".to_string()),
            success: false,
            data: None,
            error: Some("loop already running".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RpcEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    // =========================================================================
    // Termination reason tests
    // =========================================================================

    #[test]
    fn test_termination_reason_variants() {
        let reasons = [
            TerminationReason::Completed,
            TerminationReason::MaxIterations,
            TerminationReason::Interrupted,
            TerminationReason::Error,
            TerminationReason::AllTasksClosed,
            TerminationReason::BackpressureLimit,
        ];
        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let parsed: TerminationReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, parsed);
        }
    }

    // =========================================================================
    // Parsing helper tests
    // =========================================================================

    #[test]
    fn test_parse_command_valid() {
        let line = r#"{"type": "get_state", "id": "test-1"}"#;
        let cmd = parse_command(line).unwrap();
        assert!(matches!(cmd, RpcCommand::GetState { id: Some(ref i) } if i == "test-1"));
    }

    #[test]
    fn test_parse_command_empty() {
        assert!(parse_command("").is_err());
        assert!(parse_command("   ").is_err());
    }

    #[test]
    fn test_parse_command_invalid_json() {
        assert!(parse_command("{not valid}").is_err());
    }

    #[test]
    fn test_parse_command_unknown_type() {
        let line = r#"{"type": "unknown_command"}"#;
        assert!(parse_command(line).is_err());
    }

    #[test]
    fn test_emit_event() {
        let event = RpcEvent::TextDelta {
            iteration: 1,
            delta: "hello".to_string(),
        };
        let json = emit_event(&event);
        assert!(!json.ends_with('\n'));
        assert!(json.contains(r#""type":"text_delta""#));
    }

    #[test]
    fn test_emit_event_line() {
        let event = RpcEvent::TextDelta {
            iteration: 1,
            delta: "hello".to_string(),
        };
        let line = emit_event_line(&event);
        assert!(line.ends_with('\n'));
        assert_eq!(line.matches('\n').count(), 1);
    }

    // =========================================================================
    // Helper method tests
    // =========================================================================

    #[test]
    fn test_command_id() {
        let cmd = RpcCommand::GetState {
            id: Some("req-1".to_string()),
        };
        assert_eq!(cmd.id(), Some("req-1"));

        let cmd = RpcCommand::Abort {
            id: None,
            reason: None,
        };
        assert_eq!(cmd.id(), None);
    }

    #[test]
    fn test_command_type() {
        assert_eq!(
            RpcCommand::Prompt {
                id: None,
                prompt: "test".into(),
                backend: None,
                max_iterations: None
            }
            .command_type(),
            "prompt"
        );
        assert_eq!(
            RpcCommand::GetState { id: None }.command_type(),
            "get_state"
        );
        assert_eq!(
            RpcCommand::Abort {
                id: None,
                reason: None
            }
            .command_type(),
            "abort"
        );
    }

    #[test]
    fn test_success_response() {
        let event = RpcEvent::success_response(
            "get_state",
            Some("req-1".into()),
            Some(json!({"ok": true})),
        );
        match event {
            RpcEvent::Response {
                command,
                id,
                success,
                data,
                error,
            } => {
                assert_eq!(command, "get_state");
                assert_eq!(id, Some("req-1".to_string()));
                assert!(success);
                assert!(data.is_some());
                assert!(error.is_none());
            }
            _ => panic!("Expected Response event"),
        }
    }

    #[test]
    fn test_error_response() {
        let event = RpcEvent::error_response("prompt", None, "loop already running");
        match event {
            RpcEvent::Response {
                command,
                id,
                success,
                data,
                error,
            } => {
                assert_eq!(command, "prompt");
                assert!(id.is_none());
                assert!(!success);
                assert!(data.is_none());
                assert_eq!(error, Some("loop already running".to_string()));
            }
            _ => panic!("Expected Response event"),
        }
    }

    // =========================================================================
    // State types tests
    // =========================================================================

    #[test]
    fn test_rpc_state_roundtrip() {
        let state = RpcState {
            iteration: 3,
            max_iterations: Some(10),
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            completed: false,
            started_at: 1_700_000_000_000,
            iteration_started_at: Some(1_700_000_005_000),
            task_counts: RpcTaskCounts {
                total: 5,
                open: 2,
                closed: 3,
                ready: 1,
            },
            active_task: Some(RpcTaskSummary {
                id: "task-123".to_string(),
                title: "Fix bug".to_string(),
                status: "running".to_string(),
            }),
            total_cost_usd: 0.15,
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: RpcState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn test_rpc_iteration_info_roundtrip() {
        let info = RpcIterationInfo {
            iteration: 2,
            hat: "builder".to_string(),
            backend: "claude".to_string(),
            duration_ms: 5000,
            cost_usd: 0.05,
            loop_complete_triggered: false,
            content: Some("iteration content here".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: RpcIterationInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, parsed);
    }

    // =========================================================================
    // Pi protocol alignment tests (naming conventions)
    // =========================================================================

    #[test]
    fn test_pi_aligned_naming() {
        // Verify our event types follow pi conventions
        let text_delta = RpcEvent::TextDelta {
            iteration: 1,
            delta: "test".to_string(),
        };
        let json = serde_json::to_string(&text_delta).unwrap();
        assert!(json.contains(r#""type":"text_delta""#));

        let tool_start = RpcEvent::ToolCallStart {
            iteration: 1,
            tool_name: "Bash".to_string(),
            tool_call_id: "id".to_string(),
            input: json!({}),
        };
        let json = serde_json::to_string(&tool_start).unwrap();
        assert!(json.contains(r#""type":"tool_call_start""#));

        let tool_end = RpcEvent::ToolCallEnd {
            iteration: 1,
            tool_call_id: "id".to_string(),
            output: String::new(),
            is_error: false,
            duration_ms: 0,
        };
        let json = serde_json::to_string(&tool_end).unwrap();
        assert!(json.contains(r#""type":"tool_call_end""#));
    }
}
