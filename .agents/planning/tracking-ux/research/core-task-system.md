# Research: Core Task System Deep Dive

## Summary

The API task system (`ralph-api/src/task_domain.rs`) is being deprecated. The core task system (`ralph-core/src/task.rs` + `task_store.rs`) is the source of truth. This document analyzes the core system's capabilities and gaps for the Kanban use case.

## Core Task Model

### Current Fields
```rust
pub struct Task {
    pub id: String,              // task-{timestamp}-{4hex}
    pub title: String,
    pub description: Option<String>,
    pub key: Option<String>,     // stable key for idempotent ensure
    pub status: TaskStatus,      // Open, InProgress, Closed, Failed
    pub priority: u8,            // 1-5
    pub blocked_by: Vec<String>, // dependency IDs
    pub loop_id: Option<String>, // owning loop
    pub created: String,         // ISO 8601
    pub started: Option<String>, // when entered InProgress
    pub closed: Option<String>,  // when closed/failed
}
```

### Current TaskStatus Enum
```rust
pub enum TaskStatus {
    Open,       // Not started
    InProgress, // Being worked on
    Closed,     // Complete
    Failed,     // Failed/abandoned
}
```

### TaskStore Capabilities
- JSONL persistence at `.ralph/agent/tasks.jsonl`
- File locking for multi-loop safety (shared reads, exclusive writes)
- `with_exclusive_lock()` for atomic read-modify-write
- Query: `all()`, `open()`, `ready()` (unblocked open tasks)
- Mutations: `add()`, `close()`, `start()`, `fail()`, `reopen()`, `ensure()` (idempotent by key)
- `has_open_tasks()`, `has_pending_tasks()`

### What's Missing for Kanban
1. **No `active_hat` field** — no way to know which hat is working on a task
2. **No `iteration` field** — no way to know which iteration a task was worked on
3. **No status transition history** — no record of `{from, to, timestamp, hat}`
4. **Missing statuses** — need Blocked and InReview (Queued dropped per user)
5. **No tags** — not on the model at all
6. **No filtering API** — TaskStore has `open()` and `ready()` but no filter by status/loop_id/hat
7. **No loop context enrichment** — no way to get iteration count, cost, runtime for the parent loop

## Core Loop State

### LoopEntry (loop_registry.rs)
```rust
pub struct LoopEntry {
    pub id: String,
    pub pid: u32,
    pub started: DateTime<Utc>,
    pub prompt: String,
    pub worktree_path: Option<String>,
    pub workspace: String,
}
```

Missing: hat collection, active hat, iteration count, cost, task counts.

### RpcState (json_rpc.rs) — TUI-only
```rust
pub struct RpcState {
    pub iteration: u32,
    pub max_iterations: Option<u32>,
    pub hat: String,
    pub hat_display: String,
    pub backend: String,
    pub completed: bool,
    pub started_at: u64,
    pub iteration_started_at: Option<u64>,
    pub task_counts: RpcTaskCounts,  // total, open, closed, ready
    pub active_task: Option<RpcTaskSummary>,
    pub total_cost_usd: f64,
}
```

This has exactly the data we need (active hat, iteration, cost, task counts) but it's built in `loop_runner.rs` and only flows to the TUI via JSON-RPC stdin/stdout. It's not accessible from the `ralph-api` RPC runtime.

### How RpcState is Built
In `crates/ralph-cli/src/loop_runner.rs:380-400`:
- `iteration` — from an `AtomicU32`
- `hat` / `hat_display` — from a `Mutex<(String, String)>` updated each iteration
- `total_cost_usd` — from a `Mutex<f64>`
- `completed` — from an `AtomicBool`
- `task_counts` — defaults (not populated!)
- `active_task` — None (not populated!)

Key insight: even the TUI doesn't get real task counts or active task info from `RpcState`. The TUI state (`state.rs`) tracks iteration/hat/backend from stream events, not from `RpcState`.

## How the TUI Gets Loop State

The TUI connects via WebSocket to `ralph-api` and receives stream events. Key events:

1. `task.log.line` — `{ line, iteration, hat }` — the TUI uses this to track iteration boundaries and active hat
2. `loop.status.changed` — `{ loopId, status, iteration }` — with `status: "iteration_started"` to detect new iterations

The TUI builds its own state from these events rather than polling a state endpoint.

## Stream Infrastructure

### Already Published Events
- `task.status.changed` — on task CRUD via RPC side effects, payload: `{ from: "none|unknown", to: status }`
- `loop.merge.progress` — on merge/retry/discard
- `task.log.line` — log output lines with iteration/hat metadata

### Not Published (but topic exists)
- `loop.status.changed` — topic registered but only published by the TUI bridge, not by the core event loop

## CLI Task Commands (`ralph tools task`)

These operate on the core task store directly:
- `add`, `ensure`, `list`, `ready`, `start`, `close`, `reopen`, `fail`, `show`
- Filter: `--status open|in_progress|closed` on `list`
- Format: `--format table|json|quiet`

## Key Architectural Observations

1. **Two task systems are fully independent** — API tasks (`tasks-v1.json`) and core tasks (`tasks.jsonl`) don't share data. Since API tasks are being deprecated, we build on core tasks.

2. **Loop state is ephemeral** — `RpcState` exists only in memory during a running loop. There's no persistent record of iteration count, cost, or active hat for a loop. `LoopEntry` in the registry is minimal.

3. **The bridge gap** — The `ralph-api` RPC runtime doesn't have access to the core event loop's state. It reads from files (task store, loop registry, merge queue) but can't query the running loop's iteration/hat/cost.

4. **Stream events are the real-time channel** — Both the TUI and any future Kanban frontend would need to consume stream events to get live state. The API can serve snapshots from files, but real-time comes from the stream.

5. **Core task store needs to become the API's task backend** — The `ralph-api` `TaskDomain` currently has its own JSON store. It needs to be rewired to read/write the core JSONL store, or the core store needs to be exposed through new API methods.
