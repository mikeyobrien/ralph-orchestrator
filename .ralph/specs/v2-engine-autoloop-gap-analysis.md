---
status: research
created: 2026-07-20
updated: 2026-07-20
---

# V2 engine vs Autoloop-backed V3 gap analysis

## 1. Executive summary

Ralph V3 is a deliberate ownership split, not a line-for-line port of the V2
engine. Autoloop owns execution, routing, retry policy, completion judgment,
budgets, canonical run memory/tasks, machine events, journal, and summary.
Ralph owns config translation, TUI/headless presentation, worktrees, loop
registry, merge queue, history, dependency health, and completion coordination.

The comparison finds **38 capability rows**. Most engine behavior moved or was
replaced, but seven user-visible regressions remain:

1. per-hat backend overrides are accepted and documented but not translated
   ([`ralph-orchestrator-v3-autoloops-backend-a7e.16`](#p0));
2. Roo worked as a V2 global backend but V3 explicitly rejects it; this is not
   global-backend parity ([`ralph-orchestrator-v3-autoloops-backend-a7e.17`](#p2));
3. Ralph memories remain usable through `ralph tools memory`, but V2 automatic
   Ralph-memory prompt injection is not connected to the V3 engine's separate
   memory store ([`ralph-orchestrator-v3-autoloops-backend-a7e.23`](#p1));
4. per-hat scratchpad overrides are not translated
   ([`ralph-orchestrator-v3-autoloops-backend-a7e.22`](#p2));
5. Ralph-native direct resume is absent
   ([`ralph-orchestrator-v3-autoloops-backend-a7e.19`](#p1));
6. the web dashboard still reads the V2 live-event location
   ([`ralph-orchestrator-v3-autoloops-backend-a7e.18`](#p1)); and
7. released Autoloop has ask/respond primitives, but `ralph run` does not relay
   them through RObot
   ([`ralph-orchestrator-v3-autoloops-backend-a7e.5`](#p1)).

TUI live assistant/tool text works through private backend stream files rather
than a stable contract. Wave configuration translation exists, but real-runtime
scatter/gather certification remains open.

### Headline classification counts

| Classification | Count |
|---|---:|
| 1. Retained with equivalent behavior | 5 |
| 2. Replaced by a different mechanism | 14 |
| 3. Moved to Autoloop ownership | 2 |
| 4. Moved to Ralph observation/coordination ownership | 3 |
| 5. Intentionally removed or simplified | 4 |
| 6. Missing regression | 7 |
| 7. Deferred/blocked on an explicit upstream dependency | 0 |
| 8. New in V3 | 3 |
| **Total** | **38** |

## 2. Baselines, citation convention, and method

### Fixed baselines

- **Ralph V2:** `407cb96b25a5705a779d598282d070e6f80f712c`
  (`v2.10.1`). This is the last release before the V3 cutover; the in-house
  engine is later deleted by `4c00366c`.
- **Ralph V3 comparison point:**
  `aff233d7b29ec7a553b88ce817f9beef2c3ada99`.
- **External repository:**
  [`mikeyobrien/autoloop`](https://github.com/mikeyobrien/autoloop).
- **Autoloop hardening worktree:**
  `d14fd11018e68634d5dbc9275cbbdac00ab34002`.
- **Autoloop `main`:**
  `ab7e34cb7c0f873b44bc4f22b98b6fb345844dd5`.
- **Autoloop v0.10.1 commit:**
  `ae67c272007dbe204807f16314051db8e604ceb2`. The value
  `49e4196b8ff72e527ce280d7ff5229613a3ed077` is its annotated tag object,
  not the target commit.

### Citation resolution and local-only evidence

- `V2:path:Lx-Ly` means
  `git show 407cb96b25a5705a779d598282d070e6f80f712c:path` in the Ralph repository.
- `V3:path:Lx-Ly` means
  `git show aff233d7b29ec7a553b88ce817f9beef2c3ada99:path` in the Ralph repository.
- `AL-101:path:Lx-Ly` resolves publicly under
  [`ae67c272007dbe204807f16314051db8e604ceb2`](https://github.com/mikeyobrien/autoloop/tree/ae67c272007dbe204807f16314051db8e604ceb2).
- `AL-H:path:Lx-Ly` means `git show d14fd11018e68634d5dbc9275cbbdac00ab34002:path`
  in `/Users/rook/.herdr/worktrees/autoloop/fix-autoloop-0.10.2-hardening`.
- `AL-M:path:Lx-Ly` means `git show ab7e34cb7c0f873b44bc4f22b98b6fb345844dd5:path`
  in `/Volumes/T7/projects/autoloop`.

The pinned `AL-H` and `AL-M` objects are local-only and are not reachable from
a public GitHub ref at review time; their former tree URLs returned 404. Claims
that depend on them are reproducible in the named local checkouts but are not
independently resolvable off this machine. This is an explicit evidence and
release-integration gap, tracked by
`ralph-orchestrator-v3-autoloops-backend-a7e.13`. Release-capability claims use
`AL-101` wherever the needed source exists.

### Autoloop branch/release distinction

Hardening-only commits after `main` include standalone Claude executable
resolution, Pi cumulative-stream compaction, subprocess-test isolation, and the
0.10.2 changelog. `AL-M` also contains incremental stream-log flushing that is
not in `AL-101`. Ralph's configured minimum remains 0.10.1
(`V3:crates/ralph-core/src/autoloop_health.rs:L11-L18`), so released-engine TUI
assistant-delta behavior is an evidence gap rather than assumed parity.

### Method

The analysis followed the actual launch and completion path rather than name
searches: V2 `loop_runner` -> `EventLoop`; V3 `autoloop_engine` ->
`AutoloopRunner` -> event tailer -> TUI/headless renderer ->
`completion_coord`. It then checked config generation, preflight/doctor,
worktree/merge, task/memory, cleanup, export, and integration tests. External
claims were checked at the pinned Autoloop commits above. Legacy cassette E2E
was used only as historical capability evidence, never as a V3 gate.

## 3. Comprehensive feature matrix

| # | Capability | V2 behavior | V3 / Autoloop behavior | Class | Owner | Pinned evidence and user impact |
|---:|---|---|---|---|---|---|
| 1 | Iteration lifecycle and completion judgment | Ralph's loop runner selected hats, executed iterations, processed events, and mapped terminal state. | `ralph run` launches `autoloop run`; engine summary/terminal events decide completion. | 3 | Autoloop | `V2:crates/ralph-cli/src/loop_runner.rs:L119-L261`; `V2:crates/ralph-core/src/event_loop/mod.rs:L2415-L2475`; `V3:crates/ralph-cli/src/autoloop_engine.rs:L1-L7,L418-L477`. Ownership move, not loss. |
| 2 | Post-run history, landing, queue, deregistration | Bundled into V2 loop runner. | Engine-independent completion coordination runs for success, stop, and failure. | 4 | Ralph | `V3:crates/ralph-cli/src/completion_coord.rs:L1-L18,L53-L157`; `V3:crates/ralph-cli/tests/integration_autoloop_prompt.rs:L265-L346`. |
| 3 | Max iteration/runtime/cost budgets | V2 enforced all three. | Ralph translates units/values; released Autoloop checks iteration, journaled cost, and elapsed runtime between turns. | 1 | Autoloop | `V2:crates/ralph-core/src/event_loop/mod.rs:L638-L646`; `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L145-L177`; `V3:crates/ralph-cli/tests/integration_autoloop_prompt.rs:L582-L636`; `AL-101:packages/harness/src/index.ts:L619-L666`. |
| 4 | Consecutive-failure budget | V2 terminated at the configured consecutive-failure threshold. | Not translated because Autoloop has no equivalent generic budget; preflight warns. | 5 | Neither | `V2:crates/ralph-core/src/event_loop/mod.rs:L643-L646`; `V3:crates/ralph-core/src/preflight.rs:L221-L238`. Reduced governor fidelity is explicit. |
| 5 | Retries and backoff | V2 lifecycle hooks supported retry-backoff and wait-then-retry policies. | Autoloop classifies auth/quota/rate-limit/transient failures; transient/rate-limit errors retry with bounded exponential backoff. V2 hook retry is not retained because hooks are inert. | 2 | Autoloop | `V2:crates/ralph-cli/src/loop_runner.rs:L3403-L3629`; `AL-H:packages/harness/src/iteration.ts:L301-L355`; `V3:crates/ralph-core/src/preflight.rs:L183-L215`. Different retry target and semantics. |
| 6 | Routing backpressure and completion holds | V2 rejected malformed/invalid event flow and held completion for required events/guidance. | Autoloop rejects disallowed events, re-injects the last rejection into prompts, and can hold completion after acceptance/evidence checks. | 2 | Autoloop | `V2:crates/ralph-core/src/event_loop/loop_state.rs:L43-L46,L83-L86,L170-L176`; `V2:crates/ralph-cli/src/loop_runner.rs:L2626-L2693`; `AL-H:packages/harness/src/prompt.ts:L352-L388,L629-L648`; `AL-H:packages/harness/src/provisional.ts:L137-L165,L320-L322`. Tracking: `ralph-orchestrator-v3-autoloops-backend-au2`, `ralph-orchestrator-v3-autoloops-backend-hav`. |
| 7 | Failure/stop taxonomy | V2 exposed Ralph-specific stale, thrash, validation, restart, cancel reasons. | V3 maps Autoloop's closed stop-reason set, including typed backend/auth/quota/transient reasons. | 2 | Shared | `V2:crates/ralph-core/src/event_loop/mod.rs:L69-L147`; `V3:crates/ralph-cli/src/autoloop_engine.rs:L24-L86`; `AL-H:packages/harness/src/types.ts:L378-L452`. |
| 8 | Supported global backend mappings | V2 supported Claude, Pi, Kiro, Gemini, Codex, Forge, Amp, Copilot, OpenCode, custom, and others. | The overlapping supported set is translated and tested; unsupported values fail fast. | 1 | Shared | `V2:crates/ralph-adapters/src/cli_backend.rs`; `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L212-L317,L680-L779`. This row deliberately excludes Roo. |
| 9 | Roo global backend | V2 had a Roo adapter and user guide. | V3 explicitly rejects `roo` because its prompt-file contract cannot be represented by the current mapping. | 6 | Ralph decision | `V2:docs/guide/roo-backend.md:L1-L30`; `V2:crates/ralph-adapters/src/cli_backend.rs`; `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L259-L265,L769-L780`. Decision tracked by `ralph-orchestrator-v3-autoloops-backend-a7e.17`. |
| 10 | Per-hat backend override | Active V2 hats selected their own backend and args. | Config/docs still expose overrides, but generated roles omit backend fields although Autoloop supports them. | 6 | Ralph | `V2:crates/ralph-cli/src/loop_runner.rs:L1691-L1771`; `V3:crates/ralph-core/src/config.rs:L1928-L1936`; `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L88-L113,L355-L356`; `AL-M:packages/core/src/topology.ts:L12-L25,L458-L491`. Bead `ralph-orchestrator-v3-autoloops-backend-a7e.16`. |
| 11 | Hats, routing, handoffs | Ralph bus and registry routed events. | Ralph inverts triggers into Autoloop handoff routes and writes role prompts. | 2 | Autoloop | `V2:crates/ralph-core/src/event_loop/mod.rs:L1123-L1298`; `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L51-L120`. |
| 12 | Concurrency and aggregation | V2 used bespoke wave execution. | Hat concurrency/aggregate translate to role concurrency/wait-for-all aggregation. | 2 | Autoloop | `V2:crates/ralph-cli/src/loop_runner.rs:L5174-L5559`; `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L94-L112`; runtime certification remains `ralph-orchestrator-v3-autoloops-backend-a7e.8`. |
| 13 | `ralph wave` and wave-worker UI | Dedicated command/worker UI. | Removed in favor of declarative role concurrency; stale help remains. | 5 | Removed | `V3:docs/migration/v3-autoloop-engine.md:L23-L32`; `V3:crates/ralph-tui/src/widgets/help.rs:L113-L128`. Help fix: `ralph-orchestrator-v3-autoloops-backend-a7e.21`. |
| 14 | Engine task authority | Ralph tasks/events participated in V2 completion. | Autoloop's incompatible append-only task store alone gates engine completion; Ralph tasks are observational. | 3 | Autoloop | `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L320-L332`; `V3:crates/ralph-cli/tests/integration_autoloop_prompt.rs:L518-L580`. |
| 15 | Prompt/context construction | V2 assembled objective, active hats, routed events, ready tasks, memories, guidance, skills, and scratchpad in Ralph. | Ralph passes the objective and role instructions; Autoloop assembles routing, topology, tasks, memory, guidance, scratchpad, backpressure, state-root guidance, and tool instructions each iteration. | 2 | Autoloop | `V2:crates/ralph-core/src/event_loop/mod.rs:L1123-L1298,L1399-L1485`; `V3:crates/ralph-cli/src/autoloop_engine.rs:L266-L286,L367-L383`; `V3:crates/ralph-cli/tests/integration_autoloop_prompt.rs:L442-L517`; `AL-H:packages/harness/src/prompt.ts:L352-L411`. |
| 16 | Durable memories | V2 automatically injected `.ralph/agent/memories.md` under configured budget/filter. | Ralph memory CLI/store remains, while Autoloop injects its own project/run memory. The generator does not map Ralph's memory file or injection policy, so existing Ralph memories do not automatically reach engine prompts. | 6 | Split | `V2:crates/ralph-core/src/event_loop/mod.rs:L1423-L1485`; `V3:crates/ralph-cli/tests/integration_memory.rs:L1-L34`; `AL-H:packages/harness/src/prompt.ts:L352-L388`; `AL-H:packages/cli/src/commands/memory.ts:L1-L100`. Prompt-semantics decision: `ralph-orchestrator-v3-autoloops-backend-a7e.23`; no source proves cross-store parity. |
| 17 | Global scratchpad as engine continuity | V2 loaded a configured mutable Ralph scratchpad into each prompt. | Autoloop projects engine scratchpad from journal/state; Ralph's retained global scratchpad is coordination state, not engine authority. | 5 | Autoloop | `V2:crates/ralph-core/src/event_loop/mod.rs:L1581-L1615`; `V3:docs/concepts/memories-and-tasks.md:L151-L175`; `AL-H:packages/harness/src/prompt.ts:L389-L411`. |
| 18 | Per-hat scratchpad | V2 resolved active-hat override before prompt construction. | Fields remain accepted but generator writes only role instructions/topology. | 6 | Ralph | `V2:crates/ralph-core/src/event_loop/mod.rs:L1190-L1230`; `V3:crates/ralph-core/src/config.rs:L1948-L1951`; `V3:crates/ralph-cli/src/autoloop_preset_gen.rs:L88-L113`. Bead `ralph-orchestrator-v3-autoloops-backend-a7e.22`. |
| 19 | Ralph loop identity / `--continue` | V2 persisted loop ID and state. | V3 preserves Ralph coordination ID/task tagging only. | 4 | Ralph | `V2:crates/ralph-cli/src/loop_runner.rs:L189-L214`; `V3:crates/ralph-cli/tests/integration_continue_resume.rs:L129-L173`. |
| 20 | Direct resume/recovery | V2 restored iteration, cost, last hat, and token state. | `ralph resume` is unsupported; `run --continue` is not engine resume; users are pointed to `autoloop resume <run-id>`. | 6 | Ralph | `V2:crates/ralph-core/src/event_loop/mod.rs:L493-L550`; `V3:crates/ralph-cli/src/main.rs:L650-L657,L1731-L1734`; `V3:crates/ralph-cli/tests/integration_continue_resume.rs:L177-L190`. Bead `ralph-orchestrator-v3-autoloops-backend-a7e.19`. |
| 21 | Telegram HITL and proactive guidance | V2 advertised live `human.interact`, guidance, and restart. | Released Autoloop already provides blocking ask, correlated respond, and response injection, but Ralph's Telegram relay is inactive on the current run path. | 6 | Ralph | `V2:README.md:L144-L168`; `V3:README.md:L182-L198`; `AL-101:packages/harness/src/ask.ts:L1-L66`; `AL-101:packages/cli/src/commands/control.ts:L153-L250`. Ralph relay regression: `ralph-orchestrator-v3-autoloops-backend-a7e.5`. |
| 22 | Interruption and live control | V2 had in-process cancel/restart/suspend paths and Telegram guidance. | TUI quit/Ctrl-C terminates the Autoloop process group and coordinates completion; native engine interrupt/guide exists but Ralph does not expose the full control surface. | 2 | Shared | `V2:crates/ralph-cli/src/loop_runner.rs:L2543-L2570,L3276-L3710`; `V3:crates/ralph-cli/src/autoloop_engine.rs:L747-L855`; `AL-H:packages/cli/src/commands/control.ts:L40-L250`. Clean stop is retained; pause/guide parity is not claimed. |
| 23 | Headless live streaming | V2 autonomous path called streaming observers and a real integration emitted multiple heartbeat lines; the historical test captures output after process exit, so it does not prove pre-exit timing. | V3 integration explicitly observes iteration progress while the child is still alive. | 1 | Shared | `V2:crates/ralph-cli/src/loop_runner.rs:L4416-L4459`; `V2:crates/ralph-cli/tests/integration_run.rs:L348-L414`; `V3:crates/ralph-cli/tests/integration_autoloop_headless_stream.rs:L45-L158`. Parity is strong for streamed content, strongest timing proof is V3-only. |
| 24 | TUI live assistant/tool content | V2 wrote backend deltas directly into in-process iteration buffers. | V3 tails private `claude-stream.*`/`pi-stream.*`; authoritative events still drive lifecycle. | 2 | Ralph over private files | `V2:crates/ralph-cli/src/loop_runner.rs:L1775-L1795`; `V3:crates/ralph-adapters/README.md:L17-L40`; `V3:crates/ralph-cli/tests/integration_autoloop_tui_live_stream.rs:L94-L189`. Stable contract bead `ralph-orchestrator-v3-autoloops-backend-a7e.20`. |
| 25 | TUI retained iteration navigation/search | V2 iteration buffers supported previous/next, scrolling, and search. | The same Ralph TUI buffer/navigation layer remains and Autoloop events populate it. | 1 | Ralph | `V2:crates/ralph-tui/src/app.rs:L1-L90`; `V3:crates/ralph-tui/src/autoloop_source.rs:L1-L30`; `V3:crates/ralph-tui/src/app.rs:L639-L735`. Retention is in-memory for the running TUI, not durable engine history. |
| 26 | TUI export | V2 exported current/all iteration buffers to stable plain text. | The same export module and action tests remain. | 1 | Ralph | `V2:crates/ralph-tui/src/app.rs:L880-L940`; `V3:crates/ralph-tui/src/export.rs:L1-L120`. This proves text-buffer export only, not cost parity. |
| 27 | Cost reporting | V2 accumulated cost in `LoopState`, enforced budget, and persisted cost for continue. | Engine events/summary supply cost; headless footer calculates iteration/total cost and TUI shows final cost. | 2 | Shared | `V2:crates/ralph-core/src/event_loop/loop_state.rs:L27-L28`; `V2:crates/ralph-core/src/event_loop/mod.rs:L493-L533,L638-L642`; `V3:crates/ralph-cli/src/autoloop_engine.rs:L555-L564,L598-L606`; `V3:crates/ralph-tui/src/widgets/footer.rs:L173-L182`. No claim of identical per-backend accounting. |
| 28 | Diagnostics and operational navigation | V2 diagnostics captured prompts/agent output/orchestration/errors; loop commands exposed logs/history. | Ralph trace sessions and TUI logs remain; Autoloop adds journal/events/summary, while `ralph loops logs` still prefers legacy event filenames and falls back to Ralph history. | 2 | Shared | `V2:crates/ralph-core/src/diagnostics/mod.rs`; `V2:crates/ralph-core/src/event_loop/mod.rs:L1390-L1421`; `V3:crates/ralph-cli/src/main.rs:L878-L999`; `V3:crates/ralph-cli/src/loops.rs:L564-L637`; `V3:crates/ralph-adapters/README.md:L7-L15`. Artifact UX/root beads: `ralph-orchestrator-v3-autoloops-backend-f50`, `ralph-orchestrator-v3-autoloops-backend-a7e.12`. |
| 29 | Web dashboard live state | V2 wrote `.ralph/current-events`, which the API watches. | Engine path writes `.ralph/autoloop-events.ndjson`; API still watches the old path and `ralph web` warns. | 6 | Ralph | `V2:crates/ralph-cli/src/loop_runner.rs:L201-L214`; `V3:crates/ralph-api/src/event_watcher.rs:L37-L70`; `V3:crates/ralph-cli/src/autoloop_engine.rs:L369-L383`; `V3:crates/ralph-cli/src/web.rs:L27`. Replacement bead `ralph-orchestrator-v3-autoloops-backend-a7e.18`; no absent legacy bead is credited. |
| 30 | `--rpc` and `--record-session` | V2 supported RPC TUI modes and smoke/session recording. | Explicitly removed; fake-engine fixtures replace replay capture. | 5 | Removed | `V2:crates/ralph-tui/src/lib.rs:L12-L23`; `V3:docs/migration/v3-autoloop-engine.md:L27-L32`; `V3:crates/ralph-cli/tests/fixtures/autoloop/README.md:L1-L82`. |
| 31 | Machine events, journal, summary | V2 exposed Ralph-owned event/UI files. | V3 consumes documented `--events`, append-only journal, terminal summary, and exit status. | 8 | Autoloop | `V3:crates/ralph-adapters/README.md:L7-L15`; `V3:crates/ralph-adapters/tests/autoloop_native_contract_integration.rs:L1-L67`. Ralph/engine run-ID correlation: `ralph-orchestrator-v3-autoloops-backend-a7e.24`; per-iteration harness display remains `ralph-orchestrator-v3-autoloops-backend-j0e`. |
| 32 | Engine dependency and provisioning | V2 had no separate engine; users manually installed/authenticated a backend and `ralph doctor` checked it. | V3 must additionally resolve a compatible Autoloop binary, preferring vendored then PATH, and offers `ralph doctor --install-engine` without Node. | 8 | Ralph | `V2:docs/getting-started/installation.md:L7-L60`; `V2:crates/ralph-cli/src/doctor.rs:L15-L55`; `V3:crates/ralph-cli/tests/integration_autoloop_dependency.rs:L1-L220`; `V3:crates/ralph-core/src/autoloop_health.rs:L11-L18`. This is new operational machinery, not a V2 provisioning equivalent. |
| 33 | Parallel loops/worktrees/merge queue | V2 already isolated extra loops in worktrees and exposed list/log/history/diff/retry/merge UX. | V3 preserves registry/worktree/queue surfaces and invokes completion coordination after engine exit. | 4 | Ralph | `V2:docs/advanced/parallel-loops.md:L3-L46,L87-L133`; `V2:crates/ralph-cli/tests/integration_loops_merge.rs:L1-L31`; `V3:crates/ralph-cli/src/completion_coord.rs:L98-L157`; `V3:crates/ralph-cli/tests/integration_merge_drain_autoloop.rs`. Final live proof remains `ralph-orchestrator-v3-autoloops-backend-a7e.15`. |
| 34 | Presets and custom workflows | V2 embedded/discovered Ralph YAML presets and custom hat graphs. | V3 can translate Ralph YAML hats or run an explicit Autoloop TOML preset; discovery recognizes both shapes. | 2 | Shared | `V2:crates/ralph-cli/src/presets.rs`; `V2:crates/ralph-cli/tests/integration_preset.rs`; `V3:crates/ralph-cli/src/hats.rs:L161-L264`; `V3:crates/ralph-cli/src/autoloop_engine.rs:L321-L346`. Translation is not full config parity (rows 4, 9, 10, 16, 18). |
| 35 | Safety/preflight | V2 checked config, hooks, backend, Telegram token, git cleanliness, paths, tools, and spec completeness. | Those Ralph checks remain; V3 adds engine health plus explicit warnings for inert hooks and unenforced failure budget. Autoloop owns runtime acceptance/evidence gates. | 2 | Shared | `V2:crates/ralph-core/src/preflight.rs:L105-L180,L374-L637`; `V3:crates/ralph-core/src/preflight.rs:L105-L238,L392-L595`; `AL-H:packages/harness/src/acceptance.ts:L44-L118`. Engine hardening tracked by `ralph-orchestrator-v3-autoloops-backend-a7e.13`. |
| 36 | Authentication behavior | V2 doctor supplied backend-specific auth hints while the backend CLI owned login. | Ralph still checks hints/executable availability; Autoloop classifies runtime `auth_failed` and does not make retryable auth assumptions. | 2 | Shared | `V2:crates/ralph-cli/src/doctor.rs:L48-L55,L263-L365`; `V3:crates/ralph-cli/src/doctor.rs:L48-L55,L263-L365`; `AL-H:packages/harness/src/circuit-breaker.ts:L23-L45`. No paid auth call was made for this research. |
| 37 | Process cleanup and stale locks | V2 directly owned backend child cleanup and Ralph lock lifetime. | V3 TUI uses an owned process group; all outcomes coordinate; integration covers crash cleanup and replacement after SIGKILL. | 2 | Ralph | `V2:crates/ralph-cli/src/loop_runner.rs:L4388-L4475`; `V3:crates/ralph-adapters/src/autoloop_runner.rs:L147-L150,L281-L287`; `V3:crates/ralph-cli/tests/integration_autoloop_prompt.rs:L265-L440`. |
| 38 | Replay and real-engine contract testing | V2 used session recording and legacy cassettes. | V3 uses deterministic fake-Autoloop fixtures plus real contract/parity tests. | 8 | Shared | `V3:crates/ralph-cli/tests/fixtures/autoloop/README.md:L1-L82`; `V3:crates/ralph-adapters/tests/autoloop_native_contract_integration.rs:L1-L67`; `V3:crates/ralph-adapters/tests/autoloop_parity_integration.rs:L1-L70`. Live release smoke is still `ralph-orchestrator-v3-autoloops-backend-a7e.15`. |

## 4. Release findings

### Confirmed GA blockers / release decisions

1. **Per-hat backend translation is broken** — P0 implementation bead
   `ralph-orchestrator-v3-autoloops-backend-a7e.16`.
2. **GA needs real-runtime proof, not only fixtures** — P1 wave certification
   `ralph-orchestrator-v3-autoloops-backend-a7e.8`, P1 hardening
   `ralph-orchestrator-v3-autoloops-backend-a7e.13`, and P1 final live smoke
   `ralph-orchestrator-v3-autoloops-backend-a7e.15`.
3. **Dashboard live state is severed** — P1 dashboard bead
   `ralph-orchestrator-v3-autoloops-backend-a7e.18`. Whether an Alpha dashboard
   blocks CLI GA is a release-policy choice; the technical regression is not
   ambiguous.
4. **Roo is a real V2-to-V3 regression** — P2 support-or-remove decision bead
   `ralph-orchestrator-v3-autoloops-backend-a7e.17`. Release policy must decide
   whether that intentional decision blocks GA.

### Important remaining gaps

- Ralph-native resume (P1): `ralph-orchestrator-v3-autoloops-backend-a7e.19`.
- Stable assistant/tool live-output contract (P1):
  `ralph-orchestrator-v3-autoloops-backend-a7e.20`.
- Telegram HITL relay (P1 Ralph regression):
  `ralph-orchestrator-v3-autoloops-backend-a7e.5`.
- Ralph memory prompt semantics (P1):
  `ralph-orchestrator-v3-autoloops-backend-a7e.23`.
- Ralph/engine run-ID correlation (P1):
  `ralph-orchestrator-v3-autoloops-backend-a7e.24`.
- State-root ownership (P1) and artifact discovery (P2):
  `ralph-orchestrator-v3-autoloops-backend-a7e.12` and
  `ralph-orchestrator-v3-autoloops-backend-f50`.
- Per-iteration harness identity (P1):
  `ralph-orchestrator-v3-autoloops-backend-j0e`.
- History (P1) and backpressure (P2) fidelity:
  `ralph-orchestrator-v3-autoloops-backend-au2` and
  `ralph-orchestrator-v3-autoloops-backend-hav`.

### Intentional removals / simplifications

- `ralph wave` command and wave-worker UI (declarative concurrency replaces it).
- `ralph run --rpc` and `--record-session`.
- Generic consecutive-failure enforcement.
- Ralph scratchpad as engine-authoritative continuity.

### V3 improvements

- Explicit machine event, journal, terminal summary, and typed stop contracts.
- Separate engine dependency health, version gating, and standalone provisioning.
- Fixture replay and real-engine adapter contract tests aligned to the new
  architecture.

## 5. High-risk boundary analysis

### Prompt, tasks, memories, and scratchpad

The V3 objective and role instructions are delivered correctly, and Autoloop
constructs a rich per-iteration context. That does **not** imply Ralph-store
parity. Ralph tasks and memories have different schemas and paths from engine
tasks/memory. Current source explicitly prevents sharing the Ralph task file;
there is no analogous translation that imports `.ralph/agent/memories.md` into
Autoloop memory. Therefore:

- Autoloop tasks/memory/scratchpad are engine authority.
- Ralph tasks/memories/scratchpad remain useful coordination artifacts.
- Existing docs saying Ralph memories are automatically injected each iteration
  are not proven true on the V3 launch path.
- Per-hat scratchpad configuration is a separate confirmed translation gap.

The state-root contract work belongs to `a7e.12`; Ralph-memory prompt semantics
belong to `a7e.23`; per-hat scratchpad behavior belongs to `a7e.22`.
Recombining stores without an explicit format/authority decision would recreate
split-brain completion state.

### Resume, interruption, and cleanup

`run --continue`, `ralph resume`, and `autoloop resume` are three different
contracts. V3's first preserves Ralph coordination identity; the second is
unsupported; the third resumes canonical engine state. Ralph does cleanly kill
the TUI child process group and perform completion coordination, but it does not
yet wrap engine run identity/resume/control into one Ralph UX. That is why
resume (`a7e.19`) and Ralph/engine run-ID correlation (`a7e.24`) are distinct
from per-iteration harness presentation (`j0e`).

### Streaming, history, export, and cost

Supported lifecycle streaming uses `--events`; provisional assistant/tool text
uses private files. TUI iteration navigation and text export are Ralph-owned and
retained. Cost is separately derived from engine events/summary and is shown in
headless output and the completed TUI footer. Export source proves only buffer
export; it does **not** prove cost-accounting parity or that cost is embedded in
exports.

### Diagnostics and artifact roots

V3 has more authoritative raw contracts but more places to look:
`.ralph/autoloop-events.ndjson`, `.ralph/history.jsonl`, Ralph diagnostics/TUI
logs, and `.autoloop/` journal/run files. The old dashboard and parts of
`ralph loops logs` still assume V2 paths. `a7e.12`, `f50`, `au2`, and `hav`
should converge discovery without making private stream files authoritative.

### Presets, safety, authentication, and cleanup

Custom workflow reachability is retained through two routes: translated Ralph
YAML and explicit Autoloop TOML. Translation is intentionally lossy where
contracts differ, and preflight now warns on known losses. Backend
authentication remains external-CLI owned; Ralph supplies hints and Autoloop
supplies typed runtime failures. V3 cleanup is more complex because an engine
subprocess owns backend descendants, but process-group and stale-lock tests
cover the critical paths. No claim is made that every platform-specific kill
path was live-smoked.

## 6. Contract boundary

| Contract | Autoloop authority | Ralph responsibility |
|---|---|---|
| Iteration, routing, backpressure, retry, completion | Canonical | Translate config; render observations |
| Objective and role prompts | Builds final per-turn context | Supplies objective and translated role instructions |
| Tasks/memory/scratchpad | Canonical engine stores | Separate coordination stores and CLI |
| `--events`, journal, summary, stop reason, cost | Produces | Decode, display, coordinate |
| Assistant/tool provisional deltas | Currently private backend stream files | Opportunistic TUI tailer; must not treat as completion truth |
| Worktrees, loop registry, merge queue, landing | None | Canonical |
| Doctor/install | Engine exposes versioned binary | Resolve, provision, and explain |
| HITL ask/respond | Blocking/control primitives | Telegram relay and user-facing lifecycle |
| Resume | Canonical run ID/state | Missing first-class wrapper |
| Dashboard/TUI/headless/history/export | None | User-facing observation surfaces |

## 7. Recommendations linked to Beads

Every actionable recommendation below has an actual Bead. This document itself
is tracked by `ralph-orchestrator-v3-autoloops-backend-a7e.14`.

### P0

1. **Ralph repository — translate per-hat backends**
   (`ralph-orchestrator-v3-autoloops-backend-a7e.16`). Outcome: generated roles
   preserve supported backend/model/args, unsupported values fail before state
   mutation, and a fake integration invokes distinct role commands.

### P1

1. **Ralph repository — certify and integrate the release path**: wave runtime
   (`a7e.8`), Autoloop hardening dependency (`a7e.13`), and final six-provider
   smoke (`a7e.15`). Outcome: fake cross-process scatter/gather passes, reviewed
   engine changes are integrated, and one manual fail-closed provider smoke
   records canonical completion.
2. **Ralph repository — restore or retire dashboard live state** (`a7e.18`).
   Outcome: the Alpha dashboard either consumes supported V3 events in a real
   fake-engine run or removes misleading live-monitoring promises.
3. **Ralph repository — provide resume and identity contracts** (`a7e.19`,
   `a7e.24`). Outcome: Ralph records canonical engine run IDs and resumes an
   interrupted fake run without private-directory discovery while preserving
   registry/worktree/merge coordination.
4. **Ralph repository — finish Telegram ask/respond relay** (`a7e.5`). Outcome:
   a blocking released-engine ask is relayed, correlated, answered, resumed, and
   safely timed out through the current `ralph run` path.
5. **Autoloop and Ralph repositories — replace private stream probing**
   (`a7e.20`). Outcome: a documented bounded live-output contract carries
   assistant/tool lifecycle records and Ralph deletes all private stream-file
   inference while retaining history/export behavior.
6. **Ralph repository — decide Ralph-memory prompt semantics** (`a7e.23`).
   Outcome: bounded Ralph memories are translated through a supported contract,
   or automatic injection promises are removed with migration guidance.
7. **Ralph repository — retain trustworthy observation UX**: harness identity
   (`j0e`) and history (`au2`). Outcome: each live/historical iteration displays
   authoritative harness metadata and bounded reconciled tool evidence.

### P2

1. **Ralph repository — decide Roo support or explicit removal** (`a7e.17`).
   Outcome: Roo is mapped and fake-tested, or rejected with truthful migration
   docs and no contradictory support claim.
2. **Ralph repository — remove deleted wave-worker controls** (`a7e.21`).
   Outcome: help/input snapshots contain no dead worker mode while declarative
   concurrency remains documented.
3. **Ralph repository — resolve per-hat scratchpad configuration** (`a7e.22`).
   Outcome: supported translation exists or validation rejects the field rather
   than silently ignoring it.
4. **Ralph repository — design artifact discovery and bounded backpressure UX**
   (`f50`, `hav`, with state root `a7e.12`). Outcome: users can inspect stable
   artifacts without physical engine paths and repeated drops coalesce into one
   truthful bounded status.

No absent dashboard/help IDs are treated as coverage; replacement Beads are
specifically `a7e.18` and `a7e.21`.

## 8. Confidence and evidence gaps

### High confidence

The ownership split, Roo rejection, per-hat backend omission, separate
task/memory authority, unsupported Ralph resume, dashboard path mismatch,
private TUI stream dependency, and process cleanup paths are directly visible
in launch/config/test execution paths.

### Medium confidence / bounded claims

- **V2 headless timing:** V2 source invokes streaming observers and integration
  captures multiple heartbeat lines, but that historical test waits for process
  completion. Only V3 has an assertion that progress is visible before child
  exit. This document therefore claims content-path retention, not identical
  timing proof.
- **V2 dashboard:** file producer/consumer compatibility is proven; a pinned V2
  browser session was not replayed.
- **Cost:** V2 and V3 both carry/enforce/display cost, but backend accounting
  equivalence was not live-validated. TUI export does not establish cost parity.
- **Cleanup:** Unix crash/SIGKILL and TUI process-group paths have integration
  coverage; Windows live cleanup was not exercised here.

### Open release evidence

- Ralph requires Autoloop 0.10.1, while incremental stream flushing landed on
  `AL-M` after `AL-101`. A live released-0.10.1 TUI run is still needed before
  claiming assistant-delta parity (`a7e.13`, `a7e.15`).
- Declarative concurrency is source/test translated, but real runtime
  wave-review certification remains `a7e.8`.
- The dashboard severity remains policy-dependent because the surface is Alpha;
  the path mismatch itself is confirmed.
