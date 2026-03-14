# Implementation Plan: Per-Project Orchestrator Lifecycle Hooks (v1)

## Checklist

- [ ] Step 0: Create Cucumber BDD suite skeleton + failing AC scenarios first (`AC-01`..`AC-18`)
- [ ] Step 1: Add hooks config schema + semantic validation in `ralph-core`
- [ ] Step 2: Add `ralph hooks validate` CLI command (human + JSON output)
- [ ] Step 3: Implement `HookExecutor` (JSON stdin, timeout, output truncation)
- [ ] Step 4: Add hook telemetry logging and diagnostics integration
- [ ] Step 5: Implement `HookEngine` dispatch for `loop.start` + `iteration.start`
- [ ] Step 6: Implement `on_error` dispositions (`warn`, `block`) end-to-end
- [ ] Step 7: Implement suspend core (`wait_for_resume`) + `ralph loops resume <id>`
- [ ] Step 8: Add suspend modes `retry_backoff` and `wait_then_retry`
- [ ] Step 9: Add remaining lifecycle mappings (`plan.created`, `human.interact`, `loop.complete`, `loop.error`)
- [ ] Step 10: Add opt-in metadata mutation (JSON-only, metadata namespace only)
- [ ] Step 11: Integrate hooks validation into preflight + update docs/examples
- [ ] Step 12: Add mutation testing gates for hooks-critical modules
- [ ] Step 13: Drive BDD suite to green + finalize traceability matrix + CI gate

**Sequencing rule (TDD/BDD-first):** Step 0 is mandatory before implementation steps. Build code incrementally to satisfy failing BDD scenarios.

---

## Step 0: Create Cucumber BDD suite skeleton + failing AC scenarios first (`AC-01`..`AC-18`)

**Objective**
Establish a BDD-first baseline so implementation is explicitly driven by requirements.

**Subtasks**
- 0a. Draft `AC-01..AC-18` → scenario mapping table (initial traceability skeleton).
- 0b. Create hook feature files and scenario placeholders per AC.
- 0c. Wire minimal step definitions into the existing e2e harness.
- 0d. Run and record a CI-safe red baseline.

**Implementation Guidance**
- Create Cucumber feature files under `crates/ralph-e2e/features/hooks/`.
- Add scenario/scenario-outline placeholders for all AC IDs (`AC-01`..`AC-18`).
- Implement minimal step definitions to execute scenarios against the existing harness.
- Mark scenarios as expected-failing/red for capabilities not yet implemented.

**Test Requirements**
- BDD runner executes all hook feature scenarios.
- Scenario naming includes AC IDs for traceability.
- Red baseline is reproducible in CI-safe mode.

**Integration Notes**
- Keep fixtures deterministic and avoid network-dependent steps.
- Do not block on full implementation here; this is scaffolding + red baseline.

**Demo**
- Run BDD suite and show AC-labeled scenarios are discovered and currently failing for unimplemented behavior.

---

## Step 1: Add hooks config schema + semantic validation in `ralph-core`

**Objective**
Create first-class config types for hooks and enforce v1 guardrails at config-validation time.

**Implementation Guidance**
- Add `HooksConfig`, `HookDefaults`, `HookSpec`, and hook enums (phase-event keys, `on_error`, `suspend_mode`) in `crates/ralph-core/src/config.rs` (or a new hooks config module re-exported from config).
- Add `hooks` to `RalphConfig` with safe defaults (`enabled: false`).
- Add semantic validation:
  - valid phase-event names only
  - required `name` + `command`
  - positive `timeout_seconds` / `max_output_bytes`
  - mutation shape is opt-in and JSON-only contract
  - reject non-v1 fields (global scope, parallel execution options)
- Keep behavior inert (no runtime dispatch yet).

**Test Requirements**
- Add serde parse tests for valid/invalid hooks YAML.
- Add validation tests for enum errors, missing fields, invalid limits, and non-goal fields.
- Ensure existing config tests still pass.

**Integration Notes**
- Must not alter current runtime behavior when hooks are absent/disabled.
- Keep backward compatibility with existing configs that omit `hooks`.

**Demo**
- `cargo test -p ralph-core config` passes.
- A sample config with `hooks.enabled: true` parses and validates.

---

## Step 2: Add `ralph hooks validate` CLI command (human + JSON output)

**Objective**
Provide a dedicated pre-run validation surface for hook configuration and command wiring.

**Implementation Guidance**
- Add a new CLI namespace in `crates/ralph-cli/src/main.rs` (e.g., `Hooks(HooksArgs)`) with `validate` subcommand.
- Implement `crates/ralph-cli/src/hooks.rs` similar to existing preflight command patterns.
- Validation output modes:
  - human-readable summary
  - JSON report for automation
- Validation should include config semantics and command resolvability checks (without executing hook scripts).

**Test Requirements**
- CLI unit/integration tests for:
  - success case
  - malformed config case
  - unknown phase-event case
  - JSON output shape

**Integration Notes**
- Reuse existing config source loading (`preflight::load_config_for_preflight`) for consistent behavior.
- Keep exit codes deterministic (`0` pass, non-zero fail).

**Demo**
- `ralph hooks validate -c ralph.yml` returns PASS/FAIL with actionable diagnostics.

---

## Step 3: Implement `HookExecutor` (JSON stdin, timeout, output truncation)

**Objective**
Build the core execution primitive for one hook invocation with v1 safety guarantees.

**Subtasks**
- 3a. Implement process spawn + command/env/cwd resolution.
- 3b. Implement JSON payload stdin write path.
- 3c. Implement timeout enforcement and termination handling.
- 3d. Implement stdout/stderr capture with `max_output_bytes` truncation.
- 3e. Finalize `HookRunResult` model and conversion helpers.

**Implementation Guidance**
- Add a new module (e.g., `crates/ralph-core/src/hooks/executor.rs`).
- Execution behavior:
  - spawn command with configured cwd/env
  - write lifecycle payload JSON to stdin
  - enforce timeout
  - capture stdout/stderr
  - truncate each stream to `max_output_bytes`
- Return structured `HookRunResult` with:
  - start/end timestamps
  - duration
  - exit code
  - timed_out flag
  - truncated stdout/stderr

**Test Requirements**
- Unit tests with fixture scripts for:
  - successful run
  - non-zero exit
  - timeout
  - oversized stdout/stderr truncation
  - stdin payload delivered

**Integration Notes**
- Keep executor pure and reusable by CLI validation and runtime dispatch.
- No lifecycle wiring yet.

**Demo**
- Executor tests show expected timeout and truncation behavior deterministically.

---

## Step 4: Add hook telemetry logging and diagnostics integration

**Objective**
Persist required hook observability data for each invocation.

**Implementation Guidance**
- Add hook telemetry log model and writer (new diagnostics file, e.g., `hook-runs.jsonl`, or extend orchestration event with hook entries).
- Required fields:
  - phase-event
  - hook name
  - start/end/duration
  - exit code
  - timeout flag
  - stdout/stderr (truncated)
  - disposition
- Ensure telemetry logging works regardless of pass/fail/suspend outcomes.

**Test Requirements**
- Serialization tests for telemetry entries.
- Diagnostics integration tests verifying file creation and required fields.

**Integration Notes**
- Respect `RALPH_DIAGNOSTICS` behavior and existing diagnostics conventions.

**Demo**
- Run with diagnostics enabled and confirm hook run entries are persisted in structured JSONL.

---

## Step 5: Implement `HookEngine` dispatch for `loop.start` + `iteration.start`

**Objective**
Ship first end-to-end lifecycle hook dispatch path with deterministic sequencing.

**Subtasks**
- 5a. Build hook resolver for phase-event lookup + declaration-order sequencing.
- 5b. Implement payload builder for loop/iteration context.
- 5c. Wire `pre/post.loop.start` dispatch boundaries.
- 5d. Wire `pre/post.iteration.start` dispatch boundaries.
- 5e. Add disabled/empty-config fast-path (no-op dispatch).

**Implementation Guidance**
- Add `HookEngine` orchestrator module to resolve configured hooks by phase-event.
- Wire dispatch into runtime boundaries:
  - `pre.loop.start` / `post.loop.start`
  - `pre.iteration.start` / `post.iteration.start`
- Build payload with context fields (`active_hat`, `selected_hat`, `selected_task`, loop IDs, iteration data).
- Execute hooks sequentially in declaration order.

**Test Requirements**
- Integration tests validating exact dispatch order and event-phase selection.
- Validate no dispatch occurs when hooks disabled.

**Integration Notes**
- Keep behavior additive; no blocking policy yet beyond telemetry collection.

**Demo**
- Configure simple echo hooks for loop/iteration events and observe ordered invocations + telemetry.

---

## Step 6: Implement `on_error` dispositions (`warn`, `block`) end-to-end

**Objective**
Enable policy outcomes from hook failures without suspend complexity yet.

**Implementation Guidance**
- Implement disposition resolver:
  - `warn`: continue
  - `block`: fail lifecycle action with clear user-facing reason
- Add consistent error messages in CLI output and diagnostics.
- Ensure block behavior is deterministic and non-ambiguous per phase.

**Test Requirements**
- Integration tests for warn-vs-block behavior at both loop.start and iteration.start phases.
- Verify blocked runs terminate with expected reason/exit behavior.

**Integration Notes**
- `block` should not leave partial internal state transitions for the current boundary.

**Demo**
- A failing hook with `on_error: warn` continues; with `on_error: block` stops with explicit reason.

---

## Step 7: Implement suspend core (`wait_for_resume`) + `ralph loops resume <id>`

**Objective**
Deliver operator-controlled suspension with durable state and explicit CLI resume.

**Subtasks**
- 7a. Define suspend-state schema and atomic file I/O (`suspend-state`, `resume-requested`).
- 7b. Implement runtime suspended-wait state machine with signal precedence.
- 7c. Add `ralph loops resume <id>` CLI plumbing and loop resolution.
- 7d. Add idempotency and invalid-state behavior for resume operations.
- 7e. Add end-to-end suspend → resume → continue tests.

**Implementation Guidance**
- Add suspension persistence:
  - `.ralph/suspend-state.json`
  - `.ralph/resume-requested`
- On `on_error: suspend` (default mode), enter suspended state and wait for resume/stop/restart signals.
- Add `Resume` subcommand to `crates/ralph-cli/src/loops.rs`:
  - `ralph loops resume <id>`
  - resolve loop via existing `resolve_loop`
  - validate suspended state
  - write resume signal atomically
  - idempotent no-op messaging when already resumed/not suspended

**Test Requirements**
- Unit tests for suspend state transitions and signal precedence.
- CLI tests for `loops resume` success, missing loop, non-suspended loop, idempotent retry.
- Integration test for suspend -> resume -> continue path.

**Integration Notes**
- Preserve precedence: stop/restart should outrank resume while suspended.

**Demo**
- Run with a suspending hook, observe paused state, then `ralph loops resume <id>` unblocks run.

---

## Step 8: Add suspend modes `retry_backoff` and `wait_then_retry`

**Objective**
Complete suspend behavior options required by design with bounded retry semantics.

**Subtasks**
- 8a. Implement bounded `retry_backoff` policy (schedule, cap, termination condition).
- 8b. Implement `wait_then_retry` flow (resume gate + single retry behavior).
- 8c. Add retry attempt metadata/telemetry fields.
- 8d. Build deterministic timing/test harness for retry behavior.

**Implementation Guidance**
- Add bounded backoff policy for `retry_backoff`.
- Add `wait_then_retry` flow:
  - wait for resume
  - retry hook once
  - apply disposition based on retry result
- Include metadata/telemetry to indicate mode and retry attempt counts.

**Test Requirements**
- Integration tests for both modes, including exhausted retries and mixed outcomes.

**Integration Notes**
- Keep backoff bounded and deterministic for test reliability.

**Demo**
- Show a failing hook recovering under `retry_backoff`, and a manual release under `wait_then_retry`.

---

## Step 9: Add remaining lifecycle mappings (`plan.created`, `human.interact`, `loop.complete`, `loop.error`)

**Objective**
Reach full mandatory v1 lifecycle coverage.

**Subtasks**
- 9a. Add `pre/post.plan.created` dispatch wiring.
- 9b. Add `pre/post.human.interact` dispatch wiring.
- 9c. Add `pre/post.loop.complete` dispatch wiring.
- 9d. Add `pre/post.loop.error` dispatch wiring.
- 9e. Add explicit success/error termination mapping tests.

**Implementation Guidance**
- Add dispatch points for:
  - `pre/post.plan.created` (derived from plan-topic publications)
  - `pre/post.human.interact`
  - `pre/post.loop.complete`
  - `pre/post.loop.error`
- Ensure `loop.complete` vs `loop.error` mapping follows termination reason success semantics.

**Test Requirements**
- Integration tests for all added event-phase mappings.
- Explicit tests proving no backpressure event hooks are required/included in v1.

**Integration Notes**
- Keep plan-created derivation explicit and documented to avoid hidden behavior.

**Demo**
- End-to-end run showing hook invocations across all mandatory v1 lifecycle events.

---

## Step 10: Add opt-in metadata mutation (JSON-only, metadata namespace only)

**Objective**
Enable safe, explicit v1 mutation surface without touching prompt/events/config.

**Implementation Guidance**
- Parse hook stdout JSON only when `mutate.enabled: true`.
- Accept only `{"metadata": {...}}` contract.
- Merge into reserved namespace (e.g., `hook_metadata.<hook_name>`).
- Surface parse/schema errors through disposition policy.

**Test Requirements**
- Unit tests for opt-in gating, valid merge, invalid JSON, invalid shape.
- Integration tests proving no mutation occurs when disabled.

**Integration Notes**
- Prevent key collisions by namespacing metadata per hook.

**Demo**
- Hook emits metadata; subsequent iteration payload includes namespaced injected metadata.

---

## Step 11: Integrate hooks validation into preflight + update docs/examples

**Objective**
Make hooks validation part of normal run safety and provide clear user guidance.

**Implementation Guidance**
- Add hooks check to `PreflightRunner::default_checks()`.
- Respect preflight skip/strict behavior.
- Update docs for:
  - hooks config schema
  - lifecycle phases
  - `ralph hooks validate`
  - `ralph loops resume <id>`
- Add one minimal sample config + script examples.

**Test Requirements**
- Preflight tests verifying hooks check pass/fail and skip behavior.
- CLI docs/command help snapshot tests where applicable.

**Integration Notes**
- Keep docs aligned with v1 non-goals to avoid over-promising.

**Demo**
- `ralph preflight` fails on broken hooks config and passes after fixes.

---

## Step 12: Add mutation testing gates for hooks-critical modules

**Objective**
Increase defect resistance by enforcing mutation quality on the new hooks runtime surfaces.

**Subtasks**
- 12a. Select mutation tooling + baseline command for this repo.
- 12b. Scope mutation targets to hooks-critical modules only.
- 12c. Run baseline mutation report and calibrate threshold.
- 12d. Enforce critical-path invariants (no survivors in disposition/suspend logic).
- 12e. Wire required CI gate + artifact/report output.

**Implementation Guidance**
- Add mutation test configuration targeting new hook modules (executor, disposition resolver, suspend controller, hooks validation logic).
- Start with a practical threshold (e.g., >=70%) for scoped hook modules.
- Add stricter invariants for critical control logic:
  - no surviving mutants in `on_error` disposition mapping,
  - no surviving mutants in suspend/resume transition logic.
- Wire mutation gate into CI as a required quality check for this feature area.

**Test Requirements**
- Mutation run executes deterministically in CI-safe mode.
- Threshold failure produces actionable reports (surviving mutant locations).
- Critical-path no-survivor checks are enforced.

**Integration Notes**
- Keep scope limited to hooks feature modules in v1 to control runtime cost.
- Avoid gating unrelated legacy modules in this rollout.

**Demo**
- Mutation test command passes with threshold met and zero surviving mutants in critical paths.

---

## Step 13: Drive BDD suite to green + finalize traceability matrix + CI gate

**Objective**
Close the loop on BDD-first development by turning red baseline scenarios green and making them an enforced delivery gate.

**Subtasks**
- 13a. Drive AC scenarios `AC-01..AC-06` to green.
- 13b. Drive AC scenarios `AC-07..AC-12` to green.
- 13c. Drive AC scenarios `AC-13..AC-18` to green.
- 13d. Finalize AC-to-scenario traceability matrix.
- 13e. Harden CI execution stability and reporting for BDD runs.

**Implementation Guidance**
- Update/complete step definitions as implementation lands.
- Ensure each AC ID (`AC-01`..`AC-18`) has at least one passing scenario.
- Finalize and publish `AC -> feature scenario` traceability matrix.
- Ensure CI runs and gates on the full BDD acceptance suite.

**Test Requirements**
- BDD suite passes end-to-end for all AC IDs.
- Failing scenario output remains actionable and AC-labeled.

**Integration Notes**
- Reuse existing e2e runner and reporting patterns.
- Keep scenarios stable in mock mode where external dependencies are not required.

**Demo**
- Run BDD suite and show all AC-linked scenarios passing with traceability report.

---

## Delivery Gate

Before marking implementation complete:

- Run full test gates (`cargo test` + targeted smoke/e2e paths for hooks).
- Verify Cucumber acceptance suite passes and traceability matrix is complete (`AC-01`..`AC-18`).
- Verify mutation testing gates pass for hooks-critical modules (threshold met, no surviving mutants in disposition/suspend critical paths).
- Verify **no regressions in core TUI happy path**, including existing `ralph-tui` integration snapshot coverage and `ralph-cli` TUI loop-runner happy-path tests.
- Confirm docs and CLI help are updated and consistent with v1 scope.
