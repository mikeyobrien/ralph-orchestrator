use crate::api::HeygenApi;
use crate::error::Result;
use std::path::Path;
use tokio::fs;
use tracing::{info, warn};

/// Video generation parameters
#[derive(Debug, Clone)]
pub struct VideoGenerationParams {
    pub script: String,
    pub voice_id: String,
    pub image_path: String,
    pub video_title: String,
    pub video_orientation: Option<String>,
    pub fit: Option<String>,
}

/// Result of video generation
#[derive(Debug, Clone)]
pub struct VideoGenerationResult {
    pub video_id: String,
    pub video_url: String,
    pub thumbnail_url: String,
    pub video_data: Option<Vec<u8>>,
    pub thumbnail_data: Option<Vec<u8>>,
}

/// High-level video generator that orchestrates the entire flow
pub struct VideoGenerator {
    api: HeygenApi,
}

impl VideoGenerator {
    /// Create a new video generator
    pub fn new(api: HeygenApi) -> Self {
        Self { api }
    }

    /// Create a video generator from environment variables
    pub fn from_env() -> Result<Self> {
        let api = HeygenApi::from_env()?;
        Ok(Self::new(api))
    }

    /// Generate a video from the provided parameters
    ///
    /// This method orchestrates the entire video generation flow:
    /// 1. Ensure the voice is enabled
    /// 2. Upload the image
    /// 3. Create the video
    /// 4. Poll for completion
    /// 5. Optionally download the video and thumbnail
    pub async fn generate_video(
        &self,
        params: VideoGenerationParams,
        download_files: bool,
    ) -> Result<VideoGenerationResult> {
        info!("Starting video generation: {}", params.video_title);

        // Step 1: Ensure voice is enabled
        info!("Step 1/5: Enabling voice");
        let (heygen_voice_id, voice_name) =
            self.api.ensure_voice_enabled(&params.voice_id).await?;
        info!("Voice enabled: {} ({})", voice_name, heygen_voice_id);

        // Step 2: Upload image
        info!("Step 2/5: Uploading image");
        let image_data = fs::read(&params.image_path).await?;
        let content_type = get_content_type(&params.image_path);
        let image_key = self.api.upload_asset(image_data, &content_type).await?;
        info!("Image uploaded: {}", image_key);

        // Step 3: Create video
        info!("Step 3/5: Creating video");
        let video_id = self
            .api
            .create_video(
                &image_key,
                &params.script,
                &heygen_voice_id,
                &params.video_title,
                params.video_orientation.as_deref(),
                params.fit.as_deref(),
            )
            .await?;
        info!("Video creation initiated: {}", video_id);

        // Step 4: Poll for completion
        info!("Step 4/5: Polling for video completion");
        let (video_url, thumbnail_url) = self.api.poll_for_completion(&video_id).await?;
        info!("Video generation completed!");
        info!("  Video URL: {}", video_url);
        info!("  Thumbnail URL: {}", thumbnail_url);

        // Step 5: Optionally download files
        let (video_data, thumbnail_data) = if download_files {
            info!("Step 5/5: Downloading video and thumbnail");
            let video = self.api.download_file(&video_url).await?;
            let thumbnail = self.api.download_file(&thumbnail_url).await?;
            info!("Downloaded video ({} bytes) and thumbnail ({} bytes)", video.len(), thumbnail.len());
            (Some(video), Some(thumbnail))
        } else {
            info!("Step 5/5: Skipping download (URLs available)");
            (None, None)
        };

        Ok(VideoGenerationResult {
            video_id,
            video_url,
            thumbnail_url,
            video_data,
            thumbnail_data,
        })
    }

    /// Save downloaded video and thumbnail to disk
    pub async fn save_video(
        &self,
        result: &VideoGenerationResult,
        video_output_path: &Path,
        thumbnail_output_path: &Path,
    ) -> Result<()> {
        if let Some(video_data) = &result.video_data {
            info!("Saving video to {:?}", video_output_path);
            fs::write(video_output_path, video_data).await?;
        } else {
            warn!("No video data to save (was download_files=false?)");
        }

        if let Some(thumbnail_data) = &result.thumbnail_data {
            info!("Saving thumbnail to {:?}", thumbnail_output_path);
            fs::write(thumbnail_output_path, thumbnail_data).await?;
        } else {
            warn!("No thumbnail data to save (was download_files=false?)");
        }

        Ok(())
    }
}

/// Detect content type from file extension
fn get_content_type(path: &str) -> String {
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".jpg") || path_lower.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if path_lower.ends_with(".png") {
        "image/png".to_string()
    } else if path_lower.ends_with(".gif") {
        "image/gif".to_string()
    } else if path_lower.ends_with(".webp") {
        "image/webp".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_detection() {
        assert_eq!(get_content_type("image.jpg"), "image/jpeg");
        assert_eq!(get_content_type("image.JPEG"), "image/jpeg");
        assert_eq!(get_content_type("image.png"), "image/png");
        assert_eq!(get_content_type("image.gif"), "image/gif");
        assert_eq!(get_content_type("image.webp"), "image/webp");
        assert_eq!(get_content_type("image.unknown"), "application/octet-stream");
    }
}
