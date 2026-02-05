//! # ralph-adapters
//!
//! Agent adapters for the Ralph Orchestrator framework.
//!
//! This crate provides implementations for various AI agent backends:
//! - Claude (Anthropic)
//! - Gemini (Google)
//! - Codex (OpenAI)
//! - Pi (pi-coding-agent)
//! - Amp
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

mod auto_detect;
mod claude_stream;
mod cli_backend;
mod cli_executor;
mod pi_stream;
mod pty_executor;
pub mod pty_handle;
mod stream_handler;

pub use auto_detect::{
    DEFAULT_PRIORITY, NoBackendError, detect_backend, detect_backend_default, is_backend_available,
};
pub use claude_stream::{
    AssistantMessage, ClaudeStreamEvent, ClaudeStreamParser, ContentBlock, Usage, UserContentBlock,
    UserMessage,
};
pub use cli_backend::{CliBackend, CustomBackendError, OutputFormat, PromptMode};
pub use cli_executor::{CliExecutor, ExecutionResult};
pub use pi_stream::{
    PiAssistantEvent, PiContentBlock, PiCost, PiSessionState, PiStreamEvent, PiStreamParser,
    PiToolResult, PiTurnMessage, PiUsage, dispatch_pi_stream_event,
};
pub use pty_executor::{
    CtrlCAction, CtrlCState, PtyConfig, PtyExecutionResult, PtyExecutor, TerminationType,
};
pub use pty_handle::{ControlCommand, PtyHandle};
pub use stream_handler::{
    ConsoleStreamHandler, PrettyStreamHandler, QuietStreamHandler, SessionResult, StreamHandler,
    TuiStreamHandler,
};
