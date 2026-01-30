use thiserror::Error;

/// Result type alias for telegram operations.
pub type TelegramResult<T> = std::result::Result<T, TelegramError>;

/// Errors that can occur during Telegram bot operations.
#[derive(Debug, Error)]
pub enum TelegramError {
    /// Bot token is missing from config and environment.
    #[error(
        "telegram bot token not found: set RALPH_TELEGRAM_BOT_TOKEN or configure human.telegram.bot_token"
    )]
    MissingBotToken,

    /// Failed to start the Telegram bot (network, auth, etc.).
    #[error("failed to start telegram bot: {0}")]
    Startup(String),

    /// Failed to send a message after retries.
    #[error("failed to send telegram message after {attempts} attempts: {reason}")]
    Send { attempts: u32, reason: String },

    /// Failed to receive messages.
    #[error("failed to receive telegram messages: {0}")]
    Receive(String),

    /// Timed out waiting for a human response.
    #[error("timed out waiting for human response after {timeout_secs}s")]
    ResponseTimeout { timeout_secs: u64 },

    /// Failed to read or write state file.
    #[error("state persistence error: {0}")]
    State(#[from] std::io::Error),

    /// Failed to parse state JSON.
    #[error("state parse error: {0}")]
    StateParse(#[from] serde_json::Error),

    /// Failed to write event to JSONL.
    #[error("event write error: {0}")]
    EventWrite(String),
}
