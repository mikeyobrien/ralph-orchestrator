# Context — loop hygiene fixes

## The request

Source type: single approved spec, `.ralph/specs/loop-hygiene-fixes.spec.md` (read it; this file does not
restate its acceptance criteria). Three defects found while dogfooding ralph on this repo:

1. Cost/token accounting reads zero in headless (`--autonomous`) Claude runs, so `max_cost_usd` can't fire,
   and `context_window` is a hard-coded 200000.
2. `cargo test` leaks events (`test.topic`, `t.x`, `t.y`) into the *live* loop's `.ralph/events-*.jsonl`.
3. The end-of-run auto-commit `git add -A` sweeps run artifacts (893 files in one observed commit).

Each fix needs a regression test that fails before the fix.

## Verification commands (from the objective; use these, not variants)

```bash
cargo test -p ralph-core
cargo test -p ralph-adapters
cargo test -p ralph-cli -- --test-threads=1     # MUST be single-threaded, see below
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
./scripts/sync-embedded-files.sh check          # only if a preset or crates/ralph-core/data/ file changes
```

Known-flaky caveat, already triaged: several `ralph-cli` wave/backend tests race on `PATH` when run in
parallel on this machine. It reproduces on `main` and CI runs them fine. Run that crate with
`--test-threads=1` and do **not** treat parallel-only failures as regressions you introduced.

Keep logs in `.ralph/specs/loop-hygiene-fixes/logs/` — never at the repo root (writing `logs/` at the root
is literally defect 3).

## What the code actually looks like (verified this iteration)

### Defect 1 — the zero comes from one gate in the headless path

`crates/ralph-cli/src/loop_runner.rs:1909-1948` is the non-PTY (headless) branch. It builds
`ExecutionOutcome` from `metrics`, which is `result.session_result` from `CliExecutor`. That is
`Option<SessionResult>`, and `crates/ralph-adapters/src/cli_executor.rs:299` only populates it when
`is_pi_family_stream` is true. **Claude stream-json is not Pi-family, so `session_result` is `None` and
every `map_or` default fires: cost 0.0, tokens 0, turns 0.** `context_tokens` is additionally hard-coded
to `0` at line 1943, which is what makes `record_iteration_tokens` a no-op and
`.ralph/diagnostics/<ts>/performance.jsonl` empty.

Downstream plumbing is already correct and needs no change: `loop_runner.rs:2147` calls
`event_loop.add_cost(outcome.total_cost_usd)`, `2143` calls `record_iteration_tokens`, and
`event_loop/mod.rs:692` already compares `cumulative_cost >= max_cost`. The PTY path
(`loop_runner.rs:4540`, `4697`) already reads real values off `PtyExecutionResult`. So this is a
"populate the struct in the headless branch" fix, not a redesign.

`SessionResult` (`crates/ralph-adapters/src/stream_handler.rs:80-101`) already carries every field needed,
including `context_window`.

### Defect 1b — context window

`ClaudeStreamEvent::Result` (`crates/ralph-adapters/src/claude_stream.rs:31-36`) currently deserializes
only `duration_ms`, `total_cost_usd`, `num_turns`, `is_error`. The real window lives in the same line under
`modelUsage.<model>.contextWindow` and is not parsed at all; `resolve_context_window_for_backend` /
`context_window_for_backend` (`loop_runner.rs:65-70`) supplies the 200000 default. Models
`claude-opus-5-5` and `claude-fable-5-2` have 1M windows, so the default is wrong, not just stale.

### Defect 2 — subprocess env inheritance

`crates/ralph-cli/tests/integration_events_isolation.rs:149` spawns `ralph emit test.topic` with
`.current_dir(temp_path)` but **without clearing `RALPH_EVENTS_FILE`**. The child inherits the live loop's
env var, so it writes to the live loop's events file and the assertion on the temp file is also unsound.
The test immediately after it does clear the var — so the pattern to copy already exists in-repo.

`t.x` / `t.y` have no static source: `rg -nF -e 't.x' -e 't.y'` over the whole repo (excluding `target/`)
matches only `*.yml` filenames. They are produced dynamically. **Do not try to enumerate the sources by
reading code — write the sentinel test first and let the failure name them.**

Files that already reference `RALPH_EVENTS_FILE` (candidate sources and existing patterns):
`crates/ralph-cli/tests/integration_events_isolation.rs`,
`crates/ralph-cli/tests/integration_final_iteration_completion.rs`, `crates/ralph-cli/src/wave.rs`,
`crates/ralph-cli/src/script_hat.rs`, `crates/ralph-cli/src/main.rs`, `crates/ralph-cli/src/loop_runner.rs`,
`crates/ralph-adapters/src/{pty_executor,cli_executor}.rs`.

### Defect 3 — gitignore anchoring, and a trap in the objective

`crates/ralph-core/src/git_ops.rs:96-128` is `auto_commit_changes`; `git add -A` is line 108-111.

Root `.gitignore` (lines 106-121) does:

```
.ralph/*
!.ralph/specs/
!.ralph/specs/**
...
!.ralph/agent/decisions.md
!.ralph/agent/memories.md
```

Three consequences, all verified:

- `.ralph/*` contains a slash, so git anchors it to the repo root. Nested `crates/*/.ralph/` is **not**
  ignored. `backend/**/.ralph/` and `frontend/**/.ralph/` are ignored (lines 51-54) — that is the precedent
  to follow for `crates/`.
- `!.ralph/agent/decisions.md` and `!.ralph/agent/memories.md` are explicit negations, so the spec's open
  question is answered by the repository: **memories and decisions are meant to be tracked.** The
  auto-commit fix must keep committing them. `git ls-files` confirms `.ralph/agent/decisions.md` is tracked.
- `!.ralph/specs/**` re-includes everything under specs — **including this spec's own
  `logs/` directory.** Verification logs written where the objective says to write them would be swept into
  the auto-commit. Ignoring `.ralph/specs/**/logs/` is part of the fix, not an extra.

`logs/` at the repo root is not ignored. Only `.logs/` (line 65) and `*.log` (line 72) are, so a hat writing
`logs/report.txt` leaves it untracked-and-committable.

## Constraints

- `CLAUDE.md`: run `cargo test` before declaring anything done; backwards compatibility is explicitly not a
  goal (don't add compat shims); don't commit ephemeral files; commit atomically with the why.
- Spec 3 requires the *reason* for the chosen mechanism in the auto-commit commit message.
- Each fix needs a test that fails before it. Capture the red run in `logs/` as evidence.
