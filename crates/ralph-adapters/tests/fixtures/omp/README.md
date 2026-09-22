# OMP (oh-my-pi) Test Fixtures

This directory holds sanitized NDJSON stream fixtures that emulate the stdout of
the **OMP** backend ([can1357/oh-my-pi](https://github.com/can1357/oh-my-pi))
under `omp -p --mode json --no-session --auto-approve`.

The fake `omp` executables built in the adapter integration tests `cat` these
files (or inline equivalents) so the production `CliExecutor`, `PtyExecutor`,
and loop runner exercise the real Pi-family stream processor against a
representative OMP event stream — with no live provider call and no credentials.

## Tested baseline

- **OMP version:** `17.2.10` (`omp --version` → `omp/17.2.10`)
- **Upstream commit:** [`39477ba39bfbdc6be2cfff0efde979dd32bd7eb7`](https://github.com/can1357/oh-my-pi/tree/39477ba39bfbdc6be2cfff0efde979dd32bd7eb7)

OMP's JSON events are derived from the same Pi event family Ralph already
supports, so OMP keeps its own `OutputFormat::OmpStreamJson` identity while
sharing the one tolerant Pi-family processor (`pi_family.rs`).

## Fixture format

Raw `omp --mode json` NDJSON — one JSON object per line, using the Pi-family
field names (`assistantMessageEvent`, `toolCallId`, `toolName`, `stopReason`,
`cacheRead`/`cacheWrite`, optional `isError`). This is **not** the
`ux.terminal.write` smoke format; these fixtures drive the adapter NDJSON parser
directly.

## Available fixtures

### `session.ndjson`

A complete, clean OMP session exercising the full event surface:

- lifecycle: `session`, `agent_start`, `turn_start`, `agent_end` (OMP emits
  `agent_end`; Ralph ignores it for normal completion via `#[serde(other)]`);
- assistant text via `text_delta` (the completion marker `LOOP_COMPLETE` is in
  the final assistant text so completion is detected from **extracted text**);
- a `thinking_delta` (hidden without TUI, shown in TUI);
- a tool lifecycle: `tool_execution_start` → `tool_execution_update` (partial,
  forwarded-compat `Other`) → `tool_execution_end` with **`isError` omitted**
  (OMP's `isError` is optional and defaults to `false`);
- a terminal `turn_end` carrying `usage`/`cost`, `provider`/`model`, and
  `message.content[].text` (the mandatory final-text fallback source).

### `no_usable_events.ndjson`

Only lifecycle/header records — no actionable event. A successful OMP process
emitting this must surface a **protocol mismatch** (case 1: zero recognized
events) rather than a silent empty success.

### `malformed_mixed.ndjson`

Malformed lines interleaved with valid ones. Malformed lines are skipped (counted
+ logged at debug); well-formed records still parse. Proves tolerance.

## Sanitization

These fixtures are **synthetic**. They contain no real prompts, filesystem
paths, provider request/response payloads, account identifiers, or credentials.
`cwd`, session `id`, `timestamp`, tool args, and model identifiers are
placeholders chosen to exercise the parser, not to mirror any real session.
