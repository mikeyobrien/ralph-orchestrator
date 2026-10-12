//! JSON shapes for `POST /commands` and `GET /events`.
//!
//! Field names follow `packages/protocol` in `icedrop-labs/pi-durable`
//! (`Command`, `Result`, `ServerMessage`, `Event`). This module covers the
//! slice a chat test page needs: `submit`, `abort`, and the events that draw
//! a transcript. Unknown event `type`s stay in the JSON array so a newer
//! daemon does not break the page.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Conversation id the mock serves. A real daemon assigns its own.
pub const MOCK_CONVERSATION_ID: &str = "mock-root";

/// `agent.model` on snapshots from [`crate`]'s mock server.
pub const MOCK_MODEL: &str = "mock";

/// `whenBusy` on a submit. The phone's Steer button sends `steer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SteerMode {
    #[serde(rename = "steer")]
    Steer,
    #[serde(rename = "followUp")]
    FollowUp,
    #[serde(rename = "reject")]
    Reject,
}

/// Inbox row mode. `write` is the daemon's third mode; this page does not send it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueMode {
    #[serde(rename = "steer")]
    Steer,
    #[serde(rename = "followUp")]
    FollowUp,
    #[serde(rename = "write")]
    Write,
}

impl QueueMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Steer => "steer",
            Self::FollowUp => "followUp",
            Self::Write => "write",
        }
    }
}

/// One command, correlated by [`CommandEnvelope::id`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum Command {
    #[serde(rename = "submit")]
    Submit {
        #[serde(rename = "conversationId", skip_serializing_if = "Option::is_none")]
        conversation_id: Option<String>,
        text: String,
        #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        #[serde(rename = "whenBusy", skip_serializing_if = "Option::is_none")]
        when_busy: Option<SteerMode>,
    },
    #[serde(rename = "abort")]
    Abort {
        #[serde(rename = "conversationId", skip_serializing_if = "Option::is_none")]
        conversation_id: Option<String>,
    },
}

/// Body of `POST /commands`: `{ "id", "command" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandEnvelope {
    pub id: String,
    pub command: Command,
}

/// A command the mock understood, or a message naming the field that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    pub id: String,
    pub command: Result<Command, String>,
}

/// Answer to one command. `id` is the id the client just sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<CommandData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Failure>,
}

impl CommandResult {
    pub fn ack(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: true,
            data: Some(CommandData::Ack),
            error: None,
        }
    }

    pub fn failure(id: impl Into<String>, error: Failure) -> Self {
        Self {
            id: id.into(),
            ok: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CommandData {
    #[serde(rename = "ack")]
    Ack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub retryable: bool,
}

impl Failure {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: Some("invalid".into()),
            retryable: false,
        }
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: Some("busy".into()),
            retryable: false,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: Some("notFound".into()),
            retryable: false,
        }
    }
}

/// A transcript row as `EntrySeed` crosses the wire. Extra harness fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrySeed {
    pub id: String,
    pub kind: String,
    pub preview: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
}

/// An inbox item. `text` is present on user submits so the page can show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queued {
    pub id: String,
    pub mode: QueueMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// One SSE `data` payload. `kind: "event"` is a batch; `kind: "result"` is a command answer.
///
/// Events stay as JSON values. A `type` this client does not fold is still
/// rendered in the event log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ServerMessage {
    #[serde(rename = "event")]
    Event {
        #[serde(rename = "conversationId")]
        conversation_id: String,
        events: Vec<Value>,
    },
    #[serde(rename = "result")]
    Result { result: CommandResult },
}

impl ServerMessage {
    pub fn events(conversation_id: impl Into<String>, events: Vec<Value>) -> Self {
        Self::Event {
            conversation_id: conversation_id.into(),
            events,
        }
    }
}

/// Parse `POST /commands`. A missing id cannot be correlated, so that is `Err`.
/// A bad command still returns the id.
pub fn parse_incoming(body: &str) -> Result<Incoming, Failure> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| Failure::invalid("request: expected an object"))?;
    let Some(root) = value.as_object() else {
        return Err(Failure::invalid("request: expected an object"));
    };
    let Some(id) = non_empty_string(root.get("id")) else {
        return Err(Failure::invalid(
            "request.id: required to correlate a result",
        ));
    };
    let Some(command) = root.get("command") else {
        return Ok(Incoming {
            id,
            command: Err("request.command: required".into()),
        });
    };
    Ok(Incoming {
        id,
        command: parse_command(command),
    })
}

pub fn decode_server_message(data: &str) -> Result<ServerMessage, String> {
    serde_json::from_str(data).map_err(|err| err.to_string())
}

pub fn event_batch(conversation_id: &str, events: Vec<Value>) -> ServerMessage {
    ServerMessage::events(conversation_id, events)
}

pub fn snapshot_event(
    entries: &[EntrySeed],
    run_inputs: Option<&[String]>,
    inbox: &[Queued],
    model: &str,
) -> Value {
    let mut event = json!({
        "type": "snapshot",
        "entries": entries,
        "tools": [],
        "inbox": inbox,
        "agent": { "model": model, "thinkingLevel": "off" },
        "usage": {},
    });
    if let Some(inputs) = run_inputs {
        event["run"] = json!({ "inputs": inputs });
    }
    event
}

pub fn run_start_event(inputs: &[String]) -> Value {
    json!({ "type": "run_start", "inputs": inputs })
}

pub fn run_end_event(inputs: &[String], status: &str) -> Value {
    json!({ "type": "run_end", "inputs": inputs, "status": status })
}

pub fn message_start_event(role: &str) -> Value {
    json!({
        "type": "message_start",
        "message": { "role": role, "content": [{ "type": "text", "text": "" }] }
    })
}

pub fn text_delta_event(delta: &str) -> Value {
    json!({
        "type": "message_update",
        "changes": [{ "type": "text_delta", "contentIndex": 0, "delta": delta }]
    })
}

pub fn message_end_event(entry: &EntrySeed) -> Value {
    json!({ "type": "message_end", "entry": entry })
}

pub fn inbox_update_event(items: &[Queued]) -> Value {
    json!({ "type": "inbox_update", "items": items })
}

fn parse_command(value: &Value) -> Result<Command, String> {
    let Some(obj) = value.as_object() else {
        return Err("command: expected an object".into());
    };
    let type_name = obj.get("type").and_then(Value::as_str).unwrap_or("");
    match type_name {
        "submit" => parse_submit(obj),
        "abort" => parse_abort(obj),
        "" => Err("command.type: expected submit or abort".into()),
        other => Err(format!(
            "command.type: mock implements submit and abort, got {other}"
        )),
    }
}

fn parse_submit(obj: &Map<String, Value>) -> Result<Command, String> {
    let Some(text) = non_empty_string(obj.get("text")) else {
        return Err("command.text: required for submit".into());
    };
    Ok(Command::Submit {
        conversation_id: opt_string(obj, "conversationId")?,
        text,
        request_id: opt_string(obj, "requestId")?,
        when_busy: opt_steer(obj)?,
    })
}

fn parse_abort(obj: &Map<String, Value>) -> Result<Command, String> {
    Ok(Command::Abort {
        conversation_id: opt_string(obj, "conversationId")?,
    })
}

fn opt_string(obj: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(other) => Err(format!("command.{key}: expected text, got {other}")),
    }
}

fn opt_steer(obj: &Map<String, Value>) -> Result<Option<SteerMode>, String> {
    match obj.get("whenBusy") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => match value.as_str() {
            "steer" => Ok(Some(SteerMode::Steer)),
            "followUp" => Ok(Some(SteerMode::FollowUp)),
            "reject" => Ok(Some(SteerMode::Reject)),
            _ => Err(format!(
                "command.whenBusy: expected steer, followUp or reject, got {value}"
            )),
        },
        Some(_) => Err("command.whenBusy: expected steer, followUp or reject".into()),
    }
}

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Command, CommandEnvelope, ServerMessage, SteerMode, decode_server_message, parse_incoming,
        snapshot_event, text_delta_event,
    };

    #[test]
    fn submit_round_trips_through_the_parser() {
        let envelope = CommandEnvelope {
            id: "c1".into(),
            command: Command::Submit {
                conversation_id: Some("mock-root".into()),
                text: "Add a header".into(),
                request_id: Some("req-1".into()),
                when_busy: Some(SteerMode::Steer),
            },
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("\"whenBusy\":\"steer\""));
        let incoming = parse_incoming(&json).unwrap();
        assert_eq!(incoming.id, "c1");
        assert_eq!(incoming.command.unwrap(), envelope.command);
    }

    #[test]
    fn missing_id_cannot_be_correlated() {
        let err = parse_incoming(r#"{"command":{"type":"abort"}}"#).unwrap_err();
        assert_eq!(err.code.as_deref(), Some("invalid"));
    }

    #[test]
    fn unknown_command_keeps_the_id() {
        let incoming =
            parse_incoming(r#"{"id":"c9","command":{"type":"ask","text":"hi"}}"#).unwrap();
        assert_eq!(incoming.id, "c9");
        assert!(incoming.command.is_err());
    }

    #[test]
    fn snapshot_and_delta_decode_and_unknown_events_remain() {
        let snapshot = snapshot_event(&[], None, &[], "mock");
        let delta = text_delta_event("hi");
        let message = ServerMessage::events(
            "mock-root",
            vec![
                snapshot,
                delta,
                serde_json::json!({"type": "speech_notify", "event": "done", "speak": "ok"}),
            ],
        );
        let decoded = decode_server_message(&serde_json::to_string(&message).unwrap()).unwrap();
        let ServerMessage::Event { events, .. } = decoded else {
            panic!("expected an event batch");
        };
        assert_eq!(events[0]["type"], "snapshot");
        assert_eq!(events[1]["changes"][0]["delta"], "hi");
        assert_eq!(events[2]["type"], "speech_notify");
    }
}
