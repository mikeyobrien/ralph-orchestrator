# Task: Kanban Tracking UX — API Surface

## Objective

Extend Ralph Orchestrator's API surface so any frontend can build a Kanban-style board to track workstreams (loops) and their tasks. Tasks are cards, columns are task statuses, hat info is card metadata. Includes exposing orchestration events for activity timelines and diagnostics.

## Design & Plan

All design documents and the implementation plan are in `.agents/planning/tracking-ux/`:

- **Design**: `.agents/planning/tracking-ux/design/detailed-design.md` — full design with types, API methods, stream events, architecture diagrams
- **Implementation plan**: `.agents/planning/tracking-ux/implementation/plan.md` — 13-step incremental plan with test requirements and demo artifacts
- **Research**: `.agents/planning/tracking-ux/research/` — API audit, core task system deep dive, architectural gap analysis
- **Requirements**: `.agents/planning/tracking-ux/idea-honing.md` — 15 Q&A requirements clarification

**Read the design and implementation plan before starting any work.**

## Requirements

1. Add `Blocked` and `InReview` variants to `TaskStatus` enum (6 total: Open, Blocked, InProgress, InReview, Closed, Failed)
2. Add `last_hat`, `transitions` (status history), and `tags` fields to core `Task` struct
3. Record `StatusTransition` entries (`{from, to, timestamp, hat}`) on every status change in `TaskStore`
4. Add filtering methods to `TaskStore`: by status, loop_id, hat, priority, tag, and combined
5. Add counting methods to `TaskStore`: `counts_by_status()`, `counts_by_status_for_loop()`
6. Enrich `LoopEntry` with `hat_collection`, `active_hat`, `iteration`, `total_cost_usd`, `max_iterations`, `termination_reason`
7. Enrich `LoopHistory` `IterationStarted` with `hat`/`hat_display`, `IterationCompleted` with `cost_usd`
8. Replace API `TaskDomain` (`tasks-v1.json`) with a wrapper around core `TaskStore` (`tasks.jsonl`)
9. Add inline `loop_context` on API task responses (iteration, cost, active hat, termination reason from parent loop)
10. Enrich API `LoopRecord` with hat collection, active hat, per-status task counts
11. Create `EventDomain` in ralph-api with `event.list` method for querying orchestration events
12. Enrich `task.status.changed` stream events with actual previous status, hat, loop_id, task title
13. Add stream topics: `task.created`, `task.deleted`, `loop.started`, `loop.completed`, `event.published`
14. Bridge loop runner state to API stream via `_internal.publish` HTTP calls
15. Add `ralph tools task review` and `ralph tools task block` CLI commands
16. Update all existing transition commands to record hat info
17. Write `.ralph/current-hat` marker file from loop runner (like `current-loop-id`)
18. Update `crates/ralph-core/data/ralph-tools.md` with new commands
19. All new fields use `#[serde(default)]` for backward compatibility with existing JSONL files
20. Loose status transitions — no state machine enforcement

## Constraints

- API surface only — no frontend UI implementation
- Build on core task system (`tasks.jsonl`), not the deprecated API task store (`tasks-v1.json`)
- Queue/execution features (`run`, `run_all`, `cancel`) stay as API-layer concerns, don't move to core `TaskStore`
- Stream publishing from loop runner is fire-and-forget — API failures must not block the loop
- `unsafe_code = "forbid"` — no unsafe code
- Follow existing codebase patterns: `thiserror`/`anyhow` for errors, doc comments on public APIs, conventional commit messages
- Run `cargo test -p <crate>` after each step; `cargo test --all` at final step

## Success Criteria

The task is complete when:

- [ ] `TaskStatus` has 6 variants: Open, Blocked, InProgress, InReview, Closed, Failed
- [ ] `Task` struct has `last_hat`, `transitions`, `tags` fields
- [ ] All status changes in `TaskStore` record `StatusTransition` entries with hat info
- [ ] `TaskStore` supports filtering by status, loop_id, hat, priority, tag
- [ ] `TaskStore` provides per-status counts and per-loop counts
- [ ] `LoopEntry` persists hat_collection, active_hat, iteration, cost, max_iterations, termination_reason
- [ ] `LoopHistory` records hat and cost per iteration
- [ ] API `task.list` reads from core JSONL store with status/loop_id/hat filters
- [ ] API `task.get` includes inline `loop_context` with iteration, cost, active hat
- [ ] API `loop.list` includes hat_collection, active_hat, task_counts per loop
- [ ] API `event.list` returns orchestration events with topic/hat/iteration filters
- [ ] Stream `task.status.changed` includes actual from status, hat, loop_id, task title
- [ ] Stream topics `task.created`, `task.deleted`, `loop.started`, `loop.completed`, `event.published` are registered and published
- [ ] Loop runner publishes state changes to API stream via `_internal.publish`
- [ ] `ralph tools task review` and `ralph tools task block` commands work
- [ ] All transition commands record hat from loop context or `--hat` flag
- [ ] Old JSONL files without new fields deserialize without error (backward compat)
- [ ] `cargo test --all` passes
- [ ] `cargo run -p ralph-e2e -- --mock` passes
- [ ] Every step has demo artifacts saved as proof of completion
