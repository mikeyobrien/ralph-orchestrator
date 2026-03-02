# Summary: Per-Project Orchestrator Lifecycle Hooks (v1)

## Overview

This planning package defines how to add **per-project lifecycle hooks** to Ralph in a safe, observable, operator-controllable way, without implementing code yet.

The proposed v1 is intentionally scoped and foundational:
- per-project hooks only (no global hooks in v1),
- external command/script execution model,
- explicit `pre.<event>` / `post.<event>` phases,
- deterministic sequential execution,
- configurable per-hook failure behavior (`warn`, `block`, `suspend`),
- suspend + operator resume via `ralph loops resume <id>`,
- strong telemetry and validation surfaces.

## Artifacts Created

### Core spec documents
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/rough-idea.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/requirements.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/plan.md`

### Research documents
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/research/00-research-plan.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/research/01-lifecycle-map.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/research/02-config-and-execution-model.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/research/03-operator-resume-surface.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/research/04-external-patterns.md`
- `specs/add-hooks-to-ralph-orchestrator-lifecycle/research/05-iteration-checkpoint.md`

## Design Highlights

- **Lifecycle coverage (v1 required):**
  `loop.start`, `iteration.start`, `plan.created`, `human.interact`, `loop.complete`, `loop.error` with explicit pre/post phases.
- **Context model update:** no dedicated `task.selected`/`hat.selected` in v1; selection context travels in `iteration.start` payload.
- **Execution contract:** JSON stdin payload + minimal env vars + timeout/output safeguards.
- **Observability:** per-hook telemetry with timestamps, duration, exit code, timeout, truncated stdout/stderr, and final disposition.
- **Control model:** `warn` / `block` / `suspend` with suspend modes:
  `wait_for_resume` (default), `retry_backoff`, `wait_then_retry`.
- **Mutation scope (v1):** opt-in only, JSON-only, metadata-only namespace.
- **Validation UX:** `ralph hooks validate` plus preflight integration.
- **Acceptance discipline:** requirements mapped to BDD acceptance criteria `AC-01..AC-18`.

## Implementation Plan Highlights

The plan is now structured as incremental steps with demos and test gates, including:
- **BDD-first sequencing:** Step 0 establishes AC-labeled failing scenarios before implementation,
- progressive runtime build-out (config, executor, engine dispatch, error policies, suspend/resume),
- full lifecycle mapping,
- metadata mutation,
- preflight/docs integration,
- **mutation testing gates** for hooks-critical modules,
- final BDD green pass + CI traceability gate,
- delivery gate that explicitly includes **core TUI happy-path non-regression**.

The plan also includes explicit subtasks for larger steps (notably suspend/resume, lifecycle mappings, mutation gates, and BDD-greening).

## Suggested Next Steps

1. Approve the latest plan revision.
2. Start execution at **Step 0** (BDD red baseline).
3. Implement step-by-step, keeping all tests/diagnostics gates green.
4. Keep AC traceability and mutation reports updated during implementation.
5. Optionally create a `PROMPT.md` tailored for handing this plan to a Ralph loop.

## Connections

- [[rough-idea.md]]
- [[requirements.md]]
- [[design.md]]
- [[plan.md]]
- [[research/00-research-plan.md]]
- [[research/01-lifecycle-map.md]]
- [[research/02-config-and-execution-model.md]]
- [[research/03-operator-resume-surface.md]]
- [[research/04-external-patterns.md]]
- [[research/05-iteration-checkpoint.md]]
