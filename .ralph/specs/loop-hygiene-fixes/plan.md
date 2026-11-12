# Plan — loop hygiene fixes

Read `context.md` first; it records what the code actually looks like and three traps that cost a full
iteration to rediscover. Acceptance criteria live in `.ralph/specs/loop-hygiene-fixes.spec.md`.

## Ordering rationale

The spec numbers the defects 1-3, but this plan implements them **2 → 3 → 1**. Defect 2 is the only one
that actively corrupts the loop doing the work: every step below runs `cargo test -p ralph-cli`, and until
the leak is fixed each run injects stray topics into this loop's live events file, which can trigger
spurious hat handoffs mid-step. Defect 3 keeps the repo clean for the commits the later steps make. Fixing
the hygiene defects first makes the remaining steps verifiable; the reverse order makes each later run
noisier than the last. Splitting defect 1 into two steps (metrics, then context window) keeps each slice to
one reviewable behavior, though both ride the same `SessionResult`.

## Step 01 — Tests stop writing into the live loop's events file

Spec defect 2. Outcome you could demo: with `RALPH_EVENTS_FILE` and `RALPH_WORKSPACE_ROOT` pointed at a
sentinel file outside the workspace, run all three suites and the sentinel's bytes are byte-identical
afterward.

Build the sentinel check **first** and let it fail — it is the only reliable way to find the `t.x` / `t.y`
sources, which have no static match in the repo. Then make every test that spawns `ralph` or touches an
emit path set its own events target rather than inheriting the caller's, so results don't depend on ambient
env. A shared test helper beats patching call sites one by one, because new tests will otherwise
reintroduce the leak. The known offender to start from is
`crates/ralph-cli/tests/integration_events_isolation.rs:149`; the test right after it already shows the
correct pattern.

Test obligation: a regression test that fails before the fix and asserts the out-of-workspace events file
is untouched by the suites.

Runtime task key: `code-assist:loop-hygiene-fixes:step-01:test-event-isolation`

## Step 02 — The end-of-run auto-commit stops sweeping run artifacts

Spec defect 3. Outcome you could demo: in a scratch repo holding both an intended new source file and
untracked artifacts (nested `.ralph/diagnostics/**`, `logs/**`, a scratchpad), `auto_commit_changes` commits
the source file and leaves the artifacts out.

Two halves, and both are required by the spec:

- No test run creates a `.ralph/` directory inside a crate directory (either tests write runtime state under
  a temp dir, or nested `.ralph/` is ignored — `backend/**/.ralph/` is the in-repo precedent).
- `auto_commit_changes` (`crates/ralph-core/src/git_ops.rs:96`) no longer stages artifacts. Pick the
  mechanism yourself and **explain the choice in the commit message** — the spec asks for that explicitly.
  `.gitignore` also needs `logs/` and `.ralph/specs/**/logs/`; see context.md for why the latter is not
  optional.

Keep `.ralph/agent/memories.md` and `.ralph/agent/decisions.md` committing — the repository's existing
negations settle the spec's open question in favour of tracking them.

Test obligation: a test that fails before the fix, plus a companion test that an intended new source file
is still committed, so the fix cannot degrade into "never add untracked files".

Runtime task key: `code-assist:loop-hygiene-fixes:step-02:auto-commit-artifacts`

## Step 03 — Headless Claude iterations report real cost and tokens

Spec defect 1, first two acceptance criteria. Outcome you could demo: a headless Claude-backend iteration
whose stream carries `total_cost_usd` and usage writes those same numbers into its `iteration.summary`,
adds them to the loop's cumulative cost, and a `max_cost_usd` set below the first iteration's cost
terminates the loop with `max_cost`.

The defect is one gate, not a redesign: `cli_executor.rs:299` only builds a `SessionResult` for Pi-family
streams, so the headless branch at `loop_runner.rs:1909-1948` defaults every metric to zero. `add_cost`,
`record_iteration_tokens` and the `max_cost` comparison downstream are already correct. Remember
`context_tokens` is separately hard-coded to `0` at `loop_runner.rs:1943` — that one keeps
`performance.jsonl` empty even once cost works.

Provisional: this step may reveal that the headless and PTY paths should share one metrics-extraction
helper. Decide that when you see both; don't pre-build the abstraction.

Test obligation: tests that fail before the fix for both the summary values and `max_cost` termination.

Runtime task key: `code-assist:loop-hygiene-fixes:step-03:headless-claude-metrics`

## Step 04 — `context_window` comes from the stream, not a hard-coded default

Spec defect 1, third acceptance criterion. Outcome you could demo: a stream `result` line reporting a
context window for its model produces that value in `iteration.summary`, so a 1M-window model no longer
reports 200000.

The value is in the `result` line under `modelUsage.<model>.contextWindow`, which
`ClaudeStreamEvent::Result` does not deserialize today. `SessionResult.context_window` already exists to
carry it. Fall back to the configured default only when the stream reports nothing.

Provisional: depends on how step 03 routes `SessionResult` into the outcome; the field plumbing may already
be in place by then, leaving only the parser change and the fallback rule.

Test obligation: a test that fails before the fix, asserting the stream-reported window wins over the
200000 default.

Runtime task key: `code-assist:loop-hygiene-fixes:step-04:stream-context-window`

## Plan Changes

(none yet)
