//! # ralph-adapters
//!
//! Agent adapters for the Ralph Orchestrator framework.
//!
//! This crate provides implementations for various AI agent backends:
//! - Claude (Anthropic)
//! - Gemini (Google)
//! - Codex (OpenAI)
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
mod cli_backend;
mod cli_executor;
mod pty_executor;

pub use auto_detect::{detect_backend, detect_backend_default, is_backend_available, NoBackendError, DEFAULT_PRIORITY};
pub use cli_backend::{CliBackend, CustomBackendError, PromptMode};
pub use cli_executor::{CliExecutor, ExecutionResult};
pub use pty_executor::{
    CtrlCAction, CtrlCState, PtyConfig, PtyExecutionResult, PtyExecutor, TerminationType,
};
