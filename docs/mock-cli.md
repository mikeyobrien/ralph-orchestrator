# Mock CLI: Cost-Free E2E Testing

## Problem Statement

Running E2E tests against real AI backends (Claude, Kiro, Gemini, etc.) has several problems:

1. **Cost**: Each test run consumes API credits
2. **Speed**: Network latency and API rate limits slow down CI/CD
3. **Reliability**: Network issues and API availability affect test stability
4. **Determinism**: AI responses vary, making tests non-deterministic

Teams need a way to run E2E tests that:
- Costs nothing (no API calls)
- Runs fast (no network latency)
- Is deterministic (same output every time)
- Validates the full orchestration loop (not just unit tests)

## Solution Overview

The `mock-cli` subcommand replays pre-recorded JSONL cassettes instead of invoking real AI backends. This enables:

- **Zero-cost testing**: No API calls, no credits consumed
- **Fast execution**: Instant or accelerated replay (10x+ speed)
- **Deterministic output**: Same cassette = same output every time
- **Full integration**: Tests the complete orchestration loop via PTY

The mock CLI acts as a drop-in replacement for real backends by implementing the same command-line interface that Hats expects.

## How It Works

### Architecture

```
hats-e2e --mock
    │
    ├─ Writes hats.yml with custom backend
    │  cli:
    │    backend: custom
    │    command: hats-e2e
    │    args: ["mock-cli", "--cassette", "path/to/cassette.jsonl"]
    │
    └─ hats run (orchestrator)
        │
        └─ Spawns: hats-e2e mock-cli --cassette cassettes/e2e/connect.jsonl
            │
            ├─ SessionPlayer: Reads JSONL cassette
            │   └─ Extracts ux.terminal.write events
            │
            ├─ Replays output to stdout (via PTY)
            │
            └─ WhitelistExecutor: Runs approved local commands
                └─ hats task add, hats tools memory add, etc.
```

### Cassette Format

Cassettes are JSONL files containing timestamped events from the SessionRecorder:

```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"UE9ORw==","stdout":true,"offset_ms":0}}
{"ts":1100,"event":"bus.publish","data":{"command":"hats task add 'test'"}}
{"ts":1200,"event":"ux.terminal.write","data":{"bytes":"RG9uZQ==","stdout":true,"offset_ms":200}}
```

Each line is a JSON object with:
- `ts`: Unix timestamp in milliseconds
- `event`: Event type (e.g., `ux.terminal.write`, `bus.publish`)
- `data`: Event-specific payload

The mock CLI extracts:
1. **Terminal writes** (`ux.terminal.write`) → replayed to stdout
2. **Commands** (`bus.publish` with command field) → executed if whitelisted

### Cassette Naming Convention

Cassettes are stored in `cassettes/e2e/` with the following resolution order:

1. **Backend-specific**: `<scenario-id>-<backend>.jsonl`
   - Example: `connect-claude.jsonl`, `task-add-kiro.jsonl`
   - Used when backend-specific behavior differs

2. **Generic fallback**: `<scenario-id>.jsonl`
   - Example: `connect.jsonl`, `task-add.jsonl`
   - Used when behavior is identical across backends

If neither exists, the test fails fast with a clear error.

## Usage Guide

### Running E2E Tests in Mock Mode

```bash
# Run all E2E tests with mock backends (zero cost)
hats-e2e --mock

# Run with accelerated replay (10x speed)
hats-e2e --mock --mock-speed 10.0

# Run with instant replay (no delays)
hats-e2e --mock --mock-speed 0.0

# Run specific scenarios
hats-e2e --mock --filter connect

# Custom cassette directory (default: cassettes/e2e)
hats-e2e --mock --cassette-dir /path/to/cassettes
```

### Direct Mock CLI Invocation

The mock CLI is typically invoked by Hats as a custom backend, but you can run it directly for testing:

```bash
# Basic replay
hats-e2e mock-cli --cassette cassettes/e2e/connect.jsonl

# With speed adjustment (10x faster)
hats-e2e mock-cli --cassette cassettes/e2e/connect.jsonl --speed 10.0

# With command execution whitelist
hats-e2e mock-cli \
  --cassette cassettes/e2e/task-add.jsonl \
  --allow "hats task add,hats tools memory add"

# Check version (for backend availability checks)
hats-e2e mock-cli --version
```

### Prerequisites

1. **Cassette files**: Must exist in `cassettes/e2e/` directory
2. **Hats installed**: Required for whitelisted command execution
3. **Workspace setup**: Mock CLI runs in the scenario workspace directory

### Recording New Cassettes

To create cassettes for new scenarios:

```bash
# Run E2E test with real backend and session recording
hats run --record-session cassettes/e2e/my-scenario-claude.jsonl

# Or use the E2E harness with recording enabled
# (Implementation detail: E2E harness should support --record flag)
```

## API Reference

### Command-Line Interface

```
hats-e2e mock-cli [OPTIONS]

OPTIONS:
    --cassette <PATH>       Path to JSONL cassette file (required)
    --speed <FLOAT>         Replay speed multiplier (default: 0.0 = instant)
                           1.0 = real-time, 10.0 = 10x faster
    --allow <CSV>           Comma-separated command prefixes to whitelist
                           Example: "hats task add,hats tools memory add"
    --version              Print version and exit (for availability checks)
    -h, --help             Print help information
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success - cassette replayed successfully |
| 1 | Cassette file not found or unreadable |
| 2 | Cassette parse error (invalid JSONL) |
| 3 | Replay error (I/O failure during output) |
| 4 | Command execution error (whitelisted command failed) |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `HATS_MOCK_ALLOW` | Command whitelist (overrides `--allow`) | None |

### Cassette Resolution API

The `CassetteResolver` provides programmatic access to cassette resolution:

```rust
use hats_e2e::mock::{CassetteResolver, MockConfig};
use hats_e2e::Backend;

// Create resolver
let resolver = CassetteResolver::new("cassettes/e2e");

// Resolve cassette for scenario + backend
let path = resolver.resolve("connect", Backend::Claude)?;
// Returns: cassettes/e2e/connect-claude.jsonl (or connect.jsonl fallback)

// Get all candidate paths (for debugging)
let candidates = resolver.candidates("connect", Backend::Claude);
// Returns: ["cassettes/e2e/connect-claude.jsonl", "cassettes/e2e/connect.jsonl"]
```

### Mock Configuration API

```rust
use hats_e2e::mock::MockConfig;

// Default config (instant replay, standard whitelist)
let config = MockConfig::default();

// Custom config
let config = MockConfig::new("/custom/cassettes")
    .with_speed(10.0)
    .with_allow_commands("hats task add,hats task close");

// Disable command execution
let config = MockConfig::default().without_commands();
```

## Edge Cases and Limitations

### What Happens When...

#### Cassette is missing?

**Behavior**: Test fails immediately with clear error message

```
Error: cassette not found for scenario 'connect' backend 'claude'
Tried:
  - cassettes/e2e/connect-claude.jsonl
  - cassettes/e2e/connect.jsonl
```

**Solution**: Record a cassette for this scenario or use a generic fallback

#### Cassette contains invalid JSONL?

**Behavior**: Parse error with line number and details

```
Error: cassette parse error in cassettes/e2e/connect.jsonl
Line 5: expected value at line 1 column 1
```

**Solution**: Validate cassette format or re-record

#### Command is not whitelisted?

**Behavior**: Command is skipped with warning to stderr

```
[mock-cli] Skipping non-whitelisted command: rm -rf /
```

**Solution**: Add command to whitelist if it's safe and necessary

#### Whitelisted command fails?

**Behavior**: Warning logged, replay continues (non-fatal)

```
[mock-cli] Warning: command 'hats task close invalid-id' exited with status 1
```

**Rationale**: Command failures during replay shouldn't break the test unless the scenario explicitly checks for them

#### Cassette contains no terminal writes?

**Behavior**: Mock CLI outputs nothing, exits successfully

**Use case**: Scenarios that only test side effects (tasks, memories) without output validation

#### Speed is negative?

**Behavior**: Clamped to 0.0 (instant replay)

```rust
let config = MockConfig::default().with_speed(-5.0);
assert_eq!(config.speed, 0.0);
```

#### Multiple backends use same cassette?

**Behavior**: Generic cassette (`<scenario>.jsonl`) is used for all backends

**Use case**: Scenarios where backend behavior is identical (e.g., connectivity checks)

### Limitations

1. **No shell features**: Command whitelist does NOT support pipes, redirects, or variable expansion
   - ✅ Allowed: `hats task add 'test'`
   - ❌ Not allowed: `hats task add 'test' | grep foo`

2. **No network access**: Mock CLI cannot make real API calls or network requests
   - Use real backend mode for integration tests requiring network

3. **Timing approximation**: Replay timing is approximate, not exact
   - Sufficient for E2E validation, not for performance benchmarking

4. **Command execution is synchronous**: Whitelisted commands run sequentially
   - No parallel execution or background processes

5. **PTY limitations**: Mock CLI outputs to PTY, which may affect ANSI escape sequences
   - Most terminal output works correctly, but complex TUI interactions may differ

## Examples

### Example 1: Basic Connectivity Test

**Scenario**: Verify Hats can connect to backend and receive output

**Cassette** (`cassettes/e2e/connect.jsonl`):
```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"UE9ORw==","stdout":true,"offset_ms":0}}
```

**Usage**:
```bash
hats-e2e --mock --filter connect
```

**Expected**: Test passes, output contains "PONG"

### Example 2: Task Creation with Side Effects

**Scenario**: Verify Hats can create tasks via `hats task add`

**Cassette** (`cassettes/e2e/task-add.jsonl`):
```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"Q3JlYXRpbmcgdGFzaw==","stdout":true,"offset_ms":0}}
{"ts":1100,"event":"bus.publish","data":{"command":"hats task add 'test task' -p 1"}}
{"ts":1200,"event":"ux.terminal.write","data":{"bytes":"VGFzayBjcmVhdGVk","stdout":true,"offset_ms":100}}
```

**Usage**:
```bash
hats-e2e mock-cli \
  --cassette cassettes/e2e/task-add.jsonl \
  --allow "hats task add"
```

**Expected**: 
- Output contains "Creating task" and "Task created"
- `.agent/tasks.jsonl` contains new task entry

### Example 3: Accelerated Replay for CI

**Scenario**: Run full E2E suite quickly in CI pipeline

**Usage**:
```bash
# Run all tests with 10x speed (no delays)
hats-e2e --mock --mock-speed 0.0
```

**Expected**: All tests complete in seconds instead of minutes

### Example 4: Backend-Specific Behavior

**Scenario**: Test Claude-specific output format

**Cassettes**:
- `cassettes/e2e/format-claude.jsonl` (Claude-specific)
- `cassettes/e2e/format-kiro.jsonl` (Kiro-specific)
- `cassettes/e2e/format.jsonl` (generic fallback)

**Usage**:
```bash
# Runs with backend-specific cassettes
hats-e2e --mock --filter format
```

**Expected**: Each backend uses its specific cassette, falls back to generic if missing

### Example 5: Error Scenario Testing

**Scenario**: Verify Hats handles backend timeout gracefully

**Cassette** (`cassettes/e2e/timeout-handling.jsonl`):
```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"U3RhcnRpbmc=","stdout":true,"offset_ms":0}}
{"ts":31000,"event":"ux.terminal.write","data":{"bytes":"VGltZW91dA==","stdout":true,"offset_ms":30000}}
```

**Usage**:
```bash
hats-e2e --mock --filter timeout-handling --mock-speed 10.0
```

**Expected**: Test validates timeout handling (3 seconds at 10x speed)

## Troubleshooting

### Problem: "cassette not found" error

**Symptoms**:
```
Error: cassette not found for scenario 'my-test' backend 'claude'
```

**Solutions**:
1. Check cassette file exists: `ls cassettes/e2e/my-test*.jsonl`
2. Verify naming convention: `<scenario-id>-<backend>.jsonl` or `<scenario-id>.jsonl`
3. Record a new cassette with real backend
4. Use generic cassette (remove backend suffix)

### Problem: Cassette parse error

**Symptoms**:
```
Error: cassette parse error in cassettes/e2e/test.jsonl
```

**Solutions**:
1. Validate JSONL format: `jq . cassettes/e2e/test.jsonl`
2. Check for trailing commas or invalid JSON
3. Re-record cassette from scratch

### Problem: Commands not executing

**Symptoms**: Expected side effects (tasks, memories) not present

**Solutions**:
1. Verify whitelist includes command: `--allow "hats task add"`
2. Check cassette contains `bus.publish` events with commands
3. Ensure commands are in correct format (no shell features)
4. Run with verbose logging to see skipped commands

### Problem: Output differs from real backend

**Symptoms**: Mock output doesn't match real backend behavior

**Solutions**:
1. Re-record cassette with latest backend version
2. Check for backend-specific cassette: `<scenario>-<backend>.jsonl`
3. Verify cassette was recorded in same environment (PTY vs non-PTY)

### Problem: Tests pass in mock mode but fail with real backend

**Symptoms**: Mock tests pass, real E2E tests fail

**Root cause**: Cassette is outdated or doesn't reflect real behavior

**Solutions**:
1. Re-record cassettes with current backend
2. Run real E2E tests periodically (e.g., nightly)
3. Use mock mode for fast feedback, real mode for validation

## Best Practices

### When to Use Mock Mode

✅ **Use mock mode for**:
- Fast feedback during development
- CI/CD pipelines (cost and speed)
- Regression testing (deterministic output)
- Testing error scenarios (timeouts, failures)

❌ **Don't use mock mode for**:
- Validating new backend integrations
- Testing actual AI behavior changes
- Performance benchmarking
- Network-dependent scenarios

### Cassette Management

1. **Version control**: Commit cassettes to git for reproducibility
2. **Naming convention**: Use descriptive scenario IDs
3. **Backend-specific**: Only create when behavior differs
4. **Regular updates**: Re-record when backend behavior changes
5. **Minimal cassettes**: Keep cassettes small and focused

### Whitelist Safety

1. **Principle of least privilege**: Only whitelist necessary commands
2. **No destructive commands**: Never whitelist `rm`, `mv`, etc.
3. **Prefix matching**: Use specific prefixes (`hats task add`, not `hats`)
4. **Review regularly**: Audit whitelist for unnecessary entries

### Testing Strategy

1. **Mock for speed**: Run mock tests on every commit
2. **Real for validation**: Run real E2E tests nightly or weekly
3. **Hybrid approach**: Mock for most scenarios, real for critical paths
4. **Cassette freshness**: Re-record cassettes quarterly or when backends update

