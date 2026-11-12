---
status: approved
created: 2026-11-12
related:
  - https://github.com/mikeyobrien/ralph-orchestrator/pull/417
  - https://github.com/mikeyobrien/ralph-orchestrator/pull/420
---

# Loop hygiene fixes

Three defects found while running ralph loops headlessly and dogfooding ralph on this repo. Each needs a regression test that fails before the fix.

## 1. Cost and token accounting reads zero in headless runs

**Observed.** In `--autonomous` runs on the Claude backend (stream-json output), every `iteration.summary` event in `.ralph/events-*.jsonl` reports `cost_usd: 0.0`, `input_tokens: 0`, `output_tokens: 0`, and `num_turns: 0`, and `.ralph/diagnostics/<ts>/performance.jsonl` is empty. The same run's log contains stream-json lines with `"type":"result"` whose `total_cost_usd` is non-zero (for example $0.29 for one Planner iteration). So `event_loop.max_cost_usd` cannot trigger in these runs. The summaries also report `context_window: 200000` for models with a 1M-token context (`claude-opus-5-5`, `claude-fable-5-2`). The Claude `result` line carries the real value in `modelUsage.<model>.contextWindow`.

**Where to start.** `add_cost` and `record_iteration_tokens` in `crates/ralph-cli/src/loop_runner.rs`; the `iteration.summary` payload; `resolve_context_window_for_backend`; and the Claude stream parser in `crates/ralph-adapters/src/claude_stream.rs`.

**Acceptance criteria**
- Given a Claude-backend iteration whose stream reports `total_cost_usd` and token usage, when the iteration ends, then that iteration's `iteration.summary` carries the same cost and token counts, and the loop's cumulative cost includes it.
- Given `event_loop.max_cost_usd` below the cost of the first iteration, when the first iteration ends, then the loop terminates with `max_cost`.
- Given a stream `result` line that reports a context window for its model, when the summary is written, then `context_window` is that value rather than a hard-coded 200000 default.

## 2. Tests leak events into a live loop's events file

**Observed.** When `cargo test` runs inside a ralph loop (as any hat verifying its work does), stray events with topics such as `test.topic`, `t.x`, and `t.y` appear in the live loop's `.ralph/events-*.jsonl`. One source is `crates/ralph-cli/tests/integration_events_isolation.rs` (about line 149), which spawns `ralph emit test.topic …` without clearing `RALPH_EVENTS_FILE`, while the test after it does clear it. There are other sources, because `t.x` and `t.y` aren't emitted from that file.

**Acceptance criteria**
- Given `RALPH_EVENTS_FILE` (and `RALPH_WORKSPACE_ROOT`) set to a file outside the workspace, when the full test suite runs (`cargo test -p ralph-core`, `-p ralph-adapters`, `-p ralph-cli -- --test-threads=1`), then that file is unchanged afterward.
- Tests that spawn `ralph` or call emit paths set their own events target, so their result does not depend on the caller's environment.

## 3. The end-of-run auto-commit sweeps run artifacts into the branch

**Observed.** `auto_commit_changes` (`crates/ralph-core/src/git_ops.rs`) runs `git add -A`. In a dogfood run on this repo its commit, `chore: auto-commit before merge (loop primary)`, contained 893 artifact files:
- `crates/ralph-core/.ralph/diagnostics/**` and `crates/ralph-cli/.ralph/diagnostics/**`, written by test runs whose working directory is a crate directory;
- `crates/ralph-core/.ralph/agent/scratchpad.md`;
- `logs/**`, which a hat wrote at the repo root.

`.gitignore` ignores nested `.ralph/` under `backend/` and `frontend/`, but not under `crates/`. Decide from the repository whether `.ralph/agent/memories.md` and `.ralph/agent/decisions.md` are meant to be tracked (`CLAUDE.md` calls memories "persistent learning"); keep tracked files tracked.

**Acceptance criteria**
- Given a test run of this workspace, when it finishes, then it has created no `.ralph/` directory inside any crate directory. Either tests write their runtime state under a temp dir, or nested `.ralph/` directories are ignored.
- Given a loop whose workspace contains untracked run artifacts (diagnostics, logs, scratch files), when the end-of-run auto-commit runs, then the commit contains the loop's work but not those artifacts. Choose the mechanism (ignore rules, an exclusion list, or staging only tracked changes plus paths the loop created on purpose) and explain the choice in the commit message.
- The auto-commit still includes intended new source files. Cover that with a test, so the fix doesn't become "never add untracked files" by accident.
