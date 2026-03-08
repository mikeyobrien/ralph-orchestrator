# Agent Waves: Implementation Plan

## Checklist

- [ ] Step 1: Event model extensions (wave metadata)
- [ ] Step 2: HatConfig extensions (concurrency, aggregate)
- [ ] Step 3: WaveTracker state machine
- [ ] Step 4: Wave CLI tool (`ralph wave emit`)
- [ ] Step 5: Wave worker prompt builder
- [ ] Step 6: Loop runner wave execution
- [ ] Step 7: Context injection for NL dispatch
- [ ] Step 8: Nested wave prevention
- [ ] Step 9: Diagnostics and observability
- [ ] Step 10: Smoke tests and E2E
- [ ] Step 11: Documentation and example presets

---

## Step 1: Event Model Extensions

**Objective:** Add optional wave correlation metadata to the event system so wave events can be identified, tracked, and correlated throughout the pipeline.

**Implementation guidance:**
- Add `wave_id: Option<String>`, `wave_index: Option<u32>`, `wave_total: Option<u32>` to `Event` struct in `crates/ralph-proto/src/event.rs`
- Add builder methods: `with_wave(wave_id, index, total)`, `is_wave_event()`
- Add same fields to `EventRecord` in `crates/ralph-core/src/event_logger.rs` with `#[serde(skip_serializing_if = "Option::is_none")]`
- Update `EventReader` in `crates/ralph-core/src/event_reader.rs` to parse wave fields from JSONL using `#[serde(default)]` for backwards compatibility
- Update `EventRecord::new()` and `EventRecord::from_agent_event()` to propagate wave fields from `Event`

**Test requirements:**
- Unit: wave metadata round-trips through serialize/deserialize
- Unit: events without wave fields parse correctly (backwards compat)
- Unit: `is_wave_event()` returns correct results
- Unit: `EventReader` parses JSONL with and without wave fields

**Integration notes:** This is the foundation — every subsequent step depends on these fields existing. No behavioral changes yet; existing functionality is unaffected.

**Demo:** `cargo test -p ralph-proto` and `cargo test -p ralph-core` pass. Manually write a JSONL line with wave fields, verify `EventReader` parses it.

---

## Step 2: HatConfig Extensions

**Objective:** Add `concurrency` and `aggregate` configuration fields to hat definitions so preset authors can declare wave-capable and aggregator hats.

**Implementation guidance:**
- Add `concurrency: u32` (default 1) to `HatConfig` in `crates/ralph-core/src/config.rs`
- Add `aggregate: Option<AggregateConfig>` to `HatConfig`
- Define `AggregateConfig { mode: AggregateMode, timeout: u32 }` and `AggregateMode::WaitForAll`
- Add validation in `RalphConfig::validate()`:
  - `concurrency >= 1`
  - Error if `aggregate` set on hat with `concurrency > 1`
  - Warn if `concurrency > 1` but no downstream hat has `aggregate`
- Propagate `concurrency` to `Hat` struct in `crates/ralph-proto/src/hat.rs` if needed for runtime access

**Test requirements:**
- Unit: YAML with `concurrency: 3` and `aggregate: { mode: wait_for_all, timeout: 600 }` parses correctly
- Unit: YAML without new fields parses with defaults (concurrency=1, aggregate=None)
- Unit: validation rejects `concurrency: 0`
- Unit: validation rejects aggregate on concurrent hat
- Unit: existing preset YAML files still parse correctly

**Integration notes:** Pure config — no runtime behavior changes. Existing presets are unaffected because defaults preserve current behavior.

**Future failure modes (v2+):** v1 is hardcoded best-effort (continue-on-failure) — partial results are almost always useful, so the wave continues when instances fail and the aggregator gets structured failure metadata. `on_failure: fail_fast` is worth adding in v2 for all-or-nothing use cases (e.g., parallel builds where one failure invalidates everything). Failure thresholds and must-pass hats are better handled by aggregator instructions — the aggregator already sees which instances failed and can decide how to react. If we see patterns where people keep writing the same "abort if X failed" logic in aggregator instructions, that's the signal to promote it to config.

**Demo:** Write a test hat collection YAML with wave config, verify it parses and validates.

---

## Step 3: WaveTracker State Machine

**Objective:** Build the core state machine that tracks active waves, records results and failures, manages timeouts, and determines when aggregation gates should open.

**Implementation guidance:**
- New file: `crates/ralph-core/src/wave_tracker.rs`
- Core structs: `WaveTracker`, `WaveState`, `WaveInstance`, `InstanceStatus`, `WaveResult`, `WaveFailure`, `CompletedWave`, `WaveProgress`
- Key methods:
  - `register_wave(wave_id, events, worker_hat, timeout)` — creates new wave state
  - `record_result(wave_id, event)` → `WaveProgress` (InProgress or Complete)
  - `record_failure(wave_id, index, error, duration)` — records instance failure
  - `is_complete(wave_id)` — all results + failures == expected total
  - `check_timeouts()` → `Vec<String>` — returns timed-out wave IDs
  - `take_wave_results(wave_id)` → `CompletedWave` — consumes completed wave
  - `has_active_waves()` — any waves in progress
- Add `mod wave_tracker` to `crates/ralph-core/src/lib.rs` and export

**Test requirements:**
- Unit: register wave, record results one by one, verify Complete on last
- Unit: register wave, record some results + failure, verify completion accounting
- Unit: timeout detection with mocked time
- Unit: `take_wave_results` returns all results and failures, removes wave
- Unit: multiple concurrent waves tracked independently

**Integration notes:** Pure data structure — no I/O, no async. Can be tested entirely with synchronous unit tests. Will be integrated into the loop runner in Step 6.

**Demo:** `cargo test -p ralph-core wave_tracker` — all state transitions exercised.

---

## Step 4: Wave CLI Tool

**Objective:** Build the `ralph wave emit` command for atomic batch wave dispatch, and enhance `ralph emit` to support wave worker env vars.

**Implementation guidance:**
- New file: `crates/ralph-cli/src/wave.rs`
- Add `Wave(wave::WaveArgs)` to `Commands` enum in `crates/ralph-cli/src/main.rs`
- Wire up `wave::execute()` in the command dispatch match
- v1 only supports batch emission — no `start`/`end` subcommands (deferred to v2)
- `ralph wave emit <topic> --payloads "a" "b" "c"`:
  1. Check `RALPH_WAVE_WORKER` env var — if set, exit with error (nested wave prevention)
  2. Generate wave ID (timestamp-based hex: `w-{:08x}` from nanos mod `0xFFFF_FFFF`)
  3. Resolve events file from `.ralph/current-events` marker (falling back to `.ralph/events.jsonl`)
  4. Write N events to JSONL, each with `wave_id`, `wave_index: 0..N-1`, `wave_total: N`
  5. Print wave ID to stdout
- Enhance existing `ralph emit` (in `main.rs:emit_command`):
  - Check `RALPH_EVENTS_FILE` env var — if set, write to that file instead of default
  - Check `RALPH_WAVE_ID` + `RALPH_WAVE_INDEX` env vars — if set, auto-tag events with wave metadata
  - When no wave env vars are present, behavior is unchanged (backwards compatible)

**Test requirements:**
- Unit/CLI: `ralph wave emit topic --payloads a b c` writes 3 tagged events atomically
- Unit/CLI: `ralph emit` with `RALPH_WAVE_ID` and `RALPH_WAVE_INDEX` env vars tags events correctly
- Unit/CLI: `ralph emit` with `RALPH_EVENTS_FILE` env var writes to specified file
- Unit/CLI: `ralph emit` without wave env vars works unchanged
- Unit/CLI: `RALPH_WAVE_WORKER=1 ralph wave emit` fails with error

**Integration notes:** This is the agent-facing interface. The events file is the handoff — CLI writes tagged JSONL, loop runner reads and detects waves in Step 6. The env var approach keeps `ralph emit` backwards compatible while transparently supporting wave workers.

**Demo:** Run `ralph wave emit review.file --payloads "src/main.rs" "src/lib.rs" "src/config.rs"`. Inspect events file — three events with matching wave_id, sequential indices, wave_total=3.

---

## Step 5: Wave Worker Prompt Builder

**Objective:** Build the prompt constructor for wave worker instances — each worker gets focused context with the hat's instructions and its specific event payload.

**Implementation guidance:**
- New file: `crates/ralph-core/src/wave_prompt.rs`
- Add `mod wave_prompt` to `crates/ralph-core/src/lib.rs` and export
- `build_wave_worker_prompt(HatConfig, Event, WaveWorkerContext) -> String`
- `WaveWorkerContext` contains: `wave_id`, `wave_index`, `wave_total`, `result_topics` (from hat's `publishes`)
- Prompt sections:
  1. Hat instructions (from config)
  2. Wave context metadata (wave_id, your index, total instances)
  3. Event payload (the work item)
  4. Event writing guide (how to emit results — topic from `publishes`, env vars handle correlation transparently)
  5. Nested wave guard ("Do NOT use `ralph wave` commands")
- Keep it simple — no HATS table, no objective, no scratchpad. Workers are focused executors.
- The events file path and wave correlation metadata are communicated via env vars (Step 6), not embedded in the prompt. The prompt only includes what the agent needs to understand its task.

**Test requirements:**
- Unit: prompt includes hat instructions
- Unit: prompt includes event payload
- Unit: prompt includes wave metadata
- Unit: prompt includes nested wave prohibition
- Unit: prompt includes correct result topic from hat's `publishes`

**Integration notes:** Used by the loop runner (Step 6) when spawning wave backends. Pure string construction — no I/O.

**Demo:** Call `build_wave_worker_prompt()` with test data, inspect output string for all required sections.

---

## Step 6: Loop Runner Wave Execution

**Objective:** The core integration — detect wave events after a normal iteration, spawn concurrent backends for wave workers, collect results, merge into the main events file, and resume the normal loop. The loop runner owns the entire wave lifecycle; the event loop remains wave-agnostic.

**Implementation guidance:**
- In `crates/ralph-cli/src/loop_runner.rs`, after `process_events_from_jsonl()`:
  - New struct `DetectedWave { wave_id, target_hat: HatId, hat_config: HatConfig, events: Vec<Event>, total: u32 }`
  - New function `detect_wave_events(events, registry) -> Option<DetectedWave>` — groups events by `wave_id`, validates consistency, resolves target hat from event topic via `HatRegistry`
  - When wave detected, enter wave execution mode:
    1. Create per-worker events files (`.ralph/wave-{wave_id}-{index}.jsonl`)
    2. Register wave in `WaveTracker`
    3. Spawn concurrent backends using `tokio::sync::Semaphore` for concurrency limiting
    4. Each backend gets env vars: `RALPH_WAVE_WORKER=1`, `RALPH_WAVE_ID`, `RALPH_WAVE_INDEX`, `RALPH_EVENTS_FILE` (pointing to per-worker file)
    5. Each backend uses worker hat's backend config, prompt from `build_wave_worker_prompt()`
    6. Collect results as instances complete — read events from each per-worker file
    7. Handle failures — call `wave_tracker.record_failure()`
    8. Race against aggregate timeout (resolved from downstream aggregator hat's config)
    9. On timeout: cancel running instances (SIGTERM, then SIGKILL after 250ms)
    10. Merge all result events from per-worker files into the main events file
    11. Clean up per-worker files
    12. Increment worker hat's activation count by number of instances
    13. Accumulate costs into global `max_cost` check
  - Resume normal loop — aggregator hat sees all results as pending events on next iteration
- `WaveInstanceResult { index, status, events, cost, tokens, duration }` — returned by each instance
- The event loop never sees partial wave results. By the time it processes events on the next iteration, all results have been merged.

**Test requirements:**
- Integration: mock backend that emits result events, verify wave lifecycle end-to-end
- Integration: concurrency limiting — 5 instances, concurrency=2, verify max 2 concurrent
- Integration: timeout fires, verify partial results collected, instances terminated
- Integration: instance failure, verify wave continues, failure recorded
- Integration: activation counts incremented per instance
- Integration: cost accumulated across all instances
- Integration: per-worker event files created, read, merged, and cleaned up
- Integration: main events file not written to during wave execution

**Integration notes:** This is the largest and most complex step. It touches the main loop's execution path. Consider implementing incrementally: first get sequential wave execution working (concurrency=1), then add the semaphore-based concurrency. The aggregator gate is implicit — by merging all results at once, the event loop's existing `determine_active_hats()` naturally picks up the aggregator hat.

**Demo:** Create a test hat collection with dispatcher + worker (concurrency=2) + aggregator. Run a small wave (3 items) with a mock backend. Verify concurrent execution, result collection, and aggregator activation.

---

## Step 7: Context Injection for NL Dispatch

**Objective:** Enhance the HATS table in Ralph's prompt to include downstream hat descriptions and wave dispatch instructions, enabling natural language wave dispatch.

**Implementation guidance:**
- In `crates/ralph-core/src/hatless_ralph.rs`, in the HATS table generation (`hats_section`):
  - When the active hat has `publishes` targeting wave-capable hats (`concurrency > 1`):
    - Add "Available Downstream Hats" section with topic, name, description, concurrency
    - Add wave emission instructions (brief `ralph wave emit` usage)
  - When the active hat `publishes` target multiple different hats (scatter-gather):
    - Same enrichment, showing each target hat
- Use existing `HatInfo` and topology resolution — this already resolves `publishes` → downstream hats
- Keep it concise — a few-line table + one-liner wave instruction, not a tutorial

**Test requirements:**
- Unit: dispatcher hat with wave-capable downstream → prompt includes downstream table
- Unit: dispatcher hat with non-wave downstream (concurrency=1) → no wave context injected
- Unit: scatter-gather hat with multiple downstream hats → all listed
- Unit: prompt includes wave emission instructions
- Unit: hat with no publishes → no downstream section

**Integration notes:** Pure prompt construction — extends existing HATS table logic. No runtime behavior changes. This is what makes NL dispatch possible: the model sees what's available and decides what to fan out.

**Demo:** Build prompt for a dispatcher hat in a wave-capable collection. Inspect prompt for downstream hat table and wave instructions.

---

## Step 8: Nested Wave Prevention

**Objective:** Prevent wave workers from emitting further waves, avoiding complexity explosion in v1.

**Implementation guidance:**
- **Hard enforcement:** In `ralph wave emit` (wave.rs), check `RALPH_WAVE_WORKER` env var. If set, print error and exit with non-zero status. (This check is already partially implemented in Step 4, but verify it's in place.)
- **Soft enforcement:** In `build_wave_worker_prompt()` (Step 5), include "Do NOT use `ralph wave` commands. Nested waves are not supported."
- The loop runner (Step 6) sets `RALPH_WAVE_WORKER=1` on each wave worker backend process

**Test requirements:**
- Unit/CLI: `RALPH_WAVE_WORKER=1 ralph wave emit topic --payloads a b` exits with error
- Unit: wave worker prompt includes nested wave prohibition text

**Integration notes:** Small and self-contained. The env var is already set in Step 6; this step verifies the check in the CLI and prompt.

**Demo:** `RALPH_WAVE_WORKER=1 ralph wave emit topic --payloads a b` → error message.

---

## Step 9: Diagnostics and Observability

**Objective:** Ensure wave execution is visible in diagnostics, logs, and the TUI.

**Implementation guidance:**
- **Event logger:** Wave events logged with wave metadata fields in `orchestration.jsonl`
- **Diagnostics collector:** Log wave lifecycle events:
  - `wave.started` — wave_id, expected_total, worker_hat, concurrency
  - `wave.instance.started` — wave_id, index, backend
  - `wave.instance.completed` — wave_id, index, duration, cost
  - `wave.instance.failed` — wave_id, index, error, duration
  - `wave.completed` — wave_id, total_results, total_failures, timed_out, duration
- **Session recorder:** Include wave metadata in session records
- **TUI:** Show wave progress indicator (e.g., "Wave w-abc: 3/5 workers complete")
- **`ralph loops`/status:** If a wave is in progress, show it in loop status

**Test requirements:**
- Unit: diagnostics records include wave lifecycle events
- Unit: session recorder captures wave metadata
- Integration: run wave with diagnostics enabled, verify `.ralph/diagnostics/` files contain wave events

**Integration notes:** Observability layer — doesn't affect correctness but critical for debugging. Can be implemented incrementally alongside Step 6.

**Demo:** Run a wave with `RALPH_DIAGNOSTICS=1`. Inspect diagnostics output for wave lifecycle events.

---

## Step 10: Smoke Tests and E2E

**Objective:** Comprehensive test coverage using replay-based smoke tests and E2E framework.

**Note:** If BDD/cucumber acceptance tests land (lifecycle hooks experiment), consider restructuring to write acceptance tests earlier in the implementation sequence — potentially as a first step to drive development.

**Implementation guidance:**
- **Smoke test fixtures** in `crates/ralph-core/tests/fixtures/`:
  - `wave-basic.jsonl` — dispatcher emits 3 wave events, 3 worker results, aggregator fires
  - `wave-partial-failure.jsonl` — 5 wave events, 1 worker fails, aggregator gets 4 results + failure
  - `wave-timeout.jsonl` — wave with slow instances, aggregator fires on timeout with partial results
  - `wave-scatter-gather.jsonl` — dispatcher fans out to 3 different worker hats
- **E2E scenarios** in `crates/ralph-e2e/`:
  - Mock mode: full wave lifecycle with mock backend
  - Verify: concurrent execution, result collection, aggregator activation, cost accounting
- **Config validation tests:**
  - All existing preset YAML files still parse and validate correctly
  - New wave-enabled presets validate correctly

**Test requirements:**
- Smoke: all fixtures pass `cargo test -p ralph-core smoke_runner`
- E2E mock: `cargo run -p ralph-e2e -- --mock --filter wave` passes
- Regression: `cargo test` full suite still passes

**Integration notes:** Test step — no production code changes. Depends on all previous steps being functional.

**Demo:** `cargo test` green. `cargo run -p ralph-e2e -- --mock --filter wave` passes.

---

## Step 11: Documentation and Example Presets

**Objective:** Document the wave system for preset authors and ship ready-to-use presets that demonstrate the key wave patterns.

**Implementation guidance:**
- **Update `crates/ralph-core/data/ralph-tools.md`:** Add `ralph wave` command reference (required per CLAUDE.md)
- **Update CLAUDE.md:** Add wave section to architecture overview, mention `concurrency` and `aggregate` config
- **Document failure behavior:** How best-effort works, what the aggregator receives on partial failure/timeout, how to write aggregator instructions that handle missing results
- **Ship presets** in `presets/`:

  **Splitter pattern:**
  - `wave-research.yml` — parallel deep research with synthesis (NL dispatch). Planner identifies topics → researchers investigate in parallel → synthesizer combines findings.

  **Scatter-gather pattern:**
  - `wave-review.yml` — specialized parallel code review. Dispatcher reads diff → fan out to security/performance/architecture/correctness reviewers → synthesizer produces unified review.

  **Multi-round debate pattern:**
  - `wave-debate.yml` — moderator-debater with dynamic participant selection. Moderator dispatches to domain-specific debaters, aggregates responses, selectively re-dispatches based on unresolved disagreements. Uses `max_activations` to cap rounds.

  **Multi-phase pipeline:**
  - `wave-review-and-document.yml` — reviews branch changes vs main and writes documentation updates. Change analyzer → parallel specialized reviewers → doc planner → parallel doc writers (one per file) → doc reviewer with approve/revise decision loop. Demonstrates chaining multiple wave fan-outs in a single workflow with a decision-point aggregator.

  Each preset should be repo-agnostic where possible (referencing `git diff main...HEAD` rather than hardcoded paths) with comments indicating what to customize for specific repos.

**Test requirements:**
- All presets parse and validate: `cargo test` covers config validation
- Tool documentation is accurate: manual review

**Integration notes:** Documentation step. Presets serve as both documentation and starting points — users should be able to copy a preset and adapt it to their domain by changing reviewer specialties, doc paths, and concurrency limits.

**Demo:** `ralph run --config presets/wave-research.yml --dry-run` (or similar) validates the preset loads correctly.
