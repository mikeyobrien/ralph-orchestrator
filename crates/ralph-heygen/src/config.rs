use crate::error::{HeygenError, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct HeygenConfig {
    pub api_key: String,
    pub elevenlabs_key_id: String,
    pub default_video_orientation: String,
    pub default_fit: String,
    pub polling_timeout_seconds: u64,
    pub polling_interval_seconds: u64,
}

impl HeygenConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        // Try to load .env file if it exists (ignore if it doesn't)
        let _ = dotenvy::dotenv();

        let api_key = env::var("HEYGEN_API_KEY")
            .map_err(|_| HeygenError::ConfigError("HEYGEN_API_KEY not set".to_string()))?;

        let elevenlabs_key_id = env::var("HEYGEN_IMPORTED_ELEVENLABS_KEY_ID").map_err(|_| {
            HeygenError::ConfigError("HEYGEN_IMPORTED_ELEVENLABS_KEY_ID not set".to_string())
        })?;

        let default_video_orientation = env::var("HEYGEN_DEFAULT_VIDEO_ORIENTATION")
            .unwrap_or_else(|_| "vertical".to_string());

        let default_fit =
            env::var("HEYGEN_DEFAULT_FIT").unwrap_or_else(|_| "contain".to_string());

        let polling_timeout_seconds = env::var("HEYGEN_POLLING_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(600);

        let polling_interval_seconds = env::var("HEYGEN_POLLING_INTERVAL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        Ok(Self {
            api_key,
            elevenlabs_key_id,
            default_video_orientation,
            default_fit,
            polling_timeout_seconds,
            polling_interval_seconds,
        })
    }

    /// Create a new configuration with explicit values
    pub fn new(
        api_key: String,
        elevenlabs_key_id: String,
        default_video_orientation: Option<String>,
        default_fit: Option<String>,
        polling_timeout_seconds: Option<u64>,
        polling_interval_seconds: Option<u64>,
    ) -> Self {
        Self {
            api_key,
            elevenlabs_key_id,
            default_video_orientation: default_video_orientation
                .unwrap_or_else(|| "vertical".to_string()),
            default_fit: default_fit.unwrap_or_else(|| "contain".to_string()),
            polling_timeout_seconds: polling_timeout_seconds.unwrap_or(600),
            polling_interval_seconds: polling_interval_seconds.unwrap_or(3),
        }
    }
}
