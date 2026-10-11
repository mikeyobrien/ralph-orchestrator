# V3 successor: one agent, many hats

Status: proposal, revised. Anchored to `integration/v3-prerelease` at `a01826d`, and to `@earendil-works/pi-durable` as of `earendil-works/pi` commit `8a9c051` (`packages/durable`). The package is experimental: the API changes without notice. This revision replaces the earlier draft that drove `pi --mode rpc` as a subprocess and invented a Ralph event log, job queue, and crash retry.

No code in this change. Implementation specs, when this is accepted, go in `.ralph/specs/` and cite this file.

## Vision

Ralph is an opinionated workflow on a [pi-durable](https://github.com/earendil-works/pi/tree/8a9c051f80090895abbc64262bc90d95b42103c4/packages/durable) Harness. One root conversation is the agent. Hats are modes that conversation wears. The user can talk, interrupt, and steer at any moment, including while a run is busy. Long work is a durable child task or a subagent conversation. Cancel aborts. A crash is `Harness.open` on the same storage plus `resume()`.

pi-durable already commits every conversation entry, document edit, and task checkpoint before it is shown. It already queues input on a busy conversation (`pi.inbox`), watches commits, derives agent events, compacts, resets, tracks usage, and resumes a run the last process left unfinished. Ralph does not build a second copy of any of that.

What Ralph adds is the hat graph: which instructions are active, which events a hat may publish, which hat wakes next, and when the goal is done. That policy is a pi-durable extension plus one document. It is not a new engine.

## Non-goals

- A second durability layer. No `.ralph/events.jsonl` authority, no `session.json`, no Ralph job table, no crash-retry loop beside the Harness.
- Driving the `pi` CLI (`--mode rpc`, `--mode json`, `--no-session`). That is a different process model. `specs/pi-agent-support/` describes it; this design does not use it.
- Autoloop as a hidden engine. Role dispatch, journals, and `autoloop control` do not sit under this loop.
- A swarm of peer agents. The root conversation is the one the user talks to. Other conversations are children with a frozen brief.
- A multi-backend product. Claude, Kiro, Gemini, Codex, Copilot, OpenCode, Roo, ACP, and OMP are out of this path. The model is whatever the root conversation's `pi.agent` selects through pi-ai.
- Rewriting pi-durable, wrapping it in a Rust port, or embedding it through N-API.
- Pixel parity with v2 shell hooks, per-hat backends, the wave CLI, and cassette replay.
- Rebuilding the web dashboard in the first slices.

## What is wrong with current v3

`ralph run` always enters `autoloop_engine::run_autoloop_engine` (`crates/ralph-cli/src/main.rs`). Hats are lowered to an autoloop preset (`crates/ralph-cli/src/autoloop_preset_gen.rs`): one role file per hat, a fresh backend process per iteration. The Pi mapping launches `pi -p --mode json --no-session` (`crates/ralph-adapters/src/cli_backend.rs`), which throws away Pi session identity.

The restored surfaces are adapters onto that subprocess:

- `--rpc` (commit messages tag this `#343` / `#368`) translates autoloop's coarse `--events` stream into `RpcEvent`. Two mappers exist (`crates/ralph-cli/src/rpc_events.rs`, `crates/ralph-adapters/src/autoloop_rpc.rs`). A turn arrives as one blob of `backend.output`.
- `ralph resume` (tagged `#344`) stores an autoloop `run_id` under `.ralph/autoloop` and execs `autoloop resume`. `engine_state.rs` plants a `.autoloop` symlink so autoloop's CLI can find that state.
- Human questions are `ask.pending`. Autoloop blocks until `autoloop control respond` (`crates/ralph-cli/src/autoloop_robot.rs`).
- Configured lifecycle hooks are inert (`crates/ralph-core/src/preflight.rs`).
- `generate_preset` writes `id`, `name`, `emits`, `concurrency`, `aggregate`, and `prompt_file`. It drops `backend`, `default_publishes`, `max_activations`, `disallowed_tools`, and `timeout`. Skills and memories are not injected. Ralph tasks and autoloop tasks are two stores (`.ralph/specs/v2-engine-autoloop-gap-analysis.md`).
- The web watcher still follows `.ralph/current-events` (`crates/ralph-api/src/event_watcher.rs`).

`HatRegistry`, `EventBus`, and `EventLogger` are not on the run path. `EventLogger`'s `.ralph/events.jsonl` does not become the authority. The Harness transcript and documents do.

PR `#377` (open against `main`) stops Ralph's CLI executor from treating a silent Pi tool call as idle. This path does not use that executor. pi-durable checkpoints a tool as a task and bounds streams with `settings.stream.timeoutMs`. Do not reintroduce a silence killer in front of the Harness.

## Rust and TypeScript

pi-durable is TypeScript. Extensions, tools, hooks, and tasks are TypeScript. The README states that one process owns a storage at a time and there is no cross-process locking. A Rust thread cannot open the same SQLite while the Harness runs, and it cannot call `submit` or `commit` except through a process that holds the Harness.

Three shapes:

| Shape | What it means | Verdict |
|---|---|---|
| TS core, Rust TUI/CLI over a protocol | A Node host opens the Harness, installs the Ralph extension, and owns hat routing. `ralph` spawns that host. Stdin is commands. Stdout is `watchEvents` batches plus task-graph frames. Rust renders and forwards keystrokes. Rust never opens the database. | **This one.** |
| Rewrite Ralph in TypeScript | The same host, and also a new TUI, merge queue, worktrees, doctor, and Telegram. | The end state only if the Rust client stays a thin skin. It is the wrong first slice. The ratatui TUI and git coordination are large and are not the pivot. |
| Rust stays the orchestrator and shells out to a TS sidecar for "a turn" | Rust keeps hat selection, a queue, and a log, and asks the sidecar to run the model. | The autoloop mistake again. `pi.inbox`, task checkpoints, and `requestId` only stay true if the process that owns storage also decides what happens next. A second queue will diverge. |

The process boundary is allowed. A second brain is not. Headless `ralph run` is the host with no TUI. The TUI is a client that can die and reattach: the host is still the Harness, and `resume()` continues work the client never saw.

Distribution: the Rust binary spawns `node` with a pinned `@earendil-works/pi-durable`. Pin the commit this doc was read against (`8a9c051`) until a release exists. Do not vendor the package.

### Protocol

Commands, one JSON line each: `submit` (content, `whenBusy`, `requestId`), `abort` (`background` flag), `configure` (hat id), `answer` (a follow-up input). The host assigns `requestId`s the client did not, so a reattach does not double-submit.

Events out:

- `watchEvents` for the root conversation: `snapshot`, `message_*`, `tool_execution_*`, `inbox_update`, `compaction_*`, `usage_changed`, `agent_changed`. Spec §9.4. This stream covers one conversation, not its children.
- The same stream for each live child conversation. The task graph and the workflow document say which ids are live.
- `watchTaskGraph` frames (spec §9.5): live tasks, owner edges, `waiting` / `background` / `abortRequested`.
- Snapshots of the `ralph.workflow` document when it changes (`watchDoc`).

`RpcEvent` in `ralph-proto` can be a projection of those agent events for old clients. It is not a log, and it is not the protocol the host speaks to itself. Agent events are marked experimental in the package. Pin the commit, and treat a protocol break as a host change, not a reason to invent a parallel event bus.

## Map

| Ralph idea | pi-durable | Ralph still writes |
|---|---|---|
| The agent | `harness.root()`. One conversation, one `pi.provider` session id. | The decision to have exactly one root the user talks to. |
| A hat | `configure({ instructions })` on that root. The instruction body renders last, as the `instructions` section. | Hat YAML: triggers, publishes, instruction text, activation cap. |
| Hat with different tools | `configure({ modelTools })` or an extension added the way `27-plan-mode.ts` adds plan mode. | Which hats need a tool change. Default is none. |
| Hat with a different model or a long brief | A child conversation (`23-subagent-background.ts`, `28-reviewer.ts`). Fresh `pi.provider` id. | When work leaves the root. |
| Workflow event | Display entry `ralph.event` (no `model` field) plus `ralph.workflow.lastEvent`, both in the emit tool's commit. | The topic allow-list and who wakes next. |
| Current hat, counts, goal | Document `ralph.workflow`. | The schema and the routing that updates it. |
| User talks while busy | `submit` with `whenBusy: "steer"` or the default follow-up. Queue is `pi.inbox` (`20-inbox.ts`). | The choice that a user keystroke is a steer, not a new run. |
| UI | `viewState` / `watch` / `watchEvents` (`05-watches.ts`, `19-json.ts`, `21-late-join.ts`). | The TUI layout. |
| Background agent work | Subagent conversation owned by a background anchor, reporter posts the answer back (`23-subagent-background.ts`). | The tool the root calls to start one, and the brief. |
| Fan-out / waves | A task that creates children and commits `waiting` (`24-child-tasks.ts`). `allSettled` is `aggregate: wait_for_all`. `failFast` aborts the rest. | Hat `concurrency` as the cap. |
| Cancel | `conversation.abort(context, { background: true })`. | Ctrl-C means that flag. Background work does not survive cancel. |
| Resume | `Harness.open(same sqlite)` then `resume()`. | Nothing. |
| Idempotency | `requestId` on submit. `replay: "safe"` tools reuse nested calls after a crash. | `requestId`s derived from the logical action, not a Ralph retry loop. |
| Compaction, reset, usage | Built in. `pi.usage`, `harness.usage()`, `compact()`, `reset()`. | Budgets that read `pi.usage` and refuse another submit. |
| Hooks | Extension hooks on `GenerationTask` and `ToolTask`. | A guard hook for the emit allow-list. v2 shell hooks are not ported. |
| Memories and tasks | Documents, or a section that renders a file. | One authority. See migration. |

## Hats

A hat is data loaded from the existing `HatConfig` YAML. The host installs one extension, `ralph`:

```typescript
defineExtension({
  name: "ralph",
  sections: [
    section("identity", () => IDENTITY, { tag: false }), // frozen text
    section("catalog", () => hatCatalog, { tag: false }), // ids, descriptions, triggers, publishes
  ],
  tools: [emitTool],
})
```

`IDENTITY` and `hatCatalog` are pure functions of the config file. They do not read the clock, the inbox, or the open-task list.

The active hat's instruction body is the conversation's `instructions`, set with `configure({ instructions })` when the hat changes (`15-system-prompt.ts`, spec §7.4). `instructions` renders last. A change that keeps section order appends one `pi.system` delta with the new `instructions` value. The prefix the model already saw stays. That is the cache-safe switch.

Rules that follow from spec §7.4:

- Do not put the triggering event, job list, or time into a section. A section that changes every request appends a system delta and defeats the provider cache. Those facts go in the `submit` content.
- Do not reorder sections when the hat changes. An order mismatch deletes every section and re-adds them, which is a new baseline.
- Do not `reset()` to switch hats. Reset hides prior entries from the model. The transcript is the point of one agent.
- Do not `compact()` to switch hats. Compaction writes a new baseline on purpose. Leave compaction to the Harness settings.
- Keep the root tool list stable. A `modelTools` or tool-declaration change is its own system delta. Hats that only differ by prose do not touch tools. A hat that must be read-only uses the plan-mode shape (`27-plan-mode.ts`): add an extension and narrow tools for that stretch, and put the tool list back after. That is a deliberate delta, not the common path.
- Model and thinking level are request options, not prompt text (spec §7.4). Changing them on the root does not rewrite sections, and `pi.provider` survives a model change. Still: the root keeps one model. A cheaper reviewer is a child conversation (`28-reviewer.ts`), which gets a fresh provider session and cannot invalidate the root cache.

The host picks the hat after a run settles. The model does not. Each trigger string already maps to one hat (`AmbiguousRouting` in `config.rs`). The host reads `ralph.workflow.lastEvent`, configures `instructions` if the hat changed, and `submit`s the next wake with `requestId` `wake:<event id>`. A `target` on the event forces the hat. `max_activations` lives in the document. The next trigger records topic `hat.exhausted` and does not submit.

Empty `hats:` is one default hat, `ralph`, triggering on the goal, on user input, and on child reports, publishing `loop.complete`. Hatless is not a second runtime.

### Emit

The agent publishes by calling the `emit` tool. In that tool's commit the host writes two things, and only those:

- `ralph.workflow.lastEvent`, which routing reads.
- A display entry `{ kind: "ralph.event", data: { topic, payload } }` with no `model` field, so the UI has a line and the model is not handed a second user message in the middle of the turn. The model sees the tool result.

`kind` must not start with `pi.` (README, busy conversations). A topic outside the active hat's `publishes` fails the tool before the commit, and the model gets an error result. `build.done` still has to carry the evidence `event_parser` already understands. A failed check is that error result. If a run ends with no `lastEvent` and the hat sets `default_publishes`, the host commits that topic itself before routing. No extra model turn, and no side file.

User-visible notes that are not hat publishes use `submit({ type: "write", entry: { kind, data } })`. A write with only `kind` and `data` appends immediately, even mid-run.

### Completion

The run is done when a hat emits `loop.complete` and the `ralph.tasks` document has nothing open. Open tasks make the emit tool return an error. The legacy completion-promise string is accepted by the host as that same topic. `max_iterations` is a counter in `ralph.workflow`. `max_cost` reads `pi.usage`. `max_runtime` is host wall clock between submits. `max_consecutive_failures` counts generation failures the Harness surfaces as unanswered submissions. The Harness already retries provider errors via `settings.retry`; Ralph's counter is for runs that still fail after that.

## Durability

Storage is one SQLite file, `openNodeSqliteStorage(".ralph/harness.sqlite")`. WAL, `synchronous = NORMAL`: commits survive process death; the newest may be lost on power loss (README, storage). JSONL keeps the whole history in memory; it is for tests, not the product. One Harness per file. A worktree loop is another directory, so it gets its own file and its own process. The merge queue does not open that file.

Layout Ralph owns:

```text
.ralph/harness.sqlite    # the Harness. Conversations, entries, tasks, documents.
```

There is no `session.json`. The root conversation id comes from `harness.root()`. Provider cache affinity is `pi.provider`, which survives reopen, retry, reset, compaction, and model changes. A child or a fork gets a new id (README, persist and resume).

Resume is:

```typescript
const harness = await Harness.open(await openNodeSqliteStorage(path), options, context);
const root = await harness.root(context);
harness.resume();
```

Work left pending stays pending. Submitting or waiting also starts the scheduler. A retried `submit` with the same `requestId` returns the existing submission. `harness.submission(id)` reacquires a wait after restart. A tool that is not `replay: "safe"` does not rerun after a crash; the model gets an `interrupted` result with the output committed so far. Ralph does not add another retry on top. A storage error fails the Harness (`SessionFailed`); the host process exits and the Rust parent reopens. `harness.closed` is that signal.

`ralph.workflow` holds `{ hat, activations, goal, wokenEntry }`. Routing reads it. The transcript is the history. Folding a JSONL log to rebuild this is the design that was withdrawn.

## Interactivity

The input path is `submit`. A running turn, a running child, and an unanswered question do not close it. A busy root queues the submission in `pi.inbox`.

| User action | What the host calls |
|---|---|
| Message while idle | `submit({ type: "input", content, requestId })` |
| Message during a turn | `submit({ type: "input", content, whenBusy: "steer", requestId })`. Placed after the current tool round. |
| Message that should wait for the answer | default follow-up (`whenBusy` omitted). |
| Cancel | `root.abort(context, { background: true })`. Queued inputs are withdrawn. Background anchors and their conversations abort. The root stays open. |
| Quit | that abort, then `harness.close()`. |
| Answer a question | a follow-up or steer whose content is the answer. |

Cancel is not quit. After cancel the user is in the same root and can submit again. Ordinary `abort()` without `background: true` leaves background anchors running (`23-subagent-background.ts`). Ralph's cancel passes `background: true` because cancel stops everything. A future "leave it running" command can call the narrower abort. That is not the default.

Asks do not block the Harness. A hat that needs the user emits a `ralph.event` topic `ask` and stops enqueueing children. The inbox still accepts steers. Nothing in the host waits on Telegram.

### Surfaces

- **Host.** The TypeScript process above. Headless attaches stdout to the event stream.
- **CLI.** `ralph` with a TTY spawns the host and attaches. `ralph run -p "..."` submits the goal and attaches. `--detach` spawns the host and returns. `ralph attach` reconnects: the host is still up, or a new client opens nothing and a new host does `Harness.open` + `resume` if the previous host died.
- **TUI.** A client. Transcript from agent events, hat badge from `ralph.workflow`, job strip from the task graph. Enter submits. Ctrl-C aborts with `background: true`. Ctrl-D quits. Late join uses the `snapshot` event (`21-late-join.ts`); nothing is replayed from a Ralph log.
- **Telegram and web.** Same commands. They are clients. They are not on the critical path of the first three milestones.

The TUI is not a second loop. It does not tail `.ralph/autoloop-events.ndjson` or a private `pi-stream` file.

## Background work

The root conversation is one run at a time. That is the Harness rule, and it is what keeps the conversation intelligible. Parallelism is owned work.

**Agent-shaped work** (review this diff, implement this step) follows `23-subagent-background.ts`:

- A background anchor task owns the child conversation, so the root's idle wait and a plain abort stop at the anchor.
- The child starts as a copy of the root agent, then `configure` narrows it: no nested subagents, the brief as `instructions`, read-only tools when the hat says so.
- A reporter task submits to the child with `requestId: "job:<anchorId>"` and, when the child answers, submits a follow-up to the root with `requestId: "report:<anchorId>"`. A restart returns the same submissions.
- The child's transcript is not copied into the root. The report is the payload. That is what keeps the root cache and the root context small.

**Fan-out** (many files, then one synthesizer) follows `24-child-tasks.ts`. One parent task creates N children and commits `status: "waiting"` with `policy: "allSettled"` or `"failFast"`. The parent runs no code while it waits. Hat `concurrency` caps how many children the parent creates at once. Hat `aggregate.timeout` is host policy on that wait, not an autoloop field. There is no `ralph wave` command and no `job.enqueue` record outside the task graph.

A "run cargo test" job is a `replay: "safe"` tool on a child, or a child task whose phase execs through the conversation's `ExecutionEnv`, with its own idempotency. It is not a row in a Ralph queue. Nested tool calls already record their task id and, for a replay-safe caller, return the call a crash interrupted (`README`, calling tools from tools).

Cancel walks from the root with `background: true`, which aborts anchors, reporters, and payment-style children bottom-up (spec §5.4). Ralph does not kill a process group it invented. The Harness aborts the tasks; tool code must honor the context signal. Spec §13 says the Harness will not force-kill non-cooperative extension code inside the process. A bash tool that ignores abort is a tool bug, fixed in the tool, not with a supervisor SIGKILL table.

## Prompt cache

The stable prefix is the `ralph` sections: identity and the hat catalog, rendered identically on every request. The variable suffix is the newest user message (the wake) plus the model's reply. Hat instructions change as a one-key `pi.system` patch, which is the provider-cache behavior spec §7.4 describes: only what changed is sent again.

Acceptance for the cache slice is structural, checked against the transcript the way `15-system-prompt.ts` and `27-plan-mode.ts` print system entries:

- The second wake of a session, with the same hat, appends no `pi.system` entry.
- A hat switch appends one `pi.system` whose `sections` touch `instructions` and do not null out the catalog.
- A section render that includes a timestamp or the live task list fails that test.

`pi.usage` is the cost authority. Do not scrape a CLI stream for `cacheRead`. If a provider reports cache tokens inside pi-ai usage, they show up in `usage_changed`. The gate is the delta shape, which we control.

Child conversations are separate sessions with short prompts. They do not share the root's provider session id, so they cannot invalidate it.

## What stays from this repo

### Keep

Coordination the Harness does not do:

- Worktrees, merge queue, loop registry, loop lock, loop history, landing, handoff, summary writer, `completion_coord.rs`. Each worktree is its own Harness file. The primary loop still holds `.ralph/loop.lock` and drains the queue when the host reports the root done, cancelled, or failed.
- `HatConfig` and the YAML the user already writes: triggers, publishes, instructions, description, `extra_instructions`, `default_publishes`, `max_activations`, `disallowed_tools`, `concurrency`, `aggregate`. The loader can stay in Rust and pass the parsed config to the host, or move with the host. The schema stays.
- The TUI widgets, as a client of the protocol.
- Telegram and the web robot as transports that send `submit` and `abort`.
- Diagnostics and doctor, minus engine-binary install.
- `event_parser` evidence checks, called from the emit tool (port the predicate, or have the host reject a payload the Rust helper rejects).

### Adapt

- `ralph run` / `ralph resume` spawn or reattach the host. Resume is `Harness.open` + `resume()`, not `autoloop resume`.
- The TUI source (`ralph-tui/src/autoloop_source.rs`) consumes agent events and the task graph.
- `RpcEvent` becomes a projection written by the host for legacy clients.
- `urgent_steer` becomes `whenBusy: "steer"`.
- Memories and tasks move into documents. The CLI can project them back to files for `ralph tools` during migration. The document is the authority.
- `engine_state.rs` fail-closed path checks apply to the directory that contains `harness.sqlite`. They stop creating `.ralph/autoloop` and the `.autoloop` symlink.

### Drop

- Autoloop subprocess machinery: `autoloop_runner`, `autoloop_backend`, `autoloop_events`, `autoloop_event_tailer`, `autoloop_journal`, `autoloop_rpc`, `autoloop_preset_gen`, `autoloop_health`, `engine_install`, `engine_provision`, fake-autoloop fixtures as the CI gate.
- `core.engine` and `core.autoloop_preset`.
- Per-hat `backend` and `backend_args`.
- The v2 shell hook engine. It does not run on this branch. pi-durable hooks are the extension point.
- `pi_stream.rs` and `CliBackend::pi()` on this path, including `--no-session`.
- A Ralph-owned event log, job queue, session file, and crash-retry policy.
- Scratchpad injection. The transcript and `ralph.workflow` are the handoff.
- Cassette replay of the deleted in-house loop. Tests drive a Harness with the faux provider, as the package examples do.

`docs/migration/v3-autoloop-engine.md` describes this branch as it is. This document replaces that direction once accepted.

## Migration

### From v2 `ralph.yml`

| Input | Successor |
|---|---|
| Hat triggers, publishes, instructions, description, `extra_instructions` | Unchanged schema. Loaded into the `ralph` extension and `instructions`. |
| `default_publishes`, `max_activations`, `disallowed_tools`, `concurrency`, `aggregate` | Enforced in the host. v3 accepted these and dropped them in `generate_preset`. |
| `timeout` | Child-task and tool bounds via Harness settings, not a per-hat process kill. |
| `hats.*.backend`, `backend_args` | Config error. One root model. A child may pin a model in its own `configure`. |
| `cli.backend` other than the pi-durable host | Config error on this path. |
| `event_loop` budgets, `completion_promise`, `starting_event` | Host policy above. The promise string is `loop.complete`. |
| `memories`, `tasks` | Imported once into `ralph.memories` and `ralph.tasks`. Files become a projection. |
| `hooks` | Config error. An extension hook is the replacement. |
| Scratchpad | Ignored. |
| `ralph wave` | Already gone. `concurrency` and `aggregate` are child tasks. |

### From v3

| Input | Successor |
|---|---|
| `core.engine`, `core.autoloop_preset` | Config error. |
| `ralph resume`, `run --continue` | Reopen `.ralph/harness.sqlite`. |
| `.ralph/autoloop/`, `.autoloop` symlink | Not created. |
| `ralph run --rpc` | The host protocol. Legacy `RpcEvent` is a projection. |
| Autoloop task and memory files | Not read. |
| Dashboard `.ralph/current-events` | Unwired until a client speaks the protocol. Say so. Do not point it at a log we are not writing. |

Hat YAML presets keep loading through `-H`. An autoloop TOML topology has to be rewritten as hats.

## Milestones

Until M6, `ralph run` on this branch stays on autoloop. The host lands beside it. Each milestone is a program you can kill and reopen.

**M1. Host.** A TypeScript package opens SQLite, `root()`, submits one input, prints `watchEvents`. `kill -9`, then `Harness.open` + `resume()`, and the unfinished run continues. Same `requestId` does not double-submit. No hats.

**M2. Hats.** The `ralph` extension, frozen catalog section, `configure({ instructions })` on switch, `ralph.workflow` document, `emit` tool writing `ralph.event`. Trigger routing. `default_publishes`, `max_activations`, `build.done` evidence. Default hat when YAML is empty. Completion is `loop.complete` and no open tasks.

**M3. Children.** Background subagents as in `23-subagent-background.ts`. Fan-out as in `24-child-tasks.ts` with `allSettled` / `failFast` and a concurrency cap. Cancel is `abort({ background: true })`. Idempotency is `requestId` only.

**M4. Clients.** Rust TUI and CLI speak the protocol. Steer and follow-up. Task-graph strip. Telegram sends the same commands. A second client can attach after the first disconnects and get a `snapshot`.

**M5. Cache.** Tests on the faux provider assert the system-entry shape in the cache section. No timestamp in a section. Hat switch is a one-key delta.

**M6. Cut.** `ralph run` spawns the host. Delete the autoloop spawn path, preset generator, engine installer, symlink, and both RPC mappers. v2 YAML loads under the table above. Worktree, merge, and landing run off the host's terminal status. Faux-provider Harness tests replace fake-autoloop fixtures.

## Open questions

1. Pin `8a9c051`, or wait for a release tag before M1? The README says the API changes without notice. The host should depend on an exact version either way.
2. Is `configure({ instructions })` the hat switch you want for every same-model hat, with `27-plan-mode.ts` reserved for hats that change tools? A child conversation for every hat would throw away the root cache.
3. Default for "run the tests": a replay-safe bash on a child conversation, or a child task that never calls a model? Agent work stays on the example 23 shape either way.
4. The TUI protocol is experimental `watchEvents` plus the task graph. Raw `watch()` Chord ops are the normative stream. Prefer events unless you want the UI to apply Chord frames.
5. Ctrl-C is `abort({ background: true })`, so background work dies. A narrower abort leaves it running. Confirm cancel means everything.
6. v2 shell hooks become a config error. Any hook command that has to survive as an extension in M6?
7. Memories: a document the section renders, knowing that an edit appends one system delta. Is a mid-run memory write acceptable, or should memory edits wait until the root is idle?
8. Does accepting this stop autoloop feature work on `integration/v3-prerelease`, or do both continue until M6?
9. Spawn `node`, or bundle a runtime next to the Rust binary? One process must own the SQLite file.
10. The web dashboard stays unwired through M5. Is a warning enough, or does M4 include a thin client?
