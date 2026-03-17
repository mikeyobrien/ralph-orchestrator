# Software Factory Workers — Changelog

## 2026-03-16 (worker inspect)

### Added

- **`ralph worker inspect <worker-id>`** — New CLI subcommand that resolves a worker's worktree from its `current_task_id`, lists all `.ralph/agent/**/*.md` files with contents (truncated at 50 lines), and displays `tasks.jsonl` subtasks in table format. Handles idle workers (no task), missing worktrees, and absent files gracefully.
- **6 integration tests** in `crates/ralph-cli/tests/integration_worker_inspect.rs` covering: worker not found, idle worker, missing worktree, agent file display, subtask table rendering, and long file truncation.
- **ralph-tools.md documentation** — Added `ralph worker inspect <worker-id>` to the Worker Commands reference and an "Inspecting Workers" subsection explaining agent files, subtasks, and worktree resolution.

### Crates affected

- `ralph-cli` — `worker_cli.rs` (new `Inspect` variant + `InspectArgs`, `inspect_worker()` function), `main.rs` (command wiring)
- `ralph-core` — `data/ralph-tools.md` (documentation update)

## Suggested AGENTS.md Updates

- **Code Locations table**: Add `worker_cli.rs` entry — `crates/ralph-cli/src/worker_cli.rs` now includes `inspect_worker()` for worktree agent file discovery alongside existing `list`/`show`/`deregister`/`reclaim`/`summary` commands.
- **Worker → worktree mapping convention**: Workers resolve their worktree path as `.worktrees/<current_task_id>/` relative to workspace root. This pattern is used by `inspect` and should be followed by any future commands that need to access worker-local state.

## 2026-03-16 (git status)

### Added

- **`git.status` RPC method** — Read-only method returning current branch, changed files (porcelain format), and clean/dirty flag from workspace root.
- **GitStatusPanel component** — New `frontend/ralph-web/src/components/factory/GitStatusPanel.tsx` showing branch name and file-level changes with color-coded status labels.
- **Factory page integration** — GitStatusPanel wired into FactoryPage between stats and workers sections, polling every 10 seconds.

### Crates / packages affected

- `ralph-api` — `dispatch.rs` (dispatch_git, git_status), `protocol.rs` (KNOWN_METHODS), `rpc-v1-schema.json`
- `@ralph-web/dashboard` — `trpc.ts`, `FactoryPage.tsx`, new `GitStatusPanel.tsx`

## 2026-03-16

### Added

- **Canonical board states** — Replaced legacy queue statuses (`open`, `running`, `completed`) with `backlog`, `ready`, `in_progress`, `in_review`, `blocked`, `done`, `cancelled` across task domain, RPC, schema, and tests.
- **Worker assignment and lease fields** — Tasks now persist `assigneeWorkerId`, `claimedAt`, and `leaseExpiresAt` through the full RPC round-trip with field-specific `INVALID_PARAMS` validation on malformed values.
- **Worker registry** (`worker_domain`) — New `crates/ralph-api/src/worker_domain.rs` backed by `.ralph/workers.json` with lock-backed register/list/get/deregister semantics, explicit duplicate/missing-worker errors, and cross-handle freshness.
- **Worker heartbeat** — `worker.heartbeat` refreshes `currentTaskId`, `currentHat`, `status`, and `lastHeartbeatAt` for registered workers while keeping identity fields register-only.
- **Claim-next behavior** — `worker.claim_next` lets idle workers atomically claim one `ready` task (priority + created_at ordering), persisting task ownership/lease metadata and worker busy state under the worker→task lock order.
- **Lease expiry and reclaim** — `worker.reclaim_expired` scans stale workers by heartbeat deadline, returns expired `in_progress` tasks to `ready`, clears ownership fields, marks stale workers `dead`, and records deterministic reclaim evidence in `task.error_message`. Dead workers are automatically purged from the registry after `LEASE_DURATION + DEAD_PURGE_MINUTES` (2 + 5 = 7 minutes) of inactivity.
- **Worker task completion** — `worker.complete_task` completes a claimed task (success → `done`, failure → `ready` for reclaim). Handles the race with `reclaim_expired`: on success it force-closes the task; on failure it skips the reset since reclaim already handled it.
- **Worker RPC surface** — 8 worker RPC methods (`worker.register`, `worker.deregister`, `worker.list`, `worker.get`, `worker.heartbeat`, `worker.claim_next`, `worker.reclaim_expired`, `worker.complete_task`) wired through dispatch, protocol, and schema.
- **Task enrichment** — All task RPC responses now include `isClaimed`, `isStale`, `currentLoopId`, and `currentHat` computed from worker registry at response time.
- **Loop enrichment** — `loop.list` responses now include `workerId`, `workerStatus`, `currentTaskId`, `currentHat`, and `lastHeartbeatAt` computed from worker registry at response time.
- **State transition validation** — `is_valid_transition()` enforces the spec's allowed-transitions table on `task.update`, `task.close`, `task.cancel`, and `task.retry` with `PRECONDITION_FAILED` errors including from/to context and allowed targets.
- **Task promote** — `task.promote` RPC for explicit `backlog→ready` promotion.
- **Create status validation** — `task.create` only accepts `backlog` or `ready` as initial status.
- **Review queue** — `task.submit_for_review` (`in_progress→in_review`), `task.request_changes` (`in_review→in_progress`), and `task.in_review` (query) methods with ownership preservation.
- **Operator control room** — `board.summary` RPC returning task counts by status, enriched workers, stale/blocked/review items, recent completions, and actionable recommendations.
- **Throughput metrics** — `board.metrics` RPC returning cycle time stats (avg/min/max/p50), queue age, reclaim count, and summary statistics including `aliveWorkers`/`deadWorkers` counts with utilization computed as `activeWorkers / aliveWorkers` (excluding dead workers from denominator).
- **122+ tests** across `rpc_v1_task_loop`, `rpc_v1_worker`, `worker_domain`, and existing suites — 0 regressions.

### Changed

- `task.create` defaults to `ready` status (was `open`).
- Loop merge-task creation uses `ready` instead of legacy `open`.
- `worker_domain` read paths (`list`, `get`) reload from disk under shared lock instead of serving from handle-local cache.
- Utilization metrics exclude dead workers; `board.metrics.summary` reports `aliveWorkers`/`deadWorkers` instead of `totalWorkers`.
- Dead workers cannot be revived by `busy` heartbeats — only `idle` heartbeats can revive a dead worker (allows the factory loop's idle heartbeat to restore a worker that finished its task after being marked dead).
- `event_loop_ralph` integration tests no longer mutate process-global cwd; workspace root is resolved from config.
- Documentation updated across REST API reference, task system guide, and CLI reference to reflect factory worker commands, board states, and deprecation of legacy statuses.

### Files changed

| File | Lines | Purpose |
|------|-------|---------|
| `crates/ralph-api/src/worker_domain.rs` | new | Worker registry, heartbeat, claim-next, reclaim |
| `crates/ralph-api/src/task_domain.rs` | +311 | Board states, lease fields, transitions, review, promote |
| `crates/ralph-api/src/runtime/dispatch.rs` | +505 | Worker/task/loop/board dispatch, enrichment |
| `crates/ralph-api/src/protocol.rs` | +21 | New RPC method constants |
| `crates/ralph-api/src/runtime.rs` | +21 | WorkerDomain wiring into RpcRuntime |
| `crates/ralph-api/data/rpc-v1-schema.json` | +2638 | Full schema for all new methods |
| `crates/ralph-api/tests/rpc_v1_worker.rs` | new (+547) | Worker RPC integration tests |
| `crates/ralph-api/tests/rpc_v1_task_loop.rs` | +1880 | Task/loop enrichment, transitions, review, board tests |
| `crates/ralph-api/tests/worker_domain.rs` | +541 | Worker domain unit/integration tests |
| `crates/ralph-api/README.md` | +50 | Documentation for all new RPC methods |
| `crates/ralph-core/src/event_loop/mod.rs` | fix | Workspace root resolution for event paths |
| `crates/ralph-core/tests/event_loop_ralph.rs` | fix | Removed cwd mutation from integration tests |
| `docs/api/rest-api.md` | +10 | Deprecation admonitions for legacy statuses |
| `docs/advanced/task-system.md` | +8 | Board-state cross-ref, two-task-system note |
| `docs/guide/cli-reference.md` | +37 | Worker/factory CLI commands |

## Suggested Doc Updates

- **AGENTS.md / Code Locations**: Add `git.status` dispatch to the dispatch.rs entry — `dispatch_git` handles the `git.` method prefix.
- **AGENTS.md / Architecture section**: Add `worker_domain` to the Code Locations table (`crates/ralph-api/src/worker_domain.rs` — Worker registry, heartbeat, claim/lease/reclaim).
- **AGENTS.md / Key Files**: Add `.ralph/workers.json` — Worker registry state (parallel to `.ralph/api/tasks-v1.json`).
- **New pattern: lock ordering**: Worker→task lock order is now a codebase invariant. Any future code that touches both worker and task state must acquire the worker lock first to prevent deadlocks. Worth documenting in a conventions section.
- **New pattern: enrichment helpers**: `enrich_task()` / `enrich_tasks()` / `enrich_loop()` resolve cross-domain data (worker names, lease status, task counts) at the dispatch layer. Future RPC methods returning tasks or loops should use these rather than raw domain results.
- **New pattern: snapshot-based metrics**: `board.metrics` computes from current state snapshots, not event history. If event sourcing is added later, metrics should migrate to event-derived computation for accuracy.
- **Task status lifecycle**: The canonical transition table (`is_valid_transition` in `task_domain.rs`) is now enforced at runtime. Any new status or transition must be added there, not just in tests.
- **Cache coherence caveat**: Cross-domain writes (e.g., `worker.claim_next` writing task state via its own `TaskDomain` instance) are not visible to the runtime's cached `TaskDomain` until the next disk reload. `board.summary` and `board.metrics` work around this by reloading from disk, but other methods may need the same treatment if they read task state after worker mutations.
