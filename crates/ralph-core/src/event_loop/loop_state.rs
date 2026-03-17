//! Loop state tracking for the event loop.
//!
//! This module contains the `LoopState` struct that tracks the current
//! state of the orchestration loop including iteration count, failures,
//! timing, and hat activation tracking.

use ralph_proto::{Event, HatId};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, Instant};

/// Fingerprint of the last emitted event for stale loop detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSignature {
    pub topic: String,
    pub source: Option<HatId>,
    pub payload_fingerprint: u64,
}

/// Current state of the event loop.
#[derive(Debug)]
pub struct LoopState {
    /// Current iteration number (1-indexed).
    pub iteration: u32,
    /// Number of consecutive failures.
    pub consecutive_failures: u32,
    /// Cumulative cost in USD (if tracked).
    pub cumulative_cost: f64,
    /// When the loop started.
    pub started_at: Instant,
    /// The last hat that executed.
    pub last_hat: Option<HatId>,
    /// Consecutive blocked events from the same hat.
    pub consecutive_blocked: u32,
    /// Hat that emitted the last blocked event.
    pub last_blocked_hat: Option<HatId>,
    /// Per-task block counts for task-level thrashing detection.
    pub task_block_counts: HashMap<String, u32>,
    /// Tasks that have been abandoned after 3+ blocks.
    pub abandoned_tasks: Vec<String>,
    /// Count of times planner dispatched an already-abandoned task.
    pub abandoned_task_redispatches: u32,
    /// Consecutive malformed JSONL lines encountered (for validation backpressure).
    pub consecutive_malformed_events: u32,
    /// Whether a completion event has been observed in JSONL.
    pub completion_requested: bool,

    /// Per-hat activation counts (used for max_activations).
    pub hat_activation_counts: HashMap<HatId, u32>,

    /// Hats for which `<hat_id>.exhausted` has been emitted.
    pub exhausted_hats: HashSet<HatId>,

    /// When the last Telegram check-in message was sent.
    /// `None` means no check-in has been sent yet.
    pub last_checkin_at: Option<Instant>,

    /// Hat IDs that were active in the last iteration.
    /// Used to inject `default_publishes` when agent writes no events.
    pub last_active_hat_ids: Vec<HatId>,

    /// Topics seen during the loop's lifetime (for event chain validation).
    pub seen_topics: HashSet<String>,

    /// The last event signature emitted (for stale loop detection).
    pub last_emitted_signature: Option<EventSignature>,

    /// Consecutive times the same event signature was emitted (for stale loop detection).
    pub consecutive_same_signature: u32,

    /// Set to true when a loop.cancel event is detected.
    pub cancellation_requested: bool,
}

impl Default for LoopState {
    fn default() -> Self {
        Self {
            iteration: 0,
            consecutive_failures: 0,
            cumulative_cost: 0.0,
            started_at: Instant::now(),
            last_hat: None,
            consecutive_blocked: 0,
            last_blocked_hat: None,
            task_block_counts: HashMap::new(),
            abandoned_tasks: Vec::new(),
            abandoned_task_redispatches: 0,
            consecutive_malformed_events: 0,
            completion_requested: false,
            hat_activation_counts: HashMap::new(),
            exhausted_hats: HashSet::new(),
            last_checkin_at: None,
            last_active_hat_ids: Vec::new(),
            seen_topics: HashSet::new(),
            last_emitted_signature: None,
            consecutive_same_signature: 0,
            cancellation_requested: false,
        }
    }
}

impl LoopState {
    /// Creates a new loop state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the elapsed time since the loop started.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Record that an event has been seen during this loop run.
    ///
    /// Tracks consecutive identical-signature emissions for stale loop detection.
    /// The signature includes topic, source, and a hash of the payload, so events
    /// with different payloads (different task IDs, wave numbers, etc.) are
    /// naturally distinguished without needing a topic exemption list.
    pub fn record_event(&mut self, event: &Event) {
        self.seen_topics.insert(event.topic.to_string());

        let signature = EventSignature::from_event(event);
        if self.last_emitted_signature.as_ref() == Some(&signature) {
            self.consecutive_same_signature += 1;
        } else {
            self.consecutive_same_signature = 1;
            self.last_emitted_signature = Some(signature);
        }
    }

    /// Check if all required topics have been seen.
    pub fn missing_required_events<'a>(&self, required: &'a [String]) -> Vec<&'a String> {
        required
            .iter()
            .filter(|topic| !self.seen_topics.contains(topic.as_str()))
            .collect()
    }
}

impl EventSignature {
    pub fn from_event(event: &Event) -> Self {
        Self {
            topic: event.topic.to_string(),
            source: event.source.clone(),
            payload_fingerprint: fingerprint_payload(&event.payload),
        }
    }
}

fn fingerprint_payload(payload: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    payload.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::LoopState;
    use ralph_proto::Event;

    #[test]
    fn same_topic_different_payloads_does_not_accumulate() {
        // Events with the same topic but different payloads have different
        // fingerprints, so the counter resets each time.
        let mut state = LoopState::new();

        state.record_event(&Event::new("task.complete", "task 1 complete"));
        assert_eq!(state.consecutive_same_signature, 1);

        state.record_event(&Event::new("task.complete", "task 2 complete"));
        assert_eq!(state.consecutive_same_signature, 1);

        state.record_event(&Event::new("task.complete", "task 3 complete"));
        assert_eq!(state.consecutive_same_signature, 1);
    }

    #[test]
    fn same_topic_same_payload_accumulates() {
        // Identical events (same topic + same payload) accumulate, indicating
        // the loop is stuck repeating the same action.
        let mut state = LoopState::new();

        state.record_event(&Event::new("task.resume", "same payload"));
        state.record_event(&Event::new("task.resume", "same payload"));
        state.record_event(&Event::new("task.resume", "same payload"));

        assert_eq!(state.consecutive_same_signature, 3);
        assert_eq!(
            state
                .last_emitted_signature
                .as_ref()
                .map(|s| s.topic.as_str()),
            Some("task.resume")
        );
    }

    #[test]
    fn different_topics_reset_counter() {
        let mut state = LoopState::new();

        state.record_event(&Event::new("tasks.ready", "wave 1"));
        state.record_event(&Event::new("review.ready", r#"{"task_id":"t1"}"#));
        state.record_event(&Event::new("task.next", "continue"));
        state.record_event(&Event::new("review.ready", r#"{"task_id":"t2"}"#));
        state.record_event(&Event::new("wave.complete", "wave 1 done"));

        assert_eq!(state.consecutive_same_signature, 1);
    }

    #[test]
    fn wave_flow_with_distinct_payloads_never_accumulates() {
        // Simulates a multi-wave flow where every event carries a distinct payload.
        let mut state = LoopState::new();

        // Wave 1
        state.record_event(&Event::new("tasks.ready", r#"{"wave":1}"#));
        state.record_event(&Event::new("review.ready", r#"{"task_id":"1a","wave_done":false}"#));
        state.record_event(&Event::new("task.next", "continue 1a"));
        state.record_event(&Event::new("review.ready", r#"{"task_id":"1b","wave_done":true}"#));
        state.record_event(&Event::new("wave.complete", "wave 1"));

        // Wave 2
        state.record_event(&Event::new("tasks.ready", r#"{"wave":2}"#));
        state.record_event(&Event::new("review.ready", r#"{"task_id":"2a","wave_done":true}"#));
        state.record_event(&Event::new("wave.complete", "wave 2"));

        // No event repeated with the same payload, so counter stays at 1
        assert_eq!(state.consecutive_same_signature, 1);
    }
}
