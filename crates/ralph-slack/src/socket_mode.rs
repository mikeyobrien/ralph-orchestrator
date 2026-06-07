use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::daemon::{LoopSpawner, SlackDaemon, ThreadNotifier};
use crate::error::{SlackError, SlackResult};
use crate::handler::SlackMessageEvent;

#[derive(Debug, Deserialize)]
pub struct SocketEnvelope {
    #[serde(default)]
    pub envelope_id: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

pub async fn run_socket_mode<S, N>(socket_url: &str, daemon: SlackDaemon<S, N>) -> SlackResult<()>
where
    S: LoopSpawner,
    N: ThreadNotifier,
{
    let (mut ws, _) = connect_async(socket_url).await?;
    while let Some(message) = ws.next().await {
        let message = message?;
        let Message::Text(text) = message else {
            continue;
        };
        let envelope: SocketEnvelope = serde_json::from_str(&text)?;
        if let Some(envelope_id) = envelope.envelope_id.as_deref() {
            ws.send(Message::Text(
                json!({"envelope_id": envelope_id}).to_string().into(),
            ))
            .await?;
        }
        if let Some(event) = slack_message_event_from_payload(&envelope.payload) {
            daemon.handle_event(event).await?;
        }
    }
    Ok(())
}

pub fn slack_message_event_from_payload(payload: &serde_json::Value) -> Option<SlackMessageEvent> {
    match payload.get("type").and_then(|value| value.as_str()) {
        Some("slash_commands") => return slash_command_event(payload),
        Some("block_actions") => return block_action_event(payload),
        _ => {}
    }

    let event = payload.get("event")?;
    let event_type = event.get("type").and_then(|value| value.as_str())?;
    if event_type != "app_mention" && event_type != "message" {
        return None;
    }

    let channel_id = event.get("channel")?.as_str()?.to_string();
    let text = event
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let ts = event.get("ts")?.as_str()?.to_string();
    let user_id = event
        .get("user")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let thread_ts = event
        .get("thread_ts")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let bot_id = event
        .get("bot_id")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let event_id = payload
        .get("event_id")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    Some(SlackMessageEvent {
        event_id,
        channel_id,
        user_id,
        text,
        ts,
        thread_ts,
        bot_id,
        app_mention: event_type == "app_mention",
    })
}

fn slash_command_event(payload: &serde_json::Value) -> Option<SlackMessageEvent> {
    let channel_id = payload.get("channel_id")?.as_str()?.to_string();
    let user_id = payload.get("user_id")?.as_str()?.to_string();
    let text = payload
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let trigger_id = payload
        .get("trigger_id")
        .and_then(|value| value.as_str())
        .unwrap_or("slash");
    let ts = payload
        .get("message_ts")
        .or_else(|| payload.get("thread_ts"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| trigger_id.replace('.', "-"));
    Some(SlackMessageEvent {
        event_id: payload
            .get("envelope_id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        channel_id,
        user_id: Some(user_id),
        text,
        ts,
        thread_ts: None,
        bot_id: None,
        app_mention: true,
    })
}

fn block_action_event(payload: &serde_json::Value) -> Option<SlackMessageEvent> {
    let channel_id = payload.get("channel")?.get("id")?.as_str()?.to_string();
    let user_id = payload.get("user")?.get("id")?.as_str()?.to_string();
    let action = payload.get("actions")?.as_array()?.first()?;
    let action_id = action.get("action_id")?.as_str()?;
    let text = block_action_text(action_id)?;
    let message = payload.get("message")?;
    let message_ts = message.get("ts")?.as_str()?;
    let thread_ts = message
        .get("thread_ts")
        .or_else(|| {
            payload
                .get("container")
                .and_then(|container| container.get("thread_ts"))
        })
        .and_then(|value| value.as_str())
        .unwrap_or(message_ts)
        .to_string();
    let ts = payload
        .get("action_ts")
        .and_then(|value| value.as_str())
        .unwrap_or(message_ts)
        .to_string();
    let event_id = payload
        .get("trigger_id")
        .or_else(|| payload.get("envelope_id"))
        .and_then(|value| value.as_str())
        .map(str::to_string);

    Some(SlackMessageEvent {
        event_id,
        channel_id,
        user_id: Some(user_id),
        text: text.to_string(),
        ts,
        thread_ts: Some(thread_ts),
        bot_id: None,
        app_mention: false,
    })
}

fn block_action_text(action_id: &str) -> Option<&'static str> {
    match action_id {
        "ralph_slack_status" => Some("status"),
        "ralph_slack_tail" => Some("tail 10"),
        "ralph_slack_stop" => Some("stop"),
        "ralph_slack_approve" => Some("approved"),
        "ralph_slack_request_changes" => Some("request changes"),
        _ => None,
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for SlackError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        SlackError::Websocket(error.to_string())
    }
}
