# Hooks BDD AC Traceability Matrix (Step 13 Final)

This document is the finalized Step 13 traceability artifact for:

- `specs/add-hooks-to-ralph-orchestrator-lifecycle/plan.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md`

It maps every acceptance criterion (`AC-01..AC-18`) to:

1. A stable, AC-labeled BDD scenario in `crates/ralph-e2e/features/hooks/*.feature`
2. A deterministic evaluator in `crates/ralph-e2e/src/hooks_bdd.rs`
3. Green CI-safe execution (`--hooks-bdd --mock`)

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
| AC-10 | Suspend default mode | `crates/ralph-e2e/features/hooks/error-dispositions.feature` → `Scenario: AC-10 Suspend default mode` | `evaluate_ac_10` | pass |
| AC-11 | CLI resume path | `crates/ralph-e2e/features/hooks/error-dispositions.feature` → `Scenario: AC-11 CLI resume path` | `evaluate_ac_11` | pass |
| AC-12 | Resume idempotency | `crates/ralph-e2e/features/hooks/error-dispositions.feature` → `Scenario: AC-12 Resume idempotency` | `evaluate_ac_12` | pass |
| AC-13 | Mutation opt-in only | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-13 Mutation opt-in only` | `evaluate_ac_13` | pass |
| AC-14 | Metadata-only mutation surface | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-14 Metadata-only mutation surface` | `evaluate_ac_14` | pass |
| AC-15 | JSON-only mutation format | `crates/ralph-e2e/features/hooks/metadata-mutation.feature` → `Scenario: AC-15 JSON-only mutation format` | `evaluate_ac_15` | pass |
| AC-16 | Hook telemetry completeness | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-16 Hook telemetry completeness` | `evaluate_ac_16` | pass |
| AC-17 | Validation command | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-17 Validation command` | `evaluate_ac_17` | pass |
| AC-18 | Preflight integration | `crates/ralph-e2e/features/hooks/telemetry-and-validation.feature` → `Scenario: AC-18 Preflight integration` | `evaluate_ac_18` | pass |

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

- This matrix supersedes the initial Step 0 skeleton and red placeholder baseline.
- CI and delivery-gate review should treat this file as the single traceability reference for hooks AC coverage.
