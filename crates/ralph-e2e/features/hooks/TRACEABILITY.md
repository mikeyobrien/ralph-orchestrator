# Hooks BDD AC Traceability Matrix (Step 13 Final)

This document is the finalized Step 13 traceability artifact for:

- `specs/add-hooks-to-ralph-orchestrator-lifecycle/plan.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md`

It maps every acceptance criterion (`AC-01..AC-18`) to:

1. A stable, AC-labeled BDD scenario in `crates/ralph-e2e/features/hooks/*.feature`
2. A deterministic evaluator in `crates/ralph-e2e/src/hooks_bdd.rs`
3. Runtime integration backpressure (mapped `cargo test` checks executed by the BDD harness)
4. Green CI-safe execution (`--hooks-bdd --mock`)

## AC Mapping Matrix

| AC ID | Acceptance intent | Feature scenario (stable title) | Deterministic evaluator | CI-safe status |
|---|---|---|---|---|
| AC-01 | Per-project scope only | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-01 Per-project scope only` | `evaluate_ac_01` | pass |
| AC-02 | Mandatory lifecycle events supported | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-02 Mandatory lifecycle events supported` | `evaluate_ac_02` | pass |
| AC-03 | Pre/post phase support | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-03 Pre/post phase support` | `evaluate_ac_03` | pass |
| AC-04 | Deterministic ordering | `crates/ralph-e2e/features/hooks/scope-and-dispatch.feature` → `Scenario: AC-04 Deterministic ordering` | `evaluate_ac_04` | pass |
| AC-05 | JSON stdin contract | `crates/ralph-e2e/features/hooks/executor-safeguards.feature` → `Scenario: AC-05 JSON stdin contract` | `evaluate_ac_05` | pass |
| AC-06 | Timeout safeguard | `crates/ralph-e2e/features/hooks/executor-safeguards.feature` → `Scenario: AC-06 Timeout safeguard` | `evaluate_ac_06` | pass |
| AC-07 | Output-size safeguard | `crates/ralph-e2e/features/hooks/executor-safeguards.feature` → `Scenario: AC-07 Output-size safeguard` | `evaluate_ac_07` | pass |
| AC-08 | Per-hook warn policy | `crates/ralph-e2e/features/hooks/error-dispositions.feature` → `Scenario: AC-08 Per-hook warn policy` | `evaluate_ac_08` | pass |
| AC-09 | Per-hook block policy | `crates/ralph-e2e/features/hooks/error-dispositions.feature` → `Scenario: AC-09 Per-hook block policy` | `evaluate_ac_09` | pass |
| AC-10 | Suspend default mode | `crates/ralph-e2e/features/hooks/suspend-resume.feature` → `Scenario: AC-10 Suspend default mode` | `evaluate_ac_10` | pass |
| AC-11 | CLI resume path | `crates/ralph-e2e/features/hooks/suspend-resume.feature` → `Scenario: AC-11 CLI resume path` | `evaluate_ac_11` | pass |
| AC-12 | Resume idempotency | `crates/ralph-e2e/features/hooks/suspend-resume.feature` → `Scenario: AC-12 Resume idempotency` | `evaluate_ac_12` | pass |
| AC-13 | Mutation opt-in only | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-13 Mutation opt-in only` | `evaluate_ac_13` | pass |
| AC-14 | Metadata-only mutation surface | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-14 Metadata-only mutation surface` | `evaluate_ac_14` | pass |
| AC-15 | JSON-only mutation format | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-15 JSON-only mutation format` | `evaluate_ac_15` | pass |
| AC-16 | Hook telemetry completeness | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-16 Hook telemetry completeness` | `evaluate_ac_16` | pass |
| AC-17 | Validation command | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-17 Validation command` | `evaluate_ac_17` | pass |
| AC-18 | Preflight integration | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-18 Preflight integration` | `evaluate_ac_18` | pass |

## Runtime Integration Backpressure Mapping

Each AC evaluator executes one or more runtime tests before AC-specific assertions.
These checks run from the workspace root and produce command artifacts under
`.ralph/hooks-bdd-artifacts/<ac-*/>/`.

| AC ID | Runtime checks executed by hooks BDD harness |
|---|---|
| AC-01 | `cargo test -p ralph-core test_hooks_config_boundary_accepts_valid_file`, `cargo test -p ralph-core test_hooks_config_boundary_rejects_non_v1_scope_field` |
| AC-02 | `cargo test -p ralph-cli test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order` |
| AC-03 | `cargo test -p ralph-cli test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order` |
| AC-04 | `cargo test -p ralph-core resolve_phase_event_preserves_declaration_order`, `cargo test -p ralph-cli test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order` |
| AC-05 | `cargo test -p ralph-core run_writes_json_payload_to_hook_stdin` |
| AC-06 | `cargo test -p ralph-core run_marks_timed_out_when_command_exceeds_timeout` |
| AC-07 | `cargo test -p ralph-core run_truncates_stdout_and_stderr_at_max_output_bytes` |
| AC-08 | `cargo test -p ralph-cli test_loop_start_dispatch_warn_continues_and_block_aborts` |
| AC-09 | `cargo test -p ralph-cli test_loop_start_dispatch_warn_continues_and_block_aborts` |
| AC-10 | `cargo test -p ralph-cli test_iteration_start_suspend_waits_for_resume_and_clears_artifacts_before_continuing` |
| AC-11 | `cargo test -p ralph-cli test_wait_for_resume_if_suspended_resumes_and_clears_suspend_artifacts` |
| AC-12 | `cargo test -p ralph-cli test_wait_for_resume_if_suspended_is_noop_without_suspend_dispositions` |
| AC-13 | `cargo test -p ralph-cli test_ac13_mutation_disabled_json_output_is_inert_for_accumulator_and_downstream_payloads` |
| AC-14 | `cargo test -p ralph-cli test_ac14_mutation_enabled_updates_only_namespaced_metadata_in_downstream_payloads`, `cargo test -p ralph-cli test_parse_hook_mutation_stdout_accepts_metadata_only_payload_and_namespaces_by_hook` |
| AC-15 | `cargo test -p ralph-cli test_ac15_dispatch_phase_event_hooks_non_json_mutation_warn_continues_through_block_gate`, `cargo test -p ralph-cli test_ac15_dispatch_phase_event_hooks_non_json_mutation_block_surfaces_invalid_output_reason`, `cargo test -p ralph-cli test_ac15_dispatch_phase_event_hooks_non_json_mutation_suspend_uses_wait_for_resume_gate` |
| AC-16 | `cargo test -p ralph-cli test_dispatch_phase_event_hooks_retry_backoff_recovers_before_exhaustion`, `cargo test -p ralph-core test_diagnostics_collector_logs_hook_run_telemetry` |
| AC-17 | `cargo test -p ralph-cli test_hooks_validate_json_success_report_and_exit_code` |
| AC-18 | `cargo test -p ralph-cli test_preflight_check_config_json`, `cargo test -p ralph-core default_checks_include_hooks_check_name` |

## CI-safe Acceptance Evidence (Current Green Baseline)

Full suite:

- Command: `cargo run -p ralph-e2e -- --hooks-bdd --mock --quiet`
- Deterministic summary: `Summary: 18 passed, 0 failed, 18 total`
- Exit: `0`

Focused reproducibility check:

- Command: `cargo run -p ralph-e2e -- --hooks-bdd --mock --filter AC-18`
- Deterministic summary: `Summary: 1 passed, 0 failed, 1 total`
- Exit: `0`

## Notes

- Scenario discovery uses `cucumber-rs` (`cucumber::gherkin`) parsing in `hooks_bdd.rs`.
- This matrix supersedes the initial Step 0 skeleton and red placeholder baseline.
- CI and delivery-gate review should treat this file as the single traceability reference for hooks AC coverage.
