# Changelog: Per-Project Lifecycle Hooks

**Feature Release Date:** 2026-03-01

## Summary

This release implements Ralph v1 per-project lifecycle hooks, enabling operators to define external commands/scripts that execute at specific points in the orchestration loop lifecycle.

## 2026-03-01 — Hooks BDD Finalization (AC-01..AC-18)

### Added

- Executable AC evaluators in `crates/ralph-e2e/src/hooks_bdd.rs` now assert concrete source evidence for AC-01 through AC-18 instead of returning stubbed `Ok(())`.
- Explicit AC-10..AC-12 suspend/resume coverage in `crates/ralph-e2e/features/hooks/suspend-resume.feature` and evaluator dispatch.
- Failure-path tests that verify actionable missing-evidence diagnostics (`assert_workspace_source_contains_reports_missing_snippets`, `evaluate_green_acceptance_reports_actionable_missing_evidence_failures`).

### Changed

- Updated hooks feature scenarios to spec-aligned Given/When/Then text with stable `@AC-01`..`@AC-18` traceability tags across:
  - `scope-and-dispatch.feature`
  - `executor-safeguards.feature`
  - `error-dispositions.feature`
  - `suspend-resume.feature`
  - `metadata-mutation.feature`
  - `telemetry-and-validation.feature`
- Replaced evaluator placeholders with primary-source checks covering dispatch, safeguards, dispositions, suspend/resume, mutation, telemetry, validation command wiring, and preflight integration.

### Crates Affected

- **ralph-e2e** — hooks feature files and `hooks_bdd.rs` acceptance evaluator/test harness.

## Added

### Core Hooks System

- **Per-project hooks configuration** in YAML (`ralph.hooks.yml`)
- **Explicit phase events**: `pre.plan_created`, `post.plan_created`, `pre.iteration`, `post.iteration`, `pre.cycle_complete`, `post.cycle_complete`, `pre.loop_complete`, `post.loop_complete`, `human.interact`
- **Pre/post phase support** for all lifecycle events
- **Sequential deterministic execution** in declaration order
- **External command/script execution** with JSON stdin + env vars

### Error Dispositions

- **`on_error: warn`** - Log hook failure, continue execution
- **`on_error: block`** - Fail loop on hook failure
- **`on_error: suspend`** - Suspend loop, wait for operator resume

### Suspend Modes

- **`wait_for_resume`** (default) - Loop suspends indefinitely until `ralph loops resume <id>`
- **`retry_backoff`** - Retry with exponential backoff (bounded)
- **`wait_then_retry`** - Wait configured duration, then retry once

### Safeguards

- **`timeout_seconds`** - Maximum execution time per hook
- **`max_output_bytes`** - Truncate stdout/stderr at configured limit

### Telemetry

- **HookRunTelemetryEntry** with: timestamp, loop_id, phase_event, hook_name, started_at, ended_at, duration_ms, exit_code, timed_out, stdout, stderr, disposition, suspend_mode, retry_attempt, retry_max_attempts

### Operator Controls

- **`ralph loops resume <id>`** - Resume suspended loop
- **`ralph hooks validate -c <path>`** - Validate hooks configuration
- **Preflight integration** - HooksValidationCheck in default preflight suite

### Mutation & Opt-in Features

- **Mutation namespace** (`mutations.<hook_name>`) - Modify event payload before processing
- **JSON-only format** - Strict JSON stdin contract
- **Metadata-only scope** - Access only to metadata, not full payload

### BDD Acceptance Suite

- 18 acceptance criteria (AC-01 through AC-18) covering:
  - Project scope isolation
  - Mandatory lifecycle events
  - Pre/post phase support
  - Deterministic ordering
  - JSON stdin contract
  - Timeout safeguard
  - Output truncation safeguard
  - Warn/block/suspend error policies
  - Suspend mode behaviors
  - Resume idempotency
  - Telemetry completeness
  - Validation command
  - Preflight integration
  - Mutation opt-in
  - JSON mutation format

## Changed

- **Loop lifecycle** - Extended with pre/post phase hooks for each event
- **Event payloads** - Extended to include hook metadata context
- **Configuration schema** - Added hooks section to ralph-core config

## Crates Affected

- **ralph-cli** - Added hooks subcommand, lifecycle hook wiring, resume command
- **ralph-core** - Added hooks module (engine, executor, suspend_state), config extensions, preflight integration, diagnostics
- **ralph-e2e** - Added hooks BDD test suite with 18 acceptance criteria

## Quality Gates

- ✅ All 18 AC evaluators green
- ✅ Traceability matrix complete
- ✅ CI hooks-bdd gate enforced
- ✅ Full repository test suite passes

## Suggested AGENTS.md Updates

1. **Source-evidence evaluator pattern** — For BDD acceptance checks that verify implementation by source inspection, require file:line mapping plus actionable missing-snippet errors (path + description + exact missing literal).
2. **NixOS `/bin/bash` compatibility gate** — Document that full-suite runs can fail locally when tests execute shebang scripts with `#!/bin/bash`; use a bound shell path (for example `proot -b /run/current-system/sw/bin/bash:/bin/bash`) when validating `cargo test --all` on NixOS.
3. **Cargo test filter usage** — Note that `cargo test` accepts one positional test filter; avoid passing multiple positional filters when running targeted BDD discovery tests.

---

*Generated 2026-03-01 for Ralph v1 per-project lifecycle hooks feature*
