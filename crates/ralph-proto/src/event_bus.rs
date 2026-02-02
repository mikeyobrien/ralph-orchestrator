//! Event bus for pub/sub messaging.
//!
//! The event bus routes events to subscribed hats based on topic patterns.
//! Multiple observers can be added to receive all published events for
//! recording, TUI updates, and benchmarking purposes.

use crate::{Event, Hat, HatId};
use std::collections::HashMap;

/// Type alias for the observer callback function.
type Observer = Box<dyn Fn(&Event) + Send + 'static>;

/// Central pub/sub hub for routing events between hats.
#[derive(Default)]
pub struct EventBus {
    /// Registered hats indexed by ID.
    hats: HashMap<HatId, Hat>,

    /// Pending events for each hat.
    pending: HashMap<HatId, Vec<Event>>,

    /// Pending human interaction events (human.*).
    human_pending: Vec<Event>,

    /// Observers that receive all published events.
    /// Multiple observers can be registered (e.g., session recorder + TUI).
    observers: Vec<Observer>,
}

impl EventBus {
    /// Creates a new empty event bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an observer that receives all published events.
    ///
    /// Multiple observers can be added (e.g., session recorder + TUI).
    /// Each observer is called before events are routed to subscribers.
    /// This enables recording sessions by subscribing to the event stream
    /// without modifying the routing logic.
    pub fn add_observer<F>(&mut self, observer: F)
    where
        F: Fn(&Event) + Send + 'static,
    {
        self.observers.push(Box::new(observer));
    }

    /// Sets a single observer, clearing any existing observers.
    ///
    /// Prefer `add_observer` when multiple observers are needed.
    /// This method is kept for backwards compatibility.
    #[deprecated(since = "2.0.0", note = "Use add_observer instead")]
    pub fn set_observer<F>(&mut self, observer: F)
    where
        F: Fn(&Event) + Send + 'static,
    {
        self.observers.clear();
        self.observers.push(Box::new(observer));
    }

    /// Clears all observer callbacks.
    pub fn clear_observers(&mut self) {
        self.observers.clear();
    }

    /// Registers a hat with the event bus.
    pub fn register(&mut self, hat: Hat) {
        let id = hat.id.clone();
        self.hats.insert(id.clone(), hat);
        self.pending.entry(id).or_default();
    }

    /// Publishes an event to all subscribed hats.
    ///
    /// Returns the list of hat IDs that received the event.
    /// If an observer is set, it receives the event before routing.
    #[allow(clippy::needless_pass_by_value)] // Event is cloned to multiple recipients
    pub fn publish(&mut self, event: Event) -> Vec<HatId> {
        // Notify all observers before routing
        for observer in &self.observers {
            observer(&event);
        }

        if event.topic.as_str().starts_with("human.") {
            self.human_pending.push(event);
            return Vec::new();
        }

        let mut recipients = Vec::new();

        // If there's a direct target, route only to that hat
        if let Some(ref target) = event.target {
            if self.hats.contains_key(target) {
                self.pending
                    .entry(target.clone())
                    .or_default()
                    .push(event.clone());
                recipients.push(target.clone());
            }
            return recipients;
        }

        // Route with priority: specific subscriptions > fallback wildcards
        // Per spec: "If event has subscriber → Select that hat's backend"
        //           "If no subscriber → Select Ralph's backend (cli.backend)"

        // First, find hats with specific (non-global-wildcard) subscriptions
        let mut specific_recipients = Vec::new();
        let mut fallback_recipients = Vec::new();

        for (id, hat) in &self.hats {
            if hat.has_specific_subscription(&event.topic) {
                // Hat has a specific subscription for this topic
                specific_recipients.push(id.clone());
            } else if hat.is_subscribed(&event.topic) {
                // Hat matches only via global wildcard (fallback)
                fallback_recipients.push(id.clone());
            }
        }

        // Use specific subscribers if any, otherwise fall back to wildcard handlers
        let chosen_recipients = if specific_recipients.is_empty() {
            fallback_recipients
        } else {
            specific_recipients
        };

        for id in chosen_recipients {
            self.pending
                .entry(id.clone())
                .or_default()
                .push(event.clone());
            recipients.push(id);
        }

        recipients
    }

    /// Takes all pending events for a hat.
    pub fn take_pending(&mut self, hat_id: &HatId) -> Vec<Event> {
        self.pending.remove(hat_id).unwrap_or_default()
    }

    /// Takes all pending human interaction events.
    pub fn take_human_pending(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.human_pending)
    }

    /// Returns a reference to pending events for a hat without consuming them.
    pub fn peek_pending(&self, hat_id: &HatId) -> Option<&Vec<Event>> {
        self.pending.get(hat_id)
    }

    /// Returns a reference to pending human interaction events without consuming them.
    pub fn peek_human_pending(&self) -> &[Event] {
        &self.human_pending
    }

    /// Checks if there are any pending events for any hat.
    pub fn has_pending(&self) -> bool {
        !self.human_pending.is_empty() || self.pending.values().any(|events| !events.is_empty())
    }

    /// Checks if there are any pending human interaction events.
    pub fn has_human_pending(&self) -> bool {
        !self.human_pending.is_empty()
    }

    /// Returns the next hat with pending events.
    pub fn next_hat_with_pending(&self) -> Option<&HatId> {
        self.pending
            .iter()
            .find(|(_, events)| !events.is_empty())
            .map(|(id, _)| id)
    }

    /// Gets a hat by ID.
    pub fn get_hat(&self, id: &HatId) -> Option<&Hat> {
        self.hats.get(id)
    }

    /// Returns all registered hat IDs.
    pub fn hat_ids(&self) -> impl Iterator<Item = &HatId> {
        self.hats.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_to_subscriber() {
        let mut bus = EventBus::new();

        let hat = Hat::new("impl", "Implementer").subscribe("task.*");
        bus.register(hat);

        let event = Event::new("task.start", "Start implementing");
        let recipients = bus.publish(event);

        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0].as_str(), "impl");
    }

    #[test]
    fn test_no_match() {
        let mut bus = EventBus::new();

        let hat = Hat::new("impl", "Implementer").subscribe("task.*");
        bus.register(hat);

        let event = Event::new("review.done", "Review complete");
        let recipients = bus.publish(event);

        assert!(recipients.is_empty());
    }

    #[test]
    fn test_direct_target() {
        let mut bus = EventBus::new();

        let impl_hat = Hat::new("impl", "Implementer").subscribe("task.*");
        let review_hat = Hat::new("reviewer", "Reviewer").subscribe("impl.*");
        bus.register(impl_hat);
        bus.register(review_hat);

        // Direct target bypasses subscription matching
        let event = Event::new("handoff", "Please review").with_target("reviewer");
        let recipients = bus.publish(event);

        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0].as_str(), "reviewer");
    }

    #[test]
    fn test_take_pending() {
        let mut bus = EventBus::new();

        let hat = Hat::new("impl", "Implementer").subscribe("*");
        bus.register(hat);

        bus.publish(Event::new("task.start", "Start"));
        bus.publish(Event::new("task.continue", "Continue"));

        let hat_id = HatId::new("impl");
        let events = bus.take_pending(&hat_id);

        assert_eq!(events.len(), 2);
        assert!(bus.take_pending(&hat_id).is_empty());
    }

    #[test]
    fn test_human_events_use_separate_queue() {
        let mut bus = EventBus::new();

        let hat = Hat::new("ralph", "Ralph").subscribe("*");
        bus.register(hat);

        bus.publish(Event::new("human.interact", "question"));
        bus.publish(Event::new("human.response", "hello"));
        bus.publish(Event::new("human.guidance", "note"));

        assert_eq!(bus.peek_human_pending().len(), 3);
        assert_eq!(
            bus.peek_pending(&HatId::new("ralph"))
                .map(|events| events.len())
                .unwrap_or(0),
            0
        );

        let taken = bus.take_human_pending();
        assert_eq!(taken.len(), 3);
        assert!(!bus.has_human_pending());
    }

    #[test]
    fn test_self_routing_allowed() {
        // Self-routing is allowed to handle LLM non-determinism.
        // Spec acceptance criteria: planner emits build.done (even though builder "should"),
        // event routes back to planner, planner continues (no source-based blocking).
        let mut bus = EventBus::new();

        let planner = Hat::new("planner", "Planner").subscribe("build.done");
        bus.register(planner);

        // Planner emits build.done (wrong hat, but LLMs are non-deterministic)
        let event = Event::new("build.done", "Done").with_source("planner");
        let recipients = bus.publish(event);

        // Event SHOULD route back to planner (self-routing allowed, no source filtering)
        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0].as_str(), "planner");
    }

    #[test]
    fn test_observer_receives_all_events() {
        use std::sync::{Arc, Mutex};

        let mut bus = EventBus::new();
        let observed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let observed_clone = Arc::clone(&observed);
        bus.add_observer(move |event| {
            observed_clone.lock().unwrap().push(event.payload.clone());
        });

        let hat = Hat::new("impl", "Implementer").subscribe("task.*");
        bus.register(hat);

        // Publish events - observer should see all regardless of routing
        bus.publish(Event::new("task.start", "Start"));
        bus.publish(Event::new("other.event", "Other")); // No subscriber
        bus.publish(Event::new("task.done", "Done"));

        let captured = observed.lock().unwrap();
        assert_eq!(captured.len(), 3);
        assert_eq!(captured[0], "Start");
        assert_eq!(captured[1], "Other");
        assert_eq!(captured[2], "Done");
    }

    #[test]
    fn test_multiple_observers() {
        use std::sync::{Arc, Mutex};

        let mut bus = EventBus::new();
        let observer1_count = Arc::new(Mutex::new(0));
        let observer2_count = Arc::new(Mutex::new(0));

        let count1 = Arc::clone(&observer1_count);
        bus.add_observer(move |_| {
            *count1.lock().unwrap() += 1;
        });

        let count2 = Arc::clone(&observer2_count);
        bus.add_observer(move |_| {
            *count2.lock().unwrap() += 1;
        });

        bus.publish(Event::new("test", "1"));
        bus.publish(Event::new("test", "2"));

        // Both observers should have received both events
        assert_eq!(*observer1_count.lock().unwrap(), 2);
        assert_eq!(*observer2_count.lock().unwrap(), 2);
    }

    #[test]
    fn test_clear_observers() {
        use std::sync::{Arc, Mutex};

        let mut bus = EventBus::new();
        let count = Arc::new(Mutex::new(0));

        let count_clone = Arc::clone(&count);
        bus.add_observer(move |_| {
            *count_clone.lock().unwrap() += 1;
        });

        bus.publish(Event::new("test", "1"));
        assert_eq!(*count.lock().unwrap(), 1);

        bus.clear_observers();
        bus.publish(Event::new("test", "2"));
        assert_eq!(*count.lock().unwrap(), 1); // Still 1, observers cleared
    }

    #[test]
    fn test_peek_pending_does_not_consume() {
        let mut bus = EventBus::new();

        let hat = Hat::new("impl", "Implementer").subscribe("*");
        bus.register(hat);

        bus.publish(Event::new("task.start", "Start"));
        bus.publish(Event::new("task.continue", "Continue"));

        let hat_id = HatId::new("impl");

        // Peek at pending events
        let peeked = bus.peek_pending(&hat_id);
        assert!(peeked.is_some());
        assert_eq!(peeked.unwrap().len(), 2);

        // Peek again - should still be there
        let peeked_again = bus.peek_pending(&hat_id);
        assert!(peeked_again.is_some());
        assert_eq!(peeked_again.unwrap().len(), 2);

        // Now take them - should consume
        let taken = bus.take_pending(&hat_id);
        assert_eq!(taken.len(), 2);

        // Peek after take - should be empty
        let peeked_after_take = bus.peek_pending(&hat_id);
        assert!(peeked_after_take.is_none() || peeked_after_take.unwrap().is_empty());
    }
}
