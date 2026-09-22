# Preserve historical tool calls and coalesce stream-backpressure notices

Beads:
- `ralph-orchestrator-v3-autoloops-backend-au2` (P1)
- `ralph-orchestrator-v3-autoloops-backend-hav` (P2)

## Acceptance approval evidence

The user explicitly confirmed that the bead acceptance was approved before the
loop launched. No date or additional approval claim is inferred here.

## Mission

Fix both confirmed Ralph TUI observability defects without weakening stream bounds or moving engine judgment into Ralph:

1. Completed iteration history loses all tool-call summaries when `backend.output` reconciles provisional live output.
2. Fast-growing backend streams permanently append one `… N bytes skipped …` line per poll, flooding the pane and consuming the 2,000-line budget.

## Reproduction evidence

Live run `fair-harbor` on 2026-07-20:

- iteration 5 history displayed only the final planner response;
- `pi-stream.5.jsonl` was 23,268,353 bytes and still contained 10 `tool_execution_start` plus 18 `message_end` records;
- iteration 6 reached 183,527,530 bytes;
- its pane showed dozens of skip notices from roughly 6 KiB through 928 KiB, obscuring useful tool summaries;
- live tool summaries could repeat because cumulative Pi records replay prior tool data.

Current seams:

- `crates/ralph-tui/src/autoloop_source.rs:210-231` truncates the entire provisional live region on `backend.output`, disables the tailer, then appends only final text.
- `crates/ralph-adapters/src/backend_stream_tailer.rs:93-184` bounds each poll to 256 KiB and returns a new agent-text skip marker for every oversized poll.

The upstream quadratic Pi log root cause is tracked separately as `autoloop-harness-heap-oom-0ze`; proposed compaction exists at autoloop commit `c373da6`. Ralph must remain correct even when any backend legitimately outpaces its UI reader.

## Required behavior A — completed history

- Preserve a bounded history of distinct tool-call summaries after `backend.output` arrives.
- Reconcile provisional assistant text to the authoritative final output exactly once.
- Do not duplicate tool summaries merely because cumulative backend records repeat prior events.
- Do not collapse two genuinely different tool calls just because their rendered summaries are textually identical; use stable event identity when available or a defensible bounded identity model.
- Historical iteration navigation and exported iteration history must agree.
- Once authoritative output arrives, the old stream must not resume polling.

## Required behavior B — backpressure presentation

- Repeated oversized polls produce at most one visible skip/backpressure status per iteration.
- Update or coalesce that status with the truthful cumulative skipped-byte count.
- The status must not repeatedly consume the 2,000-line content budget.
- Keep the newest useful bounded assistant/tool lines visible.
- Preserve the existing 256 KiB per-poll, 2,000-line per-iteration, and 4 KiB per-line safety bounds unless a stronger bounded design is proven.
- Do not silently hide that bytes were dropped.

## Blocking acceptance gates

1. Add a red-capable integration test at the real stream → TUI reconciliation seam:
   - feed assistant/tool records incrementally;
   - observe live summaries;
   - deliver `backend.output`;
   - assert completed history contains each distinct tool summary exactly once and authoritative final output exactly once;
   - navigate away/back and assert the same history remains.
2. Export the completed iteration and assert it contains the same retained tool summaries.
3. Prove provisional assistant text is removed/reconciled without duplicate final text.
4. Add a backpressure test with many consecutive oversized polls. Assert one coalesced visible status with the correct cumulative skipped-byte count and useful newest lines retained.
5. Cover an oversized single unterminated/NDJSON record and a normal stream with no skip status.
6. Cover cumulative/repeated backend records and prove duplicate renderings are suppressed while distinct same-text tool calls remain distinct when IDs differ.
7. Keep every buffer/read bound enforced; no unbounded raw stream retention in TUI state.
8. Run focused tests for `ralph-adapters` and `ralph-tui`, then full `cargo test` before completion.
9. Inspect the final diff for unrelated changes and run `cargo fmt --all -- --check` plus clippy on touched crates.
10. Perform a manual TUI smoke using a synthetic or live fast-growing stream: current iteration shows useful newest tools with one skip status; previous iteration retains tool history after reconciliation. Paste exact captures into the run evidence.

## Guardrails

- Do not modify autoloop in this worktree.
- Do not simply remove skip notices or raise byte/line limits.
- Do not retain raw unbounded JSONL in memory.
- Do not preserve all provisional assistant text alongside authoritative output.
- Do not deduplicate solely by rendered text when stable tool-call identity exists.
- Do not add retries, sleeps, or source-only placeholder assertions in place of runtime tests.
- Keep changes scoped to the adapter/TUI history model and tests.
- Use small coherent commits and leave no ephemeral artifacts.

Emit `task.complete` / `LOOP_COMPLETE` only after both beads' acceptance criteria and the full repository test gate pass with concrete evidence.
