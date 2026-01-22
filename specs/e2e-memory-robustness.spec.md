# E2E Memory Test Robustness

## Problem Statement

The E2E memory tests are flaky due to workspace path resolution issues. Ralph's `CoreConfig.workspace_root` defaults to `current_dir()` at startup, but the E2E executor runs tests in isolated workspaces. This causes memory injection to fail silently because Ralph looks for `.agent/memories.md` in the wrong location.

## Root Cause Analysis

1. **`workspace_root` is `#[serde(skip)]`** — Cannot be set via YAML config
2. **E2E executor doesn't pass workspace** — Sets `current_dir()` but Ralph ignores it
3. **Silent failures** — `prepend_memories()` returns early without agent-visible errors
4. **Loose assertions** — Tests accept empty files as valid

## Solution

Pass workspace root via environment variable from E2E executor to Ralph.

## Implementation

### Phase 1: Environment Variable Support

**File:** `crates/ralph-core/src/config.rs`

Add environment variable reading to `CoreConfig::default()`:

```rust
impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            scratchpad: default_scratchpad(),
            specs_dir: default_specs_dir(),
            guardrails: default_guardrails(),
            workspace_root: std::env::var("RALPH_WORKSPACE_ROOT")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))),
        }
    }
}
```

### Phase 2: E2E Executor Integration

**File:** `crates/ralph-e2e/src/executor.rs`

In `run_with_timeout()`, add the environment variable:

```rust
.env("RALPH_WORKSPACE_ROOT", &self.workspace)
```

This should be added alongside the existing `.env("RALPH_DIAGNOSTICS", "1")` call.

### Phase 3: Tighten Memory Test Assertions

**File:** `crates/ralph-e2e/src/scenarios/memory.rs`

Update `MemoryAddScenario::verify()` to reject empty files:

```rust
// After checking file exists
let content = std::fs::read_to_string(&memories_path).unwrap_or_default();
if content.trim().is_empty() {
    return VerificationResult::fail("Memory file exists but is empty");
}
```

Update `MemoryInjectionScenario::verify()` to pre-verify the memory file:

```rust
// At start of verify()
let memories_path = workspace.join(".agent/memories.md");
if !memories_path.exists() {
    return VerificationResult::fail(format!(
        "Memory file not found at {:?} - injection cannot work",
        memories_path
    ));
}
```

## Acceptance Criteria

1. `RALPH_WORKSPACE_ROOT` env var is read by `CoreConfig::default()`
2. E2E executor passes `RALPH_WORKSPACE_ROOT` pointing to test workspace
3. `memory-injection` test passes consistently (no flakiness)
4. `memory-add` test rejects empty memory files
5. All 21 E2E tests pass: `cargo run -p ralph-e2e -- claude --skip-analysis`
6. Existing unit tests pass: `cargo test`

## Test Plan

```bash
# Unit tests still pass
cargo test

# E2E tests pass consistently
cargo run -p ralph-e2e -- claude --skip-analysis

# Run memory-injection specifically 3 times to verify no flakiness
for i in 1 2 3; do
  cargo run -p ralph-e2e -- claude --scenario memory-injection --skip-analysis
done
```

## Files Changed

- `crates/ralph-core/src/config.rs` — Read `RALPH_WORKSPACE_ROOT` env var
- `crates/ralph-e2e/src/executor.rs` — Pass workspace root to Ralph
- `crates/ralph-e2e/src/scenarios/memory.rs` — Tighten assertions

## Non-Goals

- Adding CLI flag for workspace root (env var is sufficient for E2E)
- Changing memory file format
- Adding new test scenarios
