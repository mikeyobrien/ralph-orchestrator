# Existing Diagnostic Infrastructure

## Current Logging Stack

Ralph uses `tracing` + `tracing-subscriber` with basic configuration:

```rust
// TUI mode: conditional file logging
if std::env::var("RALPH_DEBUG_LOG").is_ok() {
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .init();
}

// Non-TUI: logs to stdout
tracing_subscriber::fmt().with_env_filter(filter).init();
```

### Log Level Control
- `--verbose` flag → "debug" level
- Default → "info" level
- No RUST_LOG environment variable support (only static filter)

### Current Logging Coverage
- **167 log macro calls** across 14 files
- Highest density: `pty_executor.rs` (48), `main.rs` (40), `event_loop.rs` (26)

## Existing Diagnostic Files

| File | Purpose |
|------|---------|
| `.agent/ralph.log` | Debug logs (TUI mode only, requires `RALPH_DEBUG_LOG=1`) |
| `.ralph/events.jsonl` | Event history (all events with rich metadata) |
| `.agent/summary.md` | Loop termination summary |
| `.agent/scratchpad.md` | Working state / handoff |
| `.agent/memories.md` | Persistent learning |

## CLI Diagnostic Flags

| Flag | Purpose |
|------|---------|
| `--verbose` / `-v` | Enable debug output |
| `--quiet` / `-q` | Suppress streaming output |
| `--dry-run` | Show config without executing |
| `--record-session <FILE>` | Record to JSONL for replay |
| `ralph events` | Query event history |

## Environment Variables

| Variable | Effect |
|----------|--------|
| `RALPH_DEBUG_LOG=1` | Enable file logging in TUI mode |
| `RALPH_VERBOSE` | Enable verbose mode |
| `RALPH_QUIET` | Enable quiet mode |

## Gaps Identified

1. **No RUST_LOG support** — only `--verbose` flag
2. **No structured logging** — plain text format only
3. **No `#[instrument]` spans** — no automatic function tracing
4. **No log rotation** — single file, no size limits
5. **Two separate systems** — tracing logs vs EventLogger not integrated
6. **TUI logging suppressed by default** — requires env var to enable
