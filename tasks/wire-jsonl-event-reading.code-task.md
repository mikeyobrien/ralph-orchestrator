---
status: completed
created: 2026-01-15
completed: 2026-01-15
---
# Task: Wire JSONL Event Reading into Main CLI Loop

## Description
The `EventLoop::process_events_from_jsonl()` method exists but is never called in the main CLI loop. This means when Claude writes events to `.agent/events.jsonl`, they are never read and routed to hats. The hat delegation system is broken because of this missing integration.

## Problem Evidence
From evaluation logs:
```
Claude: Now I'll publish the `tdd.start` event to delegate to the Test Writer hat:
[Tool] Bash: echo '{"topic": "tdd.start", ...}' >> .agent/events.jsonl

WARN ralph: No pending events after iteration. Agent may have failed to publish a valid event. Expected one of: [].
```

The agent correctly wrote the event to disk, but Ralph never read it.

## Root Cause
- `EventLoop::process_events_from_jsonl()` exists at `event_loop.rs:735`
- It's only called in tests (`event_loop_ralph.rs:46`)
- The main CLI loop (`main.rs`) never calls it

## Technical Requirements

### Location
File: `crates/ralph-cli/src/main.rs`
After: `event_loop.process_output()` call (around line 1295)
Before: `has_pending_events()` check (around line 1311)

### Implementation
Add a call to `process_events_from_jsonl()` after processing output:

```rust
// Process output
if let Some(reason) = event_loop.process_output(&hat_id, &output, success) {
    // ... existing termination handling ...
}

// NEW: Read events from JSONL that agent may have written
if let Err(e) = event_loop.process_events_from_jsonl() {
    warn!(error = %e, "Failed to read events from JSONL");
}

// Precheck validation: Warn if no pending events after processing output
if !event_loop.has_pending_events() {
    // ... existing warning logic ...
}
```

### Why This Location
1. After `process_output()` - agent has finished writing, safe to read
2. Before `has_pending_events()` - newly read events will be detected
3. Before `inject_fallback_event()` - avoid unnecessary fallbacks if events exist

## Files to Modify
- `crates/ralph-cli/src/main.rs` - Add `process_events_from_jsonl()` call in main loop

## Dependencies
- `EventLoop::process_events_from_jsonl()` already implemented in `ralph-core`
- `EventReader` already reads from `.agent/events.jsonl`
- No new dependencies needed

## Acceptance Criteria

1. **Events Written by Agent Are Read**
   - Given Claude writes `{"topic": "tdd.start", ...}` to `.agent/events.jsonl`
   - When the iteration completes
   - Then `process_events_from_jsonl()` reads the event
   - And the event is routed to the appropriate hat

2. **Hat Triggers on Disk Events**
   - Given `test_writer` hat triggers on `tdd.start`
   - When agent writes `tdd.start` event to JSONL
   - Then the next iteration should activate `test_writer` hat
   - And NOT show "No pending events" warning

3. **Error Handling**
   - Given JSONL file has corrupt JSON
   - When `process_events_from_jsonl()` is called
   - Then errors are logged but don't crash the loop
   - And valid events are still processed

4. **No Regression**
   - Given existing event parsing from XML tags in output
   - When this change is added
   - Then XML-based events still work
   - And `cargo test` passes

## Testing

After implementation:
```bash
# Build
cargo build

# Run tests
cargo test

# Manual test: Run TDD evaluation
./tools/evaluate-preset.sh tdd-red-green claude

# Verify in output log:
# - Test Writer hat should activate after tdd.start is written
# - No "No pending events" warning after agent publishes event
```

## Notes
- The `process_events_from_jsonl()` method already handles orphan events (events with no subscriber) by routing them to Ralph
- This completes the feedback loop: Agent → JSONL → EventReader → EventBus → Hat
- Consider also calling this in `ralph-bench/src/main.rs` for consistency (secondary fix)
