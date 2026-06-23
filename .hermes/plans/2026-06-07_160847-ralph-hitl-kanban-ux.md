# Ralph Human-in-the-Loop Kanban UX Plan

> **For Hermes:** This is a brainstorm/implementation plan, not an execution run. Use `subagent-driven-development` or Ralph itself only after Mikey chooses the UX shape.

**Goal:** Add a Hermes-Kanban-like UX to `ralph-orchestrator` where human-in-the-loop Ralph loops automatically create visible work tickets, advance them through status, and stop at explicit human review gates.

**Architecture:** Treat a Ralph loop as a durable "work item" with child task/review tickets derived from Ralph events. The Slack/Telegram/chat surface becomes the primary operator UI: it renders a live ticket stack, posts status transitions, and converts human replies/buttons into `human.response`, `human.guidance`, `review.approved`, or `review.rejected` events. Keep the underlying source of truth in Ralph event/state files so the UX is inspectable and provider-agnostic.

**Tech Stack:** Rust workspace in `/Users/rook/projects/ralph-orchestrator.slack-surface`; likely crates `ralph-core`, `ralph-cli`, `ralph-proto`, `ralph-slack`, existing Telegram patterns, Slack Socket Mode/API, JSONL event store.

---

## Product shape

### UX principle

Borrow the best part of Hermes Kanban: the operator should never wonder "what is Ralph doing?" or "what am I being asked to decide?"

A human-in-loop loop should create a visible progression:

```text
Ticket: Build Slack thread surface
  Planning       ✅ plan.ready
  Task 1         🔵 running — daemon wiring
  Task 2         ⏳ queued — renderer polish
  Review Gate    🟡 waiting for Mikey
  Closeout       ⏳ blocked by review
```

### Status vocabulary

Use a small stable vocabulary that maps across Slack, Telegram, TUI, and future surfaces:

- `queued`
- `running`
- `needs_human`
- `reviewing`
- `approved`
- `rejected`
- `blocked`
- `done`
- `failed`
- `cancelled`

### Ticket hierarchy

- **Loop ticket**: one top-level Ralph loop / one Slack thread / one Telegram conversation binding.
- **Task ticket**: derived from `tasks.ready`, `task.started`, `task.completed`, `task.failed`, etc.
- **Review gate ticket**: derived from `review.ready` or explicit `human.interact` with `kind=review`.
- **Guidance ticket/comment**: human steering that gets attached to the active task rather than becoming a separate loop.

---

## Design decisions to make together

### 1. Where should the source of truth live?

Recommended: Ralph event log + compact derived state file.

- Events remain append-only and auditable.
- Derived state can power renderers without replaying huge logs every poll.
- Chat messages can be re-rendered after daemon restart.

Candidate files:

- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/src/event_loop/mod.rs`
- Modify/Create: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/src/hitl_board.rs`
- Modify/Create tests: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/tests/hitl_board.rs`

### 2. Should this integrate with Hermes Kanban or stay native Ralph?

Recommended: native Ralph first, optional Hermes bridge later.

- Native Ralph avoids coupling runtime UX to Hermes internals.
- The API can intentionally resemble Hermes Kanban: ticket id, parent id, status, assignee, review gate, evidence.
- Later bridge could mirror Ralph tickets onto a Hermes board for cross-project dashboards.

### 3. What is the human review gate?

Recommended: make review gates first-class events, not just prompt text.

Example event shape:

```json
{
  "topic": "review.gate.ready",
  "loop_id": "slack-C...",
  "ticket_id": "review-final",
  "title": "Final human approval",
  "summary": "What changed, verification, risks",
  "options": ["approve", "revise <instruction>", "continue", "reject <reason>"],
  "blocks_ticket_ids": ["closeout"]
}
```

Human replies should become:

- `review.approved`
- `review.rejected`
- `human.guidance`
- `task.resume`

### 4. Should chat UI update one message or post a stream?

Recommended hybrid:

- Keep one pinned/anchor "board card" message updated in place when provider supports it.
- Also post concise thread updates at major transitions.
- For Slack, update the root or first bot reply with Block Kit; post detailed evidence as thread replies/files.
- For Telegram, edit a status message if possible and send separate replies for decisions.

---

## Implementation plan

### Phase 0: Decide UX contract

**Objective:** Lock down the event vocabulary and rendered operator states before code.

**Files:**
- Create: `/Users/rook/projects/ralph-orchestrator.slack-surface/docs/design/hitl-kanban-ux.md`
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/docs/guide/index.md` or equivalent docs index

**Acceptance criteria:**
- Document has the ticket hierarchy, status vocabulary, human reply semantics, provider rendering model, and failure states.
- It explicitly says chat authorization gates must run before any ticket mutation or command handling.

### Phase 1: Add core ticket state model

**Objective:** Add provider-agnostic data structures for loop/task/review tickets.

**Likely files:**
- Create: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/src/hitl_board.rs`
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/src/lib.rs`
- Test: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/tests/hitl_board.rs`

**Core types:**
- `HitlBoard`
- `HitlTicket`
- `HitlTicketKind::{Loop, Task, ReviewGate, Guidance}`
- `HitlTicketStatus`
- `ReviewDecision::{Approve, Continue, Revise, Reject}`

**Validation:**

```bash
cd /Users/rook/projects/ralph-orchestrator.slack-surface
cargo test -p ralph-core hitl_board
```

### Phase 2: Derive ticket updates from Ralph events

**Objective:** Convert existing loop events into deterministic ticket transitions.

**Likely files:**
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/src/event_loop/mod.rs`
- Modify/Create: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/src/hitl_board.rs`
- Test: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-core/tests/hitl_board.rs`

**Event mapping draft:**
- `plan.start` → create loop ticket, status `running`
- `tasks.ready` → create task tickets, status `queued`
- `task.started` / task execution equivalent → `running`
- `review.ready` → create review gate, status `needs_human`
- `human.interact` with review intent → review gate `needs_human`
- `human.response approve` → `review.approved`, unblock closeout
- `human.response revise/continue` → attach guidance, resume active task
- `work.done` / completion token → loop `done` only if no open review gates

**Validation:** replay fixture events into board reducer and assert final ticket states.

### Phase 3: Render board cards in Slack

**Objective:** Make Slack show the auto-created ticket stack and live progress.

**Likely files:**
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/renderer.rs`
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/service.rs`
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/daemon.rs`
- Test: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/tests/slack_renderer.rs`
- Test: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/tests/slack_service.rs`

**Slack rendering:**
- Header: loop title + loop id + current status
- Section list: 3-7 ticket rows, newest active emphasized
- Context: last event timestamp, current hat/iteration if available
- Actions on review gate: Approve, Continue, Revise, Reject if Slack interactivity is configured; otherwise textual commands

**Validation:** snapshot-ish renderer tests should assert semantics without brittle exact full Block Kit JSON.

### Phase 4: Route human review decisions safely

**Objective:** Turn human replies/buttons into review/guidance events only after auth and thread binding checks.

**Likely files:**
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/handler.rs`
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/state.rs`
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/socket_mode.rs`
- Test: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/tests/slack_routing.rs`
- Test: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/tests/slack_daemon.rs`

**Rules:**
- Wrong channel/user/thread cannot mutate board/ticket state.
- If a review gate is pending, `approve`, `continue`, `revise ...`, `reject ...` are parsed as gate decisions.
- If no review gate is pending, human text becomes guidance/follow-up, not approval.
- `@loop-id` selectors must be validated before path lookup.

### Phase 5: Add provider-agnostic operator commands

**Objective:** Make the UX discoverable and controllable.

**Command ideas:**
- `!board` / `/board` — show current ticket board
- `!status` — concise loop + active ticket status
- `!tail 10` — recent events/evidence
- `!approve` — approve pending gate
- `!revise <instruction>` — reject gate with actionable guidance
- `!park` — mark blocked/parked without losing state

**Likely files:**
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/handler.rs`
- Modify: `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/src/renderer.rs`
- Possibly mirror later in `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-telegram/src/commands.rs`

### Phase 6: Fake E2E, then live smoke

**Objective:** Prove the control plane deterministically before relying on real Slack.

**Likely tests:**
- `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/tests/slack_daemon.rs`
- `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/tests/slack_service.rs`
- `/Users/rook/projects/ralph-orchestrator.slack-surface/crates/ralph-slack/tests/slack_routing.rs`

**Fake E2E cases:**
1. Human starts loop from allowed Slack thread.
2. Ralph emits task/review events.
3. Slack board updates show queued/running/needs-human.
4. Unauthorized reply cannot approve.
5. Authorized `approve` emits `review.approved` and loop closes.
6. Authorized `revise` emits guidance and resumes the task.

**Live smoke:**
- Use real allowed user, not bot-authored self-message, for inbound path.
- Verify root thread binding, board update, pending review prompt, approve/revise path, and final done status.
- Do not print token values in logs/docs.

---

## Open questions for Mikey

1. Should Ralph tickets mirror into Hermes Kanban, or should this feel Kanban-like but stay native Ralph?
2. In Slack, do you prefer one continuously edited board message, noisy but transparent thread updates, or the hybrid?
3. Should review gates be explicit buttons where possible, or text-first commands so Telegram/Slack stay symmetrical?
4. What should happen if Mikey never responds: park, timeout-fail, or let the loop keep working on non-blocked tasks?
5. Should every Ralph loop have this UX, or only loops with `RObot.mode: director|operator` / `human_review: true`?

---

## Suggested first implementation slice

Build the narrowest useful thing:

1. Core `HitlBoard` reducer in `ralph-core`.
2. Slack renderer for a board card.
3. Slack command/reply path for `approve` and `revise` against a pending gate.
4. Fake E2E proving unauthorized users cannot approve and authorized revise resumes the loop.

This gives the Kermes/Hermes Kanban feel without prematurely building a full dashboard or Hermes bridge.
