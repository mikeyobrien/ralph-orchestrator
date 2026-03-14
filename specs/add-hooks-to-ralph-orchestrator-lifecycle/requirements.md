# Requirements Clarification

## Q1
**Question:** Which orchestrator lifecycle phases do you want hook points for (for example: loop start/end, iteration start/end, task selection, plan creation, pre/post backpressure checks, merge queue events, human interaction events)?

**Answer:** Include hook points across the full orchestrator lifecycle, wherever they provide value for management and observability.

## Q2
**Question:** Who should consume these hooks first, and through what mechanism (e.g., structured events to diagnostics JSONL, internal Rust callbacks/trait, external command/webhook integration, or all of the above)?

**Answer:** Hooks should be a foundational mechanism to build on, similar to hook systems in Cloud Code and other agent harnesses.

## Q3
**Question:** For the first version, should hook handlers run as external executables/commands (like git hooks), in-process Rust plugins, or both?

**Answer:** v1 should support user-configured hooks in YAML (project config and possibly global Ralph config), where users attach scripts/commands to lifecycle events, similar to Cloud Code-style hooks.

## Q4
**Question:** Should hook definitions support both scopes in v1 — (a) global/default hooks for all runs and (b) per-project hooks that can override or extend global ones?

**Answer:** v1 should be per-project only. Global/default hook scope will be a separate future feature.

## Q5
**Question:** What should happen if a hook fails (non-zero exit, timeout, invalid output): fail the lifecycle step, warn and continue, or configurable per hook?

**Answer:** Behavior should be configurable per hook.

## Q6
**Question:** Should hooks be allowed to block orchestration (synchronous pre-hooks), or should v1 be non-blocking/observability-only?

**Answer:** Hooks should be able to block or at least suspend orchestration.

## Q7
**Question:** Do you want each hook invocation to receive a structured JSON payload on stdin (event name, run/loop IDs, iteration, timestamps, context), or should v1 pass only env vars/CLI args?

**Answer:** Use structured JSON on stdin as the primary contract, plus minimal convenience env vars.

## Q8
**Question:** Which initial lifecycle events are mandatory for v1 (pick a minimal but useful set)?

**Answer:** Include all events from the proposed starter set except backpressure events (exclude both `backpressure.passed` and `backpressure.failed`).

## Q9
**Question:** Should hooks be split into explicit `pre.<event>` and `post.<event>` phases for the same lifecycle point (so users can run guards before and notifications after)?

**Answer:** Yes, include explicit `pre.<event>` and `post.<event>` phases.

## Q10
**Question:** If multiple hooks are configured for the same event phase, should they run in declaration order (sequential), in parallel, or configurable?

**Answer:** Run sequentially in declaration order in v1 to minimize surprises.

## Q11
**Question:** Should v1 include built-in safeguards per hook (e.g., timeout_seconds, max_output_bytes, redaction of sensitive fields) with defaults?

**Answer:** Include `timeout_seconds` and `max_output_bytes` safeguards in v1.

## Q12
**Question:** For observability, what should Ralph persist for each hook run (minimum set): start/end time, duration, exit code, timed_out flag, stdout/stderr (truncated), and associated lifecycle event?

**Answer:** Yes—start with that telemetry set.

## Q13
**Question:** You said hooks may block or suspend orchestration. In v1, should “suspend” mean pause and wait for an explicit human/operator resume signal, or just delayed retry/backoff of the same step?

**Answer:** Support a combination in v1: `wait_for_resume`, `retry_backoff`, and hybrid `wait_then_retry`. Default should be `wait_for_resume`.

## Q14
**Question:** For operator resume, what is the first control surface in v1: a new CLI command (e.g., `ralph loops resume <id>`), a file signal, web API action, Telegram command, or multiple?

**Answer:** Add a new CLI command: `ralph loops resume <id>`.

## Q15
**Question:** Should hooks be able to mutate execution context in v1 (e.g., return JSON to modify prompt/events/config), or be side-effect-only with pass/fail/suspend decisions?

**Answer:** Yes, allow mutation in v1, but only as explicit opt-in. It must be clearly called out and user-configured/acknowledged; default behavior should not mutate context.

## Q16
**Question:** What mutation surface should v1 allow initially (to keep risk bounded): add structured metadata only, inject additional prompt guidance, enqueue extra events, or full config mutation?

**Answer:** Keep v1 simple: allow only structured metadata injection (no prompt/event/config mutation).

## Q17
**Question:** For that metadata contract, should v1 support JSON only (recommended for consistency with stdin payload), or both JSON and XML?

**Answer:** JSON only in v1.

## Q18
**Question:** Should v1 include a dry-run/validation command (e.g., `ralph hooks validate`) to catch misconfigured hook commands and schemas before starting a loop?

**Answer:** Yes. Add a validation command and reuse it in pre-flight checks.

## Q19
**Question:** What are your primary success criteria for this feature (top 3): e.g., reliable event coverage, low performance overhead, clear failure semantics, easy config UX, and testability?

**Answer:** Extensibility, easy config UX, and strong testability.

## Q20
**Question:** Are there any explicit non-goals for v1 we should lock now (e.g., global hooks, parallel hook execution, remote/webhook destinations, XML output, full context mutation)?

**Answer:** Yes—lock these as v1 non-goals: global hooks, parallel hook execution, XML output, and full prompt/event/config mutation.

## Q21
**Question:** Do we need a dedicated `task.selected` or `hat.selected` lifecycle hook event in v1?

**Answer:** No. Remove dedicated `task.selected`/`hat.selected` events in v1 to avoid overlap/confusion. Use `iteration.start` payload to carry selected hat/task context instead.
