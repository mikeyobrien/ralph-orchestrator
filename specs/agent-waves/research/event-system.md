# Event System Research

## Event Structure

**File:** `crates/ralph-proto/src/event.rs`

```rust
pub struct Event {
    pub topic: Topic,              // Routing key (e.g., "build.done")
    pub payload: String,           // Content/data (string or JSON-stringified)
    pub source: Option<HatId>,     // Hat that published
    pub target: Option<HatId>,     // Direct target for handoff (bypasses subscription routing)
}
```

Key observations:
- No correlation ID, wave ID, or batch metadata today
- `target` allows direct hat-to-hat handoff
- Payload is always a string (JSON objects are stringified)

## Event Bus

**File:** `crates/ralph-proto/src/event_bus.rs`

```rust
pub struct EventBus {
    hats: BTreeMap<HatId, Hat>,
    pending: BTreeMap<HatId, Vec<Event>>,  // Queue of events per hat
    human_pending: Vec<Event>,              // Separate queue for human.* events
    observers: Vec<Observer>,               // Observer callbacks
}
```

Routing flow:
1. Observers notified first (enables session recording)
2. If `event.target` set → route only to target hat
3. Otherwise → match topic against hat subscriptions (specific > wildcard)
4. Events with `human.*` topic go to separate queue

Key methods:
- `publish(event) -> Vec<HatId>` — returns recipients
- `take_pending(&hat_id) -> Vec<Event>` — destructive consume
- `next_hat_with_pending()` — BTreeMap order, first hat with events

## Event Emission from Agents

**File:** `crates/ralph-cli/src/main.rs:2249-2317`

`ralph emit <topic> [--payload "text"] [--json] [--ts] [--file]`

Flow:
1. Builds JSON record with topic, payload, ts
2. Reads `.ralph/current-events` marker to find active events file
3. Falls back to `.ralph/events.jsonl`
4. Appends single JSONL line atomically

**No direct EventBus interaction** — events written to file, read by EventReader.

## Event Reading

**File:** `crates/ralph-core/src/event_reader.rs`

- Incremental reading (tracks file position)
- Handles both string and object payloads
- Returns `ParseResult` with events + malformed lines for backpressure

## Event Logging

**File:** `crates/ralph-core/src/event_logger.rs`

```rust
pub struct EventRecord {
    pub ts: String,
    pub iteration: u32,
    pub hat: String,
    pub topic: String,
    pub triggered: Option<String>,
    pub payload: String,           // Truncated to 500 chars
    pub blocked_count: Option<u32>,
}
```

## Extension Points for Waves

Adding wave/correlation metadata requires changes to:

1. **Event struct** — Add `wave_id: Option<String>`, `wave_index: Option<u32>`, `wave_total: Option<u32>`
2. **EventRecord** — Add corresponding optional fields (skip_serializing_if)
3. **EventBus** — Track active waves, buffer correlated events, completion conditions
4. **EventReader** — Parse new fields from JSONL
5. **`ralph emit`** — Add `--wave-id`, `--wave-index`, `--wave-total` flags

The architecture supports optional fields well — serde skip_serializing_if is used throughout.

## Topic Matching

**File:** `crates/ralph-proto/src/topic.rs`

Supports glob-style patterns:
- Exact: `impl.done`
- Suffix wildcard: `impl.*`
- Prefix wildcard: `*.done`
- Global wildcard: `*`
