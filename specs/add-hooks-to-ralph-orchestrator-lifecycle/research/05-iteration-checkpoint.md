# Iteration Checkpoint

## Status Snapshot

### Requirements
Requirements clarification is complete (Q1–Q20 captured in `../requirements.md`).

### Research
Completed:
- `01-lifecycle-map.md`
- `02-config-and-execution-model.md`
- `03-operator-resume-surface.md`
- `04-external-patterns.md`

## Consolidated Direction

### Product Direction (v1)
- Per-project lifecycle hooks configured in YAML.
- Hooks are foundational/extensible (Cloud Code-style), with explicit pre/post lifecycle phases.
- External script/command handlers in v1.

### Hook Runtime Semantics
- Sequential execution in declaration order.
- Per-hook failure policy is configurable.
- Blocking/suspend behavior supported.
- Suspend modes: `wait_for_resume` (default), `retry_backoff`, `wait_then_retry`.
- First operator surface: `ralph loops resume <id>`.

### Contracts & Safety
- Invocation payload: structured JSON on stdin (+ minimal env vars).
- Safeguards: `timeout_seconds`, `max_output_bytes`.
- Telemetry per hook run: event/phase, timestamps, duration, exit code, timeout flag, truncated stdout/stderr, disposition.
- Context mutation allowed only as explicit opt-in and initially limited to structured metadata injection (JSON-only).

### Validation & Ops
- Add `ralph hooks validate` and integrate with preflight checks.

### Success Criteria
- Extensibility
- Easy config UX
- Strong testability

### v1 Non-goals
- Global hooks
- Parallel hook execution
- XML output
- Full prompt/event/config mutation

## Research-driven Architecture Notes

- Existing orchestrator boundaries in `run_loop_impl` and `EventLoop` are suitable hook insertion points.
- `human.interact` already demonstrates blocking/wait semantics in the current loop.
- `task.selected` and `plan.created` are not first-class orchestrator lifecycle events today and will need synthetic hook events if included in v1.
- Existing stop/restart signal-file control flow provides a strong precedent for suspended-state + resume signaling.

## Open Design Items to Resolve Next
1. Precise v1 event taxonomy/mapping to current runtime boundaries.
2. Suspended-state persistence schema and transition rules (including idempotent resume).
3. Hook output schema for metadata injection and safe merge strategy.
4. Diagnostics/preflight integration shape for hook telemetry + validation results.
