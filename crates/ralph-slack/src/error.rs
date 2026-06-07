use thiserror::Error;

/// Result alias for Slack integration operations.
pub type SlackResult<T> = Result<T, SlackError>;

/// Errors surfaced by the Slack integration seam.
#[derive(Debug, Error)]
pub enum SlackError {
    #[error("Slack bot token is required")]
    MissingBotToken,

    #[error("Slack app token is required for Socket Mode")]
    MissingAppToken,

    #[error("Slack API error: {0}")]
    Api(String),

    #[error("Slack websocket error: {0}")]
    Websocket(String),

    #[error("Slack event write error: {0}")]
    EventWrite(String),

    #[error("Slack config error: {0}")]
    Config(String),

    #[error("Slack file path rejected: {0}")]
    FilePath(String),

    #[error("invalid Slack loop id: {0}")]
    InvalidLoopId(String),

    #[error("Slack HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Slack I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Slack JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
