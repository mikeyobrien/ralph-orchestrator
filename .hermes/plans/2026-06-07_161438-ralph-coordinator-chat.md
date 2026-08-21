# Ralph Coordinator Chat UX Plan

> **For Hermes:** This supersedes the earlier "Ralph HITL Kanban UX" framing. Ralph already has role/hat, memory, task, event, and human-in-loop systems. Do **not** clone Hermes Kanban inside Ralph. Add a conversational Coordinator surface that exposes and operates those existing systems.

**Goal:** Let a user chat with Ralph's Coordinator during a loop to understand status, steer work, inspect tasks/memory, and delegate approve/revise/continue decisions to the Coordinator when policy allows — without talking directly to every worker hat.

**Architecture:** Add a first-class Coordinator conversation path that routes authorized Slack/Telegram/user messages into a coordinator role/event, while reusing Ralph's existing `HatRegistry`, `TaskStore`, `MarkdownMemoryStore`, event bus, `RobotService`, and Slack/Telegram handlers. The Coordinator becomes the human-facing coordinator and delegated operator, not a new task board.

**UX correction:** Users should not need to type `/btw`, `/ask`, or `talk` for normal inquiries. Any authorized natural-language inquiry in a bound loop routes to the Coordinator. Explicit commands remain useful shortcuts, but ambient chat is the default.

**Working branch inspected:** `/Users/rook/projects/ralph-orchestrator.slack-surface`

---

## What already exists

### Roles / hats

Relevant files:
- `crates/ralph-proto/src/hat.rs`
- `crates/ralph-core/src/hat_registry.rs`
- `crates/ralph-core/src/instructions.rs`
- `presets/code-assist.yml`

Ralph already has configurable roles/hats:
- `Hat { id, name, description, subscriptions, publishes, instructions }`
- `HatRegistry::from_config()` builds roles from YAML.
- `InstructionBuilder::build_custom_hat()` injects role behavior from pub/sub contracts.
- `HatlessRalph` is always registered as fallback coordinator.

`presets/code-assist.yml` already encodes an important policy:

> `human.interact` is reserved for scope/direction questions from the Coordinator only.

So the product concept exists in the preset language, but not yet as a chat UX/control-plane primitive.

### Tasks

Relevant files:
- `crates/ralph-core/src/task.rs`
- `crates/ralph-core/src/task_store.rs`
- `crates/ralph-core/src/event_loop/mod.rs`

Ralph already has durable tasks:
- Stored as `.ralph/agent/tasks.jsonl`
- `TaskStatus::{Open, InProgress, Closed, Failed}`
- priorities, dependencies, stable keys, loop ownership
- prompt injection via `<ready-tasks>` in `EventLoop::prepend_ready_tasks()`
- safe concurrent access via `TaskStore` file locking

This is already close to the "ticket" substrate. We should render/summarize it, not duplicate it.

### Memory

Relevant files:
- `crates/ralph-core/src/memory.rs`
- `crates/ralph-core/src/memory_store.rs`
- `crates/ralph-core/src/event_loop/mod.rs`

Ralph already has durable memory:
- Stored as `.ralph/agent/memories.md`
- `MemoryType::{Pattern, Decision, Fix, Context}`
- auto-injected when enabled
- CLI skills injected as `ralph-tools`, `ralph-tools-tasks`, `ralph-tools-memories`

Coordinator should be allowed to query/summarize these memories and record decisions, not add another memory layer.

### Human/chat plumbing

Relevant files:
- `crates/ralph-proto/src/robot.rs`
- `crates/ralph-core/data/robot-interaction-skill.md`
- `crates/ralph-core/src/event_loop/mod.rs`
- `crates/ralph-slack/src/handler.rs`
- `crates/ralph-slack/src/renderer.rs`
- `crates/ralph-telegram/src/handler.rs`
- `crates/ralph-telegram/src/service.rs`

Ralph already has a provider-agnostic `RobotService`:
- `send_question()`
- `wait_for_response()`
- `send_checkin()`
- `send_file()`

Slack already gates inbound messages by channel/user, binds a Slack thread to a loop, and maps replies:
- pending question exists → `human.response`
- otherwise → `human.guidance`

Slack commands currently include:
- `help`
- `status`
- `tail [n]`
- `stop` / `cancel`

Slack rendering currently has:
- `start_card`
- `progress_card`
- `final_card`
- `status_card`
- `help_card`

---

## Multi-harness agnostic design

The Coordinator must be a Ralph control-plane concept, not a Claude/Kiro/Codex/Slack implementation detail.

### Boundaries

```text
Chat Surface        Ralph Core Control Plane          Agent Harness
------------        ------------------------          -------------
Slack/Telegram  ->  coordinator.message event        ->     coordinator hat prompt
TUI/API         ->  intent parser + state      ->     Claude/Kiro/Codex/Pi/ACP/etc.
                 <- coordinator.reply/render event   <-     ralph emit / structured output
```

### Rule

Surfaces know users/threads/auth. Harnesses know how to run a model/tooling process. Ralph core owns tasks, memory, events, loop state, and Coordinator semantics.

Do not let:
- Slack call Claude directly.
- Claude-specific stream parsing define Coordinator UX.
- ACP/Kiro capabilities leak into event vocabulary.
- Coordinator create a provider-specific state store.

### Existing adapter substrate

Relevant files:
- `crates/ralph-adapters/src/lib.rs`
- `crates/ralph-adapters/src/cli_backend.rs`
- `crates/ralph-adapters/src/acp_executor.rs`
- `crates/ralph-adapters/src/auto_detect.rs`
- `crates/ralph-core/src/config.rs`

Ralph already supports backend/harness variation through:
- `CliBackend`
- `OutputFormat::{Text, StreamJson, CopilotStreamJson, PiStreamJson, Acp}`
- `HatBackend::{Named, NamedWithArgs, KiroAgent, Custom}`
- per-hat `backend` and `backend_args`
- auto-detected backend priority

So Coordinator should just be another hat/virtual role whose backend is selected through the same mechanism.

### Coordinator event envelope

Use a normalized event payload regardless of surface or harness:

```json
{
  "topic": "coordinator.message",
  "payload": {
    "text": "what is blocked?",
    "source": {
      "surface": "slack",
      "channel_id": "C…",
      "thread_ts": "178…",
      "user_id": "U…"
    },
    "loop_id": "slack-C…",
    "intent_hint": "status"
  }
}
```

Coordinator or deterministic control-plane code can reply with:

```json
{
  "topic": "coordinator.reply",
  "payload": {
    "text": "Builder is active; 1 ready task, 2 blocked; pending review gate…",
    "render": "summary",
    "actions": ["approve", "revise <instruction>", "tail 10"]
  }
}
```

### Deterministic fast path vs harness path

Use two paths:

1. **Deterministic commands:** `status`, `tasks`, `tail`, `approve`, `revise`, `continue`, `stop`
   - Handled in Ralph core / provider service.
   - No model call.
   - Works identically regardless of harness.

2. **Conversational Coordinator:** “what should I do?”, “why is it blocked?”, “summarize risk”
   - Emits `coordinator.message`.
   - Activates `coordinator` hat.
   - Hat backend can be Claude, Kiro ACP, Codex, Pi, custom, etc.
   - Hat sees normalized context: tasks, memory, recent events, pending gates, source constraints.

This preserves responsiveness and keeps harness dependence behind the existing adapter layer.

### Capability model

Add an optional provider/surface capability struct, not backend branches:

```rust
pub struct CoordinatorSurfaceCapabilities {
    pub can_update_message: bool,
    pub can_thread_reply: bool,
    pub can_buttons: bool,
    pub can_upload_files: bool,
    pub max_message_chars: usize,
}
```

Slack renderer can use blocks/buttons; Telegram can use text/edit-message; TUI/API can use structured JSON. Same Coordinator summary, different rendering.

### Coordinator prompt context

When `coordinator` hat runs, inject:
- active loop id and source surface
- recent events
- `<ready-tasks>` / task counts from `TaskStore`
- memories from `MarkdownMemoryStore`
- pending human question/review gate
- allowed actions / surface capabilities
- instruction: publish exactly one of `coordinator.reply`, `human.response`, `human.guidance`, `review.rejected`, `queue.advance`, etc.

### Multi-harness invariant

A harness only needs to satisfy Ralph’s normal contract:

1. receive prompt
2. run model/tooling process
3. stream text/tool info if possible
4. emit a Ralph event via `ralph emit` or equivalent structured output
5. exit

If a harness cannot stream tool calls or blocks, Coordinator still works because the stable interface is events, tasks, and memory — not stream internals.

---

## Product model

### User mental model

The user should feel like they are chatting with one operator:

> "Coordinator, what's going on?"
> "Approve the review."
> "Pause and focus on the Slack auth bug."
> "What tasks are still open?"
> "What did we learn that should become memory?"

The Coordinator replies with a concise operational view and turns user intent into existing Ralph events/task/memory operations.

### Coordinator responsibilities

1. **Status synthesis**
   - Summarize current loop state, current hat, latest event, open/ready/blocked tasks, pending question, recent failures.

2. **Review gate handling**
   - If a human decision is pending, classify replies as approve / revise / continue / reject.
   - Emit existing `human.response` or proposed richer review events.

3. **Guidance routing**
   - Convert proactive human steering into `human.guidance` and persist it into scratchpad via existing `EventLoop::update_robot_guidance()` path.

4. **Task operations**
   - Show ready/open/blocked tasks from `TaskStore`.
   - Optionally create/reopen/fail tasks through existing `ralph tools task` semantics.

5. **Memory operations**
   - Search and summarize `MarkdownMemoryStore`.
   - Recommend or create `decision`/`fix` memories when human steering establishes durable policy.

6. **Escalation discipline**
   - Only Coordinator should ask human-blocking scope/direction questions in normal code-assist mode.
   - Worker hats should continue to avoid `human.interact` unless explicitly configured otherwise.

---

## Recommended event vocabulary

Keep existing events as the base:
- `human.guidance`
- `human.interact`
- `human.response`
- `tasks.ready`
- `review.ready`
- `review.passed`
- `review.rejected`
- `queue.advance`
- `LOOP_COMPLETE`

Add only the minimal Coordinator-specific events if needed:

```text
coordinator.message          user/chat message intended for Coordinator
coordinator.status.request   explicit status query
coordinator.status.response  Coordinator synthesized reply, if represented in event log
coordinator.decision         parsed approve/revise/reject/continue decision
coordinator.task.update      Coordinator changed task state
coordinator.memory.note      Coordinator recorded/recommended memory
coordinator.reply            provider-neutral user-facing reply
```

Preferred first slice can avoid most new events by mapping chat commands directly to existing `human.guidance` / `human.response` plus rendering from state. Add `coordinator.message` and `coordinator.reply` when freeform conversational Coordinator is introduced.

---

## Slack UX sketch

### Existing behavior to preserve

- Top-level app mention starts/binds a loop.
- Thread replies are authorized by channel/user.
- Bot-authored messages are ignored to prevent loops.
- Known thread commands are parsed before generic guidance.
- If pending question exists, plain replies answer it only when they look like explicit answers; inquiries/ambiguous replies still route to Coordinator.

### New commands

Add text commands first; buttons can come later.

```text
coordinator / coord / control          show Coordinator summary
<freeform question/inquiry>   route to Coordinator automatically
talk <message> / ask <message> optional explicit aliases for Coordinator routing
status                       keep current status command, but upgrade with task + pending decision context
tasks                        show open/ready/blocked task list
memory <query>               search Ralph memories
approve                      approve pending human review/question
revise <instruction>         reject/continue with concrete guidance
continue                     continue with current plan
park                         pause/mark blocked with reason, if supported
```

### Example interaction

```text
Mikey: coordinator
Ralph Coordinator: Current loop: slack-C…
- Active role: Builder
- Latest event: review.ready from task task-…
- Tasks: 1 ready, 2 open, 4 closed
- Pending human decision: yes — final review approval
Options: approve | revise <instruction> | continue

Mikey: revise make the Slack auth failure path explicit in tests
Ralph Coordinator: Routed as revision guidance. Reopened task task-… and resumed the loop.
```

---

## Surface interaction flow and UX

### Slack: thread-native project control

Slack is the rich work surface. The unit of UX is one Slack thread = one Ralph loop. The channel selects the repo/workspace via `RObot.slack.channel_repos`; the user does not pick arbitrary repos in chat text.

#### Slack start flow

```text
#project-channel
Mikey: @Ralph implement the Coordinator chat UX

Ralph: 🧭 Coordinator loop started
Repo: ralph-orchestrator
Loop: slack-C123-178...
Status: planning
Actions: Status | Tasks | Tail | Stop
```

Behind the scenes:
1. Slack daemon receives top-level app mention.
2. Auth gates run: allowed channel + allowed user + bot/self filtering + dedupe.
3. Channel maps to repo/workspace root.
4. Ralph binds the root message `ts` as the thread id.
5. Ralph starts a loop in that workspace/worktree.
6. Loop-local service posts updates/questions back into the bound thread.

#### Slack ongoing UX

The thread should have one compact Coordinator anchor card plus sparse milestone replies.

Commands in the thread:

```text
coordinator           # show synthesized state
coord                 # alias
control               # alias
tasks                 # ready/open/blocked task summary
status                # process + loop + pending gate state
tail 10               # recent events
approve               # approve pending question/review gate
revise <instruction>  # route revision guidance
continue              # continue current plan
talk/ask <message>    # optional explicit Coordinator aliases; plain inquiries also work
stop                  # cancel loop
```

Users do **not** need `talk`/`ask` for ordinary questions. A plain message like `why is this blocked?` or `would you approve this?` should route to the Coordinator automatically.

Example:

```text
Mikey: coordinator
Ralph Coordinator:
Loop slack-C123-178… is in review.
- Active role: Fresh-Eyes Critic
- Latest event: review.ready
- Tasks: 1 ready, 2 open, 4 closed
- Pending decision: yes — final review gate
Options: approve | revise <instruction> | tail 10

Mikey: revise make the Slack auth failure-path tests explicit
Ralph Coordinator:
Routed revision guidance and resumed the loop.
Event: human.guidance
```

#### Slack review gate UX

When Ralph needs a decision, Slack should post a decision card:

```text
Ralph Coordinator: Human review needed
What changed: Slack Coordinator commands + task summary renderer
Verification: cargo test -p ralph-slack slack_routing slack_renderer
Risk: Telegram parity not yet implemented

Reply:
- approve
- revise <specific instruction>
- continue
- stop
```

If Slack interactivity is configured later, add buttons for Approve / Revise / Continue / Stop. Text commands remain the source-of-truth fallback so the flow works in every Slack client and in thread replies where slash commands are awkward.

#### Slack freeform Coordinator UX

```text
Mikey: why is this stuck?
```

The handler classifies this as a Coordinator inquiry, emits `coordinator.message` with source context and loop id, and runs the `coordinator` hat through the normal harness adapter when deterministic state is insufficient. The Coordinator may answer with `coordinator.reply` or make a delegated `coordinator.decision` when policy permits.

### Telegram: lightweight direct control

Telegram is the lightweight async control surface. It should optimize for concise status, direct decisions, and low ceremony. Unlike Slack, Telegram does not naturally provide rich project/channel/thread context, so Ralph must make loop selection explicit when more than one loop exists.

#### Telegram start / attach flow

Recommended modes:

1. **Single active loop:** messages default to the current loop.
2. **Multiple loops:** bot shows a short loop picker or accepts `@loop-id` prefix.

Example:

```text
Mikey: /start implement Coordinator chat UX
Ralph: 🧭 Started loop ralph-20260607-abc123
Repo: ralph-orchestrator
Use /status, /tasks, /tail, /stop, or just send guidance.
```

If the loop was started elsewhere, Telegram can attach:

```text
Mikey: /attach slack-C123-178...
Ralph: Attached to loop slack-C123-178…
```

#### Telegram ongoing UX

Commands:

```text
/status        concise Coordinator summary
/tasks         ready/open/blocked tasks
/tail 10       recent events
/approve       approve pending gate
/revise ...    send revision guidance
/continue      continue current plan
/ask ...       optional explicit Coordinator message; plain inquiries work too
/loops         list active loops
/attach <id>   choose active loop
/stop          cancel active loop
```

Example:

```text
Mikey: /status
Ralph: 🧭 Coordinator
Loop: slack-C123-178…
State: review needed
Active role: Critic
Tasks: 1 ready / 2 open / 4 closed
Pending: final review approval
Reply /approve or /revise <instruction>.
```

#### Telegram review gate UX

```text
Ralph: 🟡 Human review needed
Changed: Coordinator status + task commands
Verified: cargo test -p ralph-slack slack_routing
Risk: Telegram command parity pending

Reply:
/approve
/revise <instruction>
/continue
/stop
```

Telegram can later use inline keyboard buttons, but text commands should remain canonical.

#### Telegram guidance UX

If a pending question exists, a normal reply becomes `human.response` only when it looks like an explicit answer. If no question is pending, clearly imperative guidance becomes `human.guidance`; inquiries and ambiguous replies route to Coordinator, same as Slack.

For multiple active loops, require either an attached active loop or explicit prefix:

```text
@slack-C123-178 revise tighten the auth tests
```

### Ambient Coordinator inquiries and delegated decisions during ongoing loops

Ongoing loops need a third inbound lane besides `human.response` and `human.guidance`: an **ambient Coordinator turn**. It is not purely read-only. The Coordinator receives the user's message, current loop state, pending gates, tasks, memory, and surface capabilities; then it may answer, recommend, ask a clarifying question, or act on the user's behalf within a configured decision policy.

The user often wants to ask:

```text
what is it doing now?
why is it stuck?
what does this review failure mean?
which task is next?
should I approve this?
```

Those should not automatically become raw worker guidance, and they should not accidentally answer a pending `human.interact` question without Coordinator interpretation.

#### Inbound lane classification

Every authorized message in a bound loop should be classified in this order:

1. **Control command** — `status`, `tasks`, `tail`, `stop`, `approve`, `revise`, etc.
2. **Explicit decision answer** — `approve`, `continue`, `reject`, `revise <instruction>` when a pending gate exists.
3. **Explicit steering/guidance** — `steer ...`, `guidance ...`, `revise ...` when no gate is pending.
4. **Ambient Coordinator turn** — any remaining authorized natural-language inquiry or ambiguous message.
5. **Fallback plain text** — only after the Coordinator classifier determines the message is direct worker guidance rather than an inquiry/decision.

Memory from earlier design still applies: command forms override pending questions. For example, `what are you asking me to approve?` must route to the Coordinator and must not clear the pending question.

#### Ambient Coordinator path

A Coordinator input message is just a user turn plus context. Mutation is decided by Coordinator output and decision policy, not by requiring the user to pre-label the turn:

```json
{
  "topic": "coordinator.message",
  "payload": {
    "kind": "inquiry",
    "text": "why is this stuck?",
    "requested_effect": "coordinator_decide",
    "decision_policy": "delegate_safe",
    "loop_id": "slack-C123-178...",
    "source": {"surface": "slack", "thread_ts": "178...", "user_id": "U..."}
  }
}
```

The Coordinator may render a pure reply:

```json
{
  "topic": "coordinator.reply",
  "payload": {
    "kind": "answer",
    "text": "It is waiting on review.ready. The critic passed tests but flagged missing auth-path coverage. Say `revise add the auth-path tests` to steer, or `approve` to accept current risk."
  }
}
```

A delegated decision is a separate event with explicit authority, rationale, and mapped side effects:

```json
{
  "topic": "coordinator.decision",
  "payload": {
    "action": "revise",
    "rationale": "The auth negative path is missing; approving would accept avoidable review risk.",
    "authority": "delegated_by_policy",
    "emits": ["human.guidance", "task.resume"]
  }
}
```

Explicit verbs remain shortcuts that bypass the Coordinator's need to infer intent:

```text
revise add the auth-path tests
steer prioritize Telegram parity
continue
approve
```

#### Same-loop vs ambient Coordinator sidecar execution

For deterministic questions (`status`, `tasks`, `tail`, pending gate explanation), answer directly from Ralph state without invoking a harness.

For richer questions (`should I approve this?`, `summarize the risk`, `what would you do?`), use an **ambient Coordinator sidecar turn** instead of injecting the question directly into the main worker sequence:

- out-of-band: runs beside the active loop and does not interrupt the current planner/builder/reviewer/finalizer turn;
- delegated operator, not read-only-only: receives a snapshot of recent events, task store, memory snippets, pending gate, decision policy, and surface capabilities;
- backend selected through normal `HatBackend` (`claude`, `kiro-acp`, `codex`, `pi`, `custom`);
- allowed outputs: `coordinator.reply`, `coordinator.decision`, and explicit follow-up clarification;
- mutation requires auth + policy permission + an auditable `coordinator.decision`, not necessarily an explicit user action verb;
- main loop keeps running unless it is already blocked on a human gate.

This is the Ralph analogue of Hermes `/btw` internally, but **not** a user-facing command requirement. Users just chat. Do **not** require ACP or a separate Chat agent for this. ACP is one optional harness behind `HatBackend`; the stable boundary remains Ralph events + task/memory state + `RobotService`.

Implementation implication: add a small Coordinator runner/service that can spawn one Coordinator hat invocation from a state snapshot, publish `coordinator.reply` back to the bound surface, and optionally publish `coordinator.decision` for policy-authorized mutations. It should reuse existing adapter execution (`CliBackend`/`HatBackend`) rather than inventing a new chat-agent runtime.

#### Slack examples

```text
Mikey: ask why is this blocked?
Ralph Coordinator: The Builder emitted build.blocked because cargo fmt failed. No human decision is needed yet; Ralph is expected to repair it next. Use `tail 10` for evidence.
```

With pending review:

```text
Ralph Coordinator: Human review needed: approve or revise.
Mikey: what would you do?
Ralph Coordinator: I would revise: the auth negative test is missing. Policy allows delegated safe revisions, so I reopened the review with guidance to add a negative auth test for unauthorized Slack users.
```

The pending review is resolved by an auditable `coordinator.decision`.

#### Telegram examples

```text
Mikey: what is the current blocker?
Ralph: The loop is not blocked; it is running Critic on review.ready. Last evidence: cargo test -p ralph-slack slack_routing passed.
```

Multiple loops:

```text
Mikey: @slack-C123-178 should I approve?
Ralph: Not yet. The implementation lacks Telegram parity; approve only if Slack-first is acceptable.
```

### Shared UX rules across Slack and Telegram

- Auth before every side effect.
- Known commands before generic guidance.
- Command/question forms override pending questions.
- Pending review/question replies become `human.response` only when they are explicit answers.
- Clearly imperative freeform steering with no pending question becomes `human.guidance`; ambiguous/plain inquiries go through Coordinator first.
- Natural-language inquiries become `coordinator.message` without needing `/ask`, `/btw`, or `talk`; they may produce `coordinator.reply` or policy-authorized `coordinator.decision`.
- All rendered status is derived from Ralph state: events, `TaskStore`, `MarkdownMemoryStore`, process/thread binding.
- Never expose tokens, raw provider payloads, or arbitrary filesystem paths.

### Parallel execution semantics

Ralph has two different parallelism shapes, and Coordinator must handle both without becoming a racey second scheduler.

#### 1. Multiple loops in parallel

Ralph can run multiple loops at once via git worktrees: the primary loop owns `.ralph/loop.lock`; additional loops run in `.worktrees/<loop-id>/` with isolated events/tasks/scratchpad and shared memories. Coordinator routing should therefore be **loop-scoped by default**.

Rules:
- Slack thread binding selects the loop. A message inside a Ralph thread talks to that loop's Coordinator only.
- Telegram/TUI/API need an active loop binding; if multiple loops are active and no loop is selected, Coordinator should show a loop picker instead of guessing.
- Every `coordinator.message`, `coordinator.reply`, and `coordinator.decision` includes `loop_id`.
- Side effects are confined to the selected loop/worktree unless the user explicitly asks for a fleet/global action.
- Cross-loop/global actions use a separate `scope=global` policy and should be rare: list loops, summarize fleet health, stop all only with explicit confirmation.

Example fleet answer:

```text
Mikey: what is Ralph doing?
Ralph Coordinator:
- slack-C123… review gate pending, 2/3 tasks closed
- telegram-9ab… Builder running in worktree, last event tests.started
- ralph-xyz… queued for merge
Say @loop-id <question> or pick one to act.
```

#### 2. Intra-loop waves / parallel workers

Ralph waves let one hat fan out N backend workers in a single loop iteration. Coordinator should treat a wave as structured loop sub-state, not as N separate user conversations.

Add a parallel summary to `CoordinatorSummary`:

```rust
pub struct CoordinatorParallelSummary {
    pub active_loops: Vec<LoopSummary>,
    pub active_waves: Vec<WaveSummary>,
}

pub struct WaveSummary {
    pub loop_id: String,
    pub wave_id: String,
    pub worker_hat: String,
    pub expected_total: usize,
    pub completed: usize,
    pub failed: usize,
    pub timed_out: bool,
    pub aggregate_waiting: bool,
}
```

Wave rules:
- Coordinator can answer from partial wave state: “2/3 reviewers done; docs reviewer still running.”
- Coordinator should not approve/reject the aggregate result until the wave joins or a policy explicitly allows partial decisions.
- Safe delegated actions can be wave-scoped: retry failed worker, extend timeout, cancel a stuck worker, wait for all.
- Loop-level actions like `stop` propagate to all running wave workers.
- Worker-level decisions require `wave_id` + `worker_index` or a resolver that maps natural language to a specific worker.

#### Decision serialization and stale-state protection

Parallel execution makes the decision applier more important than the Coordinator hat. The hat may propose; the applier validates and serializes.

Every `coordinator.decision` should include:

```json
{
  "loop_id": "slack-C123-178...",
  "scope": "loop|wave|worker|task|global",
  "observed_event_seq": 184,
  "expected_pending_gate_id": "review-42",
  "wave_id": "w-...",
  "worker_index": 1,
  "action": "retry_worker",
  "authority": "delegated_by_policy",
  "rationale": "The docs reviewer timed out; retry is safe and local."
}
```

The decision applier must:
1. Re-read latest loop state before applying.
2. Reject or rebase stale decisions when `observed_event_seq` / pending gate / wave state no longer matches.
3. Emit side effects through existing Ralph events only after validation.
4. Record the original Coordinator rationale for audit.

This gives the UX we want: the user can ask “what would you do?” while workers are running, and Coordinator can say “wait for the final reviewer,” retry a failed worker if safe, or revise after the aggregate gate — without racing the parallel workers.

---

## Implementation plan

### Phase 1 — Core Coordinator state + intent model, no LLM

**Objective:** Add provider-agnostic state gathering and inbound intent classification in core.

Likely files:
- Create: `crates/ralph-core/src/coordinator.rs`
- Modify: `crates/ralph-core/src/lib.rs`
- Test: `crates/ralph-core/tests/coordinator.rs` or colocated unit tests

Core types:
- `CoordinatorSummary`
- `CoordinatorTaskSummary`
- `CoordinatorPendingGate`
- `CoordinatorSurfaceCapabilities`
- `CoordinatorParallelSummary`, `LoopSummary`, `WaveSummary`
- `CoordinatorDecisionPolicy::{ObserveOnly, Recommend, DelegateSafe, DelegateAllWithAudit, RequireHuman}`
- `CoordinatorIntent::{Status, Tasks, Tail, Approve, Revise, Continue, Stop, Inquiry, Guidance, Unknown}`
- `CoordinatorMessage { kind, text, requested_effect, decision_policy, source, loop_id }`
- `CoordinatorDecision { scope, loop_id, observed_event_seq, expected_pending_gate_id, wave_id, worker_index, action, rationale, authority, emits }`

State sources:
- events via existing event reader/log paths
- tasks via `TaskStore`
- memories via `MarkdownMemoryStore`
- pending questions/review gates via existing RObot/slack/telegram state
- process/thread binding from provider state
- loop registry / worktree metadata for multi-loop summaries
- wave events and diagnostics for intra-loop parallel worker summaries

Key rule: deterministic status/task/tail/review-explanation answers are generated from `CoordinatorSummary` without invoking any agent harness.

Validation:

```bash
cd /Users/rook/projects/ralph-orchestrator.slack-surface
cargo test -p ralph-core coordinator
```

### Phase 2 — Slack deterministic Coordinator UX

**Objective:** Teach Slack handler/renderer to use core Coordinator intents and summaries.

Likely files:
- Modify: `crates/ralph-slack/src/handler.rs`
- Modify: `crates/ralph-slack/src/renderer.rs`
- Modify: `crates/ralph-slack/src/service.rs`
- Test: `crates/ralph-slack/tests/slack_renderer.rs`
- Test: `crates/ralph-slack/tests/slack_routing.rs`

Add/extend `ThreadCommand` variants:
- `Coordinator`
- `Tasks`
- `Memory { query: String }`
- `CoordinatorMessage { message: String, explicit: bool }`
- `Approve`
- `Revise { instruction: String }`
- `Continue`
- `Guidance { instruction: String }`

Routing rules:
- auth gates before all commands
- known commands before generic guidance
- command/question forms override pending questions
- ambient inquiry/question-shaped/ambiguous messages emit `coordinator.message` without requiring `/ask`, `/btw`, or `talk`
- `approve` / `revise` explicit commands mutate when authorized; Coordinator-inferred decisions mutate only through policy-authorized `coordinator.decision`
- wrong user/channel/thread cannot mutate anything

Validation:

```bash
cargo test -p ralph-slack slack_routing
cargo test -p ralph-slack slack_renderer
```

### Phase 3 — Ambient Coordinator sidecar decision runner

**Objective:** Support richer ongoing-loop inquiries and delegated decisions without perturbing the active worker/reviewer/finalizer sequence.

Do **not** require ACP and do **not** add a separate Chat agent runtime. Implement a small sidecar query runner that reuses Ralph's existing harness abstraction.

Likely files:
- Create/Modify: `crates/ralph-core/src/coordinator.rs`
- Modify: `crates/ralph-cli/src/loop_runner.rs` if a loop-local runner hook is needed
- Possibly modify: `crates/ralph-adapters/src/cli_executor.rs` only if current executor cannot be cleanly reused
- Test with mock/replay backend: `crates/ralph-core/tests/coordinator.rs` or `crates/ralph-cli/tests/...`

Sidecar behavior:
1. Receive `coordinator.message` with `kind=inquiry` or `kind=ambiguous_user_turn`.
2. Build a state snapshot prompt from `CoordinatorSummary`, recent events, ready/open/blocked tasks, memory snippets, pending gate, decision policy, and surface capabilities.
3. Invoke the configured `coordinator` hat/backend through existing `HatBackend`/`CliBackend` plumbing (`claude`, `kiro-acp`, `codex`, `pi`, `custom`, etc.). ACP is only one possible backend.
4. Accept `coordinator.reply` for answer-only turns, or `coordinator.decision` for policy-authorized mutations.
5. Render the reply/decision rationale back to the originating Slack/Telegram/TUI surface.
6. Apply decision side effects through existing event paths (`human.response`, `human.guidance`, `task.resume`, `review.rejected`, etc.) only after auth + policy checks pass.
7. Do not inject raw user inquiry text directly into the main worker sequence.
8. For parallel loops/waves, include the selected `loop_id`, active wave state, and last observed event sequence in the prompt and returned decision.

Safety gates:
- sidecar prompt includes the configured decision policy and must emit auditable rationale for every mutation;
- sidecar output cannot clear pending `human.interact` questions except via policy-authorized `coordinator.decision`;
- sidecar output cannot close/reopen/fail tasks except via policy-authorized `coordinator.decision` mapped to existing event/store operations;
- decision applier rejects stale decisions when the loop/wave/gate changed after the Coordinator snapshot;
- cross-loop/global actions require explicit loop selector or global policy;
- one-way-door actions can still require `RequireHuman` policy even when safe revisions are delegated;
- timeout and cost limits should be shorter than full build hats.

Validation:

```bash
cargo test -p ralph-core coordinator_sidecar
# plus fake backend tests proving:
# - plain inquiry -> coordinator.message -> coordinator.reply without worker progression
# - plain "what would you do?" at a safe review gate -> coordinator.decision -> human.guidance/task.resume
# - parallel wave in progress -> Coordinator recommends wait/retry worker instead of approving incomplete aggregate
# - stale coordinator.decision after wave state changes is rejected or revalidated
```

### Phase 4 — Coordinator role in presets

**Objective:** Make Coordinator an explicit hat where useful, but do not require it for deterministic status commands.

Likely files:
- Modify: `presets/code-assist.yml`
- Modify: `crates/ralph-cli/presets/code-assist.yml`
- Tests: config/preset tests under `crates/ralph-api/tests/` or CLI preset tests

Potential hat:

```yaml
coordinator:
  name: "🧭 Coordinator"
  description: "Human-facing coordinator and delegated operator that answers ambient inquiries, synthesizes status, routes guidance, and owns scope/direction escalation under policy."
  triggers: ["coordinator.message"]
  publishes: ["coordinator.reply", "coordinator.decision", "human.interact", "human.guidance", "human.response", "review.rejected", "queue.advance"]
  backend:
    type: claude # or kiro-acp/codex/pi/custom; same HatBackend path as other hats
  timeout: 120
```

Prompt contract:
- default mode is interpret-and-coordinate: publish `coordinator.reply` for answers or `coordinator.decision` for policy-authorized actions;
- delegated mutations must include action, rationale, authority, confidence/uncertainty, and mapped side effects;
- publish `human.guidance`, `human.response`, `review.rejected`, or `queue.advance` only through the Coordinator decision applier, not as arbitrary raw hat output;
- worker hats still must not call `human.interact`; Coordinator owns scope/direction escalation.

### Phase 5 — Memory/task operations from Coordinator

**Objective:** Let Coordinator expose existing task/memory systems in chat without bypassing existing stores.

Use existing systems:
- Task read/update through `TaskStore` or existing tools command path.
- Memory search/add through `MarkdownMemoryStore` or existing tools command path.

Commands:
- `tasks` → summary list
- `task <id>` → detail
- `memory <query>` → search
- `remember decision: ...` → add `MemoryType::Decision`, behind explicit command only

### Phase 6 — Provider-generalize after Slack proves it

Telegram and later TUI/API should reuse the same core summary/intent parser after Slack tests pass.

Likely files:
- Modify: `crates/ralph-telegram/src/commands.rs`
- Modify: `crates/ralph-telegram/src/handler.rs`
- Modify: `crates/ralph-telegram/src/service.rs`

Do not duplicate Coordinator semantics per provider; keep parser/summary/sidecar core-owned where possible.

---

## What NOT to build

- Do not create a second Kanban/ticket database.
- Do not make every task a Slack message by default.
- Do not bypass `TaskStore` or `MarkdownMemoryStore` with new ad hoc files.
- Do not let arbitrary chat text directly mutate tasks/memory without authorization and intent parsing.
- Do not make `/btw`, `/ask`, or `talk` required ceremony for normal inquiries; ambient chat should route to Coordinator.
- Do not hardcode the Coordinator as read-only-only; it is allowed to make delegated decisions through explicit policy and audited events.
- Do not let worker hats ask blocking human questions if `code-assist` policy says only Coordinator owns that.
- Do not require ACP for Coordinator chat. ACP is optional behind `HatBackend`.
- Do not add a separate Chat agent runtime. Reuse Ralph hats/adapters/events.
- Do not couple Coordinator to Claude, Kiro, ACP, or Slack. Coordinator speaks Ralph events and state.

---

## Suggested first implementation slice

Build this in one narrow PR:

1. Add `CoordinatorSummary`, `CoordinatorIntent`, `CoordinatorDecisionPolicy`, and `CoordinatorDecision` in `ralph-core`.
2. Add Slack `coordinator|coord|control` deterministic summary.
3. Upgrade Slack `status` with task + pending decision context.
4. Add `tasks` command.
5. Add `approve` / `revise <instruction>` parsing with explicit-answer semantics.
6. Keep writes as existing `human.response` / `human.guidance` events.
7. Add ambient inquiry routing: plain authorized questions/ambiguous turns → `coordinator.message` without requiring `/ask`, `/btw`, or `talk`.
8. Add `CoordinatorDecisionPolicy` and mocked sidecar proofs: answer-only `coordinator.reply`, plus delegated safe `coordinator.decision` that maps to existing `human.guidance` / `task.resume` without advancing the worker sequence incorrectly.
9. Add loop/wave scoping to Coordinator decisions: `loop_id`, `scope`, `observed_event_seq`, optional `wave_id` / `worker_index`.
10. Add negative auth, negative policy, cross-loop, and stale-decision tests.

This gives Mikey the desired UX: chat naturally with the Coordinator, see progress, let the Coordinator decide when authorized, and still preserve Ralph's existing role/memory/task architecture and multi-harness agnostic adapter model.

