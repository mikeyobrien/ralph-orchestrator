//! Bridges RPC v1 stream events into TUI state updates.
//!
//! Connects to a ralph-api server via WebSocket subscription and translates
//! stream events into the same `TuiState` mutations that the in-process
//! observer produces. This makes the TUI work identically whether embedded
//! in the orchestration process or attached remotely.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use ratatui::text::Line;
use serde_json::Value;
use tokio::sync::watch;
use tracing::{debug, info, warn};

use crate::rpc_client::{RpcClient, StreamEvent};
use crate::state::{TaskCounts, TaskSummary, TuiState};
use crate::state_mutations::{
    append_error_line, apply_loop_completed, apply_task_active, apply_task_close,
};

/// Topics the TUI subscribes to via the RPC stream.
const TUI_STREAM_TOPICS: &[&str] = &[
    "task.log.line",
    "task.status.changed",
    "loop.status.changed",
    "loop.merge.progress",
    "system.heartbeat",
    "system.lifecycle",
    "error.raised",
    "stream.keepalive",
];

/// Runs the RPC bridge: subscribes to the stream, processes events, and
/// updates TuiState until the cancellation signal fires.
///
/// On WebSocket disconnect, automatically reconnects with the last known
/// cursor for seamless replay.
pub async fn run_rpc_bridge(
    client: RpcClient,
    state: Arc<Mutex<TuiState>>,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<()> {
    // Fetch initial state via HTTP
    if let Err(e) = seed_initial_state(&client, &state).await {
        warn!(error = %e, "Failed to seed initial state from RPC — continuing with defaults");
    }

    // Subscribe to stream
    let sub = client
        .stream_subscribe(TUI_STREAM_TOPICS, None)
        .await
        .context("failed to create stream subscription")?;

    info!(
        subscription_id = %sub.subscription_id,
        topics = ?sub.accepted_topics,
        cursor = %sub.cursor,
        "TUI subscribed to RPC stream"
    );

    let mut cursor = sub.cursor.clone();
    let subscription_id = sub.subscription_id.clone();
    let mut reconnect_delay = Duration::from_millis(500);
    let max_reconnect_delay = Duration::from_secs(15);

    loop {
        // Connect WebSocket
        let ws_url = client.stream_ws_url(&subscription_id)?;
        debug!(url = %ws_url, "Connecting TUI WebSocket");

        let ws_result = tokio_tungstenite::connect_async(&ws_url).await;
        let (ws, _response) = match ws_result {
            Ok(pair) => {
                reconnect_delay = Duration::from_millis(500); // reset on success
                pair
            }
            Err(e) => {
                warn!(error = %e, delay_ms = reconnect_delay.as_millis(), "WebSocket connect failed, retrying");
                tokio::select! {
                    _ = tokio::time::sleep(reconnect_delay) => {}
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() { return Ok(()); }
                    }
                }
                reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay);
                continue;
            }
        };

        info!("TUI WebSocket connected");

        let (mut ws_tx, mut ws_rx) = ws.split();

        // Process messages until disconnect or cancel
        loop {
            tokio::select! {
                biased;

                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        debug!("RPC bridge cancelled");
                        let _ = ws_tx.close().await;
                        return Ok(());
                    }
                }

                msg = ws_rx.next() => {
                    match msg {
                        Some(Ok(tungstenite::Message::Text(text))) => {
                            match serde_json::from_str::<StreamEvent>(&text) {
                                Ok(event) => {
                                    cursor = event.cursor.clone();
                                    apply_stream_event(&event, &state);
                                }
                                Err(e) => {
                                    debug!(error = %e, "Failed to parse stream event");
                                }
                            }
                        }
                        Some(Ok(tungstenite::Message::Ping(data))) => {
                            let _ = ws_tx.send(tungstenite::Message::Pong(data)).await;
                        }
                        Some(Ok(tungstenite::Message::Close(_))) | None => {
                            info!("WebSocket closed, will reconnect");
                            break; // reconnect loop
                        }
                        Some(Err(e)) => {
                            warn!(error = %e, "WebSocket error, will reconnect");
                            break; // reconnect loop
                        }
                        _ => {} // Pong, Binary, Frame — ignore
                    }
                }
            }
        }

        // Ack the last cursor before reconnecting so replay skips already-seen events
        if let Err(e) = client.stream_ack(&subscription_id, &cursor).await {
            debug!(error = %e, "Failed to ack cursor before reconnect");
        }

        // Brief delay before reconnect
        tokio::select! {
            _ = tokio::time::sleep(reconnect_delay) => {}
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() { return Ok(()); }
            }
        }
        reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay);
    }
}

// ---------------------------------------------------------------------------
// Initial state seeding
// ---------------------------------------------------------------------------

async fn seed_initial_state(client: &RpcClient, state: &Arc<Mutex<TuiState>>) -> Result<()> {
    // Fetch tasks for task counts
    let tasks = client.task_list().await.unwrap_or_default();
    let total = tasks.len();
    let open = tasks.iter().filter(|t| t.status == "open").count();
    let closed = tasks.iter().filter(|t| t.status == "closed").count();
    let ready = tasks
        .iter()
        .filter(|t| t.status == "open" || t.status == "ready")
        .count();

    let active_task = tasks
        .iter()
        .find(|t| t.status == "running" || t.status == "open")
        .map(|t| TaskSummary::new(&t.id, &t.title, &t.status));

    // Fetch config for max_iterations
    let config = client.config_get().await.ok();
    let max_iterations = config
        .as_ref()
        .and_then(|c| c.get("config"))
        .and_then(|c| c.get("event_loop"))
        .and_then(|el| el.get("max_iterations"))
        .and_then(Value::as_u64)
        .map(|n| n as u32);

    // Apply to state
    if let Ok(mut s) = state.lock() {
        s.set_task_counts(TaskCounts::new(total, open, closed, ready));
        s.set_active_task(active_task);
        if let Some(max) = max_iterations {
            s.max_iterations = Some(max);
        }
    }

    debug!(
        tasks = total,
        max_iterations = ?max_iterations,
        "Seeded TUI state from RPC"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Stream event → TuiState translation
// ---------------------------------------------------------------------------

fn apply_stream_event(event: &StreamEvent, state: &Arc<Mutex<TuiState>>) {
    let Ok(mut s) = state.lock() else { return };

    match event.topic.as_str() {
        "task.log.line" => {
            apply_log_line(event, &mut s);
        }
        "task.status.changed" => {
            apply_task_status_change(event, &mut s);
        }
        "loop.status.changed" => {
            apply_loop_status_change(event, &mut s);
        }
        "system.lifecycle" => {
            apply_lifecycle(event, &mut s);
        }
        "error.raised" => {
            apply_error(event, &mut s);
        }
        "stream.keepalive" | "system.heartbeat" | "loop.merge.progress" => {
            // Update liveness indicator
            s.last_event = Some(event.topic.clone());
            s.last_event_at = Some(std::time::Instant::now());
        }
        _ => {
            debug!(topic = %event.topic, "Unhandled stream topic in TUI bridge");
        }
    }
}

/// Append a log line to the current iteration buffer.
///
/// Payload shape: `{ "line": "text...", "iteration": 3, "hat": "Builder" }`
fn apply_log_line(event: &StreamEvent, state: &mut TuiState) {
    let text = event
        .payload
        .get("line")
        .and_then(Value::as_str)
        .unwrap_or("");
    let iteration = event
        .payload
        .get("iteration")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let hat = event
        .payload
        .get("hat")
        .and_then(Value::as_str)
        .map(String::from);
    let backend = event
        .payload
        .get("backend")
        .and_then(Value::as_str)
        .map(String::from);

    // Ensure we have an iteration buffer for this iteration
    while state.total_iterations() < iteration as usize {
        state.start_new_iteration_with_metadata(hat.clone(), backend.clone());
    }

    // Append line to the latest iteration buffer
    if let Some(handle) = state.latest_iteration_lines_handle()
        && let Ok(mut lines) = handle.lock()
    {
        lines.push(Line::from(text.to_string()));
    }

    state.last_event = Some("task.log.line".to_string());
    state.last_event_at = Some(std::time::Instant::now());
}

/// Handle task status transitions → update task counts and active task.
///
/// Payload: `{ "from": "open", "to": "running" }`
fn apply_task_status_change(event: &StreamEvent, state: &mut TuiState) {
    let to = event
        .payload
        .get("to")
        .and_then(Value::as_str)
        .unwrap_or("");
    let task_id = &event.resource.id;

    match to {
        "running" => {
            apply_task_active(state, task_id, task_id, "running");
        }
        "closed" | "done" => {
            apply_task_close(state, task_id);
        }
        _ => {}
    }

    state.last_event = Some("task.status.changed".to_string());
    state.last_event_at = Some(std::time::Instant::now());
}

/// Handle loop status transitions → iteration boundary + completion.
///
/// Payload: `{ "loopId": "...", "status": "running"|"completed"|"failed", "iteration": 3 }`
fn apply_loop_status_change(event: &StreamEvent, state: &mut TuiState) {
    let status = event
        .payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("");
    let hat = event
        .payload
        .get("hat")
        .and_then(Value::as_str)
        .map(String::from);
    let backend = event
        .payload
        .get("backend")
        .and_then(Value::as_str)
        .map(String::from);

    match status {
        "iteration_started" => {
            state.start_new_iteration_with_metadata(hat, backend);
            state.iteration_started = Some(std::time::Instant::now());
        }
        "iteration_completed" => {
            state.prev_iteration = state.iteration;
            state.iteration += 1;
            state.finish_latest_iteration();
        }
        "completed" | "terminated" => {
            apply_loop_completed(state);
        }
        _ => {}
    }

    state.last_event = Some("loop.status.changed".to_string());
    state.last_event_at = Some(std::time::Instant::now());
}

/// Handle system lifecycle events (loop started, terminated).
fn apply_lifecycle(event: &StreamEvent, state: &mut TuiState) {
    let phase = event
        .payload
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("");

    if phase == "started" {
        state.loop_started = Some(std::time::Instant::now());
    } else if phase == "terminated" {
        apply_loop_completed(state);
    }

    state.last_event = Some("system.lifecycle".to_string());
    state.last_event_at = Some(std::time::Instant::now());
}

/// Surface errors in the TUI content area.
fn apply_error(event: &StreamEvent, state: &mut TuiState) {
    let message = event
        .payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    let code = event
        .payload
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");

    append_error_line(state, code, message);

    state.last_event = Some("error.raised".to_string());
    state.last_event_at = Some(std::time::Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc_client::{StreamReplay, StreamResource};
    use serde_json::json;

    fn make_state() -> Arc<Mutex<TuiState>> {
        Arc::new(Mutex::new(TuiState::new()))
    }

    fn make_event(topic: &str, payload: Value) -> StreamEvent {
        StreamEvent {
            api_version: "v1".to_string(),
            stream: "events.v1".to_string(),
            topic: topic.to_string(),
            cursor: "1234-0".to_string(),
            sequence: 0,
            ts: "2026-02-26T00:00:00Z".to_string(),
            resource: StreamResource {
                kind: "task".to_string(),
                id: "task-1".to_string(),
            },
            replay: StreamReplay {
                mode: "live".to_string(),
                requested_cursor: None,
                batch: None,
            },
            payload,
        }
    }

    #[test]
    fn log_line_creates_iteration_and_appends() {
        let state = make_state();
        let event = make_event(
            "task.log.line",
            json!({ "line": "Hello world", "iteration": 1 }),
        );

        apply_stream_event(&event, &state);

        let s = state.lock().unwrap();
        assert_eq!(s.total_iterations(), 1);
        let lines = s.iterations[0].lines.lock().unwrap();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn task_status_running_sets_active() {
        let state = make_state();
        let event = make_event(
            "task.status.changed",
            json!({ "from": "open", "to": "running" }),
        );

        apply_stream_event(&event, &state);

        let s = state.lock().unwrap();
        assert!(s.get_active_task().is_some());
    }

    #[test]
    fn task_status_closed_increments_count() {
        let state = make_state();
        {
            let mut s = state.lock().unwrap();
            s.set_task_counts(TaskCounts::new(5, 3, 2, 3));
        }

        let event = make_event(
            "task.status.changed",
            json!({ "from": "running", "to": "closed" }),
        );
        apply_stream_event(&event, &state);

        let s = state.lock().unwrap();
        assert_eq!(s.task_counts.closed, 3);
        assert_eq!(s.task_counts.open, 2);
    }

    #[test]
    fn loop_status_completed_marks_completion() {
        let state = make_state();
        let event = make_event("loop.status.changed", json!({ "status": "completed" }));

        apply_stream_event(&event, &state);

        let s = state.lock().unwrap();
        assert!(s.loop_completed);
    }

    #[test]
    fn lifecycle_started_sets_timer() {
        let state = make_state();
        // Clear the default timer to test
        {
            let mut s = state.lock().unwrap();
            s.loop_started = None;
        }

        let event = make_event("system.lifecycle", json!({ "phase": "started" }));
        apply_stream_event(&event, &state);

        let s = state.lock().unwrap();
        assert!(s.loop_started.is_some());
    }

    #[test]
    fn error_event_appends_line() {
        let state = make_state();
        // Create an iteration first
        {
            let mut s = state.lock().unwrap();
            s.start_new_iteration();
        }

        let event = make_event(
            "error.raised",
            json!({ "code": "TIMEOUT", "message": "request timed out" }),
        );
        apply_stream_event(&event, &state);

        let s = state.lock().unwrap();
        let lines = s.iterations[0].lines.lock().unwrap();
        assert_eq!(lines.len(), 1);
    }
}
