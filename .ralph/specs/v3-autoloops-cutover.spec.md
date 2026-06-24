---
status: in-progress
created: 2026-06-23
updated: 2026-06-24
bead: ralph-orchestrator-v3-autoloops-backend-a7e
related:
  - feature-parity.spec.md
  - v1-v2-feature-parity.spec.md
upstream_tracking: https://github.com/mikeyobrien/autoloop/issues/29
---

# v3 Cutover Spec: autoloop as ralph's orchestration backend

## Implementation status (2026-06-24)

Branch `feat/v3-autoloops-backend`. The autoloop engine is **wired, default, and
coordinate-complete**; the in-house engine remains behind `core.engine = "ralph"`
as an escape hatch until parity-gated deletion (slice 8) can land green.

**Done & verified green:**
- Native `--events` NDJSON consumption + journal tailer + `AutoloopRunner`
  (`ralph-adapters`: `autoloop_events.rs`, `autoloop_runner.rs`, `autoloop_journal.rs`).
- `partial-line` bug in `event_reader.rs` fixed (read_until + real byte counts).
- CLI engine wiring: `autoloop_engine.rs` (subprocess driver + stopReason→TerminationReason map)
  and `autoloop_preset_gen.rs` (generate an autoloop preset from ralph hats, incl. hatless fallback).
- **Default flipped** to `core.engine = "autoloop"` (`config.rs::default_engine`); `main.rs` branches at the
  single read site. Loop-running integration tests pinned to `engine: "ralph"`.
- **Completion-coordination parity bridge** (`completion_coord.rs`): the autoloop path now runs the
  same engine-agnostic bookkeeping the in-house loop did — summary file, loop history, merge-queue
  state transitions, landing/enqueue via `LoopCompletionHandler`, primary-loop queue drain, registry
  deregister, termination banner. This closes the **parallel-loop / merge-queue** parity gap, ralph's
  core reason to exist. The module deliberately has zero `loop_runner`/engine coupling so it survives
  the engine deletion.
- Upstream blockers **#30/#31/#32/#33/#36** implemented + adversarially reviewed in autoloop (PRs #40/#41/#42),
  proven via a live HITL round-trip.
- Suites: `ralph-core` 881 ✅ · `ralph-cli` bin 469 ✅ · `integration_events_isolation` 13 ✅ ·
  `integration_resume` 5 ✅.

**Deletion gating (why slice 8 is not yet landed):** removing the in-house engine deletes ~21K lines
(`event_loop` ~3.4K + its tests ~5.3K + `hatless_ralph` ~3K + `wave_*` + `hat_registry` + `event_bus`)
**and** requires rewiring every remaining caller of `run_loop_impl` (run, **resume**, bot daemon) plus
direct `EventLoop` users (`rpc_stdin` RPC mode, `ralph-bench`, the TUI's event observers). Those paths
depend on parity features autoloop **has not yet implemented**: resume/hooks (#38), waves (#35), cost
telemetry (#34), the stopReason contract (#37). Deleting before those land would lose real features
with no replacement — violating the cutover's feature-parity requirement. **Slice 8 is therefore
blocked on autoloop landing #34/#35/#37/#38/#39 and PRs #40–#42 merging**, all tracked as GitHub
issues per the cutover plan.

## Charter

Replace ralph's in-house orchestration engine (`ralph-core/src/event_loop`, `hatless_ralph`, `hat_registry`, `event_bus`, `wave_*`) with the **autoloop runtime** (`@mobrienv/autoloop`, Node/TS, v0.8.0) as the execution backend. ralph becomes the thin coordination/RObot/UI layer; autoloop owns loop execution, topology/role dispatch, completion, budgets, and agent-subprocess execution.

ralph already *consumes* autoloop TOML presets via `preset_source.rs:TomlPresetSource` (905 lines — the existing config-translation seam). v3 inverts the relationship.

## Integration shape (decision)

**Chosen: autoloop CLI as a Node subprocess loop engine + ralph as the thin layer** — mechanically Option A (spawn `autoloop run`, observe the structured event stream, bridge control via the control channel), implemented atop ralph's existing hardened spawn/stream stack (`ralph-adapters` `cli_backend.rs`/`cli_executor.rs`/`stream_handler.rs`, `ralph-core/src/event_reader.rs`).

**Rejected alternatives:**
- **Embed a JS engine in Rust** (deno_core/rquickjs/boa): infeasible — the harness is a Node app with ~76 `node:*` call sites and native deps (`@anthropic-ai/claude-agent-sdk`, `@agentclientprotocol/sdk`); bare JS engines lack `child_process`/`fs`, and deno_core's Node-compat effectively re-embeds Node. Research project, not an integration.
- **Reimplement autoloop semantics natively in Rust**: defeats the charter — keeps the in-house engine and perpetuates dual maintenance. ralph is already ~80% here, which is exactly why it must not be the path.

> ⚠️ **Feasibility caveat (the reason this is `draft`, not `approved`):** the chosen shape depends on cross-process seams autoloop **does not expose today**. The structured `LoopEvent` stream is in-process only (`run.ts:88` hardwires `onEvent: cliPrintEvent`; no `--events`/`--json`), `onEvent` is non-blocking (`events.ts:58`), the journal is unversioned and timestamp-less, and the `command` backend yields no cost telemetry or live control. **The cutover is gated on the upstream autoloop work tracked in [autoloop#29](https://github.com/mikeyobrien/autoloop/issues/29) (#30–#39).** A subset can proceed in parallel (read-only observation, the `event_reader` fix, preset-translation parity), but flipping the default requires the upstream blockers landed.

## Subsystems: delete vs keep

**Delete (autoloop owns these in v3):**
- `event_loop/mod.rs` (`EventLoop`, `next_hat`, `process_output`, `check_completion_event`, `request_completion_from_text_fallback`, `TerminationReason` driver) + `event_loop/tests.rs` (~5,340 lines)
- `hatless_ralph.rs` (`HatlessRalph`/`HatTopology` routing + prompt assembly), `loop_completion.rs`
- `wave_detection.rs`, `wave_prompt.rs`, `wave_tracker.rs`
- `event_parser.rs` agent-output routing portions
- `ralph-proto/src/event_bus.rs` + `hat_registry.rs` (routing core, superseded by autoloop topology)

**Keep (ralph's thin layer; autoloop verifiably lacks these):**
- `worktree.rs` + `workspace.rs`; `merge_queue.rs`; `loop_registry.rs` + `loop_lock.rs` + `loop_history.rs` + `loop_name.rs`
- `ralph-telegram/*` (RObot HITL) + `web_robot_service.rs`; `ralph-api/*` (RPC/MCP control plane)
- `ralph-tui` crate; `backend/ralph-web-server` + `frontend/ralph-web`
- `preset_source.rs` (now the config bridge to the autoloop CLI); `event_reader.rs` (repurposed to tail the autoloop journal); `skill_registry.rs`
- `memory_store.rs`/`task_store.rs` **only if** chosen as the canonical store (see #36)

## Parity matrix (condensed)

Status legend: ✅ covered · 🟡 partial (needs design/test) · 🔵 ralph-only (keep) · ⭐ autoloop gain

| Capability | Status | Note |
|---|---|---|
| Core loop / iteration engine | ✅ | autoloop `run()` owns it; ralph `EventLoop` deleted |
| Event routing / hat selection | ✅ | ralph pub/sub triggers → autoloop `[handoff]` table (already mapped by `preset_source`) |
| Completion (event + required_events + promise) | ✅ | direct map to `iteration.ts:completedViaEvent`/`resolveOutcome` |
| Completion-must-be-LAST rule | 🟡 | autoloop tests set-membership, order-insensitive → #39 / accept change |
| Backpressure **evidence** gates | 🟡 | autoloop only does topology validation → **#33** |
| Scope / file-mod audit | 🟡 | autoloop validates emit topic, no git-diff tool audit → #39 |
| Thrash/stale/malformed termination | 🟡 | different heuristics; autoloop adds stall hard-stop |
| Hard limits (iters/runtime/cost) | ✅ | autoloop superset (per-iter cap, duration strings) |
| Waves / intra-loop fan-out | 🟡 | non-isomorphic config models → **#35** |
| Tasks store + completion gate | 🟡 | dual source of truth → **#36** |
| Memory store | 🟡 | dual source of truth → **#36** |
| Worktree isolation / automerge | ✅ | autoloop adds run-scoped mode + diff preview |
| Merge queue + primary-loop lock | 🔵 | KEEP |
| Parallel/concurrent loop registry | 🟡 | KEEP ralph lock+queue; mirror autoloop registry for status |
| HITL / Telegram RObot | 🔵 | KEEP — but needs re-drive over subprocess → **#32** |
| Live steering / interrupt / guide | 🟡 | maps to control channel; `command` backend = no-op → **#34** |
| Robot RPC / MCP control plane | 🔵 | KEEP |
| Prompt construction / topology render | ✅ | autoloop owns; ralph per-hat instructions flow via preset |
| Backend / agent adapters | 🟡 | **ownership tension**: autoloop drives agents in v3 → #34 |
| Cost tracking | 🟡 | re-derive from `backend.usage` (claude-sdk/pi only) → #34 |
| Config schema (layered, hot-reload) | ✅ | `preset_source` is the anchor; autoloop superset |
| Diagnostics / journal explorer | ✅ | autoloop `inspect`; ralph keeps its diagnostics |
| Doctor / preflight | ✅ | both; autoloop adds `triage` |
| TUI | 🔵 | KEEP — consume journal/RPC instead of in-process observers |
| Web dashboard | 🔵 | KEEP — re-point runner at autoloop subprocess |
| Dynamic chains | ⭐ | autoloop GAIN (budgeted lineage) |
| Profiles (repo/user fragments) | ⭐ | autoloop GAIN |
| Metareview hygiene pass | ⭐ | autoloop GAIN |

### Subsystems the first synthesis MISSED (adversarial-review additions)

These are fully-built ralph subsystems with **no autoloop equivalent** and must be explicitly handled:

| Subsystem | ralph location | Disposition |
|---|---|---|
| **Lifecycle hooks engine** (phase hooks, suspend/resume, I/O mutation) | `hooks/engine.rs`, `hooks/suspend_state.rs`, `config.rs:HooksConfig`, `cli/hooks.rs` | **Blocker** — acts inside the iteration; → **#38** |
| **Landing "land the plane"** (auto-commit, clean stashes, handoff.md, summary.md) | `landing.rs`, `handoff.rs:HandoffWriter`, `summary_writer.rs` | KEEP — runs after subprocess exits; redesign open-task-on-landing path |
| **Session record/replay** | `session_recorder.rs`, `session_player.rs`, `testing/replay_backend.rs` | **Critical** — underpins the replay smoke corpus CLAUDE.md mandates; needs new autoloop-journal replay fixture format |
| **PDD/SOP planning entry** (`ralph plan`/`task`) | `sop_runner.rs:BundledSop`, `planning_session.rs` | KEEP — already bypasses the event loop; distinct keep/port decision |
| **Agent-facing `emit`/`events` CLI** | `main.rs:EmitArgs`/`EventsArgs` (`.ralph/events.jsonl`) | Reconcile with autoloop emit/journal; update preset/role/skill docs |
| **Skill registry + auto-injection** | `skill_registry.rs`, `skill.rs` (backend/hat-filtered `include_str!` tool contracts) | Define how built-in `ralph-tools` skills reach the agent once prompt assembly moves to autoloop |
| **init / tutorial scaffolding** | `main.rs:InitArgs`/`TutorialArgs` | Emit autoloop-preset scaffold instead of `ralph.yml`; tutorial narrative obsolete |
| **UrgentSteerStore** (file-backed, backend-agnostic steer) | `urgent_steer.rs` | Reconcile with control channel; `command` backend can't interrupt mid-iteration |

## Test threshold (the bar before flipping the default)

Must-pass behaviors, each needing a parity test that survives deletion of `event_loop/tests.rs`:

1. **Completion via event + required_events** — terminates only after completion event AND all required events appear; premature completion rejected.
2. **Completion-promise fallback** — `LOOP_COMPLETE` completes only when no invalid event occurred that turn; **a promise inside an event tag does NOT complete** (port ralph `test_promise_inside_event_tag_does_not_complete` as a NEGATIVE assertion against autoloop's `resolveOutcome`).
3. **Open-task completion gate** — open (non-soft) tasks block completion **through the subprocess boundary** (no current integration test covers this; it lived only in the deleted `event_loop/tests.rs`).
4. **Backpressure evidence gates** — unsupported `build.done`/`review.done`/`verify.passed` rejected (mechanism per #33).
5. **Termination on limits** — `max_iterations`/`max_runtime`/`max_cost_usd`/stall terminate with correct `stopReason`; **exhaustively map ALL `stopReason` literals** (`stop.ts`+`index.ts`, incl. `interrupted`/`verdict_exit`/`verdict_takeover`) with a guard test that fails on a new one (#37).
6. **Wave fan-out** — `wave-review` scatter-gather produces equivalent output under autoloop's model (#35).
7. **Journal observability / live state** — ralph derives iteration/cost/routing/completion purely from tailing the journal, **including a torn/partial final line** (see Bug below) — BLOCKER contract test pinning schema + write-ordering (#31).
8. **Live control bridge** — steer/interrupt/guide round-trip (file → SIGUSR1 → autoloop), verified where the adapter supports it (#34 for `command`).
9. **HITL blocking ask** — loop blocks on `human.interact`, Telegram delivers, response unblocks the asking turn (#32).
10. **Merge-queue coordination** — multiple autoloop **subprocess** runs queue and merge correctly (current `integration_loops_merge.rs` drives the API directly — never spawns a loop; net-new e2e needed).
11. **Cost-budget termination** — `max_cost_usd` fires under a usage-emitting backend; graceful (documented) behavior under one that does not (#34).
12. **Preset translation fidelity** — same preset behaves identically: old ralph engine vs new autoloop subprocess.
13. **Resume / `--continue`** — interrupted run resumes without redoing completed work.

**Missing test substrate (prerequisite):** there is no fixture/recorder that spawns a real `autoloop run` and tails a real journal. A new autoloop-journal replay fixture format must be built before any of the above parity tests are startable (CLAUDE.md mandates replay-based smoke over live API).

## Confirmed ralph-side bug (fix independent of the cutover)

`event_reader.rs:read_new_events` uses `BufReader::lines()`, which yields an unterminated final line, then advances `self.position` by `line.len() + 1` (`+1 for newline`) for **every** line including a torn trailing one. Against autoloop's non-`fsync`'d `appendFileSync`, a tail between writes (a) mis-parses a truncated JSON fragment as `malformed`, and (b) permanently skips the remainder when it lands → **silent event loss**. Fix: do not consume an unterminated final line; add a partial-line tailing test. (This is the load-bearing component the whole cutover depends on.)

## Upstream autoloop blockers → issues

| Blocks cutover? | Gap | Issue |
|---|---|---|
| 🔴 yes | Versioned/durable journal contract | [#31](https://github.com/mikeyobrien/autoloop/issues/31) |
| 🔴 yes | Blocking HITL ask + response verb | [#32](https://github.com/mikeyobrien/autoloop/issues/32) |
| 🔴 yes | Backpressure evidence gates | [#33](https://github.com/mikeyobrien/autoloop/issues/33) |
| 🔴 yes | Canonical task/memory store | [#36](https://github.com/mikeyobrien/autoloop/issues/36) |
| 🟠 high | Structured `--events` stream | [#30](https://github.com/mikeyobrien/autoloop/issues/30) |
| 🟠 high | `command` backend cost + control | [#34](https://github.com/mikeyobrien/autoloop/issues/34) |
| 🟠 high | Lifecycle hooks engine | [#38](https://github.com/mikeyobrien/autoloop/issues/38) |
| 🟡 med | Wave config reconciliation | [#35](https://github.com/mikeyobrien/autoloop/issues/35) |
| 🟡 med | `stopReason` contract | [#37](https://github.com/mikeyobrien/autoloop/issues/37) |
| 🟢 low | Policy parity (last-event, file audit) | [#39](https://github.com/mikeyobrien/autoloop/issues/39) |

## Spike validation (2026-06-23, empirical)

Drove `autoloop run <autocode-preset> "…" -b <mock-wrapper> --set event_loop.max_iterations=3` against the deterministic mock backend (`dist/testing/mock-backend.js` + a JSON fixture) in a throwaway git repo. Confirmed against real output:

- **CLI is driveable & deterministic.** Ran 3 iterations, terminated `max_iterations`, exit 0, printed a parseable summary block (`run_id`, `iterations`, `stop_reason`, `journal`, `memory` paths).
- **Journal event sequence** per iteration: `loop.start` → `iteration.start` (carries `suggested_roles`, `allowed_events`, full `prompt`) → `backend.start` → agent-emitted topic (`tasks.ready`) → `backend.finish` (`exit_code`, `output`) → `iteration.finish` (`exit_code`, `elapsed_s`, `output`) → … → `loop.stop` (`reason`).
- **`stop_reason` is journal-derivable** (`loop.stop.fields.reason`) and in the summary — confirms the termination path (and the need for #37's contract).
- **No `ts` on iteration/backend/stop events** — confirms #31. Only `loop.start` carries `created_at`.
- **`progress` events (role + resolved `outcome`) are stdout text only**, rendered by `cliPrintEvent`, NOT in the journal — confirms #30 (ralph tailing the journal cannot see resolved routing/outcome).
- **Backpressure surfaces as `event.invalid`** in the journal when a role emits a disallowed topic — confirms the #33 framing.
- **CI-safe harness**: a single-token wrapper script around `node …/mock-backend.js` sidesteps `-b` multi-word quoting; this is the basis for the replay-fixture substrate (bead `.9`).

This turns the integration-shape recommendation from analysis into a validated path for Slice 1.

## Phased slice plan

1. ✅ **AutoloopBackend/AutoloopRunner** in `ralph-adapters` — spawn `autoloop run …`, capture `RunSummary` on exit; one e2e run with correct `stopReason`. *(done)*
2. ✅ **Repurpose `event_reader.rs`** to tail the autoloop journal (partial-line fix landed); native `--events` consumption added. *(done)*
3. 🟡 **Live control bridge** — steer/interrupt → ControlRequest + SIGUSR1. *(autoloop control verbs exist via #32; ralph-side steer wiring on the autoloop path is deferred — depends on #34 for the `command` backend)*
4. ✅ **HITL over subprocess** — ask detected, ralph layer blocks, Telegram delivers, response written back. *(proven via live round-trip; #32 implemented)*
5. ✅ **Preset pass-through parity** — preset generated from hats (`autoloop_preset_gen`), routing/completion mapped. *(done; round-trip tested)*
6. 🟡 **Reconcile tasks/memory** — canonical store via path-override (#36 implemented upstream); end-to-end completion-gate-across-boundary test still to add.
7. 🔵 **Waves** — autoloop owns role concurrency natively; ralph `wave_*` becomes obsolete under autoloop. Gated on #35.
8. 🔴 **Delete** the in-house engine + rewire `run_loop_impl`/`EventLoop` callers (run/resume/bot/rpc/bench/tui), then drop `engine: "ralph"`. **Blocked** on autoloop #34/#35/#37/#38/#39 + PRs #40–#42 merging (see *Deletion gating* above).

Newly added this cutover (not in the original plan): **completion-coordination parity bridge** (`completion_coord.rs`) so the autoloop path keeps merge-queue/landing/registry parity *before* the engine is deleted — the precondition that makes slice 8 a pure deletion rather than a deletion+reimplementation.
