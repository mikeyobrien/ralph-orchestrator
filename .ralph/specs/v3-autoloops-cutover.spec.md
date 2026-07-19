---
status: "migrated (implementation); release-gated by v3-ga-readiness.spec.md"
created: 2026-06-23
updated: 2026-07-19
bead: ralph-orchestrator-v3-autoloops-backend-a7e
related:
  - feature-parity.spec.md
  - v1-v2-feature-parity.spec.md
  - v3-ga-readiness.spec.md
release_gate: .ralph/specs/v3-ga-readiness.spec.md
upstream_tracking: https://github.com/mikeyobrien/autoloop/issues/29
---

# v3 Cutover Spec: autoloop as ralph's orchestration backend

## Implementation status (2026-06-24; historical snapshot)

> **Status scope:** `migrated` means the engine cutover was implemented. It is
> not GA approval. The canonical release gate and current acceptance/regression
> statuses live in [v3-ga-readiness.spec.md](v3-ga-readiness.spec.md). Where this
> historical migration narrative conflicts with that gate, the GA-readiness
> spec controls.

At this snapshot on branch `feat/v3-autoloops-backend`, the autoloop engine was
**wired, default, and coordinate-complete**; the in-house engine remained behind
`core.engine = "ralph"` as an escape hatch until parity-gated deletion (slice 8)
could land green.

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

## Test threshold (superseded by the GA R-matrix)

This was the pre-cutover parity checklist, not the live release bar. GA signoff
uses the AC tables and **R1–R28 regression matrix** in
[v3-ga-readiness.spec.md](v3-ga-readiness.spec.md). The GA spec owns status;
commit presence or a passing narrow test below does not by itself close an AC.

### Branch evidence mapped to the GA gate (2026-07-19)

`git log --oneline -25` and the named tests map the post-audit fixes as follows:

| GA area | Evidence now on this branch | Remaining gate |
|---|---|---|
| A1 / R1–R3 prompt delivery | `1e4cd01` passes inline/file prompts as the final autoloop positional argument; `run_inline_prompt_reaches_autoloop_as_positional_argument`, `run_prompt_file_contents_reach_autoloop_as_positional_argument`, and the inline-precedence test cover the CLI path. Merge-drain prompt/env forwarding is covered with the B1 integration path. The bot daemon applies its supplied prompt in `autoloop_engine::start_loop`. | Canonical closure, including the dedicated bot assertion R2, is per the GA spec. |
| B1 / R9 merge lifecycle | `5bf3105`; `process_queue_observes_full_lifecycle_and_does_not_remerge` observes Queued → Merging → Merged and proves a second drain does not respawn the merge. | Final AC status is per the GA spec. |
| B2–B3 / R10–R11 stop and crash coordination | `4c48a90`; signal-mapping unit tests distinguish Ralph-requested stops from external crashes, and `crash_runs_completion_coordination` checks history, registry cleanup, needs-review disposition, and lock reacquisition. | The PTY `q` entry-point assertion required by R10 remains per the GA spec. |
| A3 / R5 budgets | `6f3707a`; the explicit-preset integration test verifies CLI max-iterations precedence plus runtime/cost overrides before the prompt. | Partial: `max_consecutive_failures` is warned as unsupported rather than enforced; full A3 status is per the GA spec. |
| A4 / R6–R7 task integration | `a200c0f`; fresh-run marker/task-isolation coverage exists and generated presets receive task/memory instructions. | Partial by design: open Ralph tasks are observation-only and produce a warning; autoloop's canonical task store owns completion. The hard-gate wording and R7 closure remain per the GA spec. |
| A2 / R4 backend selection | `f5ea0fd`; generated presets carry the selected backend, while a CLI backend conflicting with an explicit preset fails before spawning autoloop. | Final AC status is per the GA spec. |
| D1–D2 / R25 dependency health | `41dcbc7`; `autoloop_health`, doctor, run/resume preflight, release/install wiring, and integration tests cover missing, old, supported, and unversionable binaries before lock/worktree creation. | Published-runtime compatibility (D3) and final release status remain per the GA spec. |
| F1–F2 / R26 suite integrity | `6fb1223` removes sibling-repository source assertions; Ralph-owned hook ACs use runtime tests and engine-owned ACs are explicitly descoped rather than falsely certified. | Full-workspace green and the remaining F-matrix are release evidence tracked by the GA spec. |
| E1/E4 / R27 shipped concurrency surfaces | `083f25c` removes the deleted wave CLI from shipped tools, ports `wave-review` to autoloop role concurrency, and adds a shipped-artifact regression test. | Final E-gate status is per the GA spec. |

All other historical parity behaviors—including completion semantics, exhaustive
stop reasons, live control, HITL, observability, resume, replay/mock substrate,
and packaging—are governed solely by the current GA spec and its R-matrix.

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
2b. ✅ **Cost telemetry** — autoloop surfaces cumulative `cost_usd` in the summary block + structured `loop.finish`/`summary` events ([autoloop#43](https://github.com/mikeyobrien/autoloop/pull/43)); ralph parses both channels into `LoopState.cumulative_cost`. *(done)*
2c. ✅ **Resume** — `autoloop resume <run-id>` continues a terminated run without redoing completed work ([autoloop#44](https://github.com/mikeyobrien/autoloop/pull/44), part of #38); adversarially reviewed. Unblocks ralph's `ralph resume` parity. *(done)*
3. 🟡 **Live control bridge** — steer/interrupt → ControlRequest + SIGUSR1. *(autoloop control verbs exist via #32; ralph-side steer wiring on the autoloop path is deferred — depends on #34 for the `command` backend)*
4. ✅ **HITL over subprocess** — ask detected, ralph layer blocks, Telegram delivers, response written back. *(proven via live round-trip; #32 implemented)*
5. ✅ **Preset pass-through parity** — preset generated from hats (`autoloop_preset_gen`), routing/completion mapped. *(done; round-trip tested)*
6. 🟡 **Reconcile tasks/memory** — canonical store via path-override (#36 implemented upstream); end-to-end completion-gate-across-boundary test still to add.
7. 🔵 **Waves** — autoloop owns role concurrency natively; ralph `wave_*` becomes obsolete under autoloop. Gated on #35.
8. 🔴 **Delete** the in-house engine + rewire `run_loop_impl`/`EventLoop` callers (run/resume/bot/rpc/bench/tui), then drop `engine: "ralph"`. **Blocked** on autoloop #34/#35/#37/#38/#39 + PRs #40–#42 merging (see *Deletion gating* above).

Newly added this cutover (not in the original plan): **completion-coordination parity bridge** (`completion_coord.rs`) so the autoloop path keeps merge-queue/landing/registry parity *before* the engine is deleted — the precondition that makes slice 8 a pure deletion rather than a deletion+reimplementation.

### Slice 8 deletion runbook (green-at-each-step)

The in-house engine cannot be deleted piecemeal — every engine module (`event_loop`,
`hatless_ralph`, `wave_*`, `hat_registry`, `event_bus`) is still consumed by
`loop_runner::run_loop_impl`. So the order is: (A) lift engine-agnostic coordination
*out* of `loop_runner` so the autoloop path doesn't depend on it, (B) rewire/descope the
remaining `run_loop_impl` + `EventLoop` callers, (C) delete the now-dead engine + tests.
Each step must `cargo build`/`cargo test` green and be committed.

**A — decouple survivors from `loop_runner` (additive, low-risk): ✅ DONE**
- [x] `merge_processing` extracted (commit `9e41d7c`) — queue draining no longer lives in `loop_runner`.
- [x] `RunStats` (commit `4f80f21`) — `SummaryWriter`/`print_termination` take a tiny engine-agnostic stats struct instead of the engine `LoopState`; the keepers (`completion_coord`/`autoloop_engine`) no longer build a `LoopState`.
- [x] Verified `completion_coord`/`autoloop_engine`/`autoloop_preset_gen`/`merge_processing` have **zero** `EventLoop`/`hatless_ralph`/`wave_*`/`event_bus` imports (only `config.event_loop.*` config fields + keeper coordination types). The autoloop engine path is fully decoupled from the in-house engine internals. (Deferred to Phase C: `TerminationReason` re-export currently lives in the `event_loop` module — a mechanical move when that module is deleted.)

**B — rewire/descope the legacy-engine callers: ✅ DONE (commit `21e6d89`)**
- [x] `ralph run`: `run_autoloop_engine` is the sole path; TUI/RPC branches removed (descoped → #342/#343).
- [x] `ralph resume`: routes to the autoloop engine (re-drives reading on-disk state; true run_id continue → #344).
- [x] Bot daemon `start_loop` moved to `autoloop_engine.rs`, routes to the autoloop engine (in-loop RObot → #345).
- [x] RPC + in-process/subprocess TUI: descoped → #342/#343.
- [x] `ralph-bench`: EventLoop body stubbed (bails loudly) → #346. Backend forwarding → #347.

**C — delete the dead engine + tests: ✅ DONE (commits `919024d`/`4c00366`/`7c32810`/`10455e5`)**
- [x] Deleted `loop_runner.rs` (run_loop_impl + run_subprocess_tui), `wave.rs`, `rpc_stdin.rs` (ralph-cli).
- [x] Deleted `event_loop/` (mod + tests + loop_state), `hatless_ralph.rs`, `wave_tracker/detection/prompt.rs`, `LoopState` (ralph-core). KEPT `hat_registry.rs` (live `ralph hats` consumer) and `event_bus.rs` (re-exported; orphaned dead code, low-priority cleanup).
- [x] Deleted obsolete tests: `event_loop_ralph.rs`, `smoke_runner.rs`, the EventLoop diagnostics tests, `integration_events_isolation.rs`, `integration_resume.rs`, 5 legacy behavioral tests in `integration_run.rs`.
- [x] `core.engine` is now inert (autoloop unconditional); field retained for config compatibility.

**IMPLEMENTATION STATUS: the v3 engine cutover is fully migrated; GA is not signed off here.** The in-house engine is deleted (~31K lines); autoloop is the sole engine. Release status is governed by [v3-ga-readiness.spec.md](v3-ga-readiness.spec.md). At this migration snapshot, the workspace built clean (0 warnings); ralph-cli (21 test binaries), ralph-core (640 lib), ralph-tui/api/telegram/proto/bench were green. Adversarial review of that snapshot found one HIGH bug (dropped backend), then tracked it as #347. The autoloop-runtime live contract test (`ralph-adapters`) was red, gated on the local autoloop build having `--events` (autoloop PRs #40–#44), not a deletion regression. Descoped work was tracked as TUI #342, RPC #343, native-resume #344, bot HITL #345, bench #346, and backend mapping #347.
