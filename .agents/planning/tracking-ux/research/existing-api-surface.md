# Research: Existing API Surface Audit

## Summary

Audited the existing `ralph-api` RPC methods, data models, and streaming infrastructure to identify what already exists vs. what needs to be built for the Kanban tracking UX.

## Existing Task API (`task_domain.rs`)

### TaskRecord (API-level model)
```
id, title, status, priority, blocked_by, archived_at, queued_task_id,
merge_loop_prompt, created_at, updated_at, completed_at, error_message
```

### TaskListParams
- `status: Option<String>` — filter by single status ✅
- `include_archived: Option<bool>` — archive filter ✅

### Available Methods
- `task.list` — with status filter ✅
- `task.get` — by ID ✅
- `task.ready` — unblocked open tasks ✅
- `task.create` — with auto_execute option ✅
- `task.update` — title, status, priority, blocked_by ✅
- `task.close` — transition to closed ✅
- `task.archive` / `task.unarchive` ✅
- `task.delete` — only failed/closed ✅
- `task.clear` — delete all ✅
- `task.run` / `task.run_all` — queue for execution ✅
- `task.retry` — failed → open → run ✅
- `task.cancel` — pending/running → failed ✅
- `task.status` — queue position, runner PID ✅

### Gaps Identified

1. **No `loop_id` on TaskRecord** — The API-level `TaskRecord` doesn't include `loop_id`. The core `Task` struct has it, but the API model doesn't expose it.
2. **No `loop_id` filter on `task.list`** — Can't filter tasks by loop.
3. **No `active_hat` filter on `task.list`** — Can't filter by which hat is working on a task.
4. **No active hat field on TaskRecord** — No way to know which hat is currently working on a task.
5. **No loop context on TaskRecord** — No iteration count, cost, runtime, or hat collection info.
6. **No status transition history** — No record of status changes over time.
7. **Missing statuses** — Core `TaskStatus` only has Open, InProgress, Closed, Failed. Need: Queued, Blocked, InReview.
8. **No `tags` on TaskRecord** — Core `Task` doesn't have tags either (contrary to what I assumed earlier — need to verify).
9. **No `description` on TaskRecord** — Core `Task` has it, API model doesn't.
10. **Status is a free-form string** — API uses string statuses ("open", "pending", "running", "closed", "failed") rather than the core enum. "pending" and "running" are queue-related statuses that don't map to `TaskStatus`.

## Existing Loop API (`loop_domain.rs`)

### LoopRecord (API-level model)
```
id, status, location, prompt, merge_commit
```

### Available Methods
- `loop.list` — with `include_terminal` filter ✅
- `loop.status` — running/interval/last_processed ✅
- `loop.process` — process merge queue ✅
- `loop.prune` — clean stale loops ✅
- `loop.retry` — retry needs-review loop ✅
- `loop.discard` — discard loop ✅
- `loop.stop` — graceful/force stop ✅
- `loop.merge` — merge completed loop ✅
- `loop.merge_button_state` — UI merge state ✅
- `loop.trigger_merge_task` — create merge task ✅

### Gaps Identified

1. **No hat collection on LoopRecord** — Can't know which hats are configured for a loop.
2. **No active hat on LoopRecord** — Can't know which hat is currently executing.
3. **No iteration count on LoopRecord** — No iteration progress info.
4. **No cost/runtime on LoopRecord** — No cost or runtime tracking exposed.
5. **No task counts on LoopRecord** — No per-status task counts for the loop.

## Existing RPC State (json_rpc.rs)

The TUI already gets rich state via `RpcState`:
```
iteration, max_iterations, hat, hat_display, backend, completed,
started_at, iteration_started_at, task_counts (total/open/closed/ready),
active_task (id/title/status), total_cost_usd
```

And `RpcIterationInfo`:
```
iteration, hat, backend, duration_ms, cost_usd, loop_complete_triggered, content
```

**Key insight**: The data we need (active hat, iteration, cost, task counts) already exists in `RpcState` — it's just not exposed through the REST/RPC API. It flows through the TUI's WebSocket connection via JSON-RPC events.

## Existing Stream Infrastructure

### Already Supported Topics
- `task.status.changed` ✅ — published on task.create/update/close/cancel/retry/run
- `loop.status.changed` ✅ — topic exists but not actively published
- `loop.merge.progress` ✅ — published on merge/retry/discard

### Current `task.status.changed` Payload
```json
{ "from": "none|unknown", "to": "<status>" }
```

### Gaps Identified

1. **`task.status.changed` payload is sparse** — `from` is "none" or "unknown", not the actual previous status. No hat info, no loop_id.
2. **No `task.created` / `task.deleted` events** — task.create publishes a status.changed, but there's no dedicated creation/deletion event.
3. **`loop.status.changed` not actively published** — topic exists but no code publishes to it during loop lifecycle (iteration advances, hat changes, etc.).
4. **No `loop.started` / `loop.completed` events** — these would need to be added.

## Core Task Model vs. API Model Divergence

| Field | Core `Task` | API `TaskRecord` | Gap |
|-------|-------------|-------------------|-----|
| id | ✅ | ✅ | — |
| title | ✅ | ✅ | — |
| description | ✅ | ❌ | Missing from API |
| key | ✅ | ❌ | Missing from API |
| status | enum (4 values) | string (free-form) | Different representations |
| priority | ✅ | ✅ | — |
| blocked_by | Vec<String> | Option<String> | Different types |
| loop_id | ✅ | ❌ | Missing from API |
| tags | ❌ | ❌ | Neither has it |
| created/created_at | ✅ | ✅ | Different field names |
| started | ✅ | ❌ | Missing from API |
| closed/completed_at | ✅ | ✅ | Different field names |
| active_hat | ❌ | ❌ | Neither has it |
| loop_context | ❌ | ❌ | Neither has it |
| status_history | ❌ | ❌ | Neither has it |

## Two Task Systems

Important discovery: there are **two separate task systems**:

1. **Core tasks** (`ralph-core/src/task.rs` + `task_store.rs`) — JSONL-based, used by the orchestration loop, stored in `.ralph/agent/tasks.jsonl`
2. **API tasks** (`ralph-api/src/task_domain.rs` + `storage.rs`) — JSON-based, used by the web dashboard, stored in `.ralph/api/tasks-v1.json`

These are independent stores with different schemas. The API `TaskDomain` does NOT read from the core task store. This is a significant architectural consideration — we need to decide which task system the Kanban API builds on, or whether they need to be unified/bridged.
