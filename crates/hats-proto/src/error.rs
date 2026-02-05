//! Error types for the Hats framework.

use thiserror::Error;

/// Errors that can occur in the Hats.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid topic pattern: {0}")]
    InvalidTopic(String),

    #[error("Hat not found: {0}")]
    HatNotFound(String),

    #[error("Event parse error: {0}")]
    EventParse(String),

    #[error("CLI execution error: {0}")]
    CliExecution(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Loop terminated: {0}")]
    LoopTerminated(String),
}

/// Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;
