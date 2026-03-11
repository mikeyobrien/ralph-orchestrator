# Research: Ambiguities and Architectural Gaps

## 1. How Loop State Reaches the API

**Current flow:**
```
EventLoop (ralph-core) → loop_runner.rs (ralph-cli) → RpcEvent channel → rpc_stdin.rs → stdout JSON-RPC → TUI WebSocket
```

The loop runner emits `RpcEvent::IterationStart` (with hat, iteration, backend) and `RpcEvent::IterationEnd` to a channel. These flow to the TUI via JSON-RPC stdout. The `ralph-api` server is a separate process that reads files — it has no direct connection to the running loop's in-memory state.

**The bridge mechanism exists but is unused:**
- `_internal.publish` RPC method exists in `ralph-api` dispatch — it can inject events into the stream domain
- Nothing currently calls it
- The loop runner could call it to push iteration/hat/cost events into the API's stream

**Resolution needed:** The loop runner needs to publish state changes to the API's stream domain, either via `_internal.publish` HTTP calls or by writing to a shared file that the API watches.

## 2. Loop ID Assignment

**Current behavior:**
- Primary loops: `loop_id()` returns `None` from `LoopContext`, but `loop_runner.rs` generates one and writes it to `.ralph/current-loop-id`
- Worktree loops: `loop_id()` returns the worktree loop ID
- Tasks read `current-loop-id` marker file via `read_current_loop_id()` in `task_cli.rs`

**Implication:** Tasks DO get tagged with `loop_id` — both primary and worktree loops. The marker file mechanism works. No gap here.

## 3. Hat Tracking on Tasks

**Current state:** No hat information is stored on tasks at all. The event loop knows which hat is active (`determine_active_hat_ids()`), but this is never written to the task.

**Where hat info lives:**
- `EventLoop.state.last_active_hat_ids` — in-memory only, per iteration
- `RpcEvent::IterationStart { hat, hat_display }` — emitted to RPC channel
- `LoopHistory` — records `IterationStarted { iteration }` and `IterationCompleted { iteration, success }` but NOT which hat was active

**Resolution needed:** When a task transitions status (start, close, fail, reopen), the active hat should be recorded. Options:
- Add `last_hat: Option<String>` to `Task` struct
- Record hat in status transition history entries

## 4. Iteration History Exists but Lacks Hat Info

`LoopHistory` in `.ralph/history.jsonl` records:
- `LoopStarted { prompt }`
- `IterationStarted { iteration }`
- `IterationCompleted { iteration, success }`
- `LoopCompleted { reason }`

Missing: which hat was active per iteration, cost per iteration, events published. This means we can't reconstruct "Builder worked on task X in iteration 3" from history alone.

**Resolution needed:** Either enrich `HistoryEventType::IterationStarted` with hat info, or track hat on the task itself.

## 5. Two Task Stores — Migration Path

**API TaskDomain** (`tasks-v1.json`):
- Has its own CRUD, queue system, auto-execute
- Statuses: free-form strings ("open", "pending", "running", "closed", "failed")
- No `loop_id`, no `description`, no `key`

**Core TaskStore** (`tasks.jsonl`):
- Used by orchestration loop and `ralph tools task` CLI
- Statuses: enum (Open, InProgress, Closed, Failed)
- Has `loop_id`, `description`, `key`, `blocked_by` as Vec

**Migration approach:** Replace `TaskDomain` in `ralph-api` with a thin wrapper around `TaskStore`. The API methods (`task.list`, `task.get`, etc.) would read/write the core JSONL store. This unifies the two systems.

**Complications:**
- API TaskDomain has queue/execution features (`run`, `run_all`, `cancel`, `status`) that core TaskStore doesn't
- API uses camelCase JSON; core uses snake_case
- API has `archived_at`; core doesn't have archival
- Need to decide if queue/execution features move to core or stay as API-layer concerns

## 6. Stream Event Enrichment

**Current `task.status.changed` payload:**
```json
{ "from": "none", "to": "open" }  // on create
{ "from": "unknown", "to": "closed" }  // on update/close
```

The `from` field is always "none" or "unknown" — the actual previous status is never captured. This is because `rpc_side_effects.rs` only looks at the result, not the before-state.

**Resolution needed:** Capture the previous status before mutation and include it in the stream event. Also add `hat`, `loop_id`, and `task_id` to the payload.

## 7. Hat Collection per Loop

**Current state:** The hat collection (which hats are configured) is defined in `ralph.yml` and loaded at loop startup. It's not persisted per-loop — `LoopEntry` in the registry doesn't record which config was used.

**Resolution needed:** Either:
- Store the hat collection (names, descriptions, subscribe/publish topics) in `LoopEntry` at registration time
- Or store the config file path and let the API read it

The first approach is more self-contained. The hat collection is small (typically 2-5 hats) so storage cost is minimal.

## Summary of Resolutions Needed

| Gap | Proposed Resolution | Complexity |
|-----|-------------------|------------|
| API task store → core task store | Replace TaskDomain with TaskStore wrapper | Medium |
| New statuses (Blocked, InReview) | Add to TaskStatus enum | Low |
| Hat tracking on tasks | Add `last_hat: Option<String>` to Task | Low |
| Status transition history | Add `transitions: Vec<StatusTransition>` to Task | Low-Medium |
| Loop context on task responses | Join task data with loop registry + history | Medium |
| Hat collection on loop responses | Store hat collection in LoopEntry | Low |
| Active hat on loop responses | Read from RpcState or history | Medium |
| Enriched stream events | Capture before-state, add hat/loop_id | Low-Medium |
| Loop state → API bridge | Loop runner publishes to API stream | Medium |
