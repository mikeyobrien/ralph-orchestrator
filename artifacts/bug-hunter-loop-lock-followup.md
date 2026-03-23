# Scope

Follow-up wave on the stale `loop.lock` family discovered in wave 1.

Inspected:

- `crates/ralph-api/src/loop_domain.rs`
- `crates/ralph-cli/src/loops.rs`
- `crates/ralph-core/src/merge_queue.rs`

# Findings

## P1: The same stale `loop.lock` PID-reuse bug exists in `ralph-api` loop listing and force-stop paths

- Impacted files:
  - `crates/ralph-api/src/loop_domain.rs`
- Why it is a bug:
  - The API loop domain trusts on-disk lock metadata directly in `list()` and `stop()`, even though the same file only represents an active lock when `LoopLock::is_locked()` still succeeds.
- Exact evidence:
  - `list()` shows the primary loop when `read_existing()` returns metadata and `is_pid_alive(metadata.pid)` is true: `crates/ralph-api/src/loop_domain.rs:107-118`.
  - `stop()` force-kills the PID from `read_existing()` without first verifying the lock is still held: `crates/ralph-api/src/loop_domain.rs:298-320`.
  - The same file already uses the safer `LoopLock::is_locked()` pattern in `status()`: `crates/ralph-api/src/loop_domain.rs:182-188`.
- Triggering scenario:
  - A primary loop exits and leaves stale metadata behind.
  - The OS later reuses that PID.
  - `loop.list` may report a fake running primary loop, and `loop.stop(force=true)` can target the unrelated process.
- Likely impact:
  - Wrong-process termination and inaccurate API loop state.
- Recommended fix direction:
  - Mirror the CLI safe pattern: check `LoopLock::is_locked()` before trusting `read_existing()` in API list/stop paths.
- Confidence:
  - High.
- Whether current tests cover it:
  - No stale-metadata regression was visible in the inspected files.

# Duplicate-Family Coverage Notes

- `crates/ralph-core/src/merge_queue.rs`
  - `merge_button_state()` also trusts `LoopLock::read_existing()` plus PID liveness without verifying that the lock is still held: `crates/ralph-core/src/merge_queue.rs:583-595`.
  - I am treating that as the same stale-lock family rather than a separate finding.
- `crates/ralph-cli/src/loops.rs`
  - `list_loops()` already uses `LoopLock::is_locked()` first: `crates/ralph-cli/src/loops.rs:282-300`.
  - That safe pattern strengthens confidence that the API path is the outlier, not the intended behavior.

# Remaining Blind Spots

- I did not inspect every API consumer of loop-stop semantics.
- I did not run live PID-reuse reproduction.

# Recommended Next Search

- Audit other `LoopLock::read_existing()` callers for lock-held verification, but treat any matches as duplicates of this same defect family unless the user-facing impact differs materially.
