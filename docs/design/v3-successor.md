# V3 successor: one agent, many hats

Status: proposal. Anchored to `integration/v3-prerelease` at `a01826d`.
No code in this change. Implementation specs, when this is accepted, go in `.ralph/specs/` and cite this file.

Ralph 3 as it stands on this branch is a TUI and coordination shell around the autoloop subprocess. Autoloop owns the loop, picks a role, and starts a new agent process for that role. That got feature-parity work done. It is the wrong shape for the product.

The successor is the spiritual successor of classic Ralph, built on Pi. One Pi session is the agent. Hats are modes that session wears when events say so. The user can talk, interrupt, and steer at any moment. Long work is a background job. The conversation does not wait on it.

## Vision

A run is a conversation with a durable Pi session, plus a durable event log that decides which hat speaks next.

The user states a goal. Ralph wears the hat that goal triggers and answers in the foreground session. When the next step is long — a test suite, a wide edit, a review of many files — the agent enqueues a job, tells the user what it queued, and returns to the prompt. A worker Pi session runs the job. Its completion is an event. That event wakes a hat, which speaks in the same foreground session. Cancel aborts the foreground turn and every worker. The conversation stays open.

Hats exist so a workflow can be planner, then builder, then reviewer, without hiring a new agent per role and without losing the thread the user is talking to.

## Non-goals

- A swarm. One foreground model, one transcript. Background jobs are fresh, short-lived Pi sessions with a frozen brief. They are not peers in the conversation.
- Autoloop as a hidden engine. Role dispatch, journals, and `autoloop control` do not sit under this loop.
- A multi-backend product. Claude, Kiro, Gemini, Codex, Copilot, OpenCode, Roo, ACP, and OMP are out of the successor path. Pi is the harness.
- Blocking human-in-the-loop. An ask is a message. It does not freeze the supervisor, the input box, or other jobs.
- Pixel parity with v2: lifecycle hooks, per-hat backends, the wave CLI, scratchpad-as-authority, and session-cassette replay.
- Rebuilding the web dashboard in the first slices. It follows the event log once the loop is real.
- Embedding Pi. The boundary is Pi's existing `--mode rpc` subprocess.

## What is wrong with current v3

`ralph run` always enters `autoloop_engine::run_autoloop_engine` (`crates/ralph-cli/src/main.rs`). The in-house event loop is already gone. Hats are lowered to an autoloop preset (`crates/ralph-cli/src/autoloop_preset_gen.rs`): one role file per hat, a `[handoff]` table from triggers, then a fresh backend process per iteration. The Pi mapping still launches `pi -p --mode json --no-session` (`crates/ralph-adapters/src/cli_backend.rs`). Session persistence, the thing Pi is for, is switched off. Each hat switch starts a cold prompt.

The restored surfaces are adapters onto that subprocess, not a conversation:

- `--rpc` (commit messages tag this `#343` / `#368`) translates autoloop's coarse `--events` stream into `RpcEvent`. Two mappers exist (`crates/ralph-cli/src/rpc_events.rs`, `crates/ralph-adapters/src/autoloop_rpc.rs`). A turn's text arrives as one `TextDelta` of `backend.output` at the iteration boundary. Pi's token stream never reaches the wire.
- `ralph resume` (tagged `#344`) stores an autoloop `run_id` under `.ralph/autoloop` and execs `autoloop resume`. `engine_state.rs` also plants a `.autoloop` → `.ralph/autoloop` symlink so autoloop's own CLI can find that state.
- Human questions are `ask.pending`. Autoloop blocks until `autoloop control respond` (`crates/ralph-cli/src/autoloop_robot.rs`). Telegram and web only relay that block.
- Configured lifecycle hooks are inert. Preflight says so (`crates/ralph-core/src/preflight.rs`).
- `generate_preset` writes `id`, `name`, `emits`, `concurrency`, `aggregate`, and `prompt_file`. It drops `backend`, `default_publishes`, `max_activations`, `disallowed_tools`, and `timeout`, which `HatConfig` still accepts. Skills and `.ralph/agent/memories.md` are not injected. Ralph tasks are observational; autoloop has a second task store. The gap analysis in `.ralph/specs/v2-engine-autoloop-gap-analysis.md` recorded these; the preset generator on this tip still behaves that way.
- The live web watcher still follows `.ralph/current-events` (`crates/ralph-api/src/event_watcher.rs`). The engine writes a different stream.

`HatRegistry`, `EventBus`, and `EventLogger` are still in the tree. They are not on the run path. `EventLogger` already appends `.ralph/events.jsonl`. That file becomes the authority. The autoloop journal does not.

Pi itself, as already researched in `specs/pi-agent-support/`, supports the loop this product wants: `--mode rpc` stays up and accepts `prompt`, `steer`, `follow_up`, and `abort`; the first NDJSON record is a `session` event with an id; `turn_end.message.usage` carries `cacheRead` and `cacheWrite`; `-c` / `--continue` resumes. Ralph's `RpcCommand` enum was copied from that protocol. The successor uses it directly instead of reimplementing it over autoloop.

PR `#377` (open against `main`, not in this branch) is relevant once Ralph owns the Pi process again: an open tool call or an in-flight model request is not inactivity. The ceiling is `adapters.pi.tool_timeout`. A silent Pi must not be killed the way print-mode used to kill it. Worker and foreground sessions both follow that rule.

## Keep, adapt, drop

### Keep

Coordination that autoloop never owned, and the hat vocabulary users already write:

- Worktrees, merge queue, loop registry, loop lock, loop history, landing, handoff, summary writer, and `completion_coord.rs`. A worktree loop is its own session and its own event log. The primary loop still holds `.ralph/loop.lock` and drains the queue.
- `HatConfig` and `HatRegistry`. Triggers, publishes, instructions, `description`, `extra_instructions`, `default_publishes`, `max_activations`, `disallowed_tools`, `concurrency`, `aggregate`.
- `ralph-proto` `Event` topics and the `RpcCommand` / `RpcEvent` vocabulary (`prompt`, `steer`, `follow_up`, `abort`, `set_hat`, hat changes, text deltas, tool calls).
- `pi_stream.rs` and `PiUsage` (`cacheRead`, `cacheWrite`, cost).
- Memories (`.ralph/agent/memories.md`) and tasks (`.ralph/agent/tasks.jsonl`). One store each.
- `file_lock`, diagnostics, preflight (minus engine-binary install), the TUI widgets, Telegram and web as transports.
- `ralph emit` as the agent's write path into the event log. The urgent-steer check in `emit_command` stays until steer text is in the turn that is about to hand off.
- `event_parser.rs` as the fallback when the model writes an `<event>` tag instead of calling `ralph emit`. Evidence checks on `build.done` stay attached to that topic.

### Adapt

- `cli_backend` Pi invocation. Foreground and workers use `pi --mode rpc` with a session. `--no-session` leaves this path.
- `rpc_events.rs`. It emits from the event log and the live Pi stream. It stops reading `AutoloopEvent`.
- `engine_state.rs`. Keep the fail-closed, no-symlink directory checks. Point them at `.ralph/pi/` and the event log. Stop creating `.ralph/autoloop` and the `.autoloop` symlink.
- `urgent_steer.rs`. Records become `user.steer` events. Pi `steer` is the delivery mechanism while a turn is in flight.
- TUI source (`ralph-tui/src/autoloop_source.rs`). It tails the event log and Pi deltas. The input box is part of the product, not a quit key.
- `autoloop_robot.rs`. Becomes a non-blocking steer and cancel bridge. `ask.pending` is not a lock.
- `ralph-bench`. Drives the supervisor.
- `event_watcher.rs`. Follows the authority log when the dashboard is touched. Not a first-slice requirement.
- `HatConfig.concurrency` and `aggregate`. They mean jobs and a wake-when-terminal hat, not autoloop role fields.
- Resume. `ralph resume` reopens the Pi session id and replays events the agent has not been shown. It does not exec `autoloop resume`.

### Drop

- The autoloop subprocess and its Ralph-side machinery: `autoloop_runner`, `autoloop_backend`, `autoloop_events`, `autoloop_event_tailer`, `autoloop_journal`, `autoloop_rpc`, `autoloop_preset_gen`, `autoloop_health`, `engine_install`, `engine_provision`, fake-autoloop fixtures as the CI gate.
- `core.engine` and `core.autoloop_preset` as runtime inputs.
- Per-hat `backend` and `backend_args`. One agent.
- The hooks engine. It does not run on this branch. Events are the extension point. Do not build the autoloop hook bridge.
- Scratchpad as something the supervisor injects. The event log and memories are the handoff.
- Cassette replay of the deleted in-house loop. New tests replay event-log fixtures and a fake Pi RPC script.
- The second task store and autoloop's project memory file.
- Treating "hatless" as a second engine. No hats means the default hat below.

`docs/migration/v3-autoloop-engine.md` describes the branch as it is. This document replaces that direction once accepted.

## Event model

The log is the commit. An in-memory bus is a projection for subscribers (TUI, RPC, Telegram). `EventBus` today is memory-only; it becomes that projection, fed by the append.

### Record

```json
{
  "id": "01JABC",
  "seq": 42,
  "ts": "2026-10-11T20:00:00Z",
  "topic": "job.done",
  "payload": "tests passed",
  "source": "builder",
  "target": "reviewer",
  "causation": "01JABA",
  "idempotency_key": "test:cargo-test"
}
```

`seq` is assigned under `file_lock` as the line is appended to `.ralph/events.jsonl`. Readers store `applied_seq`. A crash mid-append leaves a torn last line; readers ignore a trailing partial line and resume after the last good record (the bug the old `event_reader` had against autoloop's journal).

Hat topics are open strings constrained by `publishes`. Lifecycle topics are closed. The supervisor writes every lifecycle topic except the three a hat is allowed to request: `job.enqueue`, `ask.raised`, and `loop.complete`. Those three still have to be listed in `publishes`.

| Topic | Meaning |
|---|---|
| `run.start` | Session is up. Payload is the user goal. |
| `run.cancelled` | Cancel finished. Conversation still open. |
| `run.completed` | Supervisor accepted completion. |
| `user.message` | User spoke while the foreground was idle. Starts a turn. |
| `user.steer` | User spoke during a turn. Delivered with Pi `steer`. |
| `hat.activated` | Supervisor chose a hat for the next prompt. |
| `hat.exhausted` | `max_activations` tripped. Payload names the hat. |
| `event.rejected` | Agent emitted a topic outside `publishes`, or a duplicate idempotency key. |
| `job.enqueue` | Hat request. Supervisor persists it, then runs the job. |
| `job.started` / `job.progress` / `job.done` / `job.failed` / `job.cancelled` | Supervisor-only job lifecycle. |
| `ask.raised` | A question for the user. Does not pause the supervisor. |
| `ask.answered` | The user's answer, when it arrives. |
| `loop.complete` | The hat declared the goal done. |

`ralph emit TOPIC PAYLOAD` appends one record and returns `id` and `seq`. Payload is a string or a JSON value, which `ralph emit` already accepts. The agent does not invent ids. A `target` forces the next hat. Otherwise the existing rule stands: each trigger string maps to exactly one hat (`AmbiguousRouting` in `config.rs`). Overlapping globs keep the bus rule, a specific subscription ahead of a `*` fallback.

A topic outside the active hat's `publishes` is refused. The supervisor appends `event.rejected` and steers the refusal into the open turn. That is the backpressure gate. `build.done` still has to carry the evidence `event_parser` already knows how to read; a failed check is an `event.rejected`, not a silent drop.

If a turn ends with no successful emit and the hat sets `default_publishes`, the supervisor appends that topic. The same rule classic Ralph had, enforced here because the preset generator dropped it.

## Hats

A hat is data: when it wakes, what it may say, and the instructions pasted into the user message for that wake.

```yaml
hats:
  builder:
    name: "Builder"
    description: "Implements the current plan"
    triggers: ["plan.ready", "review.changes"]
    publishes: ["build.done", "build.failed", "job.enqueue"]
    max_activations: 8
    disallowed_tools: ["bash"]   # refused on the foreground session
    instructions: |
      Implement the plan named in the event.
      If the next step is a long command or a wide edit, enqueue a job and stop.
```

The supervisor picks the hat. The model does not pick its own hat. `set_hat` from the user appends a steer and forces the next wake.

Switching hats does not spawn a process and does not edit the system prompt. The next user message in the same Pi session is:

```text
Hat: builder
<instructions>

Event 01JAB9 plan.ready
<payload>

Open jobs:
- 01JABC test:cargo-test  running

You may emit: build.done, build.failed, job.enqueue
```

`disallowed_tools` is enforced by passing Pi `--tools` for that turn's process only when the session is a worker started with a restricted set. The foreground session's tool list is fixed at start (cache). A foreground hat that disallows a tool gets the restriction in the wake message, and the supervisor rejects a tool call that violates it before the result is trusted. File-write audit from v2 stays for hats that disallow `edit` or `write`.

`max_activations` increments on `hat.activated`. The next matching trigger appends `hat.exhausted` and skips the prompt.

### Default hat

Empty `hats:` is one hat, not a special loop:

```yaml
ralph:
  name: Ralph
  description: Works the objective until it is done
  triggers: ["run.start", "user.message", "job.done", "job.failed", "ask.answered"]
  publishes: ["loop.complete", "job.enqueue", "ask.raised"]
```

### Completion

The run completes when a hat emits `loop.complete` and no task is open. Open tasks produce a refusal, not a warning and not a second store's silent override. The legacy completion-promise string is accepted as an alias that the supervisor rewrites into `loop.complete`, so old prompts keep working. `event_loop.max_iterations`, `max_runtime`, and `max_cost` are enforced by the supervisor between turns. `max_consecutive_failures` is enforced again; v3 dropped it only because autoloop had no equivalent.

## Durability on Pi

### Layout

```text
.ralph/pi/session.json     # foreground session id, cwd, woken_through_seq
.ralph/pi/jobs/<job_id>/   # worker session id, pid, brief
.ralph/events.jsonl        # authority
.ralph/agent/memories.md
.ralph/agent/tasks.jsonl
```

`session.json` stores the id from Pi's `session` event. Resume launches `pi --mode rpc` against that session (`--continue` / the session id Pi accepts). State lives in the workspace so a worktree does not resume another loop's agent.

### Crash

1. Rebuild the job table and activation counts by folding `events.jsonl`.
2. Reattach the foreground session. If the session file is gone, start a new one and send a single recovery message: goal, current hat, open jobs, last 20 events. Do not replay the whole transcript into a fresh session; the old transcript is gone.
3. A job in `started` whose process is dead: if it produced no `job.progress` and `attempts < 2`, enqueue the same `idempotency_key` once more. Otherwise append `job.failed` with reason `crash` and wake the hat. No further automatic retries.
4. Prompt the foreground only for events with `seq > woken_through_seq`. Update that cursor after Pi accepts the prompt (`response` success). A duplicate delivery of an already-applied seq does not happen on the happy path; if it does, the wake message carries the seq and the hat instructions say to ignore a seq it has already handled.

### Idempotency

- Append-only log. Consumers are seq cursors.
- `job.enqueue` with an `idempotency_key` that already names a non-terminal job returns that job.
- A second `job.done` or `job.failed` for the same job id is ignored.
- The supervisor is not a git transaction manager. A worker that commits must tolerate being started twice. The job brief says that. Cancel does not `git reset`.

### What "resume" means

`ralph resume` and `ralph run --continue` are the same operation: attach to the live conversation. They do not start iteration N+1 of a dead autoloop run. If the user wants a new goal, that is a new session.

## Interactivity

The input path is always available. A running turn, a running job, and an unanswered question do not close it.

| User action | Foreground idle | Turn in flight | Jobs running |
|---|---|---|---|
| Message | `user.message`, then a hat wake | Pi `steer` (`user.steer`) | same, jobs keep going |
| Cancel | no-op on the turn | Pi `abort`, then the prompt returns | abort every worker, append `job.cancelled`, then `run.cancelled` |
| Quit | exit | abort, then exit | abort workers, then exit |
| Answer an ask | `ask.answered` | steer the answer | same |

Cancel is not quit. After cancel the user is still in the session and can say what to do next. A second signal, or an explicit quit, exits.

Asks: a hat may emit `ask.raised`. The UI shows it. The supervisor keeps accepting input and keeps running jobs. Nothing in the core waits. A hat that truly cannot proceed simply does not enqueue the next step. When `ask.answered` arrives it wakes the hat like any other event.

### Surfaces

One protocol. Pi speaks RPC; Ralph's stdin/stdout RPC is that conversation plus the bus.

- **CLI.** `ralph` with a TTY attaches to the session. `ralph run -p "..."` starts (or continues) and attaches. `--detach` enqueues `run.start` and returns the session id. `ralph attach` reconnects.
- **TUI.** Transcript, hat badge, job strip, input on the bottom row. Enter sends. Ctrl-C cancels. Ctrl-D quits. Job completions render as lines in the transcript and wake the agent.
- **RPC.** Existing `RpcCommand` values, plus `cancel_job` and `answer`. Outbound events are the real Pi stream (`TextDelta`, `ToolCallStart`, `ToolCallEnd`, usage on turn end) and every bus record as `OrchestrationEvent`. The coarse "one blob per iteration" mapping goes away.
- **Telegram and web.** Same commands: message, steer, cancel, answer. Progress notifications stay non-blocking (`ralph tools interact progress`). They are not on the critical path of M1–M3.

The TUI is a client of RPC, not a second loop. Headless and TUI cannot drift the way the autoloop event mapper and the TUI source can drift today.

## Background work and concurrency

Pi is one turn at a time per session. That is a feature. The foreground session is the conversation. Parallelism is the job queue.

A job:

```json
{
  "topic": "job.enqueue",
  "idempotency_key": "test:cargo-test",
  "payload": {
    "hat": "builder",
    "brief": "Run cargo test -p ralph-core. Report pass or the first failure.",
    "wave_id": null
  }
}
```

The supervisor starts `pi --mode rpc` in `.ralph/pi/jobs/<id>/` with a short system prompt and the brief. The worker does not get the foreground transcript, the hat catalog, or the right to append arbitrary topics. It writes a result. The supervisor appends `job.done` or `job.failed`. Waking the parent hat is the supervisor's decision, from triggers.

Default cap: one foreground turn, two workers (`jobs.max_parallel`). Hat `concurrency` raises the cap for jobs wearing that hat. Hat `aggregate: wait_for_all` means the hat does not wake on each `job.done`; it wakes once when every job sharing `wave_id` is terminal, or when `aggregate.timeout` fires with whatever has finished. That replaces waves. There is no `ralph wave` command.

Foreground instructions, including the default hat, tell the agent to enqueue anything that is a long command or a multi-file change, then yield. The user can say "do it here." The supervisor allows a foreground tool call in that case. Steer still works, because Pi `steer` exists. The input box still works. The turn occupies the session until it ends or the user cancels.

Worker limits use the `#377` rule: open tool or open model request extends to `adapters.pi.tool_timeout`; true silence with nothing open dies at `adapters.pi.timeout`.

Cancel walks every non-terminal job, sends Pi `abort`, then kills the process group. Each gets `job.cancelled`. Workers do not get a vote.

## Prompt cache

The foreground system prompt is written once, at session start, and then frozen:

1. Identity. One agent, hats are modes, the user may interrupt.
2. Hat catalog, sorted by id: id, description, triggers, publishes. Not the instruction bodies.
3. Emit and job contracts, in a fixed wording.
4. Tool list. Pi's default tools, fixed for the session.

Hat instruction bodies, the triggering event, the open-job table, a bounded memory and task suffix, and the latest steer are the new user message. They are supposed to be a cache miss. Everything before them is supposed to be a hit.

Rules that follow from that:

- Do not change the system prompt when the hat changes. `--append-system-prompt` per hat would bust the cache on every switch.
- Do not rewrite or reorder history. The supervisor does not call Pi `compact`. If Pi compacts itself, record `session.compacted` and accept one miss.
- Do not paste worker transcripts into the foreground. Paste the `job.done` payload, truncated.
- Do not inject skills, memories, or the event log into the prefix. Skills and memories are suffix, under a token budget. Prior wakes are already in the transcript; a new wake carries the new event only.
- Workers use a different, tiny system prompt. They are separate sessions, so they cannot invalidate the foreground cache. That isolation is the reason jobs are not "more turns on the same session."

Acceptance for the cache slice: on the second wake of a session, Pi's `turn_end` usage reports `cacheRead` greater than zero, and a hat switch does not drop `cacheRead` to zero. `PiUsage` already has the fields.

## Migration

### From v2 `ralph.yml`

| Input | Successor |
|---|---|
| `hats.*.triggers/publishes/instructions/description/extra_instructions` | Unchanged. |
| `default_publishes`, `max_activations`, `disallowed_tools`, `timeout`, `concurrency`, `aggregate` | Enforced. v3 accepted these and dropped them on the floor. |
| `hats.*.backend`, `backend_args` | Config error. One Pi. |
| `cli.backend` other than `pi` | Config error on this path, with the list of removed backends. |
| `event_loop` budgets, `completion_promise`, `starting_event`, `required_events` | Kept. Promise string aliases `loop.complete`. |
| `memories`, `tasks` | Same files, suffix injection, and the completion rule above. |
| `hooks` | Config error. Point at events. |
| Scratchpad | Ignored. The log is the handoff. |
| `ralph wave` | Already gone. Use `concurrency` and `aggregate`. |

### From v3 configs and ops

| Input | Successor |
|---|---|
| `core.engine`, `core.autoloop_preset` | Config error. Hats are the workflow. |
| `ralph resume`, `run --continue` | Pi session plus event cursor. |
| `.ralph/autoloop/`, `.autoloop` symlink | Not created. Leftover directories are unused. |
| `ralph run --rpc` | Same command. Richer events. |
| Autoloop task and memory files | Not read. |
| Dashboard `.ralph/current-events` | Stale until the watcher moves. Documented as unwired, not silently wrong. |

Presets that are hat YAML keep loading through `-H`. Presets that are only an autoloop TOML topology have to be rewritten as hats. No dual-engine flag.

## Milestones

Each milestone is shippable on its own and leaves `ralph run` on the previous engine until M6. New code lives beside `autoloop_engine` until the cut.

**M1. One session.** Spawn `pi --mode rpc`, record the `session` id, round-trip one user message, stream text to stdout. `kill -9` then `ralph resume` reattaches and does not repeat a finished turn. Cancel aborts and returns to a prompt. No hats, no jobs.

**M2. Log and hats.** Authority `events.jsonl` with `id` and `seq`. Load `HatConfig`. A wake is a user message in the same session. `ralph emit` routes. Enforce `publishes`, `default_publishes`, `max_activations`, and `build.done` evidence. Empty hats get the default hat. Completion requires `loop.complete` and no open tasks.

**M3. Jobs.** `job.*` events, worker sessions, `jobs.max_parallel`, idempotency keys, the one crash retry, cancel-all. `concurrency` and `aggregate` implemented as waves of jobs. Foreground copy tells the agent to enqueue long work.

**M4. Clients.** TUI input always live. Steer maps to Pi `steer`. RPC emits live deltas and bus events. Telegram and web send steer, cancel, and answers. An ask does not block.

**M5. Cache.** Frozen system prompt. Persist `cacheRead` / `cacheWrite`. A test asserts the second wake and a hat switch still read cache.

**M6. Cut.** `ralph run` uses the supervisor. Delete the autoloop spawn path, preset generator, engine installer, symlink, and both autoloop RPC mappers. v2 configs load under the table above. Worktree, merge, and landing run off `run.completed` / cancel / failure. Event-log fixtures replace fake-autoloop fixtures.

## Open questions

1. Pi binary. This design assumes the CLI in `specs/pi-agent-support/` (`--mode rpc`, `session` event, `-c`/`--continue`, `cacheRead`/`cacheWrite`). Confirm that is the harness, and whether session files must be copied under `.ralph/pi/` so resume works off this machine.
2. Jobs are separate sessions so the foreground stays short and cacheable. Is there a case that must share the foreground transcript instead?
3. May a background job pin a different Pi `--model` in v1, or is one model the rule until M6 ships?
4. Completion is `loop.complete` and zero open tasks, with the promise string as an alias. Should "zero open tasks" alone be enough, the way some v2 loops treated the task list?
5. Hooks become a config error. Are there hook commands that have to survive as event subscribers in M6?
6. Telegram in M4, or after the TTY and RPC loop have been used for real?
7. Accepting this doc: does autoloop feature work on `integration/v3-prerelease` stop, or do both tracks continue until M6?
8. The "do it here" escape lets a long tool run in the foreground while steer still works. Is that the right escape, or should long foreground tools be refused?
9. One automatic retry, only for a crash before any `job.progress`. Should auth and quota stay inside Pi and never surface as job failures the hat has to handle?
10. The web dashboard keeps reading `.ralph/current-events` until a later change. Is a loud warning enough through M6?
