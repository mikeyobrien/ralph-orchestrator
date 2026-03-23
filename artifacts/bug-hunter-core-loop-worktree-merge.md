## Scope

- `crates/ralph-core/src/event_loop/*`
- `crates/ralph-core/src/worktree.rs`
- `crates/ralph-core/src/merge_queue.rs`
- `crates/ralph-core/src/loop_registry.rs`
- `crates/ralph-core/src/loop_lock.rs`
- `crates/ralph-cli/src/loops.rs`
- `crates/ralph-cli/src/loop_runner.rs`

Priority bug classes covered:

- concurrency, races, file locking, stale state, cancellation
- merge correctness, duplicate merge / lost merge, cleanup and orphan worktrees
- persistence/log replay correctness in `.ralph` JSON/JSONL files
- retry/idempotency/exactly-once assumptions
- startup/shutdown behavior and partial failure handling
- tests-versus-implementation drift

Coverage summary:

- Highest-risk merge-queue lifecycle paths were exhausted: enqueue, process, child startup, terminal transitions, discard, retry, and CLI merge/process entrypoints.
- Worktree loop lifecycle was cross-checked from spawn in `main.rs` through stop/discard/logs handling in `loops.rs`.
- Event-loop stop/restart/current-events handling was checked for mismatches with CLI consumers.
- `worktree.rs`, `loop_registry.rs`, and `loop_lock.rs` were inspected for stale-state and locking defects. No additional P0-P2 defects were found there beyond the consumer bugs below.

## Report file

- `artifacts/bug-hunter-core-loop-worktree-merge.md`

## Findings

### P1: Pending merge entries can spawn duplicate merge workers, and the second worker does not abort

Impacted files:

- `crates/ralph-cli/src/loop_runner.rs`
- `crates/ralph-core/src/merge_queue.rs`

Why this is a bug:

- `process_pending_merges_with_command()` enumerates every `Queued` entry and spawns `ralph run ...` for each one without first reserving the queue item.
- The queue entry is not moved to `Merging` until child startup in `run_loop_impl()`.
- If `process_pending_merges_with_command()` is invoked twice before the first child updates the queue, both invocations will spawn a merge worker for the same loop.
- Even worse, a merge child that loses the race to `mark_merging()` does not exit. It logs `InvalidTransition` and continues running the merge loop anyway.

Exact evidence:

- `crates/ralph-cli/src/loop_runner.rs:4612-4753` lists queued entries and spawns children, but never marks them reserved before spawn.
- `crates/ralph-cli/src/loop_runner.rs:648-662` treats `MergeQueueError::InvalidTransition` as a debug-only condition and keeps executing.
- `crates/ralph-core/src/merge_queue.rs:243-264` only updates state when `mark_merging()` is called later by the child.

Minimal triggering scenario:

1. A worktree loop completes and enqueues `loop-X`.
2. Two callers run queue processing close together, for example primary-loop completion plus `ralph loops process`, or two manual `ralph loops process` invocations.
3. Both read `loop-X` as `Queued` and both spawn `ralph run ... RALPH_MERGE_LOOP_ID=loop-X`.
4. The first child appends `Merging`.
5. The second child gets `InvalidTransition` at startup but keeps running the merge workflow anyway.

Likely impact:

- Duplicate merge execution against the same branch.
- Competing `mark_merged` / `mark_needs_review` writes.
- Double side effects in merge hooks or merge-automation steps.

Recommended fix direction:

- Reserve queue items before spawn under one queue lock, or add a `claim_pending()` API that atomically transitions `Queued -> Merging(pid)` in the parent.
- Treat `InvalidTransition` on merge-loop startup as fatal for that child and exit immediately.

Current tests cover it:

- No. Existing tests around `process_pending_merges_with_command()` only cover missing command handling and log redirection, not duplicate processing or concurrent invocation (`crates/ralph-cli/src/loop_runner.rs:9212-9315`).

Confidence:

- High

### P1: Merge queue state transitions are non-atomic and can resurrect discarded or terminal entries

Impacted files:

- `crates/ralph-core/src/merge_queue.rs`

Why this is a bug:

- Every state transition does `get_entry()` under a shared read lock, releases that lock, and then appends the new event under a separate exclusive lock.
- That is a classic TOCTOU gap. Another process can change state between validation and append.
- Because state is replayed from append order, the later append wins even if it was validated against stale state.

Exact evidence:

- `crates/ralph-core/src/merge_queue.rs:243-264` reads state via `get_entry()` and only later appends `Merging`.
- `crates/ralph-core/src/merge_queue.rs:272-295`, `303-326`, and `334-357` use the same split read-then-append pattern for `Merged`, `NeedsReview`, and `Discarded`.
- `crates/ralph-core/src/merge_queue.rs:359-367` and `368+` derive current state solely from event order, so a stale late append can overwrite a newer terminal state.

Minimal triggering scenario:

1. Process A begins `mark_merging(loop-X)` and reads `Queued`.
2. Before A appends, process B discards the same loop and appends `Discarded`.
3. A appends `Merging` based on stale validation.
4. Replay now ends at `Merging`, reviving a loop the user explicitly discarded.

Likely impact:

- Lost discard decisions.
- Merged or review-needed entries reverting to non-terminal states.
- Duplicate or out-of-order merge execution after supposedly terminal decisions.

Recommended fix direction:

- Make each transition an atomic read-modify-append operation under one exclusive lock.
- Recheck the latest state inside the exclusive-lock section immediately before appending.

Current tests cover it:

- No. The merge-queue tests are all single-threaded happy-path transition tests (`crates/ralph-core/src/merge_queue.rs:785-1044`).

Confidence:

- High

### P1: `ralph loops stop <worktree-loop>` cannot stop a live worktree loop

Impacted files:

- `crates/ralph-cli/src/loops.rs`
- `crates/ralph-cli/src/main.rs`

Why this is a bug:

- Active worktree loops are explicitly created without a loop lock.
- `stop_loop()` still tries to read `.ralph/loop.lock` from the worktree directory for non-orphan worktree loops.
- Since no such lock exists, stopping a live worktree loop fails before it can send a signal or write `stop-requested`.

Exact evidence:

- Worktree loops are created with `lock_guard = None`; the comment explicitly says they do not hold the primary lock in `crates/ralph-cli/src/main.rs:1655-1669`.
- `stop_loop()` resolves a worktree path and then unconditionally requires `LoopLock::read_existing(&target_root)` for the non-orphan case in `crates/ralph-cli/src/loops.rs:747-857`.

Minimal triggering scenario:

1. Start a parallel loop so it runs in `.worktrees/<loop-id>`.
2. While it is still running and its worktree directory still exists, run `ralph loops stop <loop-id>`.
3. `stop_loop()` looks for `<worktree>/.ralph/loop.lock`, finds nothing, and returns `Cannot determine active loop`.

Likely impact:

- Live worktree loops cannot be stopped cleanly from the supported CLI.
- Users are forced into manual process killing or waiting for orphan cleanup after external worktree deletion.

Recommended fix direction:

- For worktree loops, use the registry PID as the authoritative live-process handle.
- Write `stop-requested` into the worktree directly when the registry says the loop is alive.

Current tests cover it:

- No. Existing stop tests cover only the primary loop lock path and the orphan-worktree fallback path (`crates/ralph-cli/src/loops.rs:1755-1805`).

Confidence:

- High

### P2: `ralph loops discard` can delete a running worktree and deregister it before the process stops

Impacted files:

- `crates/ralph-cli/src/loops.rs`

Why this is a bug:

- `discard_loop()` does not check whether the target loop is still running.
- It discards the queue entry, removes the registry entry, and force-removes the worktree immediately.
- For a live worktree loop, that destroys the filesystem and tracking metadata before the process has actually exited.

Exact evidence:

- `crates/ralph-cli/src/loops.rs:675-699` discards queue state, deregisters from the registry, and then force-removes the worktree.
- There is no `is_alive()` or PID-stop step in that path.

Minimal triggering scenario:

1. Start a live worktree loop.
2. Run `ralph loops discard <loop-id> -y` before it finishes.
3. The CLI removes registry visibility and deletes the worktree while the child process may still be executing.

Likely impact:

- Orphaned running loop with no registry entry.
- Abrupt `WorkspaceGone` termination on the next iteration boundary.
- Lost auditability and confusing cleanup state if the process keeps running briefly after discard.

Recommended fix direction:

- Reject discard for live loops unless `--force` is provided, or perform `stop` first and wait for exit before deregistering/removing the worktree.

Current tests cover it:

- No. Discard tests only cover non-running entries and missing-worktree cleanup (`crates/ralph-cli/src/loops.rs:1612-1749`).

Confidence:

- High

### P2: `ralph loops logs` reads the obsolete default events path and misses the active run log

Impacted files:

- `crates/ralph-cli/src/loops.rs`
- `crates/ralph-cli/src/loop_runner.rs`
- `crates/ralph-core/src/event_loop/mod.rs`

Why this is a bug:

- Fresh runs now write a timestamped events file and store its path in `.ralph/current-events`.
- `show_logs()` still hardcodes `.ralph/events.jsonl`.
- On a normal fresh run, `ralph loops logs` will miss the active events file and fall back to history or a false “No events file found” error.

Exact evidence:

- `crates/ralph-cli/src/loop_runner.rs:186-197` writes `.ralph/current-events` pointing at `.ralph/events-<timestamp>.jsonl`.
- `crates/ralph-core/src/event_loop/mod.rs:374-379` reads that marker to follow the active events file.
- `crates/ralph-cli/src/loops.rs:544-566` ignores the marker and only checks `.ralph/events.jsonl`.

Minimal triggering scenario:

1. Start any fresh loop.
2. The run writes `.ralph/current-events = .ralph/events-YYYYMMDD-HHMMSS.jsonl`.
3. Run `ralph loops logs <id>`.
4. The command checks only `.ralph/events.jsonl`, misses the real log, and falls back incorrectly.

Likely impact:

- Broken observability for active loops.
- Users see stale history instead of current events, or a misleading “may have crashed” error.

Recommended fix direction:

- Resolve the active events file through `.ralph/current-events`, matching the event loop and TUI logic.

Current tests cover it:

- No. The only adjacent test asserts history fallback, which encodes the old assumption instead of the new marker-based behavior (`crates/ralph-cli/src/loops.rs:1538-1551`).

Confidence:

- High

## Evidence

Files inspected directly:

- `crates/ralph-core/src/event_loop/mod.rs`
- `crates/ralph-core/src/event_loop/tests.rs`
- `crates/ralph-core/src/worktree.rs`
- `crates/ralph-core/src/merge_queue.rs`
- `crates/ralph-core/src/loop_registry.rs`
- `crates/ralph-core/src/loop_lock.rs`
- `crates/ralph-core/src/loop_completion.rs`
- `crates/ralph-cli/src/loops.rs`
- `crates/ralph-cli/src/loop_runner.rs`
- `crates/ralph-cli/src/main.rs`

Focused existing tests checked for drift/gaps:

- `crates/ralph-cli/src/loops.rs:1538-1551` only tests history fallback for logs, not marker-based events.
- `crates/ralph-cli/src/loops.rs:1755-1805` only tests primary stop and orphan-stop fallback, not live worktree stop.
- `crates/ralph-cli/src/loop_runner.rs:9212-9315` only tests queue-processing happy paths and subprocess logging, not duplicate queue processors.
- `crates/ralph-core/src/merge_queue.rs:785-1044` tests only single-threaded queue transitions.

No-finding coverage notes:

- `event_loop/mod.rs`: stop/restart/workspace-gone termination checks and marker-based event-file selection look internally consistent. The material defect is in the CLI consumer that still assumes the legacy path.
- `loop_registry.rs`: file updates are protected by one exclusive lock and zombie worktree retention is intentional. I did not find a separate P0-P2 in registry persistence within this lane.
- `loop_lock.rs`: primary lock acquisition and metadata persistence are coherent for the primary-loop use case. The material issue is misuse by worktree-loop stop handling, not the lock primitive itself.
- `worktree.rs`: create/list/remove/sync flows look sound for their intended single-operation semantics. I did not find an additional P0-P2 race there beyond the CLI deleting live worktrees without first stopping their processes.

Remaining blind spots:

- I did not inspect the merge-loop preset contents or agent instructions that actually perform `git merge`, only the orchestration code that spawns and tracks those runs.
- I did not inspect web/API consumers outside this lane that may also assume `.ralph/events.jsonl`.

Residual-risk rationale:

- The highest-risk correctness surfaces in this scope are the queue state machine and the CLI lifecycle commands around running worktrees. Those are where the confirmed defects landed.
- The remaining scoped files mostly hold lower-level helpers whose locking and persistence behavior looked coherent once called correctly.

## Areas inspected

- Merge queue lifecycle: enqueue, mark-merging, mark-merged, mark-needs-review, discard, list/replay.
- Merge queue processing: primary-loop completion hook, manual `loops process`, merge-child startup.
- Worktree loop lifecycle: spawn in `main.rs`, registry registration, stop, discard, logs.
- Event-loop termination and current-events marker handling.
- Registry stale cleanup and zombie retention behavior.
- Loop lock acquisition, metadata read/write, and consumer assumptions.

## Recommended next search

- Inspect the merge-loop preset and merge agent instructions for idempotency and branch-base assumptions, especially around repeated `git merge`, conflict recovery, and worktree cleanup after partial success.
- Sweep non-CLI consumers for the same legacy `.ralph/events.jsonl` assumption.
- Add concurrency tests around `MergeQueue` transitions and duplicate `process_pending_merges` invocations before attempting fixes.
