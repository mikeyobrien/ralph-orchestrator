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

    /// Session-scoped peak of `input + cache_read + cache_write` tokens across all iterations.
    pub peak_input_tokens: u64,

    /// Last iteration's `input + cache_read + cache_write` tokens (if any).
    pub last_input_tokens: Option<u64>,

    /// Per-hat session-scoped peak of `input + cache_read + cache_write` tokens.
    pub hat_peak_input_tokens: HashMap<HatId, u64>,
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
            peak_input_tokens: 0,
            last_input_tokens: None,
            hat_peak_input_tokens: HashMap::new(),
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

    fn event_counts_toward_stale_loop(event: &Event) -> bool {
        !matches!(event.topic.as_str(), "task.complete")
    }

    /// Record that an event has been seen during this loop run.
    ///
    /// Also tracks consecutive same-signature emissions for stale loop detection.
    pub fn record_event(&mut self, event: &Event) {
        self.seen_topics.insert(event.topic.to_string());

        if !Self::event_counts_toward_stale_loop(event) {
            self.consecutive_same_signature = 0;
            self.last_emitted_signature = Some(EventSignature::from_event(event));
            return;
        }

        let signature = EventSignature::from_event(event);
        if self.last_emitted_signature.as_ref() == Some(&signature) {
            self.consecutive_same_signature += 1;
        } else {
            self.consecutive_same_signature = 1;
            self.last_emitted_signature = Some(signature);
        }
    }

    /// Record this iteration's context-token usage for the hat that ran it.
    ///
    /// `tokens` is the iteration's `input + cache_read + cache_write` sum.
    /// No-op when `tokens == 0` (ACP / non-token backends suppressed).
    /// Peaks are session-scoped — they never reset on iteration boundaries.
    pub fn record_iteration_tokens(&mut self, hat: &HatId, tokens: u64) {
        if tokens == 0 {
            return;
        }
        let entry = self.hat_peak_input_tokens.entry(hat.clone()).or_insert(0);
        *entry = (*entry).max(tokens);
        self.peak_input_tokens = self.peak_input_tokens.max(tokens);
        self.last_input_tokens = Some(tokens);
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
    use ralph_proto::{Event, HatId};

    #[test]
    fn repeated_task_complete_does_not_accumulate_stale_loop_count() {
        let mut state = LoopState::new();

        state.record_event(&Event::new("task.complete", "task 1 complete"));
        assert_eq!(state.consecutive_same_signature, 0);

        state.record_event(&Event::new("task.complete", "task 2 complete"));
        state.record_event(&Event::new("task.complete", "task 3 complete"));

        assert_eq!(state.consecutive_same_signature, 0);
        assert_eq!(
            state
                .last_emitted_signature
                .as_ref()
                .map(|s| s.topic.as_str()),
            Some("task.complete")
        );
    }

    #[test]
    fn record_iteration_tokens_tracks_per_hat_and_global_peak() {
        let mut state = LoopState::new();
        let builder = HatId::new("builder");
        let critic = HatId::new("critic");

        state.record_iteration_tokens(&builder, 10_000);
        assert_eq!(
            state.hat_peak_input_tokens.get(&builder).copied(),
            Some(10_000)
        );
        assert_eq!(state.peak_input_tokens, 10_000);
        assert_eq!(state.last_input_tokens, Some(10_000));

        state.record_iteration_tokens(&builder, 5_000);
        assert_eq!(
            state.hat_peak_input_tokens.get(&builder).copied(),
            Some(10_000),
            "per-hat peak must retain the max across iterations"
        );
        assert_eq!(
            state.peak_input_tokens, 10_000,
            "global peak must retain the overall max"
        );
        assert_eq!(
            state.last_input_tokens,
            Some(5_000),
            "last_input_tokens tracks the most recent iteration, not the peak"
        );

        state.record_iteration_tokens(&critic, 25_000);
        assert_eq!(
            state.hat_peak_input_tokens.get(&builder).copied(),
            Some(10_000)
        );
        assert_eq!(
            state.hat_peak_input_tokens.get(&critic).copied(),
            Some(25_000)
        );
        assert_eq!(state.peak_input_tokens, 25_000);
        assert_eq!(state.last_input_tokens, Some(25_000));

        state.record_iteration_tokens(&builder, 0);
        assert_eq!(
            state.hat_peak_input_tokens.get(&builder).copied(),
            Some(10_000),
            "zero-token iteration must be a no-op"
        );
        assert_eq!(state.peak_input_tokens, 25_000);
        assert_eq!(
            state.last_input_tokens,
            Some(25_000),
            "zero tokens must not overwrite last_input_tokens"
        );
    }

    #[test]
    fn repeated_non_progress_topics_still_accumulate_stale_loop_count() {
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
}
