//! Event types for pub/sub messaging.

use crate::{HatId, Topic};
use serde::{Deserialize, Serialize};

/// An event in the pub/sub system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// The routing topic for this event.
    pub topic: Topic,

    /// The content/payload of the event.
    pub payload: String,

    /// The hat that published this event (if any).
    pub source: Option<HatId>,

    /// Optional target hat for direct handoff.
    pub target: Option<HatId>,

    /// Wave correlation ID (e.g., "w-1a2b3c4d").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_id: Option<String>,

    /// Index of this event within the wave (0-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_index: Option<u32>,

    /// Total number of events in the wave.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_total: Option<u32>,
}

impl Event {
    /// Creates a new event with the given topic and payload.
    pub fn new(topic: impl Into<Topic>, payload: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            payload: payload.into(),
            source: None,
            target: None,
            wave_id: None,
            wave_index: None,
            wave_total: None,
        }
    }

    /// Sets the source hat for this event.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<HatId>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets the target hat for direct handoff.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<HatId>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Sets wave correlation metadata on this event.
    #[must_use]
    pub fn with_wave(mut self, wave_id: impl Into<String>, index: u32, total: u32) -> Self {
        self.wave_id = Some(wave_id.into());
        self.wave_index = Some(index);
        self.wave_total = Some(total);
        self
    }

    /// Returns true if this event has wave correlation metadata.
    pub fn is_wave_event(&self) -> bool {
        self.wave_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_event_has_no_wave_metadata() {
        let event = Event::new("test.topic", "payload");
        assert!(!event.is_wave_event());
        assert!(event.wave_id.is_none());
        assert!(event.wave_index.is_none());
        assert!(event.wave_total.is_none());
    }

    #[test]
    fn test_with_wave_sets_metadata() {
        let event = Event::new("review.file", "src/main.rs").with_wave("w-1a2b3c4d", 0, 3);
        assert!(event.is_wave_event());
        assert_eq!(event.wave_id.as_deref(), Some("w-1a2b3c4d"));
        assert_eq!(event.wave_index, Some(0));
        assert_eq!(event.wave_total, Some(3));
    }

    #[test]
    fn test_wave_metadata_roundtrips_through_serde() {
        let event = Event::new("review.file", "src/main.rs")
            .with_source("dispatcher")
            .with_wave("w-abcd1234", 2, 5);

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.wave_id.as_deref(), Some("w-abcd1234"));
        assert_eq!(deserialized.wave_index, Some(2));
        assert_eq!(deserialized.wave_total, Some(5));
        assert_eq!(deserialized.topic.as_str(), "review.file");
        assert_eq!(deserialized.payload, "src/main.rs");
    }

    #[test]
    fn test_event_without_wave_serializes_without_wave_fields() {
        let event = Event::new("test.topic", "payload");
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("wave_id"));
        assert!(!json.contains("wave_index"));
        assert!(!json.contains("wave_total"));
    }

    #[test]
    fn test_event_without_wave_fields_deserializes() {
        let json = r#"{"topic":"test.topic","payload":"hello"}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(!event.is_wave_event());
        assert_eq!(event.topic.as_str(), "test.topic");
        assert_eq!(event.payload, "hello");
    }
}
