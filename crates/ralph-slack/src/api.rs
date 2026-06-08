use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{SlackError, SlackResult};
use crate::renderer::SlackRenderedMessage;

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
        let body = self.message_body(channel, thread_ts, text, None);
        self.post_message_body(body).await
    }

    pub async fn post_blocks(
        &self,
        channel: &str,
        thread_ts: Option<&str>,
        message: &SlackRenderedMessage,
    ) -> SlackResult<String> {
        let body = self.message_body(
            channel,
            thread_ts,
            &message.text,
            Some(message.blocks.clone()),
        );
        self.post_message_body(body).await
    }

    pub async fn update_blocks(
        &self,
        channel: &str,
        message_ts: &str,
        message: &SlackRenderedMessage,
    ) -> SlackResult<()> {
        let body = self.update_body(
            channel,
            message_ts,
            &message.text,
            Some(message.blocks.clone()),
        );
        self.update_message_body(body).await
    }

    pub async fn start_stream(
        &self,
        channel: &str,
        thread_ts: &str,
        recipient_user_id: Option<&str>,
        recipient_team_id: Option<&str>,
        markdown_text: Option<&str>,
        chunks: Vec<SlackStreamChunk>,
    ) -> SlackResult<String> {
        let mut body = json!({
            "channel": channel,
            "thread_ts": thread_ts,
            "task_display_mode": "timeline",
        });
        if let Some(markdown_text) = markdown_text {
            body["markdown_text"] = json!(markdown_text);
        }
        if let Some(recipient_user_id) = recipient_user_id {
            body["recipient_user_id"] = json!(recipient_user_id);
        }
        if let Some(recipient_team_id) = recipient_team_id {
            body["recipient_team_id"] = json!(recipient_team_id);
        }
        if !chunks.is_empty() {
            body["chunks"] = json!(chunks);
        }
        let envelope: StreamMessageResponse =
            self.post_api_json("/api/chat.startStream", body).await?;
        if envelope.ok {
            envelope
                .ts
                .ok_or_else(|| SlackError::Api("chat.startStream missing ts".to_string()))
        } else {
            Err(slack_api_error(envelope.error))
        }
    }

    pub async fn append_stream(
        &self,
        channel: &str,
        stream_ts: &str,
        markdown_text: &str,
        chunks: Vec<SlackStreamChunk>,
    ) -> SlackResult<()> {
        let mut body = json!({
            "channel": channel,
            "ts": stream_ts,
            "markdown_text": markdown_text,
        });
        if !chunks.is_empty() {
            body["chunks"] = json!(chunks);
        }
        self.post_stream_ack("/api/chat.appendStream", body).await
    }

    pub async fn stop_stream(
        &self,
        channel: &str,
        stream_ts: &str,
        markdown_text: Option<&str>,
        chunks: Vec<SlackStreamChunk>,
        blocks: Option<Vec<serde_json::Value>>,
        metadata: Option<serde_json::Value>,
    ) -> SlackResult<()> {
        let mut body = json!({
            "channel": channel,
            "ts": stream_ts,
        });
        if let Some(markdown_text) = markdown_text {
            body["markdown_text"] = json!(markdown_text);
        }
        if !chunks.is_empty() {
            body["chunks"] = json!(chunks);
        }
        if let Some(blocks) = blocks {
            body["blocks"] = json!(blocks);
        }
        if let Some(metadata) = metadata {
            body["metadata"] = metadata;
        }
        self.post_stream_ack("/api/chat.stopStream", body).await
    }

    pub async fn set_assistant_thread_status(
        &self,
        channel_id: &str,
        thread_ts: &str,
        status: &str,
    ) -> SlackResult<()> {
        let body = json!({
            "channel_id": channel_id,
            "thread_ts": thread_ts,
            "status": status,
        });
        let envelope: UpdateMessageResponse = self
            .post_api_json("/api/assistant.threads.setStatus", body)
            .await?;
        if envelope.ok {
            Ok(())
        } else {
            Err(slack_api_error(envelope.error))
        }
    }

    pub fn is_streaming_fallback_error(error: &SlackError) -> bool {
        matches!(
            error,
            SlackError::Api(code)
                if matches!(
                    code.as_str(),
                    "unsupported_method"
                        | "missing_scope"
                        | "method_not_supported_for_channel_type"
                        | "not_allowed_token_type"
                )
        )
    }

    async fn post_stream_ack(&self, path: &str, body: serde_json::Value) -> SlackResult<()> {
        let envelope: UpdateMessageResponse = self.post_api_json(path, body).await?;
        if envelope.ok {
            Ok(())
        } else {
            Err(slack_api_error(envelope.error))
        }
    }

    async fn post_api_json<T>(&self, path: &str, body: serde_json::Value) -> SlackResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = self
            .client
            .post(self.api_url(path))
            .bearer_auth(&self.bot_token)
            .json(&body)
            .send()
            .await?;
        Ok(response.json().await?)
    }

    async fn update_message_body(&self, body: serde_json::Value) -> SlackResult<()> {
        let response = self
            .client
            .post(self.api_url("/api/chat.update"))
            .bearer_auth(&self.bot_token)
            .json(&body)
            .send()
            .await?;
        let envelope: UpdateMessageResponse = response.json().await?;
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

    async fn post_message_body(&self, body: serde_json::Value) -> SlackResult<String> {
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

    fn message_body(
        &self,
        channel: &str,
        thread_ts: Option<&str>,
        text: &str,
        blocks: Option<Vec<serde_json::Value>>,
    ) -> serde_json::Value {
        let mut body = json!({
            "channel": channel,
            "text": text,
        });
        if let Some(thread_ts) = thread_ts {
            body["thread_ts"] = json!(thread_ts);
        }
        if let Some(blocks) = blocks {
            body["blocks"] = json!(blocks);
        }
        body
    }

    fn update_body(
        &self,
        channel: &str,
        message_ts: &str,
        text: &str,
        blocks: Option<Vec<serde_json::Value>>,
    ) -> serde_json::Value {
        let mut body = json!({
            "channel": channel,
            "ts": message_ts,
            "text": text,
        });
        if let Some(blocks) = blocks {
            body["blocks"] = json!(blocks);
        }
        body
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum SlackStreamChunk {
    #[serde(rename = "markdown_text")]
    MarkdownText { text: String },
    #[serde(rename = "task_update")]
    TaskUpdate {
        id: String,
        title: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
}

impl SlackStreamChunk {
    pub fn markdown_text(text: impl Into<String>) -> Self {
        Self::MarkdownText { text: text.into() }
    }

    pub fn task_update(
        id: impl Into<String>,
        title: impl Into<String>,
        status: impl Into<String>,
        details: Option<&str>,
        output: Option<&str>,
    ) -> Self {
        Self::TaskUpdate {
            id: id.into(),
            title: title.into(),
            status: status.into(),
            details: details.map(ToOwned::to_owned),
            output: output.map(ToOwned::to_owned),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PostMessageResponse {
    ok: bool,
    ts: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateMessageResponse {
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamMessageResponse {
    ok: bool,
    ts: Option<String>,
    error: Option<String>,
}

fn slack_api_error(error: Option<String>) -> SlackError {
    SlackError::Api(error.unwrap_or_else(|| "unknown Slack API error".to_string()))
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
