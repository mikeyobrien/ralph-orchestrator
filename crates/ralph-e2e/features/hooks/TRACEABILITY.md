# Hooks BDD AC Traceability Matrix

Complete `AC-01..AC-18` mapping with implementation status for the hooks lifecycle feature.

Sources:
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/plan.md` (Step 0 / subtask 0a)
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md` (Acceptance Criteria + Cucumber mapping requirements)

## Mapping Table

| AC ID | Acceptance intent | Feature file | Scenario title | Status | Evidence |
|---|---|---|---|---|---|
| AC-01 | Per-project scope only | `hooks/scope-and-dispatch.feature` | `Scenario: AC-01 Per-project scope only` | ✅ green | `evaluate_ac_01` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_01_in_ci_safe_mode` |
| AC-02 | Mandatory lifecycle events supported | `hooks/scope-and-dispatch.feature` | `Scenario: AC-02 Mandatory lifecycle events supported` | ✅ green | `evaluate_ac_02` in `hooks_bdd.rs` + unit test |
| AC-03 | Pre/post phase support | `hooks/scope-and-dispatch.feature` | `Scenario: AC-03 Pre/post phase support` | ✅ green | `evaluate_ac_03` in `hooks_bdd.rs` + unit test |
| AC-04 | Deterministic ordering | `hooks/scope-and-dispatch.feature` | `Scenario: AC-04 Deterministic ordering` | ✅ green | `evaluate_ac_04` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_04_deterministic_ordering` |
| AC-05 | JSON stdin contract | `hooks/executor-safeguards.feature` | `Scenario: AC-05 JSON stdin contract` | ✅ green | `evaluate_ac_05` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_05_json_stdin_contract` |
| AC-06 | Timeout safeguard | `hooks/executor-safeguards.feature` | `Scenario: AC-06 Timeout safeguard` | ✅ green | `evaluate_ac_06` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_06_timeout_safeguard` |
| AC-07 | Output-size safeguard | `hooks/executor-safeguards.feature` | `Scenario: AC-07 Output-size safeguard` | ✅ green | `evaluate_ac_07` in `hooks_bdd.rs` |
| AC-08 | Per-hook warn policy | `hooks/error-dispositions.feature` | `Scenario: AC-08 Per-hook warn policy` | ✅ green | `evaluate_ac_08` in `hooks_bdd.rs` |
| AC-09 | Per-hook block policy | `hooks/error-dispositions.feature` | `Scenario: AC-09 Per-hook block policy` | ✅ green | `evaluate_ac_09` in `hooks_bdd.rs` |
| AC-10 | Suspend default mode | `hooks/suspend-resume.feature` | `Scenario: AC-10 Suspend default mode` | ✅ green | `evaluate_ac_10` in `hooks_bdd.rs` + unit test |
| AC-11 | CLI resume path | `hooks/suspend-resume.feature` | `Scenario: AC-11 CLI resume path` | ✅ green | `evaluate_ac_11` in `hooks_bdd.rs` + unit test |
| AC-12 | Resume idempotency | `hooks/suspend-resume.feature` | `Scenario: AC-12 Resume idempotency` | ✅ green | `evaluate_ac_12` in `hooks_bdd.rs` + unit test |
| AC-13 | Mutation opt-in only | `hooks/metadata-mutation.feature` | `Scenario: AC-13 Mutation opt-in only` | ✅ green | `evaluate_ac_13` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_13_mutation_opt_in` |
| AC-14 | Metadata-only mutation surface | `hooks/metadata-mutation.feature` | `Scenario: AC-14 Metadata-only mutation surface` | ✅ green | `evaluate_ac_14` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_14_metadata_mutation` |
| AC-15 | JSON-only mutation format | `hooks/metadata-mutation.feature` | `Scenario: AC-15 JSON-only mutation format` | ✅ green | `evaluate_ac_15` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_15_json_mutation_format` |
| AC-16 | Hook telemetry completeness | `hooks/telemetry-and-validation.feature` | `Scenario: AC-16 Hook telemetry completeness` | ✅ green | `evaluate_ac_16` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_16_telemetry_completeness` |
| AC-17 | Validation command | `hooks/telemetry-and-validation.feature` | `Scenario: AC-17 Validation command` | ✅ green | `evaluate_ac_17` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_17_validation_command` |
| AC-18 | Preflight integration | `hooks/telemetry-and-validation.feature` | `Scenario: AC-18 Preflight integration` | ✅ green | `evaluate_ac_18` in `hooks_bdd.rs` + test `run_hooks_bdd_suite_passes_ac_18_preflight_integration` |

## Test Execution

Run all hooks BDD tests:
```bash
cargo test -p ralph-e2e hooks_bdd
```

Run specific AC test:
```bash
cargo test -p ralph-e2e hooks_bdd -- AC-16
```

CI-safe mode (mocked):
```bash
cargo run -p ralph-e2e -- --hooks-bdd --mock
```
