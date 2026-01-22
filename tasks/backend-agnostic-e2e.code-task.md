# Task: Make E2E Scenarios Backend-Agnostic

## Status: In Progress

## Problem

Currently, E2E scenarios are tied to specific backends. Running `cargo run -p ralph-e2e -- opencode` only runs 1 test because only `opencode-connect` has `Backend::OpenCode`. The other 26 scenarios are Claude-specific.

## Solution

Make scenarios declare which backends they support (most should support all), and pass the target backend at runtime so scenarios generate the correct configuration.

## Completed Work

### 1. Backend helper methods (`crates/ralph-e2e/src/backend.rs`) ✅
Added:
- `default_timeout(&self) -> Duration` - Returns backend-specific timeout
- `default_max_iterations(&self) -> u32` - Returns backend-specific iteration limits
- `as_config_str(&self) -> &'static str` - Returns lowercase name for config files

### 2. TestScenario trait changes (`crates/ralph-e2e/src/scenarios/mod.rs`) ✅
Changed:
- Removed: `fn backend(&self) -> Backend`
- Added: `fn supported_backends(&self) -> Vec<Backend>` with default returning all backends
- Changed: `fn setup(&self, workspace: &Path)` → `fn setup(&self, workspace: &Path, backend: Backend)`

### 3. Runner updates (`crates/ralph-e2e/src/runner.rs`) ✅
- Updated `matches_config()` to use `supported_backends().contains()` instead of `backend()`
- Updated `run()` to iterate over backends when `config.backend` is None
- Scenario IDs become `{scenario}-{backend}` when running all backends
- Test `MockScenario` updated to use new trait API

### 4. Unified connectivity scenario (`crates/ralph-e2e/src/scenarios/connectivity.rs`) ✅
Created new file that replaces `claude.rs`, `kiro.rs`, and `opencode.rs` with a single backend-agnostic scenario.

## Remaining Work

### 5. Update mod.rs exports
File: `crates/ralph-e2e/src/scenarios/mod.rs`

Add connectivity module and remove old backend-specific modules:
```rust
// Remove these:
mod claude;
mod kiro;
mod opencode;

// Add this:
mod connectivity;

// Update pub use:
pub use connectivity::ConnectivityScenario;
// Remove: pub use claude::ClaudeConnectScenario;
// Remove: pub use kiro::KiroConnectScenario;
// Remove: pub use opencode::OpenCodeConnectScenario;
```

### 6. Update orchestration.rs
File: `crates/ralph-e2e/src/scenarios/orchestration.rs`

For each scenario struct (SingleIterScenario, MultiIterScenario, CompletionPromiseScenario):
1. Remove `fn backend(&self) -> Backend` method
2. Change `fn setup(&self, workspace: &Path)` to `fn setup(&self, workspace: &Path, backend: Backend)`
3. Update config generation to use `backend.as_config_str()`:
```rust
let config_content = format!(r#"cli:
  backend: {}
event_loop:
  max_iterations: {}
  completion_promise: "LOOP_COMPLETE"
"#, backend.as_config_str(), backend.default_max_iterations());
```
4. Update timeout to use `backend.default_timeout()`
5. Update test mocks and assertions that reference `.backend()`

### 7. Update events.rs
File: `crates/ralph-e2e/src/scenarios/events.rs`

Same pattern as orchestration.rs for:
- ClaudeEventsScenario → rename to EventsScenario
- ClaudeBackpressureScenario → rename to BackpressureScenario

### 8. Update capabilities.rs
File: `crates/ralph-e2e/src/scenarios/capabilities.rs`

Same pattern for:
- ClaudeToolUseScenario → rename to ToolUseScenario
- ClaudeStreamingScenario → rename to StreamingScenario

### 9. Update hats.rs
File: `crates/ralph-e2e/src/scenarios/hats.rs`

Same pattern for:
- HatSingleScenario
- HatMultiWorkflowScenario
- HatInstructionsScenario
- HatEventRoutingScenario
- HatBackendOverrideScenario

### 10. Update memory.rs
File: `crates/ralph-e2e/src/scenarios/memory.rs`

Same pattern for all memory scenarios.

### 11. Update errors.rs
File: `crates/ralph-e2e/src/scenarios/errors.rs`

Same pattern for:
- TimeoutScenario
- MaxIterationsScenario
- AuthFailureScenario
- BackendUnavailableScenario

### 12. Update main.rs scenario registration
File: `crates/ralph-e2e/src/main.rs`

Replace backend-specific scenarios with generic ones:
```rust
fn get_all_scenarios() -> Vec<Box<dyn TestScenario>> {
    vec![
        // Tier 1: Connectivity (single scenario, works for all backends)
        Box::new(ConnectivityScenario::new()),
        // Tier 2: Orchestration
        Box::new(SingleIterScenario::new()),
        Box::new(MultiIterScenario::new()),
        Box::new(CompletionPromiseScenario::new()),
        // ... etc
    ]
}
```

### 13. Update lib.rs exports
File: `crates/ralph-e2e/src/lib.rs`

Update public exports to use new scenario names.

### 14. Delete old files
Remove:
- `crates/ralph-e2e/src/scenarios/claude.rs`
- `crates/ralph-e2e/src/scenarios/kiro.rs`
- `crates/ralph-e2e/src/scenarios/opencode.rs`

## Code Pattern for Updating Scenarios

For each scenario, apply this transformation:

**Before:**
```rust
fn backend(&self) -> Backend {
    Backend::Claude
}

fn setup(&self, workspace: &Path) -> Result<ScenarioConfig, ScenarioError> {
    let config_content = r#"cli:
  backend: claude
event_loop:
  max_iterations: 5
"#;
    // ...
    Ok(ScenarioConfig {
        timeout: Duration::from_secs(600),
        // ...
    })
}
```

**After:**
```rust
// Remove backend() method entirely - uses default supported_backends()

fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
    let config_content = format!(r#"cli:
  backend: {}
event_loop:
  max_iterations: {}
"#, backend.as_config_str(), backend.default_max_iterations());
    // ...
    Ok(ScenarioConfig {
        timeout: backend.default_timeout(),
        // ...
    })
}
```

## Test Updates

For each scenario's test module, update:
1. Remove `test_*_backend()` tests that check `scenario.backend() == Backend::X`
2. Add `test_*_supported_backends()` that checks all backends are supported
3. Update `setup()` calls to include backend parameter: `scenario.setup(&workspace, Backend::Claude)`

## Verification

After all changes:
```bash
cargo build -p ralph-e2e           # Compiles without errors
cargo test -p ralph-e2e            # All unit tests pass
cargo run -p ralph-e2e -- --list   # Shows scenarios without backend prefixes
cargo run -p ralph-e2e -- opencode # Runs ~24 scenarios instead of 1
cargo run -p ralph-e2e -- claude   # Still runs ~24 scenarios
cargo run -p ralph-e2e -- all      # Runs ~24 scenarios × 3 backends
```

## Acceptance Criteria

- [ ] All scenarios implement new `setup(&self, workspace: &Path, backend: Backend)` signature
- [ ] No scenarios have `fn backend(&self) -> Backend` method
- [ ] `cargo build -p ralph-e2e` succeeds
- [ ] `cargo test -p ralph-e2e` passes
- [ ] `cargo run -p ralph-e2e -- --list` shows ~24 scenarios (not 27 with backend prefixes)
- [ ] `cargo run -p ralph-e2e -- opencode` runs all supported scenarios
- [ ] `cargo run -p ralph-e2e -- all` runs scenarios for each available backend
