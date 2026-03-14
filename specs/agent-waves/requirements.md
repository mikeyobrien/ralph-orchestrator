# Agent Waves — Requirements

Questions and answers from requirements clarification.

---

## Q1: Hatless Ralph vs. Actual Concurrent Execution

Today Ralph uses a "Hatless Ralph" architecture — Ralph is always the single executor, and custom hats are just personas with filtered instructions. Waves fundamentally require **actual concurrent hat execution** (multiple backends running simultaneously).

How should waves interact with the Hatless Ralph model?

**Option A — Break the model for waves:** When a wave is active, the loop runner spawns actual separate backend processes per wave instance. Ralph doesn't coordinate these — each instance runs independently with its hat's instructions. Ralph resumes coordination after the aggregator collects results.

**Option B — Ralph orchestrates wave dispatch:** Ralph remains the single executor. When Ralph decides to fan out, it emits wave events and the *loop runner* (not Ralph) spawns concurrent backends. Ralph is "paused" until the wave completes and the aggregator activates.

**Option C — Waves are a loop-runner concern only:** Hats define topology and concurrency hints in config. The loop runner handles all parallelism transparently — Ralph doesn't even know waves exist. The loop runner detects when multiple events target the same hat (or correlated hats) and spawns backends in parallel.

**A1:** Option B — Ralph orchestrates wave dispatch, loop runner executes concurrently. Ralph decides WHAT to parallelize (adaptive, NL-driven), loop runner handles HOW (concurrent backends, correlation tracking, concurrency limits). Mirrors the existing `human.interact` blocking pattern. Preserves Hatless Ralph architecture.

Note: Waves are a general-purpose parallel execution primitive, not limited to code review. Use cases include deep research (parallel topic exploration), multi-perspective analysis, parallel builds, scatter-gather for any domain.

---

## Q2: Wave Instance Execution Model

When the loop runner spawns concurrent backends for a wave, what does each instance actually look like?

**Option A — Full hat execution:** Each wave instance is a complete hat activation — the backend gets the hat's full instructions, the specific event payload, and runs like a normal iteration. The instance can use tools, write files, emit events.

**Option B — Lightweight task execution:** Wave instances are simpler than full hat activations — they get a focused prompt with just the payload and instructions, run with a shorter timeout, and are limited in what they can do (e.g., no emitting further waves, no human.interact).

**Option C — Configurable per hat:** The hat config defines the execution profile. Some waves need full capability (deep research agents that use tools extensively), others need lightweight execution (quick file analysis). Let the preset author decide.

**A2:** Option A — Full hat execution. Each wave instance is a complete hat activation with full tool access. Agents are smart; let them do the work. Guardrails are structural (concurrency, max_activations, publishes whitelist, aggregate.timeout, cost tracking) — not capability restrictions. Instructions control behavior, config controls constraints. No nested waves for v1 (a wave instance cannot emit another wave).

---

## Q3: Aggregator Activation Model

When all wave results arrive, the aggregator hat activates. In the Hatless Ralph model, Ralph is always the executor. How does the aggregator work?

**Option A — Ralph as aggregator:** Ralph activates wearing the aggregator hat's persona. All wave results are injected into Ralph's prompt as pending events. Ralph synthesizes per the aggregator's instructions. This is consistent with how every other hat works today.

**Option B — Dedicated aggregator backend:** The aggregator gets its own backend process (like wave workers do), separate from Ralph's main process. This could be useful if the aggregated results are very large and need a fresh context window.

**Option C — Implicit aggregation:** No explicit aggregator hat. Wave results are simply queued as pending events for whatever hat subscribes to the result topic. The existing event routing handles it — no special aggregation semantics needed.

**A3:** Option A — Ralph as aggregator. The aggregator is just another hat with a `wait_for_all` gate. Ralph activates wearing the aggregator persona once all correlated wave results arrive. Consistent with existing hat model. Option B (dedicated backend) is a future escape hatch if context pressure becomes an issue with large wave result sets.

---

## Q4: Wave Dispatch Mechanism

The issue proposes CLI tools (`ralph wave start --expect N`, `ralph wave end`) for explicit dispatch, and @mikeyobrien asked about NL-driven dispatch where the model just decides. For v1, which dispatch mechanism should we build?

**Option A — Explicit CLI tools only:** `ralph wave start/emit/end`. The dispatcher hat's instructions tell the agent exactly which tools to call. Deterministic, easy to test and debug.

**Option B — NL dispatch only:** The loop runner detects when a hat emits multiple events targeting wave-capable hats and automatically treats them as a wave. No new CLI tools — just emit events normally and the infrastructure handles concurrency.

**Option C — Both, but NL is the primary path:** Build the CLI tools for explicit control, but also build the context injection (downstream hat descriptions in prompt) so the model can dispatch naturally. The loop runner infers wave semantics from correlated event emission. Preset authors choose the style via instructions.

**A4:** Option C — Both. CLI tools (`ralph wave start/emit/end`) are the mechanism; context injection (downstream hat descriptions in prompt) enables adaptive NL dispatch. They're the same system — the preset author controls the spectrum via instructions. The HATS table already resolves publishes → downstream hats, so context injection is partially built.

---

## Q5: Default Isolation Model for Wave Instances

The issue proposes shared workspace (no isolation) by default, with opt-in worktree isolation for write-heavy waves. For v1, should we support both, or just one?

**Option A — Shared workspace only (v1):** All wave instances share the working directory. Simple, zero overhead. Sufficient for read-heavy use cases (research, analysis, review). If instances write conflicting files, that's a preset design problem, not an infrastructure problem.

**Option B — Both shared and worktree isolation (v1):** Build `isolation: worktree` from day one using existing worktree infrastructure. The machinery exists — `create_worktree()`, `sync_working_directory_to_worktree()`, symlinks. Enables write-heavy use cases like parallel module builds immediately.

**Option C — Shared workspace default, with conflict detection:** Shared workspace only, but the loop runner detects when concurrent instances modify the same files and emits a warning event. Gives visibility without the overhead of worktrees.

**A5:** Option A — Shared workspace only for v1. Wave instances share the working directory. Zero overhead, sufficient for read-heavy and write-disjoint workloads (research, analysis, review, scatter-gather). Worktree isolation is a v2 feature — the infrastructure exists but adds complexity disproportionate to v1 use cases.

---

## Q6: Failure Handling Within a Wave

When a wave instance fails (backend error, timeout, crash), what happens to the rest of the wave?

**Option A — Fail-fast:** If any instance fails, cancel all remaining/running instances. The aggregator receives partial results plus failure info. Conservative — prevents wasted compute on a broken wave.

**Option B — Best-effort:** Failed instances are recorded but the wave continues. The aggregator activates when all non-failed instances complete (or timeout). The aggregator's prompt includes which instances failed and why. Resilient — one bad instance doesn't poison the whole wave.

**Option C — Configurable:** Add `aggregate.on_failure: fail_fast | best_effort` to hat config. Default to best-effort since most use cases (research, review) produce useful partial results.

**A6:** Option B — Best-effort, hardcoded for v1. Wave continues on instance failure. Aggregator receives all available results plus structured failure metadata (which instance, what error, duration). Aggregator instructions handle degraded results. Must be well-documented during implementation. Configurable fail-fast is a v2 option if a real use case demands it.

---

## Q7: Cost and Activation Accounting

Wave instances consume API tokens. Each concurrent backend is a separate API call. How should waves interact with the existing cost tracking and `max_activations` limits?

**Option A — Each instance counts as one activation:** A 5-instance wave counts as 5 activations against the worker hat's `max_activations` and 5 units of cost. Simple, transparent, consistent with how single-hat activations work.

**Option B — Wave counts as one activation:** The entire wave (dispatch + N workers + aggregation) counts as a single "wave activation." Simpler accounting but hides actual resource usage.

**Option C — Wave-level cost limits:** Each instance counts individually for `max_activations`, but add a new `max_wave_cost` config field to cap total spend within a single wave. Prevents runaway waves where each instance is expensive.

**A7:** Option A — Each instance counts as one activation. 5-instance wave = 5 activations, 5 units of cost. Transparent, consistent, and existing limits (`max_activations`, `max_cost`) naturally constrain wave size. No new cost config needed for v1.

---

## Q8: Aggregation Timeout

The issue proposes `aggregate.timeout` as a fail-safe for hung workers. What should the default be, and what happens when it fires?

**Option A — Required config, no default:** Preset author must specify `aggregate.timeout`. Forces intentional timeout design per use case (quick analysis vs. deep research need very different timeouts).

**Option B — Sensible default (e.g., 300s):** Default timeout of 5 minutes. Aggregator activates with whatever results have arrived. Missing results included as structured failure metadata. Preset author can override.

**Option C — No timeout for v1:** Rely on the global `max_runtime` limit to catch hung waves. Simpler — one fewer config field. The wave completes when all instances finish or the run hits its time limit.

**A8:** Option B — Sensible default of 300s, overridable via `aggregate.timeout`. When timeout fires, aggregator activates with all results received so far. Timed-out instances get structured failure metadata (`status: timeout`, `timeout_seconds: 300`). Consistent with best-effort failure model (Q6). Deep research presets can increase, quick analysis can decrease.

---

## Q9: Scope of v1

Given all the decisions so far, what's the minimum viable scope for v1? What's explicitly deferred?

**v1 includes:**
- Wave CLI tools (`ralph wave start/emit/end`)
- Event correlation metadata (`wave_id`, `wave_index`, `wave_total`)
- Concurrent backend spawning in loop runner (respecting `concurrency` limit)
- `aggregate.mode: wait_for_all` with timeout
- Context injection (downstream hat descriptions in prompt for NL dispatch)
- Best-effort failure handling
- Per-instance activation and cost accounting
- Shared workspace (no isolation)
- No nested waves

**Explicitly deferred to v2+:**
- Worktree isolation (`isolation: worktree`)
- Nested waves
- Additional aggregation modes (`first_n`, `quorum`, `external_event`)
- Configurable failure modes (`on_failure: fail_fast`)
- Wave-level cost limits (`max_wave_cost`)
- Dedicated aggregator backends
- Multi-round debate (works with v1 primitives but not explicitly optimized)

Does this scope feel right, or should anything move between v1 and v2?

**A9:** Scope confirmed as-is. Requirements clarification complete.
