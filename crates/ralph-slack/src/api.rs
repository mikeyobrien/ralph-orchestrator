use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use crate::error::{SlackError, SlackResult};

const DEFAULT_SLACK_API_BASE_URL: &str = "https://slack.com";

#[derive(Clone)]
pub struct SlackApi {
    bot_token: String,
    client: reqwest::Client,
    base_url: String,
}

impl fmt::Debug for SlackApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlackApi")
            .field("bot_token", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl SlackApi {
    pub fn new(bot_token: String, base_url: Option<String>) -> Self {
        Self {
            bot_token,
            client: reqwest::Client::new(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_SLACK_API_BASE_URL.to_string()),
        }
    }

    pub async fn post_message(
        &self,
        channel: &str,
        thread_ts: Option<&str>,
        text: &str,
    ) -> SlackResult<String> {
        let mut body = json!({
            "channel": channel,
            "text": text,
        });
        if let Some(thread_ts) = thread_ts {
            body["thread_ts"] = json!(thread_ts);
        }

        let response = self
            .client
            .post(self.api_url("/api/chat.postMessage"))
            .bearer_auth(&self.bot_token)
            .json(&body)
            .send()
            .await?;
        let envelope: PostMessageResponse = response.json().await?;
        if envelope.ok {
            envelope
                .ts
                .ok_or_else(|| SlackError::Api("chat.postMessage missing ts".to_string()))
        } else {
            Err(SlackError::Api(
                envelope
                    .error
                    .unwrap_or_else(|| "unknown Slack API error".to_string()),
            ))
        }
    }

    pub async fn open_socket_mode_url(&self, app_token: &str) -> SlackResult<String> {
        let response = self
            .client
            .post(self.api_url("/api/apps.connections.open"))
            .bearer_auth(app_token)
            .send()
            .await?;
        let envelope: SocketModeOpenResponse = response.json().await?;
        if envelope.ok {
            envelope
                .url
                .ok_or_else(|| SlackError::Api("apps.connections.open missing url".to_string()))
        } else {
            Err(SlackError::Api(
                envelope
                    .error
                    .unwrap_or_else(|| "unknown Slack API error".to_string()),
            ))
        }
    }

    pub async fn upload_file_external(
        &self,
        channel: &str,
        thread_ts: &str,
        file_path: &Path,
        filename: &str,
        length: u64,
        caption: Option<&str>,
    ) -> SlackResult<()> {
        let upload = self.get_upload_url_external(filename, length).await?;
        let bytes = tokio::fs::read(file_path).await?;
        self.client
            .post(&upload.upload_url)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(bytes)
            .send()
            .await?
            .error_for_status()?;
        self.complete_upload_external(channel, thread_ts, &upload.file_id, filename, caption)
            .await
    }

    async fn get_upload_url_external(
        &self,
        filename: &str,
        length: u64,
    ) -> SlackResult<GetUploadUrlExternalResponse> {
        let length = length.to_string();
        let response = self
            .client
            .post(self.api_url("/api/files.getUploadURLExternal"))
            .bearer_auth(&self.bot_token)
            .form(&[("filename", filename), ("length", length.as_str())])
            .send()
            .await?;
        let envelope: GetUploadUrlExternalResponse = response.json().await?;
        if envelope.ok {
            if envelope.upload_url.is_empty() || envelope.file_id.is_empty() {
                return Err(SlackError::Api(
                    "files.getUploadURLExternal missing upload_url or file_id".to_string(),
                ));
            }
            Ok(envelope)
        } else {
            Err(SlackError::Api(
                envelope
                    .error
                    .unwrap_or_else(|| "unknown Slack API error".to_string()),
            ))
        }
    }

    async fn complete_upload_external(
        &self,
        channel: &str,
        thread_ts: &str,
        file_id: &str,
        filename: &str,
        caption: Option<&str>,
    ) -> SlackResult<()> {
        let file = json!({
            "id": file_id,
            "title": caption.unwrap_or(filename),
        });
        let body = json!({
            "files": [file],
            "channel_id": channel,
            "thread_ts": thread_ts,
        });
        let response = self
            .client
            .post(self.api_url("/api/files.completeUploadExternal"))
            .bearer_auth(&self.bot_token)
            .json(&body)
            .send()
            .await?;
        let envelope: CompleteUploadExternalResponse = response.json().await?;
        if envelope.ok {
            Ok(())
        } else {
            Err(SlackError::Api(
                envelope
                    .error
                    .unwrap_or_else(|| "unknown Slack API error".to_string()),
            ))
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }
}

#[derive(Debug, Deserialize)]
struct PostMessageResponse {
    ok: bool,
    ts: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SocketModeOpenResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUploadUrlExternalResponse {
    ok: bool,
    #[serde(default)]
    upload_url: String,
    #[serde(default)]
    file_id: String,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompleteUploadExternalResponse {
    ok: bool,
    error: Option<String>,
}
