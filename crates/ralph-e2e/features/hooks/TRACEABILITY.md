# Hooks BDD AC Traceability Skeleton

Initial `AC-01..AC-18` mapping skeleton for the hooks lifecycle feature.

Sources:
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/plan.md` (Step 0 / subtask 0a)
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md` (Acceptance Criteria + Cucumber mapping requirements)

## Mapping Table (Skeleton)

| AC ID | Acceptance intent | Planned feature file | Planned scenario title (stable AC label) | Status |
|---|---|---|---|---|
| AC-01 | Per-project scope only | `hooks/scope-and-dispatch.feature` | `Scenario: AC-01 Per-project scope only` | planned |
| AC-02 | Mandatory lifecycle events supported | `hooks/scope-and-dispatch.feature` | `Scenario: AC-02 Mandatory lifecycle events supported` | planned |
| AC-03 | Pre/post phase support | `hooks/scope-and-dispatch.feature` | `Scenario: AC-03 Pre/post phase support` | planned |
| AC-04 | Deterministic ordering | `hooks/scope-and-dispatch.feature` | `Scenario: AC-04 Deterministic ordering` | planned |
| AC-05 | JSON stdin contract | `hooks/executor-safeguards.feature` | `Scenario: AC-05 JSON stdin contract` | planned |
| AC-06 | Timeout safeguard | `hooks/executor-safeguards.feature` | `Scenario: AC-06 Timeout safeguard` | planned |
| AC-07 | Output-size safeguard | `hooks/executor-safeguards.feature` | `Scenario: AC-07 Output-size safeguard` | planned |
| AC-08 | Per-hook warn policy | `hooks/error-dispositions.feature` | `Scenario: AC-08 Per-hook warn policy` | planned |
| AC-09 | Per-hook block policy | `hooks/error-dispositions.feature` | `Scenario: AC-09 Per-hook block policy` | planned |
| AC-10 | Suspend default mode | `hooks/error-dispositions.feature` | `Scenario: AC-10 Suspend default mode` | planned |
| AC-11 | CLI resume path | `hooks/error-dispositions.feature` | `Scenario: AC-11 CLI resume path` | planned |
| AC-12 | Resume idempotency | `hooks/error-dispositions.feature` | `Scenario: AC-12 Resume idempotency` | planned |
| AC-13 | Mutation opt-in only | `hooks/metadata-mutation.feature` | `Scenario: AC-13 Mutation opt-in only` | planned |
| AC-14 | Metadata-only mutation surface | `hooks/metadata-mutation.feature` | `Scenario: AC-14 Metadata-only mutation surface` | planned |
| AC-15 | JSON-only mutation format | `hooks/metadata-mutation.feature` | `Scenario: AC-15 JSON-only mutation format` | planned |
| AC-16 | Hook telemetry completeness | `hooks/telemetry-and-validation.feature` | `Scenario: AC-16 Hook telemetry completeness` | planned |
| AC-17 | Validation command | `hooks/telemetry-and-validation.feature` | `Scenario: AC-17 Validation command` | planned |
| AC-18 | Preflight integration | `hooks/telemetry-and-validation.feature` | `Scenario: AC-18 Preflight integration` | planned |

## Notes

- Step 0.1 creates only the mapping skeleton; Step 0.2 will add `.feature` files + scenario placeholders.
- Scenario titles intentionally embed stable AC labels for CI traceability.

## Step 0.4 Red Baseline (CI-safe)

- Command:
  - `cargo run -p ralph-e2e -- --hooks-bdd --mock`
- Exit status:
  - `1` (expected red baseline while hooks implementation is pending)
- Deterministic summary:
  - `Summary: 0 passed, 18 failed, 18 total`
- Reproducibility check:
  - `cargo run -p ralph-e2e -- --hooks-bdd --mock --quiet`
  - `Summary: 0 passed, 18 failed, 18 total`
- Failure shape:
  - All AC-labeled placeholders (`AC-01..AC-18`) fail with `pending: <AC-ID> placeholder scenario intentionally red until implementation`.
