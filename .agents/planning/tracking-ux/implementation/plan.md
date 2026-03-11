# Implementation Plan

## Checklist

- [ ] Step 1: Extend TaskStatus enum and Task struct in ralph-core
- [ ] Step 2: Add status transition recording to TaskStore
- [ ] Step 3: Add filtering and counting methods to TaskStore
- [ ] Step 4: Enrich LoopEntry with hat collection and runtime state
- [ ] Step 5: Enrich LoopHistory with hat and cost info
- [ ] Step 6: Replace API TaskDomain with core TaskStore wrapper
- [ ] Step 7: Add loop context enrichment to API task responses
- [ ] Step 8: Enrich API LoopDomain with hat collection, active hat, and task counts
- [ ] Step 9: Add EventDomain to ralph-api for orchestration event queries
- [ ] Step 10: Enrich stream events and add new stream topics
- [ ] Step 11: Bridge loop runner state to API stream via _internal.publish
- [ ] Step 12: Update CLI task commands for new statuses and transitions
- [ ] Step 13: End-to-end integration testing and smoke tests

## Steps

### Step 1: Extend TaskStatus enum and Task struct in ralph-core

**Objective:** Add `Blocked` and `InReview` variants to `TaskStatus`, and add `last_hat`, `transitions`, and `tags` fields to `Task`.

**Implementation guidance:**
- In `crates/ralph-core/src/task.rs`:
  - Add `Blocked` and `InReview` to the `TaskStatus` enum
  - Update `is_terminal()` — both new statuses are non-terminal
  - Add `as_str()` method if not present for serialization consistency
  - Add `StatusTransition` struct with `from`, `to`, `timestamp`, `hat` fields
  - Add `last_hat: Option<String>`, `transitions: Vec<StatusTransition>`, `tags: Vec<String>` to `Task` with appropriate serde attributes (`#[serde(default)]`, `#[serde(skip_serializing_if = ...)]`)
  - Add builder methods: `with_tags()`, `with_last_hat()`

**Test requirements:**
- Serialization round-trip for `Blocked` and `InReview` statuses
- `is_terminal()` returns false for both new statuses
- Deserialization of old Task JSON without new fields (backward compat) — `transitions` defaults to empty vec, `last_hat` defaults to None, `tags` defaults to empty vec
- `StatusTransition` serialization/deserialization

**Integration with previous work:** Foundation step — all subsequent steps depend on these types.

**Demo:** `cargo test -p ralph-core` passes. Old JSONL fixtures deserialize without error. Artifacts: test output log showing all new and existing tests green, a sample JSONL line with `"status":"blocked"` and `"status":"in_review"` round-tripping correctly.

---

### Step 2: Add status transition recording to TaskStore

**Objective:** Make all status changes in `TaskStore` record a `StatusTransition` entry on the task, capturing the from/to status, timestamp, and optional hat.

**Implementation guidance:**
- In `crates/ralph-core/src/task_store.rs`:
  - Add a `transition(&mut self, id: &str, new_status: TaskStatus, hat: Option<&str>) -> Option<&Task>` method that:
    - Reads current status
    - Pushes a `StatusTransition` to `task.transitions`
    - Updates `task.status`, `task.last_hat`
    - Updates `task.started` / `task.closed` timestamps as appropriate
  - Refactor `start()`, `close()`, `fail()`, `reopen()` to delegate to `transition()` internally
  - Add `review(&mut self, id: &str) -> Option<&Task>` — transitions to InReview
  - Add `block(&mut self, id: &str) -> Option<&Task>` — transitions to Blocked

**Test requirements:**
- `start()` records a transition from Open to InProgress
- `close()` records a transition from current status to Closed
- `review()` records a transition to InReview
- `block()` records a transition to Blocked
- `reopen()` records a transition back to Open and clears `closed` timestamp
- Multiple transitions accumulate in the `transitions` vec
- Hat is recorded when provided
- Round-trip: save tasks with transitions, reload, verify transitions persist

**Integration with previous work:** Builds on Step 1 types. All existing TaskStore tests must still pass.

**Demo:** Create a task, start it, review it, close it. Reload from disk. Verify 3 transitions recorded with correct from/to/timestamps. Artifacts: test output log, a `tasks.jsonl` snippet showing a task with 3 transition entries including hat and timestamp fields.

---

### Step 3: Add filtering and counting methods to TaskStore

**Objective:** Enable efficient querying of tasks by status, loop_id, hat, and provide per-status counts.

**Implementation guidance:**
- In `crates/ralph-core/src/task_store.rs`:
  - `filter_by_status(&self, status: TaskStatus) -> Vec<&Task>`
  - `filter_by_loop_id(&self, loop_id: &str) -> Vec<&Task>`
  - `filter_by_hat(&self, hat: &str) -> Vec<&Task>` — filters by `last_hat`
  - `filter(&self, status: Option<TaskStatus>, loop_id: Option<&str>, hat: Option<&str>, priority: Option<u8>, tag: Option<&str>) -> Vec<&Task>` — combined filter
  - `counts_by_status(&self) -> HashMap<TaskStatus, usize>`
  - `counts_by_status_for_loop(&self, loop_id: &str) -> HashMap<TaskStatus, usize>`

**Test requirements:**
- Each individual filter returns correct results
- Combined filter with multiple criteria intersects correctly
- Counts match actual task distribution
- Empty store returns empty results / zero counts
- Counts for a specific loop_id only include that loop's tasks

**Integration with previous work:** Uses Task fields from Step 1. TaskStore from Step 2.

**Demo:** Load a store with mixed tasks (different statuses, loops, hats). Verify all filter combinations return correct subsets. Verify counts match. Artifacts: test output log, a test that prints filter results and count maps to stdout showing correct filtering behavior.

---

### Step 4: Enrich LoopEntry with hat collection and runtime state

**Objective:** Persist hat collection, active hat, iteration count, cost, and termination reason on `LoopEntry` so the API can serve loop metadata without a live connection to the loop process.

**Implementation guidance:**
- In `crates/ralph-core/src/loop_registry.rs`:
  - Add `HatSummary` struct: `id`, `name`, `description`
  - Add fields to `LoopEntry`: `hat_collection: Vec<HatSummary>`, `active_hat: Option<String>`, `iteration: u32`, `total_cost_usd: f64`, `max_iterations: Option<u32>`, `termination_reason: Option<String>`
  - All new fields use `#[serde(default)]` for backward compat
  - Add `LoopRegistry::update(&mut self, id: &str, update: LoopEntryUpdate)` method for partial updates (active_hat, iteration, cost)
- In `crates/ralph-cli/src/loop_runner.rs`:
  - At loop start: register with `hat_collection` and `max_iterations`
  - Each iteration: update `active_hat`, `iteration`, `total_cost_usd`
  - At termination: update `termination_reason`

**Test requirements:**
- `LoopEntry` serialization/deserialization with new fields
- Backward compat: old `loops.json` without new fields loads correctly
- `LoopRegistry::update()` modifies only specified fields
- Hat collection persists across save/load

**Integration with previous work:** Independent of Steps 1-3. Modifies loop_runner.rs which is the CLI entry point.

**Demo:** Run a loop (or mock one in tests). Verify `loops.json` contains hat_collection, active_hat, iteration, cost. Kill and reload — data persists. Artifacts: test output log, a `loops.json` snippet showing a LoopEntry with hat_collection array, active_hat, iteration count, and total_cost_usd fields populated.

---

### Step 5: Enrich LoopHistory with hat and cost info

**Objective:** Record which hat was active and the cost per iteration in the loop history, enabling historical reconstruction of loop progress.

**Implementation guidance:**
- In `crates/ralph-core/src/loop_history.rs`:
  - Extend `HistoryEventType::IterationStarted` with `hat: String`, `hat_display: String`
  - Extend `HistoryEventType::IterationCompleted` with `cost_usd: Option<f64>`
- In `crates/ralph-cli/src/loop_runner.rs`:
  - Update history event emission to include hat and cost data

**Test requirements:**
- `HistoryEventType` serialization with new fields
- Backward compat: old history.jsonl entries without hat/cost deserialize (fields default to empty/None)
- Round-trip: write enriched history, read back, verify hat and cost present

**Integration with previous work:** Builds on Step 4 (loop_runner changes). Independent of task changes.

**Demo:** After a loop runs, `history.jsonl` entries show which hat was active per iteration and the cost. Artifacts: test output log, a `history.jsonl` snippet showing `IterationStarted` entries with `hat` and `hat_display` fields, and `IterationCompleted` entries with `cost_usd`.

---

### Step 6: Replace API TaskDomain with core TaskStore wrapper

**Objective:** Rewire `ralph-api`'s task methods to read/write the core JSONL task store instead of the separate `tasks-v1.json` store.

**Implementation guidance:**
- In `crates/ralph-api/src/task_domain.rs`:
  - Replace the internal `BTreeMap<String, TaskRecord>` + JSON persistence with `TaskStore` from ralph-core
  - `TaskDomain::new()` takes `workspace_root` and loads from `.ralph/agent/tasks.jsonl`
  - Map core `Task` → API `TaskResponse` (camelCase serialization)
  - `list()` uses `TaskStore::filter()` from Step 3
  - `get()` uses `TaskStore::get()`
  - `create()` uses `TaskStore::add()` + `TaskStore::save()`
  - `update()` uses `TaskStore::get_mut()` + `TaskStore::save()`
  - `close()` uses `TaskStore::transition()` from Step 2
  - Keep queue/execution methods (`run`, `run_all`, `cancel`, `retry`, `status`) as API-layer concerns operating on the core store
  - Remove `tasks-v1.json` storage code
- Update `crates/ralph-api/src/task_domain/storage.rs` accordingly

**Test requirements:**
- All existing API task tests pass against the new implementation
- Tasks created via CLI (`ralph tools task add`) appear in API `task.list`
- Tasks created via API appear in CLI `ralph tools task list`
- Concurrent access: API and CLI can read/write simultaneously (file locking)
- Queue/execution features still work

**Integration with previous work:** Depends on Steps 1-3 (core task model and store).

**Demo:** Start API server. Create a task via `ralph tools task add`. Call `task.list` via API — task appears. Update via API — verify via CLI. Artifacts: test output log showing API task tests pass, captured HTTP request/response pairs showing a task created via CLI appearing in API `task.list` response, and a CLI `ralph tools task list` output showing a task updated via API.

---

### Step 7: Add loop context enrichment to API task responses

**Objective:** Include inline `loop_context` on task responses with iteration count, cost, active hat, and termination reason from the parent loop.

**Implementation guidance:**
- In `crates/ralph-api/src/task_domain.rs`:
  - Add `LoopContextResponse` struct
  - When building `TaskResponse`, if `task.loop_id` is set:
    - Look up the loop in `LoopRegistry`
    - Populate `loop_context` with iteration, cost, active_hat, max_iterations, termination_reason, started
  - `TaskDomain` needs access to `LoopDomain` or `LoopRegistry` — pass as a reference or share via `RpcRuntime`
- In `crates/ralph-api/src/runtime.rs`:
  - Wire the task domain to have access to loop registry data

**Test requirements:**
- Task with valid `loop_id` gets populated `loop_context`
- Task with no `loop_id` gets `loop_context: null`
- Task with `loop_id` referencing a non-existent loop gets `loop_context: null` (not an error)
- Loop context fields match the registry entry

**Integration with previous work:** Depends on Step 4 (enriched LoopEntry) and Step 6 (API TaskDomain v2).

**Demo:** Create a task with a loop_id. Register a loop with hat/iteration/cost data. Call `task.get` — response includes `loopContext` with correct values. Artifacts: test output log, captured `task.get` JSON response showing `loopContext` object with `iteration`, `totalCostUsd`, `activeHat`, `maxIterations`, and `started` fields populated from the loop registry.

---

### Step 8: Enrich API LoopDomain with hat collection, active hat, and task counts

**Objective:** Include hat collection, active hat, and per-status task counts on loop API responses.

**Implementation guidance:**
- In `crates/ralph-api/src/loop_domain.rs`:
  - Add `HatSummaryResponse` and `TaskCountsResponse` structs
  - Update `LoopRecord` with new fields: `hat_collection`, `active_hat`, `iteration`, `total_cost_usd`, `max_iterations`, `termination_reason`, `task_counts`
  - In `list()`: read enriched `LoopEntry` from registry (Step 4), compute task counts by loading `TaskStore` and calling `counts_by_status_for_loop()`
  - `LoopDomain` needs access to `TaskStore` — pass workspace_root or share via runtime

**Test requirements:**
- `LoopRecord` includes hat_collection from registry
- Task counts are correct per-loop
- Loop with no tasks gets zero counts
- Hat collection reflects what was registered at loop start

**Integration with previous work:** Depends on Step 3 (task counts), Step 4 (enriched LoopEntry), Step 6 (API reads core store).

**Demo:** Register a loop with 3 hats. Create tasks for that loop in various statuses. Call `loop.list` — response includes hat_collection and correct task_counts breakdown. Artifacts: test output log, captured `loop.list` JSON response showing `hatCollection` array with 3 entries, and `taskCounts` object with correct per-status counts matching the created tasks.

---

### Step 9: Add EventDomain to ralph-api for orchestration event queries

**Objective:** Expose Ralph's orchestration events (pub/sub between hats) via a new `event.list` API method, enabling activity timelines and diagnostics.

**Implementation guidance:**
- Create `crates/ralph-api/src/event_domain.rs`:
  - `EventDomain` struct with `workspace_root`
  - `list(params: EventListParams) -> Vec<EventRecord>` method
  - Reads from `.ralph/events.jsonl` (current session) using `EventReader` from ralph-core
  - Supports filters: `loop_id`, `topic` (pattern match), `task_id` (payload search), `limit`, `after` (cursor)
  - `EventRecord`: `topic`, `payload`, `source_hat`, `iteration`, `timestamp`
- Register `event.list` in `crates/ralph-api/src/protocol.rs` KNOWN_METHODS
- Add dispatch in `crates/ralph-api/src/runtime/dispatch.rs`
- Wire into `RpcRuntime`

**Test requirements:**
- `event.list` returns events from JSONL file
- Topic filter matches exact and wildcard patterns
- `task_id` filter searches event payloads
- `limit` and `after` pagination works correctly
- Empty events file returns empty list
- Malformed JSONL lines are skipped gracefully

**Integration with previous work:** Independent of task/loop changes. Uses existing `EventReader` from ralph-core.

**Demo:** After a loop runs, call `event.list` — returns orchestration events with topics, payloads, hats, iterations. Filter by `topic=review.*` — returns only review events. Artifacts: test output log, captured `event.list` JSON response showing EventRecord entries with `topic`, `payload`, `sourceHat`, `iteration`, and `timestamp` fields. A second captured response showing filtered results for `topic=review.*` containing only review-related events.

---

### Step 10: Enrich stream events and add new stream topics

**Objective:** Make stream events carry enough data for a Kanban frontend to stay in sync without polling.

**Implementation guidance:**
- In `crates/ralph-api/src/protocol.rs`:
  - Add to `STREAM_TOPICS`: `"task.created"`, `"task.deleted"`, `"loop.started"`, `"loop.completed"`, `"event.published"`
- In `crates/ralph-api/src/stream_domain/rpc_side_effects.rs`:
  - Enrich `task.status.changed`: capture previous status before mutation, include `hat`, `loopId`, `taskTitle`
  - Add `task.created` event on `task.create`
  - Add `task.deleted` event on `task.delete` and `task.clear`
- In `crates/ralph-api/src/runtime/dispatch.rs`:
  - For task mutations, read the task's current state before dispatching to capture `from` status
  - Pass the before-state to `publish_rpc_side_effect`

**Test requirements:**
- `task.status.changed` payload includes actual `from` status (not "unknown")
- `task.status.changed` includes `hat`, `loopId`, `taskTitle`
- `task.created` fires on task creation with task details
- `task.deleted` fires on deletion
- New topics are accepted by `stream.subscribe`
- Existing stream consumers (TUI) are not broken by enriched payloads

**Integration with previous work:** Depends on Step 6 (API reads core store for before-state).

**Demo:** Subscribe to stream. Create a task, start it, close it. Verify `task.created`, `task.status.changed` (with correct from/to/hat), events arrive via WebSocket. Artifacts: test output log, captured WebSocket message log showing: (1) `task.created` event with task details, (2) `task.status.changed` with `from: "open"`, `to: "in_progress"`, `hat`, `loopId`, `taskTitle`, (3) `task.status.changed` with `from: "in_progress"`, `to: "closed"`.

---

### Step 11: Bridge loop runner state to API stream via _internal.publish

**Objective:** Make the loop runner publish iteration state changes and orchestration events to the API's stream domain so real-time consumers get live updates.

**Implementation guidance:**
- In `crates/ralph-cli/src/loop_runner.rs`:
  - Detect if the API server is running (check for a port file or config)
  - On iteration start: POST `_internal.publish` with `loop.status.changed` event (iteration, hat, cost)
  - On iteration end: POST `_internal.publish` with updated cost
  - On loop start: POST `loop.started` event (loopId, prompt, hatCollection)
  - On loop complete: POST `loop.completed` event (loopId, terminationReason, totalCostUsd, iterations)
  - On EventBus event publish: POST `event.published` event (topic, payload, sourceHat, iteration, loopId)
  - Use fire-and-forget async HTTP calls (don't block the loop on API failures)
  - Add an EventBus observer that forwards events to the API stream
- Consider extracting the HTTP publishing logic into a helper module for testability

**Test requirements:**
- Loop runner publishes `loop.status.changed` on each iteration (mock HTTP server)
- Loop runner publishes `loop.started` and `loop.completed` at lifecycle boundaries
- Loop runner publishes `event.published` for orchestration events
- API failures don't block or crash the loop
- When API is not running, publishing is silently skipped

**Integration with previous work:** Depends on Step 10 (new stream topics registered). Uses `_internal.publish` endpoint from ralph-api.

**Demo:** Start API server. Run a loop. Watch stream subscription — see `loop.started`, `loop.status.changed` per iteration, `event.published` for hat events, `loop.completed` at the end. Artifacts: test output log (mock HTTP server verifying calls received), captured stream event sequence showing: (1) `loop.started` with `loopId`, `prompt`, `hatCollection`, (2) multiple `loop.status.changed` with incrementing `iteration` and changing `activeHat`, (3) `event.published` entries with orchestration event topics, (4) `loop.completed` with `terminationReason` and `totalCostUsd`.

---

### Step 12: Update CLI task commands for new statuses and transitions

**Objective:** Add `review` and `block` subcommands to `ralph tools task`, and update existing commands to record hat info in transitions.

**Implementation guidance:**
- In `crates/ralph-cli/src/task_cli.rs`:
  - Add `ralph tools task review <id>` — calls `TaskStore::transition(id, InReview, hat)`
  - Add `ralph tools task block <id>` — calls `TaskStore::transition(id, Blocked, hat)`
  - Update `start`, `close`, `fail`, `reopen` to pass hat info from loop context
  - Add `--hat <hat_id>` optional flag to all transition commands
  - Auto-detect hat from `.ralph/current-hat` marker file (to be written by loop_runner, similar to `current-loop-id`)
- In `crates/ralph-cli/src/loop_runner.rs`:
  - Write `.ralph/current-hat` marker file each iteration (alongside `current-loop-id`)
- Update `crates/ralph-core/data/ralph-tools.md` with new commands (single source of truth for the skill)

**Test requirements:**
- `ralph tools task review <id>` transitions to InReview
- `ralph tools task block <id>` transitions to Blocked
- Hat is auto-detected from marker file when running inside a loop
- `--hat` flag overrides auto-detection
- Transition history records the hat
- Updated skill doc matches new commands

**Integration with previous work:** Depends on Steps 1-2 (new statuses and transition recording).

**Demo:** Create a task. Run `ralph tools task start <id>`. Run `ralph tools task review <id> --hat critic`. Run `ralph tools task close <id>`. Show task — 3 transitions with correct hats. Artifacts: test output log, captured `ralph tools task show <id> --format json` output showing the task with `status: "closed"`, `lastHat: "critic"` (or closing hat), and `transitions` array with 3 entries each containing `from`, `to`, `timestamp`, and `hat` fields. Screenshot or captured output of `ralph tools task list` showing the new `blocked` and `in_review` status columns.

---

### Step 13: End-to-end integration testing and smoke tests

**Objective:** Validate the complete Kanban API surface works end-to-end across all components.

**Implementation guidance:**
- Add integration tests in `crates/ralph-api/tests/`:
  - Full lifecycle: create task via CLI → start via CLI → query via API with loop_context → stream events arrive → close via API → verify transitions
  - Multi-loop: tasks from two loops, filter by loop_id, verify task counts per loop
  - Event query: run a loop, call `event.list`, verify orchestration events with hat/iteration data
  - Backward compat: load old JSONL fixtures, verify API serves them correctly
- Add smoke test fixtures in `crates/ralph-core/tests/fixtures/`:
  - Record a session using new task statuses (Blocked, InReview)
  - Replay and verify event parsing handles new statuses
- Update existing E2E scenarios if they touch task/loop APIs:
  - `crates/ralph-e2e/src/scenarios/tasks.rs`
  - `crates/ralph-e2e/src/scenarios/orchestration.rs`

**Test requirements:**
- All new and existing tests pass
- `cargo test --all` green
- Mock E2E tests pass: `cargo run -p ralph-e2e -- --mock`
- No regressions in TUI stream consumption

**Integration with previous work:** Depends on all previous steps.

**Demo:** Run `cargo test --all` — all green. Run mock E2E — passes. Demonstrate: start API, create tasks with various statuses, query with filters, subscribe to stream, see real-time events. Artifacts: full `cargo test --all` output log showing zero failures, `cargo run -p ralph-e2e -- --mock` output log showing all scenarios pass, captured end-to-end session log showing: (1) task created via CLI with `ralph tools task add`, (2) API `task.list` response with filters applied, (3) WebSocket stream events received in real-time during a loop, (4) `event.list` response showing orchestration events with hat/iteration data, (5) backward compat test loading old JSONL fixtures without errors.
