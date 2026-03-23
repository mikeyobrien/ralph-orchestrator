# Scope

Scoped lane `core-merge-locking`, limited to:

- `crates/ralph-core/src/worktree.rs`
- `crates/ralph-core/src/merge_queue.rs`
- `crates/ralph-core/src/loop_lock.rs`
- `crates/ralph-cli/src/loops.rs`

# Findings

## P1: Stale `loop.lock` metadata can make `ralph loops stop --force` kill an unrelated process

- Impacted files:
  - `crates/ralph-core/src/loop_lock.rs`
  - `crates/ralph-cli/src/loops.rs`
- Why it is a bug:
  - The lock metadata file survives after the flock is released. `stop_loop()` trusts that file directly and force-kills its PID without first verifying that the lock is still held.
- Exact evidence:
  - `LockGuard` drops the lock but leaves metadata on disk: `crates/ralph-core/src/loop_lock.rs:71-78`.
  - `LoopLock::read_existing()` returns metadata whenever the file exists: `crates/ralph-core/src/loop_lock.rs:247-264`.
  - `stop_loop()` reads metadata and force-kills that PID: `crates/ralph-cli/src/loops.rs:814-840`.
  - A safer pattern already exists in `list_loops()`, which checks `LoopLock::is_locked()` before trusting metadata: `crates/ralph-cli/src/loops.rs:282-300`.
- Triggering scenario:
  - A loop exits cleanly and releases the flock, but `.ralph/loop.lock` metadata remains.
  - Later, the old PID is reused by an unrelated process.
  - `ralph loops stop --force` reads the stale metadata and sends `SIGKILL` to the reused PID.
- Likely impact:
  - Wrong-process termination on a local developer machine or host running multiple Ralph-related processes.
- Recommended fix direction:
  - Require `LoopLock::is_locked()` before trusting metadata for stop operations, or remove/rotate metadata on unlock.
- Confidence:
  - High.
- Whether current tests cover it:
  - No test in the inspected files proves stop-path safety against stale metadata or PID reuse.

## P2: A crashed merge worker can wedge a loop in `Merging` indefinitely

- Impacted files:
  - `crates/ralph-core/src/merge_queue.rs`
  - `crates/ralph-cli/src/loops.rs`
- Why it is a bug:
  - The queue records a merge PID but never revalidates it. Entries stuck in `Merging` are ignored by normal retry and pending-selection flows.
- Exact evidence:
  - `mark_merging()` records a PID: `crates/ralph-core/src/merge_queue.rs:243-268`.
  - `derive_state()` replays the stored merge PID but never checks liveness: `crates/ralph-core/src/merge_queue.rs:414-456`.
  - `next_pending()` only returns `Queued` entries: `crates/ralph-core/src/merge_queue.rs:362-365`.
  - `retry_merge()` only accepts `NeedsReview`: `crates/ralph-cli/src/loops.rs:635-649`.
  - `prune_stale()` in the inspected CLI file does not repair merge-queue state.
- Triggering scenario:
  - A merge worker crashes after `mark_merging()` but before `mark_needs_review()` or `mark_merged()`.
  - The queue entry remains `Merging`.
  - It no longer appears in `next_pending()` and cannot be recovered through normal retry.
- Likely impact:
  - Stuck merge queues and manual operator intervention for otherwise recoverable failures.
- Recommended fix direction:
  - Revalidate `merge_pid` when listing/advancing merge entries and demote dead `Merging` entries into `NeedsReview` or another recoverable state.
- Confidence:
  - High.
- Whether current tests cover it:
  - Existing tests cover normal queue transitions, but not crash-after-`mark_merging` recovery.

# No-Finding Coverage Notes

- `crates/ralph-core/src/worktree.rs`
  - Checked worktree creation, removal, sync, and orphan cleanup helpers.
  - No separate P0-P2 defect confirmed in the inspected worktree file beyond adjacent stale-metadata family risk.
- `crates/ralph-cli/src/loops.rs`
  - Checked retry, discard, merge, stop, and list behavior.
  - The critical issues were confined to stale lock trust and stuck merge states.

# Remaining Blind Spots

- This lane did not inspect the merge-loop runner implementation that emits merge-queue transitions.
- This lane did not inspect other `LoopLock::read_existing()` consumers outside the requested files.

# Recommended Next Search

- Inspect the merge-loop runner to confirm where `mark_merging`, `mark_merged`, and `mark_needs_review` are emitted, then add a crash-recovery regression test.
- Audit other `LoopLock::read_existing()` consumers for the same stale-file/PID-reuse assumption.
