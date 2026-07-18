# Hooks BDD AC Traceability Matrix (Step 13 Final)

This document is the finalized Step 13 traceability artifact for:

- `specs/add-hooks-to-ralph-orchestrator-lifecycle/plan.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md`

It maps every acceptance criterion (`AC-01..AC-18`) to:

1. A stable, AC-labeled BDD scenario in `crates/ralph-e2e/features/hooks/*.feature`
2. A runtime-certified or explicitly engine-descoped evaluator in `crates/ralph-e2e/src/hooks_bdd.rs`
3. Runtime integration backpressure for Ralph-owned ACs (mapped `cargo test` checks executed by the BDD harness)
4. CI-safe execution with descoped ACs reported separately (`--hooks-bdd --mock`)

## AC Mapping Matrix

| AC ID | Acceptance intent | Feature scenario (stable title) | Deterministic evaluator | CI-safe status |
|---|---|---|---|---|
| AC-01 | Per-project scope only | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-01 Per-project scope only` | `evaluate_runtime_certified` | pass |
| AC-02 | Mandatory lifecycle events supported | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-02 Mandatory lifecycle events supported` | `evaluate_runtime_certified` | pass |
| AC-03 | Pre/post phase support | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-03 Pre/post phase support` | `evaluate_runtime_certified` | pass |
| AC-04 | Deterministic ordering | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-04 Deterministic ordering` | `evaluate_runtime_certified` | pass |
| AC-05 | JSON stdin contract | `crates/ralph-e2e/features/hooks/executor-safeguards.feature` → `Scenario: AC-05 JSON stdin contract` | `evaluate_runtime_certified` | pass |
| AC-06 | Timeout safeguard | `crates/ralph-e2e/features/hooks/executor-safeguards.feature` → `Scenario: AC-06 Timeout safeguard` | `evaluate_runtime_certified` | pass |
| AC-07 | Output-size safeguard | `crates/ralph-e2e/features/hooks/executor-safeguards.feature` → `Scenario: AC-07 Output-size safeguard` | `evaluate_runtime_certified` | pass |
| AC-08 | Per-hook warn policy | `crates/ralph-e2e/features/hooks/error-dispositions.feature` → `Scenario: AC-08 Per-hook warn policy` | `evaluate_descoped_to_engine` | descoped (engine-owned, autoloop#38) |
| AC-09 | Per-hook block policy | `crates/ralph-e2e/features/hooks/error-dispositions.feature` → `Scenario: AC-09 Per-hook block policy` | `evaluate_descoped_to_engine` | descoped (engine-owned, autoloop#38) |
| AC-10 | Suspend default mode | `crates/ralph-e2e/features/hooks/suspend-resume.feature` → `Scenario: AC-10 Suspend default mode` | `evaluate_runtime_certified` | pass |
| AC-11 | CLI resume path | `crates/ralph-e2e/features/hooks/suspend-resume.feature` → `Scenario: AC-11 CLI resume path` | `evaluate_runtime_certified` | pass |
| AC-12 | Resume idempotency | `crates/ralph-e2e/features/hooks/suspend-resume.feature` → `Scenario: AC-12 Resume idempotency` | `evaluate_runtime_certified` | pass |
| AC-13 | Mutation opt-in only | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-13 Mutation opt-in only` | `evaluate_descoped_to_engine` | descoped (engine-owned, autoloop#38) |
| AC-14 | Metadata-only mutation surface | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-14 Metadata-only mutation surface` | `evaluate_descoped_to_engine` | descoped (engine-owned, autoloop#38) |
| AC-15 | JSON-only mutation format | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-15 JSON-only mutation format` | `evaluate_descoped_to_engine` | descoped (engine-owned, autoloop#38) |
| AC-16 | Hook telemetry completeness | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-16 Hook telemetry completeness` | `evaluate_descoped_to_engine` | descoped (engine-owned, autoloop#38) |
| AC-17 | Validation command | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-17 Validation command` | `evaluate_runtime_certified` | pass |
| AC-18 | Preflight integration | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-18 Preflight integration` | `evaluate_runtime_certified` | pass |

## Runtime Integration Backpressure Mapping

Each AC evaluator executes one or more runtime tests before AC-specific assertions.
These checks run from the workspace root and produce command artifacts under
`.ralph/hooks-bdd-artifacts/<ac-*/>/`.

| AC ID | Runtime checks executed by hooks BDD harness |
|---|---|
| AC-01 | `cargo test -p ralph-core test_hooks_config_boundary_accepts_valid_file`, `cargo test -p ralph-core test_hooks_config_boundary_rejects_non_v1_scope_field` |
| AC-02 | `cargo test -p ralph-core test_hooks_config_valid_yaml_parses_and_validates` |
| AC-03 | `cargo test -p ralph-core build_payload_maps_loop_iteration_and_context_fields` |
| AC-04 | `cargo test -p ralph-core resolve_phase_event_preserves_declaration_order` |
| AC-05 | `cargo test -p ralph-core run_writes_json_payload_to_hook_stdin` |
| AC-06 | `cargo test -p ralph-core run_marks_timed_out_when_command_exceeds_timeout` |
| AC-07 | `cargo test -p ralph-core run_truncates_stdout_and_stderr_at_max_output_bytes` |
| AC-08 | descoped to autoloop engine (engine-owned hooks parity territory, autoloop#38) |
| AC-09 | descoped to autoloop engine (engine-owned hooks parity territory, autoloop#38) |
| AC-10 | `cargo test -p ralph-core test_suspend_state_record_serializes_v1_schema_shape` |
| AC-11 | `cargo test -p ralph-cli test_resume_loop_writes_resume_signal_for_in_place_loop` |
| AC-12 | `cargo test -p ralph-core test_resume_signal_is_single_use`, `cargo test -p ralph-cli test_resume_loop_is_idempotent_when_resume_already_requested` |
| AC-13 | descoped to autoloop engine (engine-owned hooks parity territory, autoloop#38) |
| AC-14 | descoped to autoloop engine (engine-owned hooks parity territory, autoloop#38) |
| AC-15 | descoped to autoloop engine (engine-owned hooks parity territory, autoloop#38) |
| AC-16 | descoped to autoloop engine (engine-owned hooks parity territory, autoloop#38); Ralph's diagnostics collector test remains regression coverage but cannot certify live dispatch |
| AC-17 | `cargo test -p ralph-cli test_hooks_validate_json_success_report_and_exit_code` |
| AC-18 | `cargo test -p ralph-cli test_preflight_check_config_json`, `cargo test -p ralph-core default_checks_include_hooks_check_name` |

## CI-safe Acceptance Evidence (Current Green Baseline)

Full suite:

- Command: `cargo run -p ralph-e2e -- --hooks-bdd --mock --quiet`
- Deterministic summary: `Summary: 12 passed, 0 failed, 6 descoped (engine-owned), 18 total`
- Exit: `0`

Focused reproducibility check:

- Command: `cargo run -p ralph-e2e -- --hooks-bdd --mock --filter AC-18`
- Deterministic summary: `Summary: 1 passed, 0 failed, 0 descoped (engine-owned), 1 total`
- Exit: `0`

## Notes

- Scenario discovery uses `cucumber-rs` (`cucumber::gherkin`) parsing in `hooks_bdd.rs`.
- This matrix supersedes the initial Step 0 skeleton and red placeholder baseline.
- CI and delivery-gate review should treat this file as the single traceability reference for hooks AC coverage.
- AC-08/09/13/14/15/16 remain discoverable but are not green-certified by Ralph; hooks execute in the autoloop engine in v3 and are tracked in autoloop#38 territory.
