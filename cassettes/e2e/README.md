# E2E Test Cassettes

This directory contains recorded cassettes for mock-mode E2E testing. Cassettes are JSONL files that record the output from real Hats sessions, enabling deterministic, cost-free test execution.

## Directory Structure

```
cassettes/e2e/
├── README.md           # This file
├── connect.jsonl       # Generic connectivity cassette (all backends)
├── events.jsonl        # Event XML parsing test
├── completion.jsonl    # LOOP_COMPLETE detection test
├── single-iter.jsonl   # Single iteration orchestration
├── multi-iter.jsonl    # Multi-iteration orchestration
└── ...
```

## Available Cassettes

| Cassette | Scenario | Status |
|----------|----------|--------|
| `connect.jsonl` | Connectivity | ✅ Passes all backends |
| `events.jsonl` | Event parsing | ✅ Passes all backends |
| `completion.jsonl` | LOOP_COMPLETE | ✅ Passes all backends |
| `single-iter.jsonl` | Single iteration | ⚠️ Scratchpad assertion fails (no file writes) |
| `multi-iter.jsonl` | Multi-iteration | ⚠️ Iteration count fails (architecture limitation) |

## Known Limitations

1. **Multi-iteration scenarios**: Mock-cli replays entire cassette in one invocation, so Hats sees only one iteration
2. **File write assertions**: Scenarios checking scratchpad/artifact content fail unless whitelisted commands execute
3. **Task/Memory scenarios**: Require cassettes with `bus.publish` events containing whitelisted commands

## Naming Convention

Cassettes follow this naming pattern:
- `<scenario-id>.jsonl` - Generic fallback cassette
- `<scenario-id>-<backend>.jsonl` - Backend-specific cassette

When running in mock mode, the cassette resolver checks for:
1. `<scenario>-<backend>.jsonl` (backend-specific, preferred)
2. `<scenario>.jsonl` (generic fallback)

## Recording New Cassettes

To record a new cassette from a live session:

```bash
# Record with hats's built-in recording
cargo run --bin hats -- run \
  -c hats.yml \
  --record-session cassettes/e2e/my-scenario.jsonl \
  -p "Your prompt here"
```

## Cassette Format

Each line is a JSON object with these fields:
- `ts`: Unix timestamp in milliseconds
- `event`: Event type (e.g., `ux.terminal.write`, `bus.publish`, `_meta.iteration`)
- `data`: Event-specific data

### Event Types

| Event | Description |
|-------|-------------|
| `ux.terminal.write` | Terminal output (base64-encoded bytes) |
| `ux.terminal.resize` | Terminal size change |
| `bus.publish` | EventBus event (includes tool calls) |
| `_meta.loop_start` | Orchestration loop started |
| `_meta.iteration` | Iteration completed |
| `_meta.termination` | Loop terminated |

### Terminal Write Data

```json
{
  "bytes": "SGVsbG8=",  // Base64-encoded output
  "stdout": true,       // true for stdout, false for stderr
  "offset_ms": 100      // Time offset from session start
}
```

## Usage

Run E2E tests in mock mode:

```bash
# Run all tests with cassettes
cargo run -p hats-e2e -- --mock

# Run with specific cassette directory
cargo run -p hats-e2e -- --mock --cassette-dir ./my-cassettes

# Run with real-time playback (1x speed)
cargo run -p hats-e2e -- --mock --mock-speed 1.0

# Check cassette availability
cargo run -p hats-e2e -- --mock --list
```

## Creating Cassettes for New Scenarios

1. Run the scenario against a live backend with recording enabled
2. Copy the recorded JSONL to the appropriate location
3. Name it according to the convention above
4. Run `hats-e2e --mock --filter <scenario-id>` to verify
