# Idea Honing

Requirements clarification for the Kanban-style tracking UX feature.

## Q1: What does a "workstream" map to in Ralph terms?

When you say "track workstreams (loops)" on a Kanban board, what's the primary unit of work you want as a card? Some options:

- **A Task** (`task.rs`) — individual work items with status (Open → InProgress → Closed/Failed)
- **A Loop** (an orchestration run) — a `ralph run` invocation with its iterations, active hat, and termination state
- **Both** — tasks as cards within a loop-scoped column/swimlane, so you see the full picture

Which of these (or something else) is the right mental model?

**A1:** Both. Tasks as cards, loops as the containing workstream. The Kanban board should show tasks within the context of their loop, giving visibility into both the individual work items and the overall orchestration run.

---

## Q2: What are the Kanban columns?

For the board layout, what should the columns represent? A few options:

- **Task status columns** — Open | In Progress | Closed | Failed (the natural `TaskStatus` states)
- **Hat/phase columns** — Planner | Builder | Reviewer | Finalizer (reflecting which hat is active)
- **Custom/configurable columns** — user-defined stages that map to tags or metadata

Task status columns seem like the most natural fit since they already exist as first-class state. Hat phases could be shown as metadata on each card rather than as columns. What's your thinking?

**A2:** A mix. There should be a backlog column for work tasks and/or specs that haven't been picked up yet. Hat phase columns should be dynamic — they change depending on which hat collection the loop uses (e.g., a 4-hat planner/builder/reviewer/finalizer collection vs. a single-hat code-assist). The system should support tabs or metadata to allow custom columns in the future.

---

## Q3: Scope — is this a new page in the existing web dashboard, or an API-only concern?

Are we designing:

- **API surface only** — ensure the RPC API exposes enough structured data for *any* frontend (the existing React dashboard, an external tool, or a future one) to build a Kanban view
- **API + a new page in the React dashboard** — build the actual Kanban UI in `frontend/ralph-web/`
- **API + abstract UI contract** — define the API and a UI spec, but defer the actual frontend implementation

Given the rough idea says "exposed such that one could build," it sounds like the primary goal is the API surface. But should we also spec out a concrete Kanban page for the existing dashboard?

**A3:** API surface only. The goal is to ensure the RPC API exposes enough structured data for any frontend to build a Kanban view. No concrete UI implementation in this scope.

---

## Q4: What data should a task card expose?

When a frontend renders a task as a Kanban card, what information should be available from the API? Here's what `Task` currently has:

- id, title, description, status, priority, tags, blocked_by, created_at, updated_at
- key (idempotent key), loop_id (owning loop)

Beyond what's already there, should a task card also surface:

- **Active hat** — which hat is currently working on it (if in progress)
- **Loop context** — iteration count, cost, runtime of the parent loop
- **Spec linkage** — if the task came from a spec, a reference back to it
- **History/activity** — recent status transitions or events related to this task

What's essential vs. nice-to-have?

**A4:** All of the suggested additions are desired:
- Active hat (which hat is working on the task)
- Loop context (iteration count, cost, runtime of parent loop)
- Spec linkage (reference back to originating spec)
- History/activity (recent status transitions or events)

These should all be available through the API alongside the existing Task fields.

---

## Q5: How should loops be represented alongside tasks?

A loop is the container for tasks. For the Kanban API, how should loop data be structured relative to tasks?

- **Swimlanes** — each loop is a horizontal lane, tasks are cards within it. The API would need a "board" endpoint that returns loops with their tasks nested.
- **Flat with filtering** — tasks are a flat list, each tagged with `loop_id`. The frontend filters/groups by loop. The API just needs good filtering on `task.list`.
- **Hierarchical** — a dedicated endpoint like `board.get` that returns a structured response: loops → hats (columns) → tasks (cards), pre-organized for rendering.

The hierarchical approach is the most opinionated but gives frontends the least work. The flat approach is the most flexible. What feels right?

**A5:** Flat with filtering for now. Tasks are a flat list, each tagged with `loop_id`. The frontend filters/groups as needed. Swimlane/hierarchical endpoints can be added later as a convenience layer on top.

---

## Q6: Real-time updates — should the API support streaming/push for board state changes?

The existing `stream.subscribe` WebSocket endpoint already pushes events. For a Kanban board to feel responsive, should we:

- **Leverage existing stream** — add task and loop state-change events to the existing `stream.subscribe` so frontends get push updates (task status changed, loop iteration advanced, hat switched, etc.)
- **Polling only** — frontends poll `task.list` and `loop.list` on an interval. Simpler but less responsive.
- **Both** — polling as baseline, streaming as enhancement

The existing stream infrastructure seems like the natural fit. Is real-time important for this, or is polling sufficient for v1?

**A6:** Leverage existing streaming. Add task and loop state-change events to the existing `stream.subscribe` WebSocket endpoint so frontends get real-time push updates (task status changes, loop iteration advances, hat switches, etc.).

---

## Q7: Backlog — where do unpicked tasks/specs come from?

You mentioned a backlog column for work tasks and/or specs. Currently:

- **Tasks** are created via `task.create` (API), `ralph task add` (CLI), or by agents during loops
- **Specs** live as markdown files in `.ralph/specs/` — they're not currently first-class API objects

For the backlog to work, should we:

- **Expose specs as API objects** — add `spec.list` / `spec.get` methods that read from `.ralph/specs/` and return structured metadata (name, status, path, linked tasks)
- **Keep specs as file-only** — backlog is just tasks with status `Open` that aren't assigned to a loop yet. Specs stay out of the API.
- **Specs as a special task type** — when a spec is created, auto-generate a corresponding task so it shows up in the backlog naturally

Which approach?

**A7:** No backlog for now. Keep specs as file-only and don't add backlog complexity. The focus is on exposing task and loop primitives for active workstreams. Backlog can be added later.

---

## Q8: Hat collection awareness — how much hat metadata should the API expose?

You mentioned that hat phase columns should be dynamic based on which collection the loop uses. Currently `loop.status` doesn't return the hat collection or active hat info in a structured way.

Should the API expose:

- **Hat collection for each loop** — which hats are configured, their names, descriptions, subscribe/publish topics
- **Active hat per loop** — which hat is currently executing in the loop's latest iteration
- **Hat transition history** — sequence of hat activations across iterations (e.g., Planner → Builder → Builder → Reviewer)

All three would let a frontend dynamically build columns from the hat collection and show where each task/loop currently sits. Is all three the right scope, or should we trim?

**A8:** No hat-based columns. Tasks are cards, columns are task statuses. Each card shows which hat is currently working on it as metadata (badge/tag). Hat info is a property of the card, not a structural element of the board. The API enriches task responses with hat context rather than needing hat-column orchestration.

---

## Q9: Are the current task statuses sufficient for Kanban columns?

Current `TaskStatus` values: **Open**, **InProgress**, **Closed**, **Failed**

That gives us 4 columns. Some potential gaps for a Kanban flow:

- **Blocked** — task has unmet `blocked_by` dependencies. Currently this is implicit (Open + non-empty `blocked_by`), not a distinct status.
- **In Review** — task is being reviewed (maps to the Reviewer hat phase). Currently just shows as InProgress.
- **Queued** — task is ready to run but waiting for a loop to pick it up. Currently just Open.

Options:
1. **Keep 4 statuses** — use tags or metadata to distinguish sub-states (e.g., an Open task with `blocked_by` renders differently)
2. **Add statuses** — promote Blocked, Queued, or InReview to first-class statuses
3. **Hybrid** — keep the 4 core statuses but add a `sub_status` or `phase` field for finer granularity

What feels right for the Kanban use case?

**A9:** Add three new statuses: **Blocked**, **Queued**, and **InReview**. This gives us 7 columns total: Open, Queued, Blocked, InProgress, InReview, Closed, Failed.

---

## Q10: Should task status transitions be enforced?

With 7 statuses, there are valid and invalid transitions. For example:

- Open → Queued → InProgress → InReview → Closed (happy path)
- InProgress → Blocked → InProgress (dependency stall)
- Any → Failed (can fail from anywhere)

Should the API:

- **Enforce a state machine** — reject invalid transitions (e.g., can't go from Open directly to InReview)
- **Loose enforcement** — allow any transition, let the frontend/agent decide what makes sense
- **Warn but allow** — log/return a warning on unusual transitions but don't block them

Given Ralph's tenet of "agents are smart, let them do the work," loose enforcement might be more aligned. But a state machine prevents bugs. What's your preference?

**A10:** Loose enforcement. Allow any transition — let the agent/frontend decide what makes sense. Consistent with the Ralph tenet of not over-prescribing.

---

## Q11: What loop metadata should be available per-task?

You said loop context (iteration count, cost, runtime) should be surfaced on task cards. Currently `task.get` returns the raw `Task` struct. To enrich it, the API would need to join task data with loop data.

Should this be:

- **Inline on task responses** — `task.list` and `task.get` include a `loop_context` field with iteration, cost, runtime, active hat, termination reason (if finished)
- **Separate lookup** — task responses just have `loop_id`, frontend calls `loop.status` separately to get loop details
- **Both** — a `task.list` parameter like `include_loop_context=true` to opt into the enriched response

The inline approach is most convenient for frontends but adds cost to every task query. The opt-in parameter keeps it flexible. Preference?

**A11:** Inline on task responses. `task.list` and `task.get` should include a `loop_context` field with iteration count, cost, runtime, active hat, and termination reason (if finished).

---

## Q12: Task history/activity — what granularity?

You said history/activity should be available on task cards. What level of detail:

- **Status transitions only** — a list of `{from_status, to_status, timestamp, hat}` entries. Lightweight, easy to store.
- **Full event trail** — every event related to this task (status changes, hat assignments, iteration references, agent output snippets). Richer but heavier.
- **Summary** — just the last N events or a condensed timeline (e.g., "Created → Queued by Planner → InProgress by Builder (iter 3) → InReview (iter 5)")

Status transitions feel like the right balance — enough to show movement on the board without bloating responses. What do you think?

**A12:** After investigating the presets (especially code-assist.yml), tasks are clearly multi-iteration. A single task bounces between Builder → Critic → Builder → Critic before landing. Status transition history is valuable. Go with lightweight status transition tracking: `{from_status, to_status, timestamp, hat}` entries per task.

---

## Q13: Filtering and grouping — what query capabilities does `task.list` need?

For a Kanban frontend to be useful, `task.list` needs good filtering. What filters should be available?

- **By status** — e.g., "all InProgress tasks" (essential for column rendering)
- **By loop_id** — "all tasks for this loop" (essential for workstream scoping)
- **By priority** — sort/filter by priority within columns
- **By tags** — filter by user-defined tags
- **By active hat** — "all tasks currently with the Builder"
- **By date range** — created/updated within a window

Which of these are must-haves vs. can-wait?

**A13:** Must-haves: by status, by loop_id, by active hat. Nice-to-have (cheap to add since fields exist): by priority, by tags. Can wait: by date range.

---

## Q14: Does anything need to change in the existing `loop.list` / `loop.status` methods?

Currently `loop.list` returns `LoopEntry` (id, status, worktree_path, branch, prompt, started_at) and `loop.status` returns loop state. For the Kanban use case, a frontend needs to:

1. List all loops (to show workstream tabs/filters)
2. Get loop details including hat collection, iteration count, cost, active hat

Is the current `loop.list` / `loop.status` response sufficient, or does it need enrichment? Specifically:

- **Hat collection** — which hats are configured for this loop (so the frontend knows what hat badges are possible)
- **Active hat** — which hat is currently executing
- **Task counts** — how many tasks in each status for this loop (so the frontend can show summary badges without fetching all tasks)

Should these be added to the loop response?

**A14:** Yes, add all three to loop responses: hat collection (configured hats for the loop), active hat (currently executing), and task counts (per-status counts for the loop).

---

## Q15: Streaming events — what specific event types should be added to `stream.subscribe`?

The existing stream already pushes some events. For a Kanban board to stay in sync, the frontend needs to know when:

- A task's status changes
- A task is created or deleted
- A loop starts, stops, or changes state
- The active hat changes (iteration boundary)

I'd propose these stream event types:

- `task.status_changed` — `{task_id, from_status, to_status, hat, loop_id}`
- `task.created` / `task.deleted` — `{task_id, loop_id}`
- `loop.state_changed` — `{loop_id, state, active_hat, iteration}`
- `loop.started` / `loop.completed` — `{loop_id, termination_reason?}`

Does this cover what you'd need, or is anything missing?

**A15:** Looks good. Stream event types: `task.status_changed`, `task.created`, `task.deleted`, `loop.state_changed`, `loop.started`, `loop.completed`.

---
