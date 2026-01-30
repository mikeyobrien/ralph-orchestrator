use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeygenError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Voice not found: {0}")]
    VoiceNotFound(String),

    #[error("Video generation failed: {0}")]
    VideoGenerationFailed(String),

    #[error("Video generation timed out after {0} seconds")]
    Timeout(u64),

    #[error("Missing field in response: {0}")]
    MissingField(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, HeygenError>;
