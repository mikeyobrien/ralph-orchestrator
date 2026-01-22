# Event System Architecture

## Core Event Structure

```rust
pub struct Event {
    pub topic: Topic,          // Routing key (e.g., "build.done")
    pub payload: String,       // Event content/data
    pub source: Option<HatId>, // Which hat emitted
    pub target: Option<HatId>, // Direct routing target
}
```

## Topic-Based Routing

Topics support pattern matching:
- **Concrete**: `task.start`, `build.done`, `build.blocked`
- **Patterns**: `build.*`, `*.done`, `*` (wildcard)

## Event Flow

```
User Prompt → task.start
                ↓
           EventBus.publish()
           ├─→ Observers (TUI, Logger) ← DIAGNOSTIC TAP POINT
           └─→ Route to subscribers
                    ↓
              build.task → Builder → build.done
                                        ↓
                              [Backpressure validation]
```

## EventLogger Output

Events logged to `.ralph/events.jsonl`:

```json
{
  "ts": "2024-01-15T10:23:45Z",
  "iteration": 1,
  "hat": "loop",
  "topic": "task.start",
  "triggered": "planner",
  "payload": "...",
  "blocked_count": 2
}
```

## Observer Pattern

The EventBus supports observers for non-routing consumption:

```rust
// Register observer (TUI, recording, diagnostics)
event_bus.add_observer(|event| {
    // Non-invasive event capture
    diagnostic_logger.log(event);
});
```

This allows **diagnostic logging without disrupting routing**.

## Backpressure Signals

| Signal | Action |
|--------|--------|
| `build.done` without evidence | Synthesize `build.blocked` |
| 3 consecutive `build.blocked` | Emit `build.task.abandoned` |
| 3 redispatches of abandoned | Terminate with `LoopThrashing` |
| 3 consecutive `event.malformed` | Terminate with `ValidationFailure` |

## Key Files

| Component | File |
|-----------|------|
| Event Definition | `crates/ralph-proto/src/event.rs` |
| Topic Matching | `crates/ralph-proto/src/topic.rs` |
| Event Bus | `crates/ralph-proto/src/event_bus.rs` |
| Event Logger | `crates/ralph-core/src/event_logger.rs` |
| Event Parser | `crates/ralph-core/src/event_parser.rs` |
