//! Wave event detection from JSONL events.
//!
//! Groups events by wave_id, validates consistency, and resolves
//! the target hat for wave execution.

use crate::config::HatConfig;
use crate::event_reader::Event;
use crate::hat_registry::HatRegistry;
use ralph_proto::HatId;
use std::collections::HashMap;
use std::time::Duration;

/// A detected wave ready for execution.
#[derive(Debug)]
pub struct DetectedWave {
    /// Wave correlation ID.
    pub wave_id: String,
    /// Hat that should process these events.
    pub target_hat: HatId,
    /// Configuration for the target hat.
    pub hat_config: HatConfig,
    /// Individual events in this wave, ordered by wave_index.
    pub events: Vec<Event>,
    /// Total expected events in the wave.
    pub total: u32,
}

/// Detect wave events from a set of events.
///
/// Groups events by wave_id, validates that all events in a wave are consistent
/// (same topic, matching wave_total), and resolves the target hat from the registry.
///
/// v1: Returns the first detected wave (one wave per iteration).
/// Events without wave metadata are ignored.
pub fn detect_wave_events(events: &[Event], registry: &HatRegistry) -> Option<DetectedWave> {
    // Group events by wave_id
    let mut wave_groups: HashMap<&str, Vec<&Event>> = HashMap::new();
    for event in events {
        if let Some(ref wave_id) = event.wave_id {
            wave_groups.entry(wave_id.as_str()).or_default().push(event);
        }
    }

    if wave_groups.is_empty() {
        return None;
    }

    // v1: Take the lexicographically first wave_id (deterministic, one wave per iteration)
    let wave_id = *wave_groups.keys().min()?;
    if wave_groups.len() > 1 {
        tracing::warn!(
            selected = wave_id,
            total_waves = wave_groups.len(),
            "Multiple waves detected in single iteration, processing only the first"
        );
    }
    let wave_events = wave_groups.remove(wave_id)?;

    // Validate: all events must have the same topic and wave_total
    let first = wave_events.first()?;
    let topic = &first.topic;
    let wave_total = first.wave_total?;

    for event in &wave_events {
        if event.topic != *topic {
            tracing::warn!(
                wave_id,
                expected_topic = %topic,
                actual_topic = %event.topic,
                "Inconsistent topic in wave events, skipping wave"
            );
            return None;
        }
        if event.wave_total != Some(wave_total) {
            tracing::warn!(
                wave_id,
                "Inconsistent wave_total in wave events, skipping wave"
            );
            return None;
        }
    }

    // Resolve target hat from the event topic
    let target_hat_id = registry.find_by_trigger(topic)?;
    let hat_config = registry.get_config(target_hat_id)?.clone();

    // Only trigger wave execution for hats with concurrency > 1
    if hat_config.concurrency <= 1 {
        return None;
    }

    // Sort events by wave_index for deterministic ordering
    let mut sorted_events: Vec<Event> = wave_events.into_iter().cloned().collect();
    sorted_events.sort_by_key(|e| e.wave_index.unwrap_or(0));

    Some(DetectedWave {
        wave_id: wave_id.to_string(),
        target_hat: target_hat_id.clone(),
        hat_config,
        events: sorted_events,
        total: wave_total,
    })
}

/// Resolve the GAP3 aggregator-wait for a wave's `join_all`.
///
/// The aggregator-wait is **orthogonal** to the per-worker deadline (R13):
/// R13 bounds each worker (`hat.timeout → override → default → 300s`, no
/// `aggregate`), while GAP3 bounds the `join_all` that collects every worker's
/// result. The aggregator is a *separate* `concurrency: 1` hat that subscribes
/// (via `triggers`) to the wave hat's first `publishes` topic and activates the
/// iteration after the wave — so the cross-hat read at the wave-execution call
/// site is intentional, not a layering defect.
///
/// Returns `Some(aggregator.aggregate.timeout)` when the wave hat publishes a
/// topic that a registered aggregator subscribes to and that aggregator has an
/// `aggregate` block. Returns `None` otherwise (no `publishes`, no subscriber,
/// or the resolved hat has `aggregate` unset), so the caller falls back to the
/// derived `per_worker_timeout × ceil(total / concurrency) + 30s`.
pub fn resolve_aggregate_wait(wave_hat: &HatConfig, registry: &HatRegistry) -> Option<Duration> {
    let publish_topic = wave_hat.publishes.first()?;
    let aggregator_id = registry.find_by_trigger(publish_topic)?;
    let aggregator = registry.get_config(aggregator_id)?;
    let aggregate = aggregator.aggregate.as_ref()?;
    Some(Duration::from_secs(u64::from(aggregate.timeout)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RalphConfig;

    fn make_wave_event(topic: &str, payload: &str, wave_id: &str, index: u32, total: u32) -> Event {
        Event {
            topic: topic.to_string(),
            payload: Some(payload.to_string()),
            ts: "2025-01-01T00:00:00Z".to_string(),
            wave_id: Some(wave_id.to_string()),
            wave_index: Some(index),
            wave_total: Some(total),
        }
    }

    fn make_registry_with_concurrent_hat() -> HatRegistry {
        let yaml = r#"
            hats:
              reviewer:
                name: "Reviewer"
                triggers: ["review.file"]
                publishes: ["review.done"]
                instructions: "Review files"
                concurrency: 4
        "#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        HatRegistry::from_config(&config)
    }

    fn make_registry_with_sequential_hat() -> HatRegistry {
        let yaml = r#"
            hats:
              builder:
                name: "Builder"
                triggers: ["build.start"]
                publishes: ["build.done"]
                instructions: "Build code"
        "#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        HatRegistry::from_config(&config)
    }

    #[test]
    fn test_detect_wave_events_basic() {
        let registry = make_registry_with_concurrent_hat();
        let events = vec![
            make_wave_event("review.file", "src/main.rs", "w-abc", 0, 3),
            make_wave_event("review.file", "src/lib.rs", "w-abc", 1, 3),
            make_wave_event("review.file", "src/config.rs", "w-abc", 2, 3),
        ];

        let wave = detect_wave_events(&events, &registry).unwrap();
        assert_eq!(wave.wave_id, "w-abc");
        assert_eq!(wave.total, 3);
        assert_eq!(wave.events.len(), 3);
        assert_eq!(wave.target_hat.as_str(), "reviewer");
        assert_eq!(wave.hat_config.concurrency, 4);
    }

    #[test]
    fn test_detect_ignores_non_wave_events() {
        let registry = make_registry_with_concurrent_hat();
        let events = vec![Event {
            topic: "review.file".to_string(),
            payload: Some("src/main.rs".to_string()),
            ts: "2025-01-01T00:00:00Z".to_string(),
            wave_id: None,
            wave_index: None,
            wave_total: None,
        }];

        assert!(detect_wave_events(&events, &registry).is_none());
    }

    #[test]
    fn test_detect_ignores_sequential_hat() {
        let registry = make_registry_with_sequential_hat();
        let events = vec![
            make_wave_event("build.start", "payload", "w-abc", 0, 2),
            make_wave_event("build.start", "payload", "w-abc", 1, 2),
        ];

        // Hat has concurrency=1 (default), so wave detection returns None
        assert!(detect_wave_events(&events, &registry).is_none());
    }

    #[test]
    fn test_detect_rejects_inconsistent_topics() {
        let registry = make_registry_with_concurrent_hat();
        let events = vec![
            make_wave_event("review.file", "src/main.rs", "w-abc", 0, 2),
            make_wave_event("review.other", "src/lib.rs", "w-abc", 1, 2),
        ];

        assert!(detect_wave_events(&events, &registry).is_none());
    }

    #[test]
    fn test_detect_sorts_by_index() {
        let registry = make_registry_with_concurrent_hat();
        // Events arrive out of order
        let events = vec![
            make_wave_event("review.file", "third", "w-abc", 2, 3),
            make_wave_event("review.file", "first", "w-abc", 0, 3),
            make_wave_event("review.file", "second", "w-abc", 1, 3),
        ];

        let wave = detect_wave_events(&events, &registry).unwrap();
        assert_eq!(wave.events[0].payload.as_deref(), Some("first"));
        assert_eq!(wave.events[1].payload.as_deref(), Some("second"));
        assert_eq!(wave.events[2].payload.as_deref(), Some("third"));
    }

    #[test]
    fn test_empty_events_returns_none() {
        let registry = make_registry_with_concurrent_hat();
        assert!(detect_wave_events(&[], &registry).is_none());
    }

    #[test]
    fn test_unknown_topic_returns_none() {
        let registry = make_registry_with_concurrent_hat();
        let events = vec![make_wave_event("unknown.topic", "payload", "w-abc", 0, 1)];

        assert!(detect_wave_events(&events, &registry).is_none());
    }

    // --- GAP3 aggregator-wait resolution (resolve_aggregate_wait) ---

    #[test]
    fn test_resolve_aggregate_wait_uses_aggregator_timeout_independent_of_per_worker() {
        // The wave hat (reviewer) has its own per-worker timeout (30s, R13), but
        // the aggregator-wait (GAP3) must come from the *separate* aggregator
        // hat's aggregate.timeout (120s), resolved via reviewer's publishes
        // topic. This proves R13 and GAP3 are orthogonal.
        let yaml = r#"
            hats:
              reviewer:
                name: "Reviewer"
                triggers: ["review.file"]
                publishes: ["review.done"]
                instructions: "Review files"
                concurrency: 4
                timeout: 30
              synthesizer:
                name: "Synthesizer"
                triggers: ["review.done"]
                publishes: ["review.complete"]
                instructions: "Synthesize"
                aggregate:
                  mode: wait_for_all
                  timeout: 120
        "#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let reviewer = registry.get_config(&HatId::new("reviewer")).unwrap();

        let wait = resolve_aggregate_wait(reviewer, &registry);

        // Aggregator's aggregate.timeout wins, NOT the wave hat's per-worker 30s.
        assert_eq!(wait, Some(Duration::from_mins(2)));
        assert_ne!(wait, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_resolve_aggregate_wait_no_subscriber_returns_none() {
        // Wave hat publishes a topic that no hat subscribes to → no aggregator.
        let yaml = r#"
            hats:
              reviewer:
                name: "Reviewer"
                triggers: ["review.file"]
                publishes: ["review.done"]
                instructions: "Review files"
                concurrency: 4
        "#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let reviewer = registry.get_config(&HatId::new("reviewer")).unwrap();

        assert_eq!(resolve_aggregate_wait(reviewer, &registry), None);
    }

    #[test]
    fn test_resolve_aggregate_wait_aggregate_unset_returns_none() {
        // A hat subscribes to review.done but has no `aggregate` block — it is not
        // an aggregator, so the wait falls back to the derived formula (None).
        let yaml = r#"
            hats:
              reviewer:
                name: "Reviewer"
                triggers: ["review.file"]
                publishes: ["review.done"]
                instructions: "Review files"
                concurrency: 4
              observer:
                name: "Observer"
                triggers: ["review.done"]
                publishes: ["review.seen"]
                instructions: "Observe"
        "#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let reviewer = registry.get_config(&HatId::new("reviewer")).unwrap();

        assert_eq!(resolve_aggregate_wait(reviewer, &registry), None);
    }

    #[test]
    fn test_resolve_aggregate_wait_no_publishes_returns_none() {
        // Wave hat publishes nothing → nothing to resolve an aggregator from.
        let yaml = r#"
            hats:
              reviewer:
                name: "Reviewer"
                triggers: ["review.file"]
                publishes: []
                instructions: "Review files"
                concurrency: 4
        "#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let reviewer = registry.get_config(&HatId::new("reviewer")).unwrap();

        assert_eq!(resolve_aggregate_wait(reviewer, &registry), None);
    }
}
