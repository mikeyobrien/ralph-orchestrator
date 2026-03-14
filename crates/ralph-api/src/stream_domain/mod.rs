mod filters;
mod rpc_side_effects;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::errors::ApiError;
use crate::loop_support::now_ts;
use crate::protocol::{API_VERSION, STREAM_NAME, STREAM_TOPICS};

use self::filters::{
    SubscriptionFilters, cursor_is_older, cursor_sequence, normalize_topics, validate_cursor,
};

pub const KEEPALIVE_INTERVAL_MS: u64 = 15_000;

const DEFAULT_REPLAY_LIMIT: usize = 200;
const HISTORY_LIMIT: usize = 2_048;
const LIVE_BUFFER_CAPACITY: usize = 256;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeParams {
    pub topics: Vec<String>,
    pub cursor: Option<String>,
    pub replay_limit: Option<u16>,
    pub filters: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamUnsubscribeParams {
    pub subscription_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamAckParams {
    pub subscription_id: String,
    pub cursor: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeResult {
    pub subscription_id: String,
    pub accepted_topics: Vec<String>,
    pub cursor: String,
}

#[derive(Debug, Clone)]
pub struct ReplayBatch {
    pub events: Vec<StreamEventEnvelope>,
    pub dropped_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEventEnvelope {
    pub api_version: String,
    pub stream: String,
    pub topic: String,
    pub cursor: String,
    pub sequence: u64,
    pub ts: String,
    pub resource: StreamResource,
    pub replay: StreamReplay,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamResource {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamReplay {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<u64>,
}

#[derive(Clone)]
pub struct StreamDomain {
    state: Arc<Mutex<StreamState>>,
    live_tx: broadcast::Sender<StreamEventEnvelope>,
}

#[derive(Debug, Clone)]
struct SubscriptionRecord {
    topics: HashSet<String>,
    filters: SubscriptionFilters,
    cursor: String,
    replay_limit: usize,
    explicit_cursor: bool,
    principal: String,
}

struct StreamState {
    sequence: u64,
    subscription_counter: u64,
    history: VecDeque<StreamEventEnvelope>,
    subscriptions: HashMap<String, SubscriptionRecord>,
}

impl StreamDomain {
    pub fn new() -> Self {
        let (live_tx, _) = broadcast::channel(LIVE_BUFFER_CAPACITY);
        Self {
            state: Arc::new(Mutex::new(StreamState {
                sequence: 1,
                subscription_counter: 0,
                history: VecDeque::with_capacity(HISTORY_LIMIT),
                subscriptions: HashMap::new(),
            })),
            live_tx,
        }
    }

    pub fn subscribe(
        &self,
        params: StreamSubscribeParams,
        principal: &str,
    ) -> Result<StreamSubscribeResult, ApiError> {
        let accepted_topics = normalize_topics(&params.topics, STREAM_TOPICS)?;
        let cursor = if let Some(cursor) = &params.cursor {
            validate_cursor(cursor)?;
            cursor.clone()
        } else {
            self.latest_cursor_or_now()?
        };

        let replay_limit = usize::from(params.replay_limit.unwrap_or(DEFAULT_REPLAY_LIMIT as u16));
        let filters = SubscriptionFilters::from_json(params.filters)?;

        let mut state = self.lock_state()?;
        state.subscription_counter = state.subscription_counter.saturating_add(1);
        let subscription_id = format!(
            "sub-{}-{:04x}",
            Utc::now().timestamp_millis(),
            state.subscription_counter
        );

        let topics = accepted_topics.iter().cloned().collect::<HashSet<_>>();
        state.subscriptions.insert(
            subscription_id.clone(),
            SubscriptionRecord {
                topics,
                filters,
                cursor: cursor.clone(),
                replay_limit,
                explicit_cursor: params.cursor.is_some(),
                principal: principal.to_string(),
            },
        );

        Ok(StreamSubscribeResult {
            subscription_id,
            accepted_topics,
            cursor,
        })
    }

    pub fn get_subscription_principal(&self, subscription_id: &str) -> Option<String> {
        let state = self.lock_state().ok()?;
        state
            .subscriptions
            .get(subscription_id)
            .map(|s| s.principal.clone())
    }

    pub fn unsubscribe(&self, params: StreamUnsubscribeParams) -> Result<(), ApiError> {
        let mut state = self.lock_state()?;
        let removed = state.subscriptions.remove(&params.subscription_id);
        if removed.is_none() {
            return Err(ApiError::not_found(format!(
                "subscription '{}' not found",
                params.subscription_id
            ))
            .with_details(json!({ "subscriptionId": params.subscription_id })));
        }

        Ok(())
    }

    pub fn ack(&self, params: StreamAckParams) -> Result<(), ApiError> {
        validate_cursor(&params.cursor)?;

        let mut state = self.lock_state()?;
        let Some(subscription) = state.subscriptions.get_mut(&params.subscription_id) else {
            return Err(ApiError::not_found(format!(
                "subscription '{}' not found",
                params.subscription_id
            ))
            .with_details(json!({ "subscriptionId": params.subscription_id })));
        };

        if cursor_is_older(&params.cursor, &subscription.cursor)? {
            return Err(ApiError::precondition_failed(
                "stream.ack cursor is older than the subscription checkpoint",
            )
            .with_details(json!({
                "subscriptionId": params.subscription_id,
                "cursor": params.cursor,
                "currentCursor": subscription.cursor
            })));
        }

        subscription.cursor = params.cursor;
        subscription.explicit_cursor = true;
        Ok(())
    }

    pub fn live_receiver(&self) -> broadcast::Receiver<StreamEventEnvelope> {
        self.live_tx.subscribe()
    }

    pub fn has_subscription(&self, subscription_id: &str) -> bool {
        self.state
            .lock()
            .ok()
            .is_some_and(|state| state.subscriptions.contains_key(subscription_id))
    }

    pub fn matches_subscription(&self, subscription_id: &str, event: &StreamEventEnvelope) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };

        let Some(subscription) = state.subscriptions.get(subscription_id) else {
            return false;
        };

        subscription.matches(event)
    }

    pub fn subscription_cursor_sequence(&self, subscription_id: &str) -> Result<u64, ApiError> {
        let state = self.lock_state()?;
        let Some(subscription) = state.subscriptions.get(subscription_id) else {
            return Err(ApiError::not_found(format!(
                "subscription '{}' not found",
                subscription_id
            ))
            .with_details(json!({ "subscriptionId": subscription_id })));
        };

        cursor_sequence(&subscription.cursor)
    }

    pub fn subscription_cursor(&self, subscription_id: &str) -> Result<String, ApiError> {
        let state = self.lock_state()?;
        let Some(subscription) = state.subscriptions.get(subscription_id) else {
            return Err(ApiError::not_found(format!(
                "subscription '{}' not found",
                subscription_id
            ))
            .with_details(json!({ "subscriptionId": subscription_id })));
        };

        Ok(subscription.cursor.clone())
    }

    pub fn replay_for_subscription(&self, subscription_id: &str) -> Result<ReplayBatch, ApiError> {
        let state = self.lock_state()?;
        let Some(subscription) = state.subscriptions.get(subscription_id) else {
            return Err(ApiError::not_found(format!(
                "subscription '{}' not found",
                subscription_id
            ))
            .with_details(json!({ "subscriptionId": subscription_id })));
        };

        let cursor_sequence = cursor_sequence(&subscription.cursor)?;
        let current_cursor = subscription.cursor.clone();
        let mut events = state
            .history
            .iter()
            .filter(|event| {
                event.sequence > cursor_sequence
                    || (event.sequence == cursor_sequence && event.cursor != current_cursor)
            })
            .filter(|event| subscription.matches(event))
            .cloned()
            .collect::<Vec<_>>();

        let dropped_count = events.len().saturating_sub(subscription.replay_limit);
        if dropped_count > 0 {
            events = events.split_off(dropped_count);
        }

        if !events.is_empty() {
            let replay_mode = if subscription.explicit_cursor {
                "resume"
            } else {
                "replay"
            };

            let batch = u64::try_from(events.len()).unwrap_or(u64::MAX);
            for event in &mut events {
                event.replay.mode = replay_mode.to_string();
                event.replay.requested_cursor = Some(subscription.cursor.clone());
                event.replay.batch = Some(batch);
            }
        }

        Ok(ReplayBatch {
            events,
            dropped_count,
        })
    }

    pub fn keepalive_event(&self, subscription_id: &str, interval_ms: u64) -> StreamEventEnvelope {
        self.ephemeral_event(
            "stream.keepalive",
            "stream",
            subscription_id,
            json!({ "intervalMs": interval_ms }),
            "live",
            None,
            None,
        )
    }

    pub fn backpressure_event(
        &self,
        subscription_id: &str,
        dropped_count: usize,
    ) -> StreamEventEnvelope {
        self.ephemeral_event(
            "error.raised",
            "stream",
            subscription_id,
            json!({
                "code": "BACKPRESSURE_DROPPED",
                "message": format!(
                    "subscription '{}' dropped {} event(s) due to backpressure",
                    subscription_id,
                    dropped_count
                ),
                "retryable": true
            }),
            "live",
            None,
            None,
        )
    }

    pub fn publish(&self, topic: &str, resource_type: &str, resource_id: &str, payload: Value) {
        if !STREAM_TOPICS.contains(&topic) {
            return;
        }

        let Ok(mut state) = self.lock_state() else {
            return;
        };

        let event = next_event(
            &mut state,
            topic,
            resource_type,
            resource_id,
            payload,
            "live",
            None,
            None,
        );

        if state.history.len() >= HISTORY_LIMIT {
            state.history.pop_front();
        }
        state.history.push_back(event.clone());
        let _ = self.live_tx.send(event);
    }

    pub fn publish_rpc_side_effect(&self, method: &str, params: &Value, result: &Value) {
        rpc_side_effects::publish_rpc_side_effect(self, method, params, result);
    }

    fn latest_cursor_or_now(&self) -> Result<String, ApiError> {
        let state = self.lock_state()?;
        Ok(state
            .history
            .back()
            .map(|event| event.cursor.clone())
            .unwrap_or_else(|| format!("{}-0", Utc::now().timestamp_millis())))
    }

    fn ephemeral_event(
        &self,
        topic: &str,
        resource_type: &str,
        resource_id: &str,
        payload: Value,
        mode: &str,
        requested_cursor: Option<String>,
        batch: Option<u64>,
    ) -> StreamEventEnvelope {
        let Ok(mut state) = self.lock_state() else {
            return StreamEventEnvelope {
                api_version: API_VERSION.to_string(),
                stream: STREAM_NAME.to_string(),
                topic: topic.to_string(),
                cursor: format!("{}-0", Utc::now().timestamp_millis()),
                sequence: 0,
                ts: now_ts(),
                resource: StreamResource {
                    kind: resource_type.to_string(),
                    id: resource_id.to_string(),
                },
                replay: StreamReplay {
                    mode: mode.to_string(),
                    requested_cursor,
                    batch,
                },
                payload,
            };
        };

        next_event(
            &mut state,
            topic,
            resource_type,
            resource_id,
            payload,
            mode,
            requested_cursor,
            batch,
        )
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, StreamState>, ApiError> {
        self.state
            .lock()
            .map_err(|_| ApiError::internal("stream state lock poisoned"))
    }
}

impl Default for StreamDomain {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionRecord {
    fn matches(&self, event: &StreamEventEnvelope) -> bool {
        self.topics.contains(&event.topic) && self.filters.matches(event)
    }
}

fn next_event(
    state: &mut StreamState,
    topic: &str,
    resource_type: &str,
    resource_id: &str,
    payload: Value,
    mode: &str,
    requested_cursor: Option<String>,
    batch: Option<u64>,
) -> StreamEventEnvelope {
    let sequence = state.sequence;
    state.sequence = state.sequence.saturating_add(1);

    StreamEventEnvelope {
        api_version: API_VERSION.to_string(),
        stream: STREAM_NAME.to_string(),
        topic: topic.to_string(),
        cursor: format!("{}-{sequence}", Utc::now().timestamp_millis()),
        sequence,
        ts: now_ts(),
        resource: StreamResource {
            kind: resource_type.to_string(),
            id: resource_id.to_string(),
        },
        replay: StreamReplay {
            mode: mode.to_string(),
            requested_cursor,
            batch,
        },
        payload,
    }
}
