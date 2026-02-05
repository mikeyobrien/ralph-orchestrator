# Test Fixtures

This directory contains JSONL session fixtures for smoke testing the Hats event loop.

## Fixture Format

Each fixture is a JSONL file with terminal write events recorded from real or simulated sessions.
The format matches the `SessionRecorder` output:

```json
{"ts": 1000, "event": "ux.terminal.write", "data": {"bytes": "<base64>", "stdout": true, "offset_ms": 0}}
```

- `ts`: Timestamp in milliseconds
- `event`: Event type (use `ux.terminal.write` for terminal output)
- `data.bytes`: Base64-encoded raw terminal output bytes
- `data.stdout`: `true` for stdout, `false` for stderr
- `data.offset_ms`: Offset from session start in milliseconds

## Available Fixtures

### basic_session.jsonl

A minimal smoke test fixture demonstrating:
- Hats startup output
- Event parsing (`build.task`, `build.done`)
- Completion event detection (`LOOP_COMPLETE`)

Contains 3 terminal write chunks and 2 parsed events.

## Creating New Fixtures

You can record fixtures from real sessions using Hats's session recording feature,
or create them programmatically using the `TerminalWrite` struct from `hats-proto`:

```rust
use hats_proto::TerminalWrite;
let write = TerminalWrite::new(b"Hello", true, 0);
```

## Usage in Tests

```rust
use hats_core::testing::{SmokeRunner, SmokeTestConfig};

let config = SmokeTestConfig::new("tests/fixtures/basic_session.jsonl");
let result = SmokeRunner::run(&config)?;

assert!(result.completed_successfully());
```
