# OMP smoke fixtures

Pre-rendered **readable OMP text** fixtures for the `ralph-core` smoke runner
(`SmokeRunner` / `EventParser` replay path). These are NOT raw `omp --mode json`
NDJSON — they are the human-readable terminal bytes OMP produces, with Ralph
`<event topic="…">` tags inline, base64-encoded into the `ux.terminal.write`
JSONL replay format (same format as `tests/fixtures/kiro-acp/`).

The smoke runner decodes each terminal-write record and replays the text bytes
through `ralph_core::EventParser` to verify `LOOP_COMPLETE` detection on
extracted text. It does not invoke the OMP NDJSON parser (that lives in
`ralph-adapters`, covered by steps 2–3) and does not inspect source text.

## Fixtures

| File | Covers |
|------|--------|
| `omp_basic_session.jsonl` | Completion: readable OMP text ending in `<event topic="LOOP_COMPLETE">`. |
| `omp_malformed_recovery.jsonl` | Malformed-line recovery: garbage text + a broken (incomplete) event tag, then a valid `LOOP_COMPLETE`. |
| `omp_nonzero_exit.jsonl` | Non-zero exit propagation: readable OMP text with a failure event but NO `LOOP_COMPLETE` (agent exited before completing). |

## Provenance & sanitization

Hand-authored from the researched OMP `17.2.10` readable-output contract (design
`detailed-design.md` §5/§7). No live provider call, credential, provider payload,
personal session data, or executable is present — only synthetic placeholder text
and Ralph event tags. Deterministic and replay-only.
