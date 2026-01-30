use crate::config::HeygenConfig;
use crate::error::{HeygenError, Result};
use crate::types::*;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, info};

const API_V1_URL: &str = "https://api.heygen.com/v1";
const API_V2_URL: &str = "https://api.heygen.com/v2";
const API2_URL: &str = "https://api2.heygen.com/v1";
const UPLOAD_URL: &str = "https://upload.heygen.com/v1";

pub struct HeygenApi {
    client: Client,
    config: HeygenConfig,
}

impl HeygenApi {
    /// Create a new HeyGen API client with configuration
    pub fn new(config: HeygenConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        Ok(Self { client, config })
    }

    /// Create API client from environment variables
    pub fn from_env() -> Result<Self> {
        let config = HeygenConfig::from_env()?;
        Self::new(config)
    }

    /// List available voices from HeyGen (via ElevenLabs)
    pub async fn list_voices(&self) -> Result<Vec<Voice>> {
        info!("Fetching available voices from HeyGen");

        let url = format!("{}/third_party/eleven_labs/voice.list", API2_URL);

        let response = self
            .client
            .get(&url)
            .header("accept", "application/json")
            .header("x-api-key", &self.config.api_key)
            .query(&[("key_id", &self.config.elevenlabs_key_id)])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HeygenError::ApiError(format!(
                "Failed to list voices: {} - {}",
                status, body
            )));
        }

        let voice_response: VoiceListResponse = response.json().await?;
        debug!("Found {} voices", voice_response.data.list.len());

        Ok(voice_response.data.list)
    }

    /// Enable a voice in HeyGen
    pub async fn enable_voice(
        &self,
        elevenlabs_voice_id: &str,
        heygen_voice_id: Option<String>,
        name: &str,
    ) -> Result<String> {
        info!("Enabling voice {} in HeyGen", name);

        let url = format!("{}/third_party/voice.enable", API2_URL);

        let request = EnableVoiceRequest {
            key_id: self.config.elevenlabs_key_id.clone(),
            id: elevenlabs_voice_id.to_string(),
            voice_id: heygen_voice_id,
            name: name.to_string(),
            enabled: true,
        };

        let response = self
            .client
            .post(&url)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HeygenError::ApiError(format!(
                "Failed to enable voice: {} - {}",
                status, body
            )));
        }

        let enable_response: EnableVoiceResponse = response.json().await?;
        debug!("Voice enabled with ID: {}", enable_response.data.voice_id);

        Ok(enable_response.data.voice_id)
    }

    /// Ensure a voice is enabled and return its HeyGen voice ID
    pub async fn ensure_voice_enabled(&self, voice_id: &str) -> Result<(String, String)> {
        info!("Ensuring voice {} is enabled", voice_id);

        let voices = self.list_voices().await?;

        let voice = voices
            .iter()
            .find(|v| v.id == voice_id)
            .ok_or_else(|| HeygenError::VoiceNotFound(voice_id.to_string()))?;

        let heygen_voice_id = if voice.enabled {
            info!("Voice {} is already enabled", voice_id);
            voice
                .voice_id
                .clone()
                .ok_or_else(|| HeygenError::MissingField("voice_id".to_string()))?
        } else {
            info!("Voice {} is not enabled, enabling now", voice_id);
            self.enable_voice(&voice.id, voice.voice_id.clone(), &voice.name)
                .await?
        };

        Ok((heygen_voice_id, voice.name.clone()))
    }

    /// Upload an asset (image) to HeyGen
    pub async fn upload_asset(&self, data: Vec<u8>, content_type: &str) -> Result<String> {
        info!("Uploading asset to HeyGen ({} bytes)", data.len());

        let url = format!("{}/asset", UPLOAD_URL);

        let response = self
            .client
            .post(&url)
            .header("accept", "application/json")
            .header("content-type", content_type)
            .header("x-api-key", &self.config.api_key)
            .body(data)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HeygenError::ApiError(format!(
                "Failed to upload asset: {} - {}",
                status, body
            )));
        }

        let upload_response: UploadAssetResponse = response.json().await?;
        debug!("Asset uploaded with key: {}", upload_response.data.image_key);

        Ok(upload_response.data.image_key)
    }

    /// Create a video from an image, script, and voice
    pub async fn create_video(
        &self,
        image_key: &str,
        script: &str,
        voice_id: &str,
        video_title: &str,
        video_orientation: Option<&str>,
        fit: Option<&str>,
    ) -> Result<String> {
        info!("Creating video in HeyGen: {}", video_title);

        let url = format!("{}/video/av4/generate", API_V2_URL);

        let request = CreateVideoRequest {
            video_orientation: video_orientation
                .unwrap_or(&self.config.default_video_orientation)
                .to_string(),
            image_key: image_key.to_string(),
            video_title: video_title.to_string(),
            script: script.to_string(),
            voice_id: voice_id.to_string(),
            fit: fit.unwrap_or(&self.config.default_fit).to_string(),
        };

        let response = self
            .client
            .post(&url)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HeygenError::ApiError(format!(
                "Failed to create video: {} - {}",
                status, body
            )));
        }

        let video_response: CreateVideoResponse = response.json().await?;
        info!("Video creation started with ID: {}", video_response.data.video_id);

        Ok(video_response.data.video_id)
    }

    /// Get video status
    pub async fn get_video_status(&self, video_id: &str) -> Result<VideoStatus> {
        debug!("Checking status for video {}", video_id);

        let url = format!("{}/video_status.get", API_V1_URL);

        let response = self
            .client
            .get(&url)
            .header("accept", "application/json")
            .header("x-api-key", &self.config.api_key)
            .query(&[("video_id", video_id)])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HeygenError::ApiError(format!(
                "Failed to get video status: {} - {}",
                status, body
            )));
        }

        let status_response: VideoStatusResponse = response.json().await?;
        Ok(status_response.data.to_status())
    }

    /// Poll for video completion
    pub async fn poll_for_completion(&self, video_id: &str) -> Result<(String, String)> {
        info!("Polling for video {} completion", video_id);

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(self.config.polling_timeout_seconds);
        let interval = Duration::from_secs(self.config.polling_interval_seconds);

        loop {
            if start.elapsed() >= timeout {
                return Err(HeygenError::Timeout(self.config.polling_timeout_seconds));
            }

            let status = self.get_video_status(video_id).await?;

            match status {
                VideoStatus::Completed {
                    video_url,
                    thumbnail_url,
                } => {
                    info!("Video {} completed successfully", video_id);
                    return Ok((video_url, thumbnail_url));
                }
                VideoStatus::Failed(error) => {
                    return Err(HeygenError::VideoGenerationFailed(error));
                }
                VideoStatus::Pending | VideoStatus::Processing => {
                    debug!("Video {} still processing, waiting...", video_id);
                    tokio::time::sleep(interval).await;
                }
            }
        }
    }

    /// Download file from URL
    pub async fn download_file(&self, url: &str) -> Result<Vec<u8>> {
        debug!("Downloading file from {}", url);

        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(120))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(HeygenError::ApiError(format!(
                "Failed to download file: {}",
                status
            )));
        }

        let bytes = response.bytes().await?;
        debug!("Downloaded {} bytes", bytes.len());

        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_status_conversion() {
        let completed = VideoStatusData {
            status: "completed".to_string(),
            video_url: Some("https://example.com/video.mp4".to_string()),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            error: None,
        };

        match completed.to_status() {
            VideoStatus::Completed { .. } => {}
            _ => panic!("Expected completed status"),
        }

        let failed = VideoStatusData {
            status: "failed".to_string(),
            video_url: None,
            thumbnail_url: None,
            error: Some("Test error".to_string()),
        };

        match failed.to_status() {
            VideoStatus::Failed(err) => assert_eq!(err, "Test error"),
            _ => panic!("Expected failed status"),
        }
    }
}
