# Changelog: Software Factory Worker Model

**Feature Release Date:** 2026-03-14

## Summary

This release lands the first real software-factory worker-model slice in `ralph-api`: task records now use canonical board states instead of the old queue-runner lifecycle, the RPC/MCP/schema surface no longer advertises queue-only task operations, and merge-task creation now produces `ready` work for later worker-assignment slices.

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
