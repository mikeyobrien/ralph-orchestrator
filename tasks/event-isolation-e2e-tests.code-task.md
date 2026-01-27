---
status: completed
created: 2026-01-20
started: 2026-01-25
completed: 2026-01-25
---
# Task: Add E2E Integration Tests for Event Isolation

## Description
Add comprehensive end-to-end integration tests that verify the event isolation feature introduced in commit a59e3696. This feature prevents stale events from previous runs (with different configs) from polluting new runs by using unique timestamped events files and a marker file for coordination.

## Background
Issue #82 reported that stale events from previous runs remained in the shared events file and polluted new runs. Events with unrecognized topics (e.g., `archaeology.start`, `map.created` from a previous preset) caused "no subscriber" errors, leading to consecutive failure detection and loop termination.

The fix implemented:
- Move events from `.agent/` to `.ralph/` directory
- Generate unique timestamped events files per run (e.g., `.ralph/events-20260120-193202.jsonl`)
- Use `.ralph/current-events` marker file to coordinate between Ralph and `ralph emit`
- Resume mode reuses existing marker file
- Backward compatible fallback to `.ralph/events.jsonl` when no marker exists

**Current test gaps:**
- No tests verify consecutive runs get isolated events files
- No tests verify the marker file `.ralph/current-events` is created correctly
- No tests verify `ralph emit` writes to the marker-specified file
- No tests verify resume mode reuses the same events file
- Existing tests in `integration_resume.rs` still reference old `.agent/events.jsonl` paths
- No regression test for the core bug (stale event pollution)

## Technical Requirements
1. Create new integration test file `crates/ralph-cli/tests/integration_events_isolation.rs`
2. Add tests that verify marker file creation and timestamped events file generation
3. Add tests that verify `ralph emit` coordination with marker file
4. Add tests that verify resume mode preserves the events file
5. Add regression test for stale event pollution (issue #82)
6. Update existing tests in `integration_resume.rs` to use correct `.ralph/` paths
7. All tests must use `tempfile::TempDir` for isolation
8. Tests should use the existing pattern of spawning `ralph` binary via `Command`

## Dependencies
- `tempfile` crate for isolated test directories
- `std::process::Command` for spawning ralph binary
- `std::fs` for file assertions
- Existing test patterns from `integration_resume.rs` and `integration_clean.rs`

## Implementation Approach

### 1. Create new test file structure
```rust
// crates/ralph-cli/tests/integration_events_isolation.rs
use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;
```

### 2. Implement core isolation tests
- `test_fresh_run_creates_timestamped_events_file`: Verify `.ralph/events-YYYYMMDD-HHMMSS.jsonl` is created
- `test_fresh_run_creates_marker_file`: Verify `.ralph/current-events` contains correct path
- `test_consecutive_runs_get_isolated_events`: Run twice, verify different events files

### 3. Implement emit coordination tests
- `test_ralph_emit_writes_to_marker_specified_file`: Emit after run, verify correct file
- `test_ralph_emit_fallback_without_marker`: No marker, verify fallback to default

### 4. Implement resume tests
- `test_resume_uses_existing_marker_file`: Resume doesn't create new events file
- `test_resume_events_continuity`: Events from same session are preserved

### 5. Implement regression test for issue #82
- `test_stale_events_dont_pollute_new_runs`: The core bug fix verification

### 6. Update existing resume tests
- Fix paths in `integration_resume.rs` from `.agent/` to `.ralph/`

## Acceptance Criteria

1. **Fresh Run Creates Timestamped Events File**
   - Given a fresh `ralph run` command
   - When the run starts and completes
   - Then a file matching `.ralph/events-YYYYMMDD-HHMMSS.jsonl` pattern exists

2. **Fresh Run Creates Marker File**
   - Given a fresh `ralph run` command
   - When the run starts
   - Then `.ralph/current-events` exists and contains path to timestamped events file

3. **Consecutive Runs Get Isolated Events**
   - Given two consecutive `ralph run` commands
   - When both runs complete
   - Then each run has a unique events file (different timestamps)
   - And events from run 1 are NOT visible in run 2's events file

4. **Ralph Emit Uses Marker File**
   - Given an active Ralph run with marker file pointing to timestamped events
   - When `ralph emit "test.topic" "payload"` is called
   - Then the event appears in the timestamped events file (not default)

5. **Ralph Emit Fallback Without Marker**
   - Given no `.ralph/current-events` marker file exists
   - When `ralph emit "test.topic" "payload"` is called
   - Then the event is written to `.ralph/events.jsonl` (fallback)

6. **Resume Uses Existing Marker**
   - Given a previous run that created `.ralph/current-events`
   - When `ralph resume` is called
   - Then it uses the existing marker file (does not create new one)
   - And events are written to the same events file

7. **Stale Events Don't Pollute New Runs (Issue #82 Regression)**
   - Given a previous run that wrote events with topics "old.topic1", "old.topic2"
   - When a new `ralph run` starts with different config
   - Then the new run's EventLoop does NOT process the old events
   - And the new run has a fresh events file

8. **Existing Resume Tests Updated**
   - Given the tests in `integration_resume.rs`
   - When checking for events files
   - Then they check `.ralph/` directory (not `.agent/`)

9. **All Tests Pass**
   - Given the new and updated tests
   - When `cargo test -p ralph-cli` is run
   - Then all integration tests pass

## Test Cases

### Test Helper Pattern
```rust
fn create_minimal_config(temp_path: &Path) -> Result<()> {
    let config = r#"
event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 1
  max_runtime_seconds: 5

cli:
  backend: "custom"
  command: "true"

core:
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config)?;
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;
    Ok(())
}
```

### Marker File Verification Pattern
```rust
// Verify marker file exists and contains valid path
let marker_content = fs::read_to_string(temp_path.join(".ralph/current-events"))?;
let events_path = marker_content.trim();
assert!(events_path.starts_with(".ralph/events-"));
assert!(events_path.ends_with(".jsonl"));
assert!(temp_path.join(events_path).exists());
```

## Files to Modify

| File | Changes |
|------|---------|
| `crates/ralph-cli/tests/integration_events_isolation.rs` | NEW: Comprehensive event isolation tests |
| `crates/ralph-cli/tests/integration_resume.rs` | UPDATE: Fix event file paths from `.agent/` to `.ralph/` |

## Metadata
- **Complexity**: Medium
- **Labels**: Testing, Integration Tests, Events, Issue-82, Regression
- **Required Skills**: Rust, Integration Testing, Process Spawning, File I/O
- **Related Issue**: https://github.com/mikeyobrien/ralph-orchestrator/issues/82
- **Related Commit**: a59e3696 (fix(events): isolate events per run)
