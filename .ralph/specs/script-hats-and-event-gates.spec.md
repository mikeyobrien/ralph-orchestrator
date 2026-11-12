---
status: draft
created: 2026-11-12
related:
  - .ralph/specs/per-hat-max-activations.spec.md
  - https://github.com/mikeyobrien/ralph-orchestrator/pull/416
  - https://github.com/mikeyobrien/autoloop/tree/main/packages/presets/presets/autowiki
---

# Script hats and event gates

## Goal

Add two orchestrator features so presets can keep deterministic work and proof of work out of the model's hands:

1. **Script hats.** A hat with a `run:` command executes that command instead of a model. It is triggered by events and publishes events, like any hat, but costs no model iteration.
2. **Event gates.** A top-level `gates:` list declares what evidence an event must carry before it is routed. Some of that evidence Ralph checks itself, such as a commit that exists or a command that passes. An event that fails its gate is rewritten into a blocked event.

## Context

autoloop began as a spin-off experiment from ralph-orchestrator and is being folded back into Ralph. This spec is the first piece of that work. It covers the two orchestrator capabilities that autoloop's presets depend on and Ralph lacks. The rest of autoloop gets a separate parity inventory: metareview, chains, single-file presets, profiles, and issue sync, among others.

## Motivation

**Deterministic steps cost full model iterations today.** Every fresh iteration pays a fixed prompt cost, measured at about $0.21 in the #415 code-assist run. Some hats exist only to run one script and report its result. In autoloop's `autowiki` preset, two of the four per-source roles are like this:
- `advance` runs `okf-advance.sh`, which decides whether to dispatch the next source, run maintenance, or finish, and emits the event itself.
- `merger` runs `okf-source-merge.sh`, which merges the source branch and emits `merge.done`.

The role prompts say why the work lives in scripts: counting the queue by reasoning "was unreliable and ended runs early". Ralph has no way to run such a step without a model. Ported as-is, autowiki would pay for 4 model iterations per source where 2 would do.

**Ralph's existing evidence checks take the agent's word for it.** `build.done` and `review.done` are checked by hard-coded string matching on the payload (`tests: pass`, `lint: pass`), in `event_loop/mod.rs` `process_parse_result`. That checks that the agent claimed success, not that anything passed. The checks also cover only those two topics, and presets can't declare their own. autoloop's `[[gate]]` (`packages/core/src/evidence.ts`) generalizes the presence check per event, with typed status and threshold checks. Its trust still comes from scripts emitting events themselves, so no agent writes the claim.

Both features follow the tenet "Backpressure Over Prescription": instead of prose asking the agent to verify, add a gate that rejects unverified work.

## Non-goals (v1)

- Script hats with `concurrency > 1` (waves).
- Changing how LLM hats are prompted or routed.
- Replacing the hard-coded `build.done` / `review.done` checks. See "Relationship to existing backpressure".
- A general workflow language. A script hat runs one command, and the command decides what to emit.

---

## Feature 1: Script hats

### Configuration

```yaml
hats:
  advance:
    name: "Queue keeper"
    description: "Dispatches the next source, a maintenance batch, or completion"
    triggers: ["queue.start", "merge.done", "index.done"]
    publishes: ["queue.advance", "maintenance.due", "queue.done"]
    run: ["sh", "scripts/okf-advance.sh"]   # argv list, or a string run via `sh -c`
    timeout: 120                             # seconds; existing hat field
    default_publishes: "queue.advance"       # optional; used when the command exits 0 without emitting
```

Validation (`RalphConfig::validate`):
- `run` is mutually exclusive with `instructions` and `extra_instructions`. Both set is an error.
- `run` together with `concurrency > 1` is an error in v1.
- `backend`, `backend_args`, `effort`, and `disallowed_tools` are ignored on a script hat. Setting them produces a config warning.
- `run` must not be empty.

### Runtime behavior

When the active hat for an iteration has `run`:
1. The orchestrator consumes the hat's pending events exactly as it does for an LLM hat. It skips prompt building and backend selection.
2. It executes the command through the existing hook executor (`HookExecutor`), with the hat's `timeout` (the adapter default otherwise), output capture, and `max_output_bytes`. The working directory is the workspace root.
3. The command receives this environment:

   | Variable | Value |
   |---|---|
   | `RALPH_BIN` | Current Ralph executable (as for LLM hats) |
   | `RALPH_HAT` | Hat id |
   | `RALPH_ITERATION`, `RALPH_LOOP_ID` | Iteration number and loop id |
   | `RALPH_TRIGGER_TOPIC` | Topic of the triggering event (the first one, if several are pending) |
   | `RALPH_TRIGGER_PAYLOAD` | Its payload, truncated to 64 KiB |
   | `RALPH_TRIGGER_EVENTS_FILE` | Path to a JSONL file holding every consumed event, untruncated |
   | the events-file variable `ralph emit` already uses | So `"$RALPH_BIN" emit` works unchanged |

4. The command publishes with `"$RALPH_BIN" emit <topic> <payload>`. Events are read back by the normal `process_events_from_jsonl` path, so gates, completion handling, and diagnostics all apply.
5. Scope enforcement is always on for script hats, whatever `enforce_hat_scope` says. A topic outside `publishes` becomes `<hat>.scope_violation`. A script has no prose to fall back on, so a wrong topic is always a bug.
6. Outcomes:
   - **Exit 0, at least one event:** a normal iteration.
   - **Exit 0, no events:** inject `default_publishes` if set. Otherwise publish `<hat>.failed` with "command exited 0 without emitting".
   - **Non-zero exit or timeout:** publish `<hat>.failed` with the exit code and the last 4 KiB of stderr, and count it toward `max_consecutive_failures`.

### Accounting

- Script iterations get an iteration number and appear in diagnostics, the TUI, and `iteration.summary`, with backend `script` and cost 0.
- They count toward the hat's `max_activations`, and toward stale-loop and thrashing detection.
- They do **not** count toward `event_loop.max_iterations`, which bounds model work. A new `event_loop.max_script_runs` (default 500) bounds them, so two script hats that trigger each other can't loop forever.
- Lifecycle hooks (`pre.iteration.start` and the others) fire for script iterations as for any iteration.

### Implementation sketch

- `crates/ralph-core/src/config.rs`: add `HatConfig.run: Option<HatRun>` (an untagged string or `Vec<String>`), the validation above, and `EventLoopConfig.max_script_runs`.
- `crates/ralph-core/src/event_loop/mod.rs`: expose the consumed trigger events for the active hat, so the runner can pass them to the script without building a prompt. Today they are consumed inside `build_prompt`. Add the script-run counter and its termination reason (`TerminationReason::MaxScriptRuns`).
- `crates/ralph-cli/src/loop_runner.rs`: where the active hat's config is resolved (the "Step 1: Get hat backend configuration" block), branch to a new `execute_script_hat(...)` that reuses `HookExecutor`, then continue into the normal event-processing path.
- Reuse `HookExecutor`'s timeout, output capture, and process-group handling rather than adding a new process runner.

---

## Feature 2: Event gates

### Configuration

```yaml
gates:
  - event: "foundation.done"
    requires:
      - field: commit
        verify: commit          # must be a real commit, reachable from HEAD, created this iteration
  - event: "merge.done"
    requires:
      - field: merged
        verify: { commit: { reachable_from: "HEAD", new: false } }
  - event: "review.ready"
    requires:
      - field: tests
        status: passed          # typed check, same parsing rules as autoloop evidence.ts
      - command: ["python3", "-m", "unittest", "-q"]   # Ralph runs it; exit 0 passes
        timeout: 300
    blocked: "review.blocked"   # optional; default is "<prefix>.blocked" (merge.done -> merge.blocked)
```

### Requirement kinds

| Kind | Passes when | Trust |
|---|---|---|
| `field: k` | `k` is present with a non-empty value. Payloads may be JSON objects or `k=v` tokens, using autoloop's `parseEvidenceEntry` rules. | Agent's claim |
| `field: k, status: passed` | `k` carries that status word | Agent's claim |
| `field: k, min/max: n` | `k` parses as a number within bounds (a trailing `%` is allowed) | Agent's claim |
| `field: k, verify: commit` | `k` resolves to a commit (`git cat-file -e <v>^{commit}`) that is an ancestor of HEAD and was not an ancestor of the HEAD recorded at iteration start. `new: false` drops that last condition, and `reachable_from` picks the ref. | Checked by Ralph |
| `command: [...]` | Ralph runs the command through `HookExecutor` and it exits 0. The event's payload is in `RALPH_EVENT_PAYLOAD` and its topic in `RALPH_EVENT_TOPIC`. | Checked by Ralph |

All requirements must pass. They are evaluated in the order listed, and evaluation stops at the first failure, so an expensive `command` check never runs when a cheap field check has already failed.

### Runtime behavior

- Evaluated in `process_parse_result` after scope enforcement and before routing, next to the existing `build.done` checks. Wave events are gated like any other event.
- A gate failure replaces the event with the `blocked` topic. The payload says which requirement failed and why (`missing`, `status`, `threshold`, `not a commit`, `not reachable`, `not new`, `command exited N` plus the stderr tail), followed by the original payload. The diagnostics entry is `BackpressureTriggered`.
- A blocked event routes like any other: to a hat that triggers on it, or to Ralph as the fallback. Repeated blocks feed the existing thrashing and stale-loop detection.
- The completion promise can be gated. A gated completion that fails does not end the loop.
- Validation: a gate whose `event` no hat publishes is a warning. More than one gate on the same event is an error. A `command` requirement with an empty argv is an error.

### Relationship to existing backpressure

The hard-coded `build.done` and `review.done` checks stay in v1. A follow-up should express them as built-in default gates, so they show up in the config, can be configured, and are implemented in one place. The repo's `CLAUDE.md` says backwards compatibility doesn't matter, so that migration can drop the string-matching special cases entirely.

### Implementation sketch

- `crates/ralph-core/src/config.rs`: add `RalphConfig.gates: Vec<GateConfig>` and the requirement types, plus validation.
- New `crates/ralph-core/src/gates.rs`: `evaluate(gate, event, ctx) -> Result<(), Shortfall>`, with the payload parsing (a port of `parseEvidenceEntry`), the git checks, and command execution through an injected runner trait, so unit tests need no real processes.
- `event_loop/mod.rs`: record HEAD at iteration start, so `new` commits can be checked. Call gate evaluation in `process_parse_result`.

---

## How autowiki uses this

| autowiki role | Ported as | LLM iterations per source |
|---|---|---|
| `advance` (runs `okf-advance.sh`) | script hat | 0 (was 1) |
| `cleaner` | LLM hat, `--model sonnet` via `backend` args | 1 |
| `foundation` / `knowledge_extractor` | LLM hats, Opus | 1 |
| `merger` (runs `okf-source-merge.sh`) | script hat | 0 (was 1) |
| maintenance batch: summarize → synthesize → compare → lint → indexer | LLM hats, run every 5 merges plus once at drain | about 1 per source, amortized |

Its gates (`clean.*`, `foundation.done`, `extract.done`, `summary.done`, `synthesis.done`, and `index.done` require `commit`; `merge.done` requires `merged`) become `verify: commit` requirements. Those are stronger than autoloop's presence checks, because Ralph confirms that the commit exists and is new. `okf-init.sh` runs as a `pre.loop.start` hook, as it does in autoloop.

## Acceptance criteria

Script hats:
- **Given** a hat with `run`, **when** its trigger is published, **then** the command runs without any backend process, and the events it emits with `ralph emit` are routed.
- **Given** a script that exits 0 without emitting and has `default_publishes`, **then** the default event is published. **Given** the same script without `default_publishes`, **then** `<hat>.failed` is published.
- **Given** a script that exits non-zero or times out, **then** `<hat>.failed` carries the exit code and the stderr tail, and `consecutive_failures` increments.
- **Given** a script that emits a topic outside `publishes`, **then** the event is dropped and `<hat>.scope_violation` is published, even when `enforce_hat_scope` is false.
- **Given** `max_iterations: 5` and a run with 5 LLM iterations plus 10 script iterations, **then** the run is not terminated by `max_iterations` before its 5th LLM iteration.
- **Given** two script hats that trigger each other forever, **then** the run terminates with `max_script_runs` (or earlier, through stale-loop detection).
- **Given** `run` and `instructions` on the same hat, **then** config validation fails with a message naming the hat.

Event gates:
- **Given** a gate requiring `field: commit, verify: commit` and an event whose `commit` value is not a commit, **then** the blocked topic is published with reason `not a commit`, and the original event is not routed.
- **Given** a commit that was already reachable from HEAD at iteration start, **then** the gate fails with `not new`. With `new: false`, it passes.
- **Given** a `command` requirement whose command exits 1, **then** the event is blocked with `command exited 1` and the stderr tail. **Given** an earlier field requirement that already failed, **then** the command never runs.
- **Given** a gate on the completion promise that fails, **then** the loop does not complete.
- **Given** no `gates` in the config, **then** behavior is unchanged.

## Testing

- Unit tests in `gates.rs` cover each requirement kind: payload parsing for JSON and tokens, vacuous values, status and threshold checks, commit checks against a temporary git repo, and command checks through a fake runner.
- `event_loop` tests cover blocked-event rewriting and completion gating.
- A `ralph-cli` integration test runs a preset made of **script hats only**. The whole loop is deterministic with no backend and no mocks, so it can test the event machinery from start to finish.
- A smoke fixture with one LLM hat followed by one script hat, using a recorded session.

## Open questions

- Should script hats also be allowed as wave workers? That's deferred in v1, but it could matter for per-item deterministic steps.
- Should `max_script_runs` default to unbounded when stale-loop detection already covers the obvious loops? The proposal is a cap of 500 for safety.
- Should a gate be able to require that an event come from a script hat (`source: script`)? That would rule out an LLM imitating a script's emission without needing `verify`.
