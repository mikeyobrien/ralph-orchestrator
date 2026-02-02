# ralph-proto

Protocol types shared across all Ralph crates.

## Overview

`ralph-proto` defines the core data structures used throughout Ralph:

- Events and Topics for communication
- Hats for persona definitions
- EventBus for routing

## Key Types

### Event

A message with topic, payload, and routing information.

```rust
pub struct Event {
    pub topic: Topic,
    pub payload: Option<String>,
    pub source_hat: Option<HatId>,
    pub target_hat: Option<HatId>,
    pub timestamp: DateTime<Utc>,
}
```

**Creating events:**

```rust
use ralph_proto::Event;

// Simple event
let event = Event::new("build.done");

// With payload
let event = Event::new("build.done")
    .with_payload("tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass");

// With source hat
let event = Event::new("build.done")
    .from_hat("builder");
```

### Topic

Event routing with glob pattern matching.

```rust
pub struct Topic(String);

impl Topic {
    pub fn matches(&self, pattern: &str) -> bool;
}
```

**Pattern matching:**

```rust
let topic = Topic::new("build.done");

topic.matches("build.done");  // true
topic.matches("build.*");     // true
topic.matches("*.done");      // true
topic.matches("test.*");      // false
```

### Hat

A specialized Ralph persona.

```rust
pub struct Hat {
    pub id: HatId,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<String>,      // Subscription patterns
    pub publishes: Vec<String>,     // Allowed event types
    pub default_publishes: Option<String>,
    pub instructions: String,
    pub backend: Option<String>,
    pub max_activations: Option<usize>,
}
```

**Creating hats:**

```rust
use ralph_proto::Hat;

let hat = Hat::builder("builder")
    .name("Builder")
    .triggers(vec!["task.start", "plan.ready"])
    .publishes(vec!["build.done", "build.failed"])
    .instructions("Implement the task...")
    .build();
```

### HatId

Unique identifier for a hat.

```rust
pub struct HatId(String);

impl HatId {
    pub fn new(id: impl Into<String>) -> Self;
}
```

### EventBus

Registry of hats and event routing.

```rust
pub struct EventBus {
    hats: HashMap<HatId, Hat>,
    pending_events: VecDeque<Event>,
    event_history: Vec<Event>,
}

impl EventBus {
    pub fn register_hat(&mut self, hat: Hat);
    pub fn publish(&mut self, event: Event);
    pub fn next_event(&mut self) -> Option<Event>;
    pub fn matching_hat(&self, event: &Event) -> Option<&Hat>;
}
```

**Using EventBus:**

```rust
use ralph_proto::{EventBus, Event, Hat};

let mut bus = EventBus::new();

// Register hats
bus.register_hat(planner_hat);
bus.register_hat(builder_hat);

// Publish events
bus.publish(Event::new("task.start"));

// Get next event and matching hat
if let Some(event) = bus.next_event() {
    if let Some(hat) = bus.matching_hat(&event) {
        // Execute hat with event
    }
}
```

## UX Events

Events for TUI interaction.

```rust
pub enum UxEvent {
    TerminalWrite(String),
    Resize { width: u16, height: u16 },
    FrameCapture(Vec<u8>),
}
```

## Error Types

```rust
pub enum ProtoError {
    InvalidTopic(String),
    InvalidHat(String),
    EventRoutingError(String),
}
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `default` | Standard features |
| `serde` | Serialization support |

## Example: Event Flow

```rust
use ralph_proto::{EventBus, Event, Hat};

// Setup
let mut bus = EventBus::new();

let planner = Hat::builder("planner")
    .triggers(vec!["task.start"])
    .publishes(vec!["plan.ready"])
    .instructions("Create a plan")
    .build();

let builder = Hat::builder("builder")
    .triggers(vec!["plan.ready"])
    .publishes(vec!["build.done"])
    .instructions("Implement the plan")
    .build();

bus.register_hat(planner);
bus.register_hat(builder);

// Start flow
bus.publish(Event::new("task.start"));

// First event matches planner
let event = bus.next_event().unwrap();
let hat = bus.matching_hat(&event).unwrap();
assert_eq!(hat.id.as_str(), "planner");

// Planner publishes plan.ready
bus.publish(Event::new("plan.ready").from_hat("planner"));

// Second event matches builder
let event = bus.next_event().unwrap();
let hat = bus.matching_hat(&event).unwrap();
assert_eq!(hat.id.as_str(), "builder");
```
