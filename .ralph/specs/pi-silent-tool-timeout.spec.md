# Pi silent-tool inactivity timeout

Status: implementation authorized by the supplied Firstmate task specification.

## Scope

Honor `adapters.pi` independently of Claude and prevent healthy Pi tools from
being killed solely because they produce no output. Keep other adapters,
including OMP, unchanged.

## Verified baseline

At `edc2b32`, `AdaptersConfig` already has a flattened per-backend override map.
`adapter_settings("pi")` honors `adapters.pi` and otherwise uses
`adapters.default`, not Claude. Preserve that implementation rather than
reintroducing typed vendor fields. Add explicit Pi regression coverage through
the production configuration parser and worker-timeout lookup.

The CLI executor currently applies inactivity timeouts between every stream
line, even between Pi tool start and tool end events.

## Protocol evidence

Pi 0.87.1's installed `docs/json.md` defines `tool_execution_start`,
`tool_execution_update`, and `tool_execution_end`, correlated by `toolCallId`.
Its print mode forwards session events through `toJsonEvent` as NDJSON.
A deterministic probe ran the installed real `runAgentLoop` and `createBashTool`
with only the model response stubbed, then serialized events with `toJsonEvent`.
For `sleep 2`, the emitted lifecycle was:

- 4 ms: `tool_execution_start`, ID `silent-bash`.
- 12 ms: one `tool_execution_update`, empty partial content.
- 2023 ms: `tool_execution_end`, `(no output)`, `isError: false`.

There were no events in the intervening 2011 ms. Updates reflect tool output,
not a periodic heartbeat; a longer silent command has the same gap.

## Implementation contract

Track outstanding tool IDs in the existing Pi-family stream processor. A start
adds its ID; a matching end removes it, including error results. Duplicate starts
must not require duplicate ends; unrelated ends must not clear another tool.
An end record whose other fields fail to deserialize still removes its
`toolCallId`. Only `OutputFormat::PiStreamJson` consults this state to replace
the CLI inactivity timeout while a tool remains open. OMP and every other format
retain their current timeout behavior. After the last matching end, the normal full
inactivity window resumes.

While a tool is open, the deadline is `adapters.pi.tool_timeout` (default 3600
seconds) measured from the oldest open tool's first start. A tool open past that
ceiling, or one whose end event was lost, is treated as stuck and the iteration
times out, logged as exceeding `tool_timeout` with the configured ceiling rather
than as an inactivity timeout. No heartbeat is introduced. A Pi process silent
without an open tool or pending model request must still time out.

## In-flight model request extension

The approved extension covers long model latency, including an iteration's
first request and silent reasoning after the response stream opens. A Pi
`turn_start` opens a pending model request that stays in flight until the
assistant `message_end`, `turn_end`, or agent completion arrives. Assistant
`message_start` and `message_update` records do not close it, nor do user and
tool-result messages. Duplicate starts must not restart the bound.

While the model request is pending, replace inactivity detection with the
existing `adapters.pi.tool_timeout` ceiling (no new setting), measured from
`turn_start`. A request exceeding the ceiling fails with a distinct model-request
timeout diagnostic. Once the assistant message ends, normal inactivity handling
resumes unless a tool is open. A silent process with neither a pending request
nor an open tool still times out normally. OMP behavior stays unchanged.

Add process-level regressions for a healthy slow first response, a silent
response after assistant `message_start`, a pending request exceeding its
ceiling, and true idle silence. Check request lifecycle
transitions and update the activity descriptions in docs.

## Acceptance and validation

- `adapters.pi.timeout`, enabled, and tool-permission values round-trip and are
  independent of Claude; actual worker timeout lookup selects Pi.
- A fake Pi subprocess completes after an open tool is silent longer than the
  configured inactivity timeout.
- Silence without an open tool or pending request, and silence after completion
  (successful or failed), still times out.
- A tool open longer than `tool_timeout` times out; an unparsable end record
  with a matching ID still closes the tool.
- Overlapping tool IDs, duplicate starts, unmatched ends, and partial updates do
  not incorrectly release the exemption.
- The same open-tool stream in OMP still times out.
- `tool_timeout` set on a named backend other than `pi` yields a config warning;
  `adapters.default.tool_timeout` may still feed Pi through the normal fallback.
- Document Pi configuration and the inactivity exemption in adapter references.
- Run format checks, clippy, affected crate tests, full `cargo test`, and
  replay-based smoke tests before committing and handing off to no-mistakes.
