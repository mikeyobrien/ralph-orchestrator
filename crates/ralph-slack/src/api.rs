use serde::Deserialize;
use serde_json::json;

use crate::error::{SlackError, SlackResult};

const DEFAULT_SLACK_API_BASE_URL: &str = "https://slack.com";

#[derive(Debug, Clone)]
pub struct SlackApi {
    bot_token: String,
    client: reqwest::Client,
    base_url: String,
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
