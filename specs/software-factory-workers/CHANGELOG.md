# Changelog: Software Factory Worker Model

**Feature Release Date:** 2026-03-14

## Summary

This release implements the full software-factory worker model: canonical board states, worker registry with heartbeat/claim/lease/reclaim lifecycle, dead worker purge, task and loop enrichment, operator control room views, and factory CLI/dashboard.

## 2026-03-16 — Factory UX Polish & Reliability

### Added

- **Worker iteration tracking** — Workers now track `iterationCount` and `totalIterations` in their state, displayed in the factory dashboard for progress visibility.
- **Per-task git worktrees** — `crates/ralph-cli/src/factory.rs`: factory workers now isolate each claimed task in its own git worktree, preventing file conflicts between concurrent workers.
- **Git status RPC** — `git.status` RPC method returning working tree status for factory monitoring; `GitStatusPanel` component in the web dashboard.
- **Code-assist 3-hat preset** — Migrated `presets/code-assist.yml` from 4-hat (Planner+Builder+Critic+Finalizer) to 3-hat (Planner+Builder+Reviewer) model for faster iteration cycles.

### Fixed

- **Infinite claim loop** — `crates/ralph-cli/src/factory.rs`: workers that fail to create a worktree now break out of the claim loop instead of retrying indefinitely.
- **Double-write in complete_task** — Consolidated non-dead success path from two disk writes to one `with_exclusive_snapshot` call.

### Crates Affected

- **ralph-cli** — `factory.rs` (worktree isolation, claim loop fix), `presets/code-assist.yml` (3-hat refactor)
- **ralph-api** — `git_status` RPC method, `task_domain.rs` (event audit trail)
- **frontend** — `GitStatusPanel`, factory dashboard iteration display

### Suggested AGENTS.md Updates

- **Per-task worktree isolation**: Factory workers create a git worktree per claimed task. When modifying `factory.rs`, ensure worktree cleanup happens on both success and failure paths.
- **3-hat preset model**: The code-assist preset now uses Planner+Builder+Reviewer instead of 4 hats. When creating new presets, prefer the 3-hat model as the baseline.
- **Claim loop safety**: Any retry loop in worker task claiming must have a bounded exit condition to prevent infinite loops on persistent failures.

---

## 2026-03-16 — Task Event Audit Trail

### Added

- **TaskEvent struct** — `crates/ralph-api/src/task_domain.rs`: new `TaskEvent` struct with `timestamp`, `event_type`, `worker_id`, and `details` fields. Builder methods `with_worker()` and `with_details()` for ergonomic construction.
- **Events field on TaskRecord** — `events: Vec<TaskEvent>` appended on every state transition: `created`, `status_changed`, `claimed`, `completed`, `failed`, `reclaimed`, `retried`, `cancelled`.
- **Worker show event trail** — `crates/ralph-cli/src/worker_cli.rs`: `ralph worker show <id>` now displays per-task event history filtered to that worker, showing timestamp, event type, and transition details.
- **4 new tests** — `crates/ralph-api/tests/worker_domain.rs`: `task_events_on_create`, `task_events_on_claim_and_complete`, `task_events_on_reclaim`, `task_events_on_failure`.

### Changed

- `complete_task` non-dead success path consolidated from double-write (close + snapshot) to single `with_exclusive_snapshot` call, eliminating redundant disk I/O.

### Crates Affected

- **ralph-api** — `task_domain.rs` (TaskEvent struct, events on all transitions), `worker_domain.rs` (events on claim/complete/fail/reclaim)
- **ralph-cli** — `worker_cli.rs` (worker show event display)

### Suggested AGENTS.md Updates

- **Task event audit trail**: All task state transitions now emit `TaskEvent` entries. When adding new task transitions, always append a `TaskEvent` with the appropriate `event_type` and `from -> to` details.
- **Single-write pattern**: Prefer `with_exclusive_snapshot` over calling a domain method followed by a separate snapshot when you need to append extra data. This avoids double-writes to disk.

## 2026-03-16 — Documentation & Finalization

### Added

- REST API deprecation admonitions for legacy statuses and removed endpoints in `docs/api/rest-api.md`.
- Two-task-system distinction and board-state cross-reference in `docs/advanced/task-system.md`.
- `ralph worker` and `ralph factory` CLI commands in `docs/guide/cli-reference.md`.
- `--worker` flag documented in `ralph run` options.
- Worker CLI (`ralph worker list/show/deregister/reclaim/summary`) in `crates/ralph-cli/src/worker_cli.rs`.
- Factory spawner (`ralph factory start`) in `crates/ralph-cli/src/factory.rs`.
- Factory dashboard page with `FactoryStats`, `FactoryTaskBoard`, and `WorkerCard` components.

### Changed

- Fixed `worker.*` family method count from 7 to 8 in `crates/ralph-api/README.md` (added `complete_task`).
- CLAUDE.md updated with worker domain code locations, `.ralph/workers.json` key file, lock ordering invariant, and factory worker usage.

### Suggested AGENTS.md Updates

- **Lock ordering invariant**: Worker→task lock order must be documented as a codebase convention. Any code touching both `.ralph/workers.json` and `.ralph/api/tasks-v1.json` must acquire worker lock first.
- **Enrichment pattern**: `enrich_task()`/`enrich_loop()` in dispatch resolve cross-domain data at the response layer. Future RPC methods returning tasks or loops should use these helpers.
- **File ownership pattern**: `crates/ralph-api/src/file_ownership.rs` provides lock-backed `read_json`/`write_json` helpers. New domain files should use this instead of raw fs operations.
- **Worker lifecycle**: Workers register → heartbeat → claim → complete → deregister. Dead workers are auto-purged after 7 minutes. Only idle heartbeats can revive dead workers.
- **Factory mode**: `ralph factory start -n <count>` spawns N worker loops. Workers claim tasks atomically from the shared pool.

## 2026-03-15 — Worker Registry, Enrichment, Board Views

### Added

- **Worker registry** (`worker_domain`) — New `crates/ralph-api/src/worker_domain.rs` backed by `.ralph/workers.json` with lock-backed register/list/get/deregister semantics, explicit duplicate/missing-worker errors, and cross-handle freshness.
- **Worker assignment and lease fields** — Tasks now persist `assigneeWorkerId`, `claimedAt`, and `leaseExpiresAt` through the full RPC round-trip with field-specific `INVALID_PARAMS` validation on malformed values.
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

- `worker_domain` read paths (`list`, `get`) reload from disk under shared lock instead of serving from handle-local cache.
- Utilization metrics exclude dead workers; `board.metrics.summary` reports `aliveWorkers`/`deadWorkers` instead of `totalWorkers`.
- Dead workers cannot be revived by `busy` heartbeats — only `idle` heartbeats can revive a dead worker (allows the factory loop's idle heartbeat to restore a worker that finished its task after being marked dead).

## 2026-03-14 — Slice 1 Finalization

### Added

- Canonical board-state semantics for `ralph-api` tasks: `backlog`, `ready`, `in_progress`, `in_review`, `blocked`, `done`, and `cancelled`.
- Integration coverage for the board-state lifecycle and error-handling guardrails in `crates/ralph-api/tests/rpc_v1_task_loop.rs`, including:
  - `ready -> in_progress -> cancelled -> ready -> done`
  - blocker unblocking when the dependency reaches `done`
  - `412 PRECONDITION_FAILED` assertions for invalid `task.cancel` and `task.retry` calls
- MCP and capability coverage that enforces the trimmed task surface and canonical status vocabulary.

### Changed

- `crates/ralph-api/src/task_domain.rs` now treats `done` and `cancelled` as the only terminal states, defaults `task.create` to `ready`, filters `task.ready` on `ready`, unblocks dependents on `done`, and enforces:
  - `task.cancel`: `in_progress -> cancelled`
  - `task.retry`: `cancelled -> ready`
  - `task.delete`: only `done | cancelled`
- Removed queue-runner artifacts from the `ralph-api` contract:
  - deleted queue-only fields such as `autoExecute` and `queuedTaskId`
  - removed `task.run`, `task.run_all`, and `task.status` from the advertised RPC surface
  - trimmed schema, MCP tool catalog wording, and stream side effects to match the surviving task API
- `crates/ralph-api/src/loop_domain.rs` `loop.trigger_merge_task` now creates merge tasks directly in `ready` and returns `{ success, taskId }` without queue-era metadata.
- `crates/ralph-api/README.md` now documents the canonical board-state contract instead of legacy `open/pending/running/closed/failed` semantics.

### Crates Affected

- **ralph-api** — task domain, loop merge-task flow, protocol/schema/runtime dispatch, MCP surface, README, and integration tests.
- **ralph-cli** — test-only workspace-root isolation used to restore the repo-wide verification gate after ambient `RALPH_WORKSPACE_ROOT` leakage surfaced during final QA.

## Verification

- ✅ `cargo test -p ralph-api`
- ✅ `cargo test -p ralph-core --features recording --test smoke_runner`
- ✅ `cargo test`

## Suggested Doc Updates

1. `docs/api/rest-api.md` still documents legacy task statuses (`open`, `running`, `closed`, `failed`, `pending`), `autoExecute`, `queuedTaskId`, and `POST /api/v1/tasks/:id/run`; either align it with the trimmed canonical task model or mark that task section as legacy-only to avoid contract drift.
2. `docs/advanced/task-system.md` still describes only the runtime task lifecycle (`Created / In Progress / Completed / Blocked`). Add a note that this is distinct from the `ralph-api` control-plane board states, or add a cross-reference to the `ralph-api` board-state contract so future worker-model slices do not reintroduce the old queue vocabulary.
3. Any docs that consume `system.capabilities` or MCP task tools should explicitly note that `task.run`, `task.run_all`, and `task.status` are gone, while `task.ready`, `task.cancel`, and `task.retry` are the supported queue-facing verbs.
