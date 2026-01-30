use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub id: String,
    pub voice_id: Option<String>,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceListResponse {
    pub data: VoiceListData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceListData {
    pub list: Vec<Voice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableVoiceRequest {
    pub key_id: String,
    pub id: String,
    pub voice_id: Option<String>,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableVoiceResponse {
    pub data: EnableVoiceData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableVoiceData {
    pub voice_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadAssetResponse {
    pub data: UploadAssetData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadAssetData {
    pub image_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVideoRequest {
    pub video_orientation: String,
    pub image_key: String,
    pub video_title: String,
    pub script: String,
    pub voice_id: String,
    pub fit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVideoResponse {
    pub data: CreateVideoData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVideoData {
    pub video_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStatusResponse {
    pub data: VideoStatusData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStatusData {
    pub status: String,
    pub video_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum VideoStatus {
    Pending,
    Processing,
    Completed {
        video_url: String,
        thumbnail_url: String,
    },
    Failed(String),
}

impl VideoStatusData {
    pub fn to_status(&self) -> VideoStatus {
        match self.status.as_str() {
            "completed" => {
                if let (Some(video_url), Some(thumbnail_url)) =
                    (&self.video_url, &self.thumbnail_url)
                {
                    VideoStatus::Completed {
                        video_url: video_url.clone(),
                        thumbnail_url: thumbnail_url.clone(),
                    }
                } else {
                    VideoStatus::Failed("Missing video or thumbnail URL".to_string())
                }
            }
            "pending" => VideoStatus::Pending,
            "processing" => VideoStatus::Processing,
            _ => VideoStatus::Failed(
                self.error
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ),
        }
    }
}
