//! Fold server events into the rows the page draws.
//!
//! A `snapshot` replaces the transcript. That is the daemon's reconnect rule:
//! there are no sequence numbers, so a late subscriber throws away local rows
//! and draws the snapshot. The trace is this page's own log of frames it saw.

use serde_json::Value;

use crate::wire::{Queued, ServerMessage};

const TRACE_LIMIT: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub live: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct View {
    pub messages: Vec<ChatMessage>,
    pub running: bool,
    pub run_status: Option<String>,
    pub inbox: Vec<Queued>,
    pub agent_model: Option<String>,
    pub trace: Vec<String>,
    pub conversation_id: Option<String>,
}

pub fn apply_server_message(view: &mut View, message: &ServerMessage) {
    match message {
        ServerMessage::Event {
            conversation_id,
            events,
        } => {
            view.conversation_id = Some(conversation_id.clone());
            for event in events {
                apply_event(view, event);
            }
        }
        ServerMessage::Result { result } => {
            let line = match &result.error {
                Some(error) => format!(
                    "result {} · {}",
                    error.code.as_deref().unwrap_or("error"),
                    error.message
                ),
                None => format!("result {} · ok", result.id),
            };
            push_trace(view, line);
        }
    }
}

fn apply_event(view: &mut View, event: &Value) {
    let kind = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match kind {
        "snapshot" => apply_snapshot(view, event),
        "run_start" => {
            view.running = true;
            view.run_status = None;
        }
        "run_end" => {
            view.running = false;
            view.run_status = event
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        "message_start" => {
            let role = event
                .pointer("/message/role")
                .and_then(Value::as_str)
                .unwrap_or("assistant");
            view.messages.push(ChatMessage {
                id: "live".into(),
                role: role.to_string(),
                text: String::new(),
                live: true,
            });
        }
        "message_update" => apply_deltas(view, event),
        "message_end" => apply_message_end(view, event.get("entry")),
        "inbox_update" => {
            view.inbox = inbox_from(event.get("items"));
        }
        _ => {}
    }
    push_trace(view, trace_line(kind, event));
}

fn apply_snapshot(view: &mut View, event: &Value) {
    view.messages = event
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().filter_map(message_from_entry).collect())
        .unwrap_or_default();
    view.inbox = inbox_from(event.get("inbox"));
    view.running = event.get("run").is_some_and(|run| !run.is_null());
    if !view.running {
        view.run_status = None;
    }
    view.agent_model = event
        .pointer("/agent/model")
        .and_then(Value::as_str)
        .map(str::to_string);
}

fn apply_deltas(view: &mut View, event: &Value) {
    let Some(changes) = event.get("changes").and_then(Value::as_array) else {
        return;
    };
    for change in changes {
        if change.get("type").and_then(Value::as_str) != Some("text_delta") {
            continue;
        }
        let Some(delta) = change.get("delta").and_then(Value::as_str) else {
            continue;
        };
        append_live(view, delta);
    }
}

fn append_live(view: &mut View, delta: &str) {
    if let Some(message) = view.messages.iter_mut().rev().find(|message| message.live) {
        message.text.push_str(delta);
        return;
    }
    view.messages.push(ChatMessage {
        id: "live".into(),
        role: "assistant".into(),
        text: delta.to_string(),
        live: true,
    });
}

fn apply_message_end(view: &mut View, entry: Option<&Value>) {
    let Some(entry) = entry else {
        return;
    };
    let Some(mut message) = message_from_entry(entry) else {
        return;
    };
    if let Some(live) = view.messages.iter_mut().rev().find(|item| item.live) {
        if message.text.is_empty() {
            message.text = live.text.clone();
        }
        *live = message;
        return;
    }
    view.messages.push(message);
}

fn message_from_entry(value: &Value) -> Option<ChatMessage> {
    let id = value.get("id").and_then(Value::as_str)?.to_string();
    let kind = value.get("kind").and_then(Value::as_str).unwrap_or("entry");
    let text = value
        .get("preview")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some(ChatMessage {
        id,
        role: role_of(kind),
        text,
        live: false,
    })
}

fn role_of(kind: &str) -> String {
    let lower = kind.to_ascii_lowercase();
    if lower.contains("user") {
        "user".into()
    } else if lower.contains("assistant") {
        "assistant".into()
    } else {
        kind.to_string()
    }
}

fn inbox_from(value: Option<&Value>) -> Vec<Queued> {
    value
        .and_then(|items| serde_json::from_value(items.clone()).ok())
        .unwrap_or_default()
}

fn trace_line(kind: &str, event: &Value) -> String {
    if kind == "message_update" {
        let delta = event
            .pointer("/changes/0/delta")
            .and_then(Value::as_str)
            .unwrap_or("");
        return format!("text_delta · {}", clip(delta, 48));
    }
    if kind == "run_end" {
        let status = event.get("status").and_then(Value::as_str).unwrap_or("");
        return format!("run_end · {status}");
    }
    if kind == "snapshot" {
        let count = event
            .get("entries")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        return format!("snapshot · {count} entries");
    }
    kind.to_string()
}

fn clip(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let clipped: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn note_trace(view: &mut View, line: impl Into<String>) {
    push_trace(view, line.into());
}

fn push_trace(view: &mut View, line: String) {
    view.trace.push(line);
    let extra = view.trace.len().saturating_sub(TRACE_LIMIT);
    if extra > 0 {
        view.trace.drain(0..extra);
    }
}

#[cfg(test)]
mod tests {
    use super::{View, apply_server_message};
    use crate::wire::{
        EntrySeed, event_batch, message_end_event, message_start_event, run_end_event,
        snapshot_event, text_delta_event,
    };

    fn user(text: &str) -> EntrySeed {
        EntrySeed {
            id: "e-user".into(),
            kind: "user".into(),
            preview: text.into(),
            at: None,
        }
    }

    #[test]
    fn deltas_append_until_message_end() {
        let mut view = View::default();
        let batch = event_batch(
            "mock-root",
            vec![
                message_start_event("assistant"),
                text_delta_event("hello "),
                text_delta_event("there"),
            ],
        );
        apply_server_message(&mut view, &batch);
        assert_eq!(view.messages.len(), 1);
        assert!(view.messages[0].live);
        assert_eq!(view.messages[0].text, "hello there");

        let entry = EntrySeed {
            id: "e2".into(),
            kind: "assistant".into(),
            preview: "hello there".into(),
            at: Some(1),
        };
        apply_server_message(
            &mut view,
            &event_batch("mock-root", vec![message_end_event(&entry)]),
        );
        assert!(!view.messages[0].live);
        assert_eq!(view.messages[0].id, "e2");
    }

    #[test]
    fn snapshot_replaces_local_rows() {
        let mut view = View::default();
        apply_server_message(
            &mut view,
            &event_batch("mock-root", vec![text_delta_event("partial")]),
        );
        assert!(view.messages[0].live);

        let healed = snapshot_event(&[user("kept")], None, &[], "mock");
        apply_server_message(&mut view, &event_batch("mock-root", vec![healed]));
        assert_eq!(view.messages.len(), 1);
        assert_eq!(view.messages[0].text, "kept");
        assert!(!view.messages[0].live);
        assert_eq!(view.agent_model.as_deref(), Some("mock"));
        assert!(!view.running);
    }

    #[test]
    fn run_end_records_aborted() {
        let mut view = View::default();
        apply_server_message(
            &mut view,
            &event_batch("c", vec![run_end_event(&["hi".into()], "aborted")]),
        );
        assert!(!view.running);
        assert_eq!(view.run_status.as_deref(), Some("aborted"));
        assert!(view.trace.iter().any(|line| line.contains("aborted")));
    }
}
