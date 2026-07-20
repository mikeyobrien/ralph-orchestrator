---
status: research
created: 2026-07-20
updated: 2026-07-20
---

# V2 engine vs Autoloop-backed V3 gap analysis

## 1. Executive summary

Ralph V3 is not a line-for-line port of the V2 engine. It is a deliberate ownership split:

- **Autoloop now owns loop execution, routing, completion judgment, budgets, stop reasons, machine-readable events, and canonical run state.** Evidence: `crates/ralph-cli/src/autoloop_engine.rs:1-7`, `docs/migration/v3-autoloop-engine.md:3-7`.
- **Ralph now owns observation and coordination around the engine**: TUI, headless rendering, loop registry, worktrees, merge queue, landing, history, doctor, and install/provisioning. Evidence: `crates/ralph-cli/src/completion_coord.rs:1-18`, `docs/migration/v3-autoloop-engine.md:3-7`.

### Bottom line

1. **Most of V2's core orchestration logic was intentionally moved, not lost.** Iteration control, routing, completion, and budgets still exist, but their owner is now Autoloop rather than Ralph.
2. **Several V2 surfaces were intentionally removed or simplified**: `ralph wave …`, `ralph run --rpc`, `ralph run --record-session`, scratchpad-as-engine-state, and the in-house engine escape hatch. Evidence: `docs/migration/v3-autoloop-engine.md:23-32`.
3. **There are still confirmed user-facing regressions.** The strongest current one is that **per-hat backend overrides are still documented and accepted by the config model, but are ignored on the Autoloop run path even though Autoloop supports per-role backend overrides**. Evidence: `docs/guide/backends.md:354-369`, `crates/ralph-core/src/config.rs:1928-1936`, `crates/ralph-cli/src/autoloop_preset_gen.rs:212-317,355-356`, `ab7e34c:packages/core/src/topology.ts:12-25,458-491`.
4. **Some V2 capabilities are explicitly deferred rather than gone.** The clearest case is Telegram HITL during `ralph run`: V2 supported it, Autoloop has native ask/respond contracts, but Ralph's Telegram relay is still inactive pending `autoloop#345`. Evidence: `407cb96b:README.md:144-168`, `README.md:182-198`, `crates/ralph-core/src/config.rs:882-883`, `d14fd11:packages/harness/src/ask.ts:1-9`.
5. **One important risk remains ambiguous:** Ralph currently vendors Autoloop `0.10.1` (`crates/ralph-core/src/autoloop_health.rs:11-18`), but live TUI assistant-text parity depends on Autoloop main commit `ab7e34c` (`fix(backends): flush stream logs incrementally`), which is newer than `v0.10.1`. I can prove the version mismatch from source; I did **not** re-run a live 0.10.1 TUI session here, so I treat this as a **high-risk evidence gap**, not a confirmed blocker.

### Headline matrix counts

Across the 24 feature rows below:

- **Retained with equivalent behavior:** 4
- **Replaced by a different mechanism:** 4
- **Moved to Autoloop ownership:** 2
- **Moved to Ralph observation/coordination ownership:** 3
- **Intentionally removed or simplified:** 4
- **Missing regression:** 3
- **Deferred/blocked on explicit upstream dependency:** 1
- **New in V3:** 3

## 2. Compared commits / versions and research method

### Pinned baselines

#### Ralph current baseline
- **Current Ralph worktree / `integration/v3-prerelease`:** `aff233d7b29ec7a553b88ce817f9beef2c3ada99`

#### Ralph V2 fixed point
- **Chosen V2 baseline:** `407cb96b` (`v2.10.1`)
- **Why this fixed point:** it is the last release commit before the V3 deletion/cutover sequence starts. The in-house engine deletion lands later at `4c00366c refactor(v3): delete the in-house engine modules from ralph-core`, and the V3 release line diverges after `v2.10.1`. Evidence: `git log --decorate -- crates/ralph-core/src/event_loop/mod.rs` shows the deletion at `4c00366c`; branch/tag history shows `407cb96b (tag: v2.10.1)` as the last representative release before the cutover.

#### Autoloop baselines
- **Hardening worktree HEAD:** `d14fd11018e68634d5dbc9275cbbdac00ab34002`
- **Autoloop main:** `ab7e34cb7c0f873b44bc4f22b98b6fb345844dd5`
- **Autoloop `v0.10.1` target commit:** `ae67c272007dbe204807f16314051db8e604ceb2`
  - Note: `49e4196b8ff72e527ce280d7ff5229613a3ed077` is the **annotated tag object**, not the commit; the tag resolves to commit `ae67c27`.

### Branch-only vs main/release Autoloop deltas

#### Present only on the hardening branch (`main..d14fd11`)
- `04f9c94 fix(backends): resolve Claude executable in standalone builds`
- `c373da6 fix(backends): compact cumulative Pi stream records`
- `51bbfb5 test(integration): isolate subprocess-heavy agent surfaces`
- `d14fd11 docs(changelog): prepare autoloop 0.10.2 hardening`

#### On Autoloop main but **not** in `v0.10.1`
- `ab7e34c fix(backends): flush stream logs incrementally`
  - This matters because Ralph's live TUI assistant-text path tails growing stream files; without incremental flush, lifecycle streaming may work while mid-iteration assistant text does not.

### Method

1. **Pinned the baselines** using Git history in both repos.
2. **Validated V2 behavior from primary sources** at `407cb96b`:
   - `README.md`
   - `crates/ralph-cli/src/loop_runner.rs`
   - `crates/ralph-core/src/event_loop/mod.rs`
   - `crates/ralph-tui/src/lib.rs`
3. **Validated V3 behavior from primary sources** in the current worktree:
   - `crates/ralph-cli/src/autoloop_engine.rs`
   - `crates/ralph-cli/src/completion_coord.rs`
   - `crates/ralph-cli/src/autoloop_preset_gen.rs`
   - `crates/ralph-core/src/preflight.rs`
   - `crates/ralph-core/src/autoloop_health.rs`
   - `crates/ralph-adapters/README.md`
   - integration/contract tests
4. **Validated engine-side capabilities from primary sources** in Autoloop:
   - `packages/core/src/topology.ts`
   - `packages/harness/src/types.ts`
   - `packages/harness/src/ask.ts`
5. **Used current docs only when they matched code**; when docs and code disagreed, I treated that as a bug or evidence gap.

## 3. Comprehensive feature matrix

| # | Domain / user capability | V2 (`407cb96b`) | V3 / Autoloop (`aff233d7` + pinned Autoloop) | Class | Owner now | Evidence | User impact |
|---|---|---|---|---|---|---|---|
| 1 | Core loop execution and completion judgment | Ralph's in-house `EventLoop` owned iteration, routing, completion, and termination. | `ralph run` spawns `autoloop run`; Autoloop owns loop execution and terminal judgment. | 3. Moved to Autoloop ownership | Autoloop | `407cb96b:crates/ralph-core/src/event_loop/mod.rs:156-177`; `crates/ralph-cli/src/autoloop_engine.rs:1-7` | Architectural move, not user loss by itself. |
| 2 | Post-run completion coordination (history, landing, merge queue, deregister) | Implicitly bundled into the old loop runner. | Reimplemented as engine-agnostic bookkeeping after the Autoloop subprocess exits. | 4. Moved to Ralph observation/coordination ownership | Ralph | `crates/ralph-cli/src/completion_coord.rs:1-18,53-157` | Preserves Ralph's unique value even after engine swap. |
| 3 | Max iterations / runtime / cost budgets | Native V2 limits in `TerminationReason`. | Forwarded into Autoloop and enforced there. | 1. Retained with equivalent behavior | Autoloop | `407cb96b:crates/ralph-core/src/event_loop/mod.rs:74-79`; `crates/ralph-cli/src/autoloop_preset_gen.rs:160-177`; `docs/migration/v3-autoloop-engine.md:36-45` | No meaningful regression. |
| 4 | Consecutive-failure budget | Native V2 `ConsecutiveFailures` termination reason. | Explicitly **not enforced**; Ralph warns because Autoloop lacks an equivalent generic budget. | 5. Intentionally removed or simplified | Neither (warning only) | `407cb96b:crates/ralph-core/src/event_loop/mod.rs:80-87`; `crates/ralph-core/src/preflight.rs:221-238`; `docs/migration/v3-autoloop-engine.md:38-42` | Reduced failure-governor fidelity versus V2. |
| 5 | Failure semantics / stop-reason taxonomy | V2 used Ralph-specific reasons such as `LoopThrashing`, `LoopStale`, `ValidationFailure`, `RestartRequested`, `Cancelled`. | V3 maps Autoloop's closed `STOP_REASONS` set onto Ralph termination reasons. | 2. Replaced by a different mechanism | Autoloop + Ralph mapper | `407cb96b:crates/ralph-core/src/event_loop/mod.rs:69-147`; `crates/ralph-cli/src/autoloop_engine.rs:24-82`; `d14fd11:packages/harness/src/types.ts:416-452` | Different semantics, but stronger closed-set engine contract. |
| 6 | Global backend selection (`cli.backend`, `-b`) | Global backend selection supported in-process. | Global backend is translated into Autoloop backend config; unsupported mappings fail fast. | 1. Retained with equivalent behavior | Ralph translates, Autoloop executes | `407cb96b:README.md:136-142`; `crates/ralph-cli/src/autoloop_preset_gen.rs:212-317`; `docs/migration/v3-autoloop-engine.md:43-45` | Equivalent at the global level. |
| 7 | **Per-hat backend override** | Supported on the V2 run path; active hat could select a custom backend + args. | **Regression:** the config model and docs still expose hat-level backend fields, but generated Autoloop presets use only global `cli.backend`, even though Autoloop supports per-role backend overrides. | 6. Missing regression | Ralph | `407cb96b:crates/ralph-cli/src/loop_runner.rs:1691-1771`; `docs/guide/backends.md:354-369`; `crates/ralph-core/src/config.rs:1928-1936`; `crates/ralph-cli/src/autoloop_preset_gen.rs:212-317,355-356`; `ab7e34c:packages/core/src/topology.ts:12-25,458-491` | Confirmed user-facing config fidelity regression. |
| 8 | Hats / event routing / role dispatch | Ralph bus + hat registry routed events among hats. | Ralph hats are translated into Autoloop topology roles + `[handoff]` routes. | 2. Replaced by a different mechanism | Autoloop | `407cb96b:crates/ralph-core/src/event_loop/mod.rs:156-177`; `crates/ralph-cli/src/autoloop_preset_gen.rs:9-24,51-120` | Same concept, different execution owner. |
| 9 | Hat concurrency / aggregation | V2 had bespoke wave/concurrency machinery. | V3 maps hat `concurrency` / `aggregate` into Autoloop declarative role concurrency / aggregation. | 2. Replaced by a different mechanism | Autoloop | `407cb96b:crates/ralph-cli/src/loop_runner.rs:2506-2509,5174-5559`; `crates/ralph-cli/src/autoloop_preset_gen.rs:13-24,94-112`; `AGENTS.md:148-179` | Scatter-gather remains possible, but the model is not identical. |
| 10 | `ralph wave …` and wave-worker UI | Explicit wave system and wave-worker UX existed in V2. | Removed; docs point users to declarative concurrency instead. Help overlay still mentions deleted wave workers. | 5. Intentionally removed or simplified | Removed | `docs/migration/v3-autoloop-engine.md:27-29`; `crates/ralph-tui/src/widgets/help.rs:113-128` | Intentional simplification with one stale UI artifact. |
| 11 | Task completion authority | Ralph-owned loop/task state participated in V2 loop control. | Autoloop owns the canonical completion gate; Ralph task records are incompatible and observational only. | 3. Moved to Autoloop ownership | Autoloop | `docs/advanced/architecture.md:57-65`; `crates/ralph-cli/src/autoloop_engine.rs:188-205`; `docs/concepts/memories-and-tasks.md:123-127` | Users must distinguish engine tasks from Ralph tasks. |
| 12 | Scratchpad as engine continuity mechanism | V2 persisted and restored loop state, scratchpad usage, and continue semantics inside Ralph. | V3 keeps scratchpad as retained Ralph state only; per-hat scratchpad fields remain accepted but are not translated into Autoloop topology. | 5. Intentionally removed or simplified | Ralph retained config only | `407cb96b:crates/ralph-core/src/event_loop/mod.rs:522-550`; `docs/concepts/memories-and-tasks.md:171-175` | Continuity is less scratchpad-centric than V2. |
| 13 | Ralph task CLI / loop-id continuity | V2 wrote `.ralph/current-loop-id` and current-events markers for Ralph-owned coordination. | V3 still preserves coordination loop IDs and Ralph task tagging under `--continue`, but only as Ralph-side coordination state. | 4. Moved to Ralph observation/coordination ownership | Ralph | `407cb96b:crates/ralph-cli/src/loop_runner.rs:189-214`; `crates/ralph-cli/tests/integration_continue_resume.rs:129-173` | Coordination continuity survives even though engine state moved. |
| 14 | Ralph-native resume (`ralph resume`) | V2 could restore persisted loop state via `restore_loop_state`. | `ralph resume` is hidden and explicitly unsupported; `run --continue` only preserves Ralph coordination state, while native engine resume is an advanced `autoloop resume <run-id>` escape hatch. | 6. Missing regression | Ralph UX gap | `407cb96b:crates/ralph-core/src/event_loop/mod.rs:522-550`; `crates/ralph-cli/src/main.rs:547-549,650-657,1731-1734`; `crates/ralph-cli/tests/integration_continue_resume.rs:177-190` | Important post-GA UX gap for interrupted runs. |
| 15 | Telegram HITL / proactive guidance during runs | V2 README advertised working `human.interact`, proactive guidance, and `/restart`. | V3 explicitly marks Telegram HITL inactive for `ralph run`, pending `autoloop#345`, even though Autoloop itself now supports blocking ask/respond. | 7. Deferred/blocked on explicit upstream dependency | Shared: Autoloop capability exists; Ralph relay missing | `407cb96b:README.md:144-168`; `README.md:182-198`; `crates/ralph-core/src/config.rs:882-883`; `d14fd11:packages/harness/src/ask.ts:1-9` | High-profile capability gap, but honestly documented now. |
| 16 | Headless live progress streaming | V2 printed per-iteration progress natively. | V3 headless mode tails Autoloop `--events` and proves progress appears before subprocess exit. | 1. Retained with equivalent behavior | Ralph + Autoloop contracts | `crates/ralph-cli/tests/integration_autoloop_headless_stream.rs:45-158` | Confirmed parity for headless progress. |
| 17 | TUI live assistant/tool content | V2 had in-process streaming handles and RPC/TUI observers. | V3 TUI is live, but provisional assistant text comes from undocumented `.autoloop/runs/<run>/claude-stream.N.jsonl` / `pi-stream.N.jsonl` files. | 2. Replaced by a different mechanism | Ralph on top of private engine files | `407cb96b:crates/ralph-tui/src/lib.rs:12-23`; `crates/ralph-adapters/README.md:17-40`; `crates/ralph-cli/tests/integration_autoloop_tui_live_stream.rs:94-156` | Works today, but stability depends on private engine internals. |
| 18 | Web dashboard live loop state | V2's engine wrote `.ralph/current-events` / `.ralph/events-*.jsonl`, matching the dashboard readers. | **Regression:** the Rust API still watches `.ralph/current-events`, while the Autoloop engine path writes `.ralph/autoloop-events.ndjson`; README and `ralph web` now warn live state does not render. | 6. Missing regression | Ralph | `407cb96b:crates/ralph-cli/src/loop_runner.rs:201-214`; `crates/ralph-api/src/event_watcher.rs:37-70`; `crates/ralph-cli/src/autoloop_engine.rs:369-383`; `crates/ralph-cli/src/web.rs:27-27` | Confirmed user-facing regression on a shipped command. |
| 19 | `ralph run --rpc` / `--record-session` old surfaces | V2 TUI supported in-process, RPC client, and subprocess RPC modes; smoke recording existed. | Both are explicitly removed; replay now uses fake-Autoloop fixtures instead. | 5. Intentionally removed or simplified | Removed / replaced | `407cb96b:crates/ralph-tui/src/lib.rs:12-23`; `docs/migration/v3-autoloop-engine.md:27-32`; `crates/ralph-cli/tests/fixtures/autoloop/README.md:1-82` | Breaking change, but now documented. |
| 20 | Machine-readable engine events / journal / summary | V2 mainly exposed Ralph-owned events and UI paths. | V3 has explicit Autoloop contracts: `--events` NDJSON, `.autoloop/journal.jsonl`, and a parseable terminal summary block. | 8. New in V3 | Autoloop | `crates/ralph-adapters/README.md:7-15`; `crates/ralph-adapters/tests/autoloop_native_contract_integration.rs:1-67`; `crates/ralph-adapters/tests/autoloop_parity_integration.rs:1-70` | Major observability improvement and clearer boundary. |
| 21 | Doctor / dependency health / zero-step engine provisioning | V2 did not need a separate engine binary health model. | V3 adds minimum-version checks, install guidance, vendored-engine resolution, and first-run provisioning. | 8. New in V3 | Ralph | `crates/ralph-core/src/autoloop_health.rs:11-18`; `docs/migration/v3-autoloop-engine.md:9-21` | Better operational reliability and clearer install story. |
| 22 | Parallel loops / worktrees / merge queue | V2 already had Ralph-owned coordination around parallel worktrees. | V3 keeps those surfaces and replays them after engine exit via completion coordination. | 4. Moved to Ralph observation/coordination ownership | Ralph | `crates/ralph-cli/src/completion_coord.rs:98-157`; `docs/migration/v3-autoloop-engine.md:46-50` | Core Ralph differentiator is preserved. |
| 23 | Replay substrate and real-engine contract testing | V2 relied on in-house smoke/session-recording infrastructure and legacy cassettes. | V3 replaces that with fake-Autoloop fixtures plus real engine contract/parity tests. | 8. New in V3 | Ralph + Autoloop contracts | `crates/ralph-cli/tests/fixtures/autoloop/README.md:1-82`; `crates/ralph-adapters/tests/autoloop_native_contract_integration.rs:1-67`; `crates/ralph-adapters/tests/autoloop_parity_integration.rs:1-70` | Better for the new architecture; old cassette parity is no longer the right gate. |
| 24 | TUI exports / retained iteration history / cost display | V2 could export iteration buffers. | V3 still exports stable text snapshots and includes engine-derived cost/iteration info in the TUI/event path. | 1. Retained with equivalent behavior | Ralph | `407cb96b:crates/ralph-tui/src/export.rs:1-120`; `crates/ralph-tui/src/export.rs:1-120` | No meaningful regression here. |

## 4. Confirmed GA blockers, important post-GA gaps, intentional removals, V3 improvements

### Confirmed GA blockers

These are the confirmed gaps most likely to invalidate a claim of "V2-equivalent user-facing configuration and observability" for V3:

1. **Per-hat backend overrides are broken on the Autoloop path.**
   - Why blocker-level: V2 supported it, current docs still advertise it, the config model still accepts it, and Autoloop already has the primitive Ralph would need.
   - Evidence: `407cb96b:crates/ralph-cli/src/loop_runner.rs:1691-1771`; `docs/guide/backends.md:354-369`; `crates/ralph-cli/src/autoloop_preset_gen.rs:212-317,355-356`; `ab7e34c:packages/core/src/topology.ts:12-25,458-491`.

2. **The web dashboard's live-state path is severed under the V3 engine.**
   - Why blocker-level: `ralph web` is still a shipped user surface; current code and warnings admit it does not show live loop state under Autoloop.
   - Evidence: `crates/ralph-api/src/event_watcher.rs:37-70`; `crates/ralph-cli/src/autoloop_engine.rs:369-383`; `crates/ralph-cli/src/web.rs:27-27`.
   - Caveat: because the dashboard is explicitly labeled Alpha, some teams may choose to downgrade this from GA blocker to post-GA gap. The source confirms the gap; severity is a release-policy decision.

### Important post-GA gaps

1. **No Ralph-native direct resume UX.**
   - `run --continue` is coordination-only; `ralph resume` is still unsupported.
   - Evidence: `crates/ralph-cli/src/main.rs:650-657,1731-1734`; `crates/ralph-cli/tests/integration_continue_resume.rs:177-190`.

2. **TUI provisional live content still depends on private engine files.**
   - Evidence: `crates/ralph-adapters/README.md:17-40`.

3. **Stale TUI help still references deleted wave workers.**
   - Evidence: `crates/ralph-tui/src/widgets/help.rs:113-128`.

4. **Per-hat scratchpad overrides are accepted but untranslated.**
   - Evidence: `crates/ralph-core/src/config.rs:1948-1951`; `docs/concepts/memories-and-tasks.md:171-175`.

### Intentional removals / simplifications

1. `ralph wave …` removed in favor of declarative hat concurrency / aggregation.
2. `ralph run --rpc` removed.
3. `ralph run --record-session` removed; replay now uses fake-Autoloop fixtures.
4. `event_loop.max_consecutive_failures` retained only as a warning, not an enforced engine limit.
5. Scratchpad is no longer the engine's authoritative continuity mechanism.

Evidence: `docs/migration/v3-autoloop-engine.md:23-42`, `crates/ralph-core/src/preflight.rs:205-236`, `docs/concepts/memories-and-tasks.md:171-175`.

### V3 improvements

1. **Stable machine-readable engine contracts** (`--events`, journal, summary). Evidence: `crates/ralph-adapters/README.md:7-15`.
2. **Explicit engine dependency health + provisioning**. Evidence: `crates/ralph-core/src/autoloop_health.rs:11-18`, `docs/migration/v3-autoloop-engine.md:9-21`.
3. **Replay substrate aligned with the new architecture** and real engine contract/parity tests. Evidence: `crates/ralph-cli/tests/fixtures/autoloop/README.md:1-82`, `crates/ralph-adapters/tests/autoloop_native_contract_integration.rs:1-67`, `crates/ralph-adapters/tests/autoloop_parity_integration.rs:1-70`.

## 5. Detailed analysis of ambiguous / high-risk areas

### 5.1 HITL: upstream capability exists, Ralph UX does not

This is the most important ownership trap in the migration.

- **V2 Ralph UX:** clearly working and user-visible. Evidence: `407cb96b:README.md:144-168`.
- **Current Ralph UX:** explicitly inactive for `ralph run`. Evidence: `README.md:182-198`, `crates/ralph-core/src/config.rs:882-883`.
- **Current Autoloop primitive:** implemented. Autoloop blocks on `ask.pending` and accepts `control respond`. Evidence: `d14fd11:packages/harness/src/ask.ts:1-9`; `crates/ralph-adapters/tests/autoloop_native_contract_integration.rs:52-67`.

**Interpretation:** this is **not** an engine-primitive gap anymore. It is a **Ralph-exposed UX gap**: users still cannot get Telegram relay during `ralph run`, even though the engine-side blocking protocol exists.

### 5.2 Resume: three different concepts are now easy to confuse

There are now three distinct semantics:

1. **V2 `ralph resume`** — restore Ralph-owned loop state. Evidence: `407cb96b:crates/ralph-core/src/event_loop/mod.rs:522-550`.
2. **V3 `ralph run --continue`** — keep Ralph coordination identity only. Evidence: `crates/ralph-cli/src/main.rs:650-657`; `crates/ralph-cli/tests/integration_continue_resume.rs:129-173`.
3. **Native engine resume** — `autoloop resume <run-id>`. Evidence: `crates/ralph-cli/src/main.rs:1731-1734`.

That distinction is technically honest, but it is a weaker Ralph UX than V2. If Ralph wants a user-facing recovery story comparable to V2, it needs a first-class Ralph wrapper around engine run IDs, not just an escape hatch message.

### 5.3 TUI evidence retention: good lifecycle parity, fragile assistant-text parity

The current TUI story is split:

- **Lifecycle progress parity is solid** via supported `--events` streaming. Evidence: `crates/ralph-cli/tests/integration_autoloop_headless_stream.rs:45-158`.
- **Assistant/tool-call live content is fragile** because it uses undocumented run-directory files. Evidence: `crates/ralph-adapters/README.md:17-40`, `crates/ralph-cli/tests/integration_autoloop_tui_live_stream.rs:94-156`.

This is acceptable as a temporary bridge, but it is not the same quality of contract as V2's fully Ralph-owned in-process stream.

### 5.4 State and artifact access: `.ralph/` vs `.autoloop/`

V3 introduces a real split-brain risk if users or auxiliary tools assume all runtime truth still lives in `.ralph/`.

- Dashboard/API watchers still look at `.ralph/current-events`. Evidence: `crates/ralph-api/src/event_watcher.rs:37-70`.
- The engine path writes live observation to `.ralph/autoloop-events.ndjson` and canonical run state to `.autoloop/`. Evidence: `crates/ralph-cli/src/autoloop_engine.rs:369-383`; `crates/ralph-adapters/README.md:7-15`.

**Practical consequence:** V3 needs sharper documentation and tooling around which files are stable contracts versus historical coordination files.

### 5.5 Task / memory authority: the migration succeeded architecturally, but the UX is weaker than the older cutover plan implied

Current code and docs are explicit:

- Ralph tasks remain valid for coordination and observation.
- Autoloop alone decides engine completion.
- The formats are incompatible, so Ralph tasks do not participate in the engine gate.

Evidence: `docs/advanced/architecture.md:57-65`, `docs/concepts/memories-and-tasks.md:123-127`, `crates/ralph-cli/src/autoloop_engine.rs:188-205`, `crates/ralph-cli/src/autoloop_preset_gen.rs:324-332`.

This is architecturally clean, but it is a genuine UX downgrade for users who previously treated `ralph tools task ...` as part of the loop's authoritative working state.

### 5.6 Configuration translation: the highest-confidence remaining Ralph-side regression

The most concrete translation bug is per-hat backend override loss:

- V2 run path honored hat backends dynamically. Evidence: `407cb96b:crates/ralph-cli/src/loop_runner.rs:1691-1771`.
- V3 docs still promise per-hat backend override. Evidence: `docs/guide/backends.md:354-369`.
- V3 config model still accepts the fields. Evidence: `crates/ralph-core/src/config.rs:1928-1936`.
- V3 preset generation ignores them and emits one global backend block. Evidence: `crates/ralph-cli/src/autoloop_preset_gen.rs:212-317,355-356`.
- Autoloop already supports per-role backend fields. Evidence: `ab7e34c:packages/core/src/topology.ts:12-25,458-491`.

This is the clearest case where V3 is missing a V2 feature **without** being blocked on the engine.

## 6. Contract boundary: what Autoloop owns vs what Ralph owns

| Surface | Autoloop owns | Ralph owns | Notes |
|---|---|---|---|
| Iteration loop / dispatch / completion | Yes | No | `crates/ralph-cli/src/autoloop_engine.rs:1-7` |
| Hat translation from `ralph.yml` | No | Yes | Ralph generates temporary Autoloop preset |
| Machine-readable live events | Yes (`--events`) | Consumes | `crates/ralph-adapters/README.md:7-15` |
| Canonical run journal / stop reason / run summary | Yes | Consumes | `crates/ralph-adapters/README.md:7-15` |
| Merge queue / landing / loop registry / worktrees | No | Yes | `crates/ralph-cli/src/completion_coord.rs:98-157` |
| Headless/TUI rendering | No | Yes | Ralph renders engine contracts |
| Telegram bot setup/status | No | Yes | Relay into run still missing |
| Human ask blocking primitive | Yes | Not yet fully exposed | `d14fd11:packages/harness/src/ask.ts:1-9` |
| Doctor / install / provision checks | No | Yes | `crates/ralph-core/src/autoloop_health.rs:11-18` |
| Task completion gate | Yes | No | Ralph tasks are observational only |
| Ralph memories/tasks/scratchpad files | No | Yes | Coordination artifacts only under V3 |
| Provisional live assistant-text files | Private internal | Consumed opportunistically | This is the unstable edge (`claude-stream.*`, `pi-stream.*`) |

## 7. Recommendations (P0 / P1 / P2)

### P0

1. **Ralph repo: restore per-hat backend override fidelity on the Autoloop path.**
   - Acceptance outcome: a config with different hat backends produces different per-role backend settings in the generated Autoloop topology, and an integration test proves the routed roles invoke distinct backend commands.

### P1

1. **Ralph repo: fix or explicitly retire dashboard live-state support under V3.**
   - Acceptance outcome: `ralph web` against a live `ralph run` from the same workspace shows current iteration / role / status from `.ralph/autoloop-events.ndjson`, or the command and docs are downgraded so users are not promised live loop monitoring.

2. **Ralph repo: provide a first-class Ralph-native resume UX over engine run IDs.**
   - Acceptance outcome: a stopped or interrupted run can be resumed through Ralph without requiring the user to invoke `autoloop resume` manually, and Ralph preserves history / registry / merge coordination coherently across the resume boundary.

### P2

1. **Autoloop repo: expose a documented live-output contract for assistant deltas and tool calls.**
   - Acceptance outcome: Ralph can remove all probing of `.autoloop/runs/<run>/claude-stream.*` / `pi-stream.*` and derive live TUI content entirely from documented engine contracts.

2. **Ralph repo: scrub stale wave-worker help and tighten docs around retained-but-untranslated config.**
   - Acceptance outcome: no shipped user-facing help or docs imply deleted wave-worker controls or unsupported per-hat scratchpad semantics.

3. **Ralph repo: clarify `.ralph/` vs `.autoloop/` artifact boundaries in user docs and APIs.**
   - Acceptance outcome: every shipped observer surface (CLI/API/dashboard/TUI docs) clearly distinguishes coordination artifacts from engine authority, and no live observer still tails legacy event files unless that file is actually written on the V3 path.

## 8. Confidence and evidence gaps

### Confidence

**High overall confidence** on the main ownership split and on the confirmed regressions listed above. The strongest findings are anchored in current code paths, not stale migration notes.

### Evidence gaps / unproven but high-risk claims

1. **Released-engine live TUI parity risk**
   - Proven facts:
     - Ralph vendors `0.10.1` (`crates/ralph-core/src/autoloop_health.rs:11-18`).
     - Autoloop main added `ab7e34c fix(backends): flush stream logs incrementally`, which is newer than `v0.10.1`.
     - Ralph's live TUI text path depends on incremental growth of backend stream files (`crates/ralph-adapters/README.md:17-40`).
   - Not proven here: an actual live TUI run on **released** Autoloop `0.10.1` showing missing assistant-text streaming.
   - Therefore I label this a **high-risk evidence gap**, not a confirmed blocker.

2. **Dashboard V2 parity beyond the file-contract level**
   - Proven facts:
     - V2 wrote `.ralph/current-events` and `.ralph/events-*.jsonl`.
     - Current dashboard/API code still watches `.ralph/current-events`.
     - V3 engine path writes `.ralph/autoloop-events.ndjson` instead.
   - Not independently replayed here: a pinned V2 live dashboard session.
   - The file-contract mismatch alone is enough to confirm the current V3 regression.

3. **Bead conversion follow-through**
   - The repo's completion gate for this task required committing only the research document, so I did **not** mutate `.beads/issues.jsonl` in this change.
   - Existing open beads already cover the dashboard gap (`ga3-c4-dashboard-dead-svf`) and stale TUI wave help (`tui-help-wave-stale-5hu`).
   - The main newly confirmed actionable gap from this research is **per-hat backend override loss**; it should become a bead in follow-up work if not filed elsewhere.
