//! # ralph-adapters
//!
//! Agent adapters for the Ralph Orchestrator framework.
//!
//! This crate provides implementations for various AI agent backends (the
//! catalogued set lives in [`ralph_core::backend`] as the single source of
//! truth, so detection order, validation, help, and install guidance never
//! drift):
//! - Claude (Anthropic)
//! - Kiro / Kiro ACP
//! - Gemini (Google)
//! - Codex (OpenAI)
//! - Amp
//! - Copilot (GitHub)
//! - OpenCode
//! - Forge
//! - Pi (pi-coding-agent)
//! - Roo (Roo Code)
//! - OMP (oh-my-pi) — shares the Pi-family stream processor
//! - Custom commands
//!
//! Each adapter implements the common CLI executor interface.
//!
//! ## Auto-Detection
//!
//! When config specifies `agent: auto`, the `auto_detect` module handles
//! detecting which backends are available in the system PATH.
//!
//! ## PTY Mode
//!
//! The `pty_executor` module provides PTY-based execution for Claude CLI,
//! preserving rich terminal UI features (colors, spinners, animations) while
//! allowing Ralph to orchestrate iterations. Supports interactive mode (user
//! input forwarded) and observe mode (output-only).

mod acp_executor;
mod auto_detect;
mod claude_stream;
mod cli_backend;
mod cli_executor;
mod copilot_stream;
mod json_rpc_handler;
mod pi_family;
mod pty_executor;
pub mod pty_handle;
mod stream_handler;
pub mod tool_preview;

pub use acp_executor::AcpExecutor;
pub use auto_detect::{
    NoBackendError, default_priority, detect_backend, detect_backend_default, is_backend_available,
};
pub use claude_stream::{
    AssistantMessage, ClaudeStreamEvent, ClaudeStreamParser, ContentBlock, Usage, UserContentBlock,
    UserMessage,
};
pub use cli_backend::{BackendConstructError, CliBackend, OutputFormat, PromptMode};
pub use cli_executor::{CliExecutor, ExecutionResult};
pub use copilot_stream::{CopilotAssistantMessage, CopilotStreamEvent, CopilotStreamParser};
pub use json_rpc_handler::{JsonRpcStreamHandler, stdout_json_rpc_handler};
pub use pi_family::{
    PiFamilyAssistantEvent, PiFamilyContentBlock, PiFamilyCost, PiFamilyEvent,
    PiFamilySessionState, PiFamilyStreamParser, PiFamilyToolResult, PiFamilyTurnMessage,
    PiFamilyUsage, dispatch_pi_family_event,
};
pub use pty_executor::{
    CtrlCAction, CtrlCState, PtyConfig, PtyExecutionResult, PtyExecutor, TerminationType,
};
pub use pty_handle::{ControlCommand, PtyHandle};
pub use stream_handler::{
    ConsoleStreamHandler, PrettyStreamHandler, QuietStreamHandler, SessionResult, StreamHandler,
    TuiStreamHandler,
};
