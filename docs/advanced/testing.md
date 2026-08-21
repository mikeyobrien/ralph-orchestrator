# Testing & Validation

Comprehensive testing approaches for Ralph development and validation.

## Test Types

| Type | Purpose | Speed | Cost |
|------|---------|-------|------|
| Unit/Integration Tests | Exercise Rust crates and runtime entry points | Fast | Free |
| Legacy E2E Inventory | Retained cassette/live-backend scenarios; not a v3 GA gate | Varies | Varies |
| TUI Validation | Verify terminal rendering | Medium | Free |

## Running Tests

### All Tests

```bash
cargo test
```

This runs the workspace's current unit, integration, and documentation tests.
For the focused CLI/core gate:

```bash
cargo test -p ralph-cli -p ralph-core
```

## Legacy E2E Inventory

The `ralph-e2e` binary and cassette scenarios target the deleted in-house
orchestration loop. They remain useful as an inventory while replacement v3
coverage is tracked in `.ralph/specs/v3-ga-readiness.spec.md`, but they are not
a v3 GA regression gate. List the scenarios without running backends:

```bash
cargo run -p ralph-e2e -- --list
```

The CLI still accepts live-backend and cassette modes. Running them can be
useful while porting a scenario, but a failing legacy scenario does not by
itself describe the autoloop-backed runtime:

```bash
# Legacy live-backend scenarios (may incur API cost)
cargo run -p ralph-e2e -- claude

# Legacy cassette suite; currently not expected to be the v3 gate
cargo run -p ralph-e2e -- --mock
```

## E2E Tests

The retained harness groups scenarios by backend and capability.

### Test Tiers

| Tier | Focus | Scenarios |
|------|-------|-----------|
| 1 | Connectivity | Backend availability, auth |
| 2 | Orchestration | Single/multi iteration |
| 3 | Events | Parsing, routing |
| 4 | Capabilities | Tool use, streaming |
| 5 | Hat Collections | Workflows, routing |
| 6 | Memory | Add, search, inject |
| 7 | Error Handling | Timeout, limits |

### Running E2E Tests

```bash
# All tests for Claude
cargo run -p ralph-e2e -- claude

# All available backends
cargo run -p ralph-e2e -- all

# Fast mode (skip analysis)
cargo run -p ralph-e2e -- claude --skip-analysis

# Debug mode (keep workspaces)
cargo run -p ralph-e2e -- claude --keep-workspace --verbose
```

### E2E Reports

Generated in `.e2e-tests/`:

```
.e2e-tests/
├── report.md      # Human-readable Markdown
├── report.json    # Machine-readable JSON
└── claude-connect/  # Test workspace (with --keep-workspace)
```

### E2E Orchestration

For E2E test development, use isolated config:

```bash
# E2E test development
ralph run -c ralph.e2e.yml -p "fix e2e tests"
```

This retained development config uses a separate Ralph scratchpad path. That
path is coordination state; autoloop still owns engine iteration state and
completion.

## TUI Validation

Validate Terminal UI rendering using LLM-as-judge.

### Quick Start

```bash
# Validate from captured output
/tui-validate file:output.txt criteria:ralph-header

# Validate live TUI via tmux
/tui-validate tmux:ralph-session criteria:ralph-full

# Custom criteria
/tui-validate command:"cargo run --example tui" criteria:"Shows header"
```

### Built-in Criteria

| Criteria | Validates |
|----------|-----------|
| `ralph-header` | Iteration count, elapsed time, hat display |
| `ralph-footer` | Activity indicator, event topic |
| `ralph-full` | Complete layout and hierarchy |
| `tui-basic` | Has content, no artifacts |

### Live TUI Capture

```bash
# 1. Start TUI in tmux
tmux new-session -d -s ralph-test -x 100 -y 30
tmux send-keys -t ralph-test "ralph run -p 'test'" Enter

# 2. Wait for render
sleep 3

# 3. Capture
tmux capture-pane -t ralph-test -p -e > tui-capture.txt

# 4. Validate
/tui-validate file:tui-capture.txt criteria:ralph-header
```

### Prerequisites

```bash
brew install charmbracelet/tap/freeze  # Screenshot tool
brew install tmux                       # Live capture
```

## Linting

```bash
# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-targets --all-features
```

## Pre-commit Hooks

Install hooks from a normal clone (the script expects `.git` to be a directory):

```bash
bash ./scripts/setup-hooks.sh
```

Hooks run CI-parity Rust checks before each commit:

- `./scripts/sync-embedded-files.sh check`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

## Testing Human-in-the-Loop (Telegram)

Telegram relay is inactive for autoloop-backed `ralph run` pending
autoloop#345. Test the retained crate with mocked bot behavior:

```bash
cargo test -p ralph-telegram
```

See the [Telegram guide](../guide/telegram.md) for the exact v3 limitation.

## Testing Best Practices

### 1. Run Tests After Changes

```bash
cargo test  # Always run before declaring done
```

### 2. Prefer Runtime Integration Tests

Exercise real v3 entry points and autoloop adapter contracts rather than
source-only assertions or old replay fixtures.

### 3. Treat Legacy E2E as Inventory

Use the GA-readiness R-matrix to choose replacement coverage. Do not make the
legacy cassette suite a v3 gate.

### 4. Validate TUI Changes

After modifying `ralph-tui`, use TUI validation.

### 5. Keep Documented Gates Current

When runtime contracts change, update the focused crate tests and the
GA-readiness R-matrix.

## Creating New Tests

### Unit Test

```rust
#[test]
fn test_event_parsing() {
    let input = r#"ralph emit "build.done" "tests pass""#;
    let event = parse_event(input).unwrap();
    assert_eq!(event.topic, "build.done");
}
```

### Integration Test

Add a Rust integration test under the owning crate's `tests/` directory and
exercise the real autoloop-backed entry point or adapter contract.

### Legacy E2E Scenario

```rust
pub struct MyScenario;

impl E2EScenario for MyScenario {
    fn name(&self) -> &str { "my-scenario" }
    fn tier(&self) -> u8 { 3 }

    async fn run(&self, ctx: &E2EContext) -> E2EResult {
        // Test implementation
    }
}
```

## Next Steps

- Explore [Diagnostics](diagnostics.md) for debugging
- Learn about [Architecture](architecture.md)
- See the [Contributing Guide](../contributing/index.md)
