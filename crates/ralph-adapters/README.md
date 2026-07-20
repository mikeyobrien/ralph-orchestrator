# Autoloop adapter boundaries

`ralph-adapters` translates stable autoloop contracts into Ralph's observation
model. Ralph is the frontend and coordination plane; it must not infer engine
state from implementation details when a supported contract exists.

## Supported contracts

| Surface | Ralph use |
|---|---|
| `autoloop run --events <path>` NDJSON | `autoloop_events` and `autoloop_event_tailer` decode and incrementally tail structured lifecycle, routing, progress, question, and terminal events. This is the primary live-observation contract. |
| `.autoloop/journal.jsonl` | `autoloop_journal` replays and tails the append-only journal using its documented record shapes. Code using the journal must go through this adapter rather than reading unrelated run files. |
| Terminal `autoloops summary` block | `autoloop_runner` captures stdout and parses the final run id, iteration count, stop reason, cost, journal path, and memory path. The captured block is protocol data and is not passed through as Ralph output. |
| Process exit semantics | `autoloop_runner` preserves documented success/non-zero status and bounded stderr diagnostics. The CLI maps that result and terminal events into Ralph termination reasons. |
| Control channel | Engine control messages are a sanctioned integration boundary when available. Ralph currently controls an interactive child through process lifecycle (including terminating its process group on TUI quit) rather than mutating private engine state. Future pause, resume, guidance, and HITL work must use the engine control channel, not private files. |

## Undocumented live-stream coupling

`backend_stream_tailer` probes
`.autoloop/runs/<run>/claude-stream.N.jsonl` and
`.autoloop/runs/<run>/pi-stream.N.jsonl`. These per-iteration files are
**undocumented engine internals**, not a stable contract. The run directory is
currently derived from the `runId` delivered by the supported `--events`
stream in `ralph-tui/src/autoloop_source.rs`.

This coupling is retained because `--events` reports lifecycle progress but
does not yet carry the backend's incremental assistant text and tool calls.
Reading the stream files lets the TUI show provisional live content while the
engine runs. The tailer treats absent files, incomplete trailing JSON, stream
rotation, and unknown records as non-fatal; authoritative structured events
still drive lifecycle and completion. Tool paths are presented through a
workspace-aware formatter so private run-directory paths do not reach the UI.
If the internal filenames or schemas change, live provisional content may be
missing, but the loop must continue and finish from supported contracts.

The upstream replacement should be a documented structured live-output stream
(or equivalent additions to `--events`) carrying iteration identity,
assistant deltas, tool calls/results, ordering, and completion/reconciliation
markers. Once that contract exists, remove the run-directory derivation and
all `claude-stream` / `pi-stream` probing.
