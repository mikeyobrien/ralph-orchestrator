//! # ralph-telegram
//!
//! Telegram integration for human-in-the-loop orchestration in Ralph.
//!
//! This crate provides bidirectional communication between AI agents and humans
//! during orchestration loops via Telegram:
//!
//! - **AI → Human**: Agents emit `human.interact` events; the bot sends questions to Telegram
//! - **Human → AI**: Humans reply or send proactive guidance via Telegram messages
//!
//! ## Key Components
//!
//! - [`StateManager`] — Persists chat ID, pending questions, and reply routing
//! - [`MessageHandler`] — Processes incoming messages and writes events to JSONL
//! - [`TelegramService`] — Lifecycle management for the bot within the event loop
//! - [`error`] — Error types for startup, send, and receive failures

mod bot;
pub mod commands;
pub mod daemon;
mod error;
mod handler;
mod loop_lock;
mod service;
mod state;

pub use bot::{BotApi, TelegramBot, escape_html, markdown_to_telegram_html};
pub use daemon::TelegramDaemon;
pub use error::{TelegramError, TelegramResult};
pub use handler::MessageHandler;
pub use service::{
    BASE_RETRY_DELAY, CheckinContext, MAX_SEND_RETRIES, TelegramService, retry_with_backoff,
};
pub use state::{PendingQuestion, StateManager, TelegramState};
