# Research Summary: Diagnostic Logging for Ralph

## Executive Summary

Ralph already has a **solid foundation** for diagnostic logging but with **significant gaps** that need addressing. The architecture is well-suited for adding comprehensive diagnostics without major refactoring.

## What Already Exists

### Logging Infrastructure
- `tracing` + `tracing-subscriber` (basic configuration)
- 167 log macro calls across codebase
- TUI-aware file logging (`.agent/ralph.log` with `RALPH_DEBUG_LOG=1`)

### Event System
- Topic-based pub/sub with observer pattern
- `.ralph/events.jsonl` with rich metadata (iteration, hat, timestamps)
- EventBus observers allow non-invasive logging

### Session Recording
- `--record-session <FILE>` captures all events to JSONL
- Supports replay for testing
- Terminal writes captured with timestamps

### CLI Diagnostics
- `ralph events` command for querying event history
- `--dry-run` for configuration inspection
- `--verbose`/`--quiet` flags

## What's Missing (Gaps)

| Gap | Impact |
|-----|--------|
| No `RUST_LOG` env var support | Can't filter by crate/module at runtime |
| No structured fields | Harder to query/analyze logs |
| No `#[instrument]` spans | No automatic function tracing |
| No log rotation | Single file grows unbounded |
| Two separate systems (tracing vs EventLogger) | No unified view |
| TUI logging suppressed by default | Must opt-in with env var |
| No agent output logging | Raw output not persisted |

## Key Architectural Insights

### StreamHandler Abstraction
All output flows through `StreamHandler` trait — diagnostic logging can:
1. **Wrap existing handlers** to capture output
2. **Add a new handler** that logs everything
3. **Use observer pattern** on EventBus

### Diagnostic Tap Points

```
Agent Output → CliCapture → StreamHandler → Display
                   ↓              ↓
              [TAP HERE]    [OR HERE]
                   ↓              ↓
           UX Events      Handler methods
```

### Observer Pattern for Non-Invasive Logging
```rust
event_bus.add_observer(|event| {
    diagnostic_logger.log(event);
});
```

## Recommended Approach

Based on research, diagnostic logging should:

1. **Unify systems** — Single diagnostic log combining tracing + events
2. **Always write to file** — Both TUI and non-TUI modes
3. **Use structured fields** — Enable querying/filtering
4. **Add `RUST_LOG` support** — Granular runtime control
5. **Capture agent output** — Raw output for debugging
6. **Consider log rotation** — Prevent unbounded growth

## Files for Deep-Dive

| Topic | Research File |
|-------|---------------|
| Current logging state | `research/existing-infrastructure.md` |
| TUI/non-TUI architecture | `research/architecture.md` |
| Event system | `research/event-system.md` |
| Industry best practices | `research/best-practices.md` |

## Key Source Files

| Component | Location |
|-----------|----------|
| Logging init | `crates/ralph-cli/src/main.rs:465-484` |
| Stream handlers | `crates/ralph-adapters/src/stream_handler.rs` |
| Event logger | `crates/ralph-core/src/event_logger.rs` |
| PTY capture | `crates/ralph-core/src/cli_capture.rs` |
| Event bus | `crates/ralph-proto/src/event_bus.rs` |
