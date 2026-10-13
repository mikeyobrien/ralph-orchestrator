//! Mock of the icedrop daemon's chat wire.
//!
//! This is not the daemon. It does not open a sqlite, wear a hat, or resume a
//! run. It speaks the two routes a v3 client uses:
//!
//! - `POST /commands` with `{ "id", "command" }` for `submit` and `abort`
//! - `GET /events` as `text/event-stream`, each `data` line a `ServerMessage`
//!
//! A `snapshot` is sent first. `requestId` values are idempotent. Prefixes
//! `wake:` and `depart:` are rejected, matching the v3 route gate. Steer while
//! a run is in progress is queued and taken at the next streamed chunk.

use std::collections::HashSet;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream::Stream;
use ralph_chat_wasm::{
    Command, CommandResult, EntrySeed, Failure, Incoming, MOCK_CONVERSATION_ID, MOCK_MODEL,
    QueueMode, Queued, ServerMessage, SteerMode, event_batch, inbox_update_event,
    message_end_event, message_start_event, parse_incoming, run_end_event, run_start_event,
    snapshot_event, text_delta_event,
};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

/// How the mock behaves for one process.
#[derive(Clone, Debug)]
pub struct MockConfig {
    pub chunk_delay: Duration,
    /// Trunk output (`index.html`, the wasm, the css). Served at `/` when it exists.
    pub dist: Option<PathBuf>,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            chunk_delay: Duration::from_millis(50),
            dist: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    world: Arc<Mutex<World>>,
    chunk_delay: Duration,
    dist: Option<PathBuf>,
}

struct World {
    conversation_id: String,
    entries: Vec<EntrySeed>,
    inbox: Vec<Queued>,
    running: bool,
    run_inputs: Vec<String>,
    cancel: Option<CancellationToken>,
    tx: broadcast::Sender<ServerMessage>,
    seen: HashSet<String>,
    next_entry: u64,
}

enum Plan {
    Start(CancellationToken),
    Queued,
    Duplicate,
    Busy,
    Missing,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Done,
    Aborted,
}

impl AppState {
    pub fn new(config: MockConfig) -> Self {
        let (tx, _) = broadcast::channel(64);
        let world = World {
            conversation_id: MOCK_CONVERSATION_ID.into(),
            entries: Vec::new(),
            inbox: Vec::new(),
            running: false,
            run_inputs: Vec::new(),
            cancel: None,
            tx,
            seen: HashSet::new(),
            next_entry: 0,
        };
        Self {
            world: Arc::new(Mutex::new(world)),
            chunk_delay: config.chunk_delay,
            dist: config.dist,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, World> {
        self.world.lock().expect("mock lock")
    }

    /// Apply one command and return the JSON body `POST /commands` sends.
    pub fn handle(&self, incoming: Incoming) -> CommandResult {
        let command = match incoming.command {
            Ok(command) => command,
            Err(message) => {
                return CommandResult::failure(&incoming.id, Failure::invalid(message));
            }
        };
        match command {
            Command::Abort { conversation_id } => self.abort(&incoming.id, conversation_id),
            Command::Submit {
                conversation_id,
                text,
                request_id,
                when_busy,
            } => self.submit(&incoming.id, conversation_id, text, request_id, when_busy),
        }
    }

    fn submit(
        &self,
        envelope_id: &str,
        conversation_id: Option<String>,
        text: String,
        request_id: Option<String>,
        when_busy: Option<SteerMode>,
    ) -> CommandResult {
        if request_id
            .as_ref()
            .is_some_and(|id| reserved_request_id(id))
        {
            return CommandResult::failure(
                envelope_id,
                Failure::invalid("requestId: prefixes wake: and depart: are minted by the daemon"),
            );
        }
        let queue_id = request_id
            .clone()
            .unwrap_or_else(|| envelope_id.to_string());
        let plan = {
            let mut world = self.lock();
            if !same_conversation(&world, conversation_id.as_deref()) {
                Plan::Missing
            } else if request_id.is_some() && !world.seen.insert(queue_id.clone()) {
                Plan::Duplicate
            } else {
                admit(&mut world, &text, &queue_id, when_busy)
            }
        };
        match plan {
            Plan::Start(token) => {
                self.spawn_turn(text, token);
                CommandResult::ack(envelope_id)
            }
            Plan::Queued | Plan::Duplicate => CommandResult::ack(envelope_id),
            Plan::Busy => CommandResult::failure(
                envelope_id,
                Failure::busy("conversation is busy and whenBusy is reject"),
            ),
            Plan::Missing => {
                CommandResult::failure(envelope_id, Failure::not_found("conversation not found"))
            }
        }
    }

    fn abort(&self, envelope_id: &str, conversation_id: Option<String>) -> CommandResult {
        let world = self.lock();
        if !same_conversation(&world, conversation_id.as_deref()) {
            return CommandResult::failure(
                envelope_id,
                Failure::not_found("conversation not found"),
            );
        }
        if let Some(token) = &world.cancel {
            token.cancel();
        }
        CommandResult::ack(envelope_id)
    }

    fn spawn_turn(&self, text: String, token: CancellationToken) {
        let state = self.clone();
        tokio::spawn(async move {
            run_turn(state, text, token).await;
        });
    }

    fn take_steer(&self) -> Option<String> {
        let mut world = self.lock();
        let index = world
            .inbox
            .iter()
            .position(|item| item.mode == QueueMode::Steer)?;
        let item = world.inbox.remove(index);
        world.emit(inbox_update_event(&world.inbox));
        item.text.filter(|text| !text.is_empty())
    }

    fn finish(&self, outcome: Outcome, spoken: &str) -> Option<String> {
        let mut world = self.lock();
        if !spoken.is_empty() {
            let entry = world.push_entry("assistant", spoken);
            world.emit(message_end_event(&entry));
        }
        world.running = false;
        world.cancel = None;
        let inputs = world.run_inputs.clone();
        let status = match outcome {
            Outcome::Done => "done",
            Outcome::Aborted => "aborted",
        };
        world.emit(run_end_event(&inputs, status));
        if outcome == Outcome::Done {
            world.take_follow_up()
        } else {
            None
        }
    }
}

fn admit(world: &mut World, text: &str, queue_id: &str, when_busy: Option<SteerMode>) -> Plan {
    if !world.running {
        return world.arm(text);
    }
    if when_busy == Some(SteerMode::Reject) {
        world.seen.remove(queue_id);
        return Plan::Busy;
    }
    let mode = if when_busy == Some(SteerMode::Steer) {
        QueueMode::Steer
    } else {
        QueueMode::FollowUp
    };
    world.inbox.push(Queued {
        id: queue_id.to_string(),
        mode,
        text: Some(text.to_string()),
    });
    world.emit(inbox_update_event(&world.inbox));
    Plan::Queued
}

impl World {
    fn arm(&mut self, text: &str) -> Plan {
        let token = CancellationToken::new();
        self.running = true;
        self.run_inputs = vec![text.to_string()];
        self.cancel = Some(token.clone());
        Plan::Start(token)
    }

    fn emit(&self, event: Value) {
        let message = event_batch(&self.conversation_id, vec![event]);
        let _ = self.tx.send(message);
    }

    fn push_entry(&mut self, kind: &str, preview: &str) -> EntrySeed {
        self.next_entry += 1;
        let entry = EntrySeed {
            id: format!("e{}", self.next_entry),
            kind: kind.into(),
            preview: preview.into(),
            at: Some(now_ms()),
        };
        self.entries.push(entry.clone());
        entry
    }

    fn snapshot_message(&self) -> ServerMessage {
        let run = self.running.then(|| self.run_inputs.clone());
        let event = snapshot_event(&self.entries, run.as_deref(), &self.inbox, MOCK_MODEL);
        event_batch(&self.conversation_id, vec![event])
    }

    fn take_follow_up(&mut self) -> Option<String> {
        let index = self
            .inbox
            .iter()
            .position(|item| item.mode == QueueMode::FollowUp)?;
        let item = self.inbox.remove(index);
        self.emit(inbox_update_event(&self.inbox));
        item.text.filter(|text| !text.is_empty())
    }
}

fn reserved_request_id(id: &str) -> bool {
    id.starts_with("wake:") || id.starts_with("depart:")
}

fn same_conversation(world: &World, id: Option<&str>) -> bool {
    match id {
        None => true,
        Some(id) => id == world.conversation_id,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn reply_chunks(text: &str) -> Vec<String> {
    let mut chunks = vec!["mock ".into(), "heard ".into(), format!("\"{text}\". ")];
    for word in [
        "The ", "stream ", "is ", "open. ", "Cancel ", "stops ", "this ", "run. ", "This ", "is ",
        "not ", "the ", "daemon.",
    ] {
        chunks.push(word.into());
    }
    chunks
}

async fn run_turn(state: AppState, text: String, token: CancellationToken) {
    if token.is_cancelled() {
        let _ = state.finish(Outcome::Aborted, "");
        return;
    }
    {
        let mut world = state.lock();
        let inputs = world.run_inputs.clone();
        let entry = world.push_entry("user", &text);
        world.emit(run_start_event(&inputs));
        world.emit(message_end_event(&entry));
        world.emit(message_start_event("assistant"));
    }
    let mut spoken = String::new();
    for chunk in reply_chunks(&text) {
        if token.is_cancelled() {
            let _ = state.finish(Outcome::Aborted, &spoken);
            return;
        }
        if let Some(steer) = state.take_steer() {
            let note = format!("[steer] {steer} ");
            spoken.push_str(&note);
            state.lock().emit(text_delta_event(&note));
        }
        spoken.push_str(&chunk);
        state.lock().emit(text_delta_event(&chunk));
        tokio::time::sleep(state.chunk_delay).await;
    }
    if token.is_cancelled() {
        let _ = state.finish(Outcome::Aborted, &spoken);
        return;
    }
    if let Some(next) = state.finish(Outcome::Done, &spoken) {
        let token = match state.lock().arm(&next) {
            Plan::Start(token) => token,
            Plan::Queued | Plan::Duplicate | Plan::Busy | Plan::Missing => return,
        };
        state.spawn_turn(next, token);
    }
}

/// Router for `POST /commands`, `GET /events`, and (when `dist` exists) the page.
pub fn router(state: AppState) -> Router {
    let dist = state.dist.clone().filter(|path| path.is_dir());
    let router = Router::new()
        .route("/commands", post(commands))
        .route("/events", get(events))
        .layer(CorsLayer::permissive())
        .with_state(state);
    if let Some(dist) = dist {
        router.fallback_service(ServeDir::new(dist))
    } else {
        router.fallback(explain)
    }
}

pub async fn serve(listener: TcpListener, state: AppState) -> std::io::Result<()> {
    axum::serve(listener, router(state)).await
}

async fn commands(State(state): State<AppState>, body: String) -> Response {
    match parse_incoming(&body) {
        Ok(incoming) => (StatusCode::OK, Json(state.handle(incoming))).into_response(),
        Err(failure) => (
            StatusCode::BAD_REQUEST,
            Json(CommandResult::failure("", failure)),
        )
            .into_response(),
    }
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (snapshot, rx) = {
        let world = state.lock();
        let rx = world.tx.subscribe();
        (world.snapshot_message(), rx)
    };
    let stream = futures::stream::unfold(Phase::Snapshot(snapshot, rx), move |phase| {
        let state = state.clone();
        async move {
            match phase {
                Phase::Snapshot(message, rx) => Some((Ok(sse_event(message)), Phase::Live(rx))),
                Phase::Live(mut rx) => match rx.recv().await {
                    Ok(message) => Some((Ok(sse_event(message)), Phase::Live(rx))),
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let message = state.lock().snapshot_message();
                        Some((Ok(sse_event(message)), Phase::Live(rx)))
                    }
                    Err(broadcast::error::RecvError::Closed) => None,
                },
            }
        }
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

enum Phase {
    Snapshot(ServerMessage, broadcast::Receiver<ServerMessage>),
    Live(broadcast::Receiver<ServerMessage>),
}

fn sse_event(message: ServerMessage) -> Event {
    Event::default()
        .json_data(message)
        .expect("server message is json")
}

async fn explain() -> &'static str {
    "ralph-chat-mock\n\
     POST /commands\n\
     GET /events\n\
     This process is a mock, not the icedrop daemon.\n\
     Pass --dist to also serve the trunk build.\n"
}
