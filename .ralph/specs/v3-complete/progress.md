# Progress

## Current Step

Steps 3, 4, 5, and 7 are closed (2026-09-23). Step 5 retired the dashboard's
live loop view on the operator's decision. Step 6 (`landing-untracked-sweep-yxv`,
scope the landing auto-commit) is next and has no runtime wave yet.

## Active Wave

Step 3 wave, on loop `primary-20260922-170542`:

- `code-assist:v3-complete:step-03:relocate-hat-registry` (`task-1790109144-0d0c`, priority 1, closed at 21:37 after `queue.advance`)
- `code-assist:v3-complete:step-03:delete-event-bus` (`task-1790109144-2c18`, priority 2, closed at 22:20 after `queue.advance`, committed `8c71400`)
- `fix:fake-autoloop-etxtbsy-flake` (`task-1790110404-2974`, priority 3, closed at 23:24 after `queue.advance`, committed `c390411`)
- `fix:autoloop-health-probe-etxtbsy` (`task-1790119597-6a5c`, priority 3, open, ready, advanced now)
- `code-assist:v3-complete:step-03:verify-remnant-and-engine-rejection` (`task-1790109144-5333`, priority 3, open, blocked-by `0d0c`, `2c18`, and `6a5c`, next behind the health row)

Wave order this pass, by priority then step role:

1. `code-assist:v3-complete:step-03:relocate-hat-registry` (`task-1790109144-0d0c`, P1) - closed, reviewed, committed at `96b7bb1`.
2. `code-assist:v3-complete:step-03:delete-event-bus` (`task-1790109144-2c18`, P2) - closed at 22:20, committed `8c71400`.
3. `fix:fake-autoloop-etxtbsy-flake` (`task-1790110404-2974`, P3) - closed at 23:24, committed `c390411`.
4. `fix:autoloop-health-probe-etxtbsy` (`task-1790119597-6a5c`, P3) - advanced now, so the gate verifies a suite whose busy-exec site is closed.
5. `code-assist:v3-complete:step-03:verify-remnant-and-engine-rejection` (`task-1790109144-5333`, P3) - the step gate, next after the health row.

Parked, not in this wave:

- `task-1790101592-f09a` (P1, autoloop 0.11.0 engine state path drift) is a
  carried Step 12 row, and its own description names that owner. Its step is not
  current, so the planner does not publish it and the Builder must not work
  ahead of the queue.

`task-1790110404-2974` was parked in the previous pass only because `2c18` was
still open, and it closed at 23:24 after the Finalizer re-ran acceptance,
falsified its flake with 30 green full-suite samples, and pushed
`c86e413..c390411`. `task-1790119597-6a5c` is the same ETXTBSY class at the
health probe, surfaced by that Finalizer pass as a new Planner finding. It is the
wave's fourth row because the gate's clause (c) runs the suite it flakes under,
so landing it after the gate would leave the gate's pasted evidence measured
against a tree that no longer exists.

## Step 2 wave (closed)

All five Step 2 rows closed, plus the operator steer:

- `code-assist:v3-complete:step-02:classify-rollup-delta` (`task-1790097921-986e`, priority 1, closed after one rejection correction, committed `f4daacb`)
- `code-assist:v3-complete:step-02:port-tui-stream-history` (`task-1790097924-b4fc`, priority 2, closed at 17:59 after `review.passed`, committed `5fd8828`)
- `code-assist:v3-complete:step-02:port-live-harness-smoke` (`task-1790097924-c8bd`, priority 2, closed at 18:27 after `review.passed`, committed `b062e52`)
- `fix:v3-complete:baseline-gate-breaks` (`task-1790099906-72c2`, priority 2, closed at `635cb8c` after `review.passed`, four commits)
- `code-assist:v3-complete:step-02:verify-reconciliation` (`task-1790097924-daa7`, priority 3, closed after eight revisions and a `review.passed` at `d776163`)
- `operator-steer:v3-complete:record-rfc-86` (`task-1790100842-ebbb`, priority 1, closed at `040a16f` after `review.passed`)

`fix:v3-complete:baseline-gate-breaks` (`task-1790099906-72c2`, P2) closed at
`635cb8c` (fmt exit 0, workspace clippy exit 0, `cargo test -p ralph-cli
--no-fail-fast` exit 0 over 32 targets and 588 passed, `cargo test -p
ralph-core` exit 0, no `crates/ralph-cli/.ralph` artifact after the suite).

The finalizer's event said "step 2 stays open". That reflected the open `f09a`
row, which `plan.md` assigns to Step 12, not the Step 2 wave. With `daa7` and
`ebbb` closed, Step 2 has no open runtime row, so the planner closes the step
and opens Step 3. `f09a` keeps its Step 12 owner and stays parked.


## Step 1 wave (closed)

All five tasks closed on loop `primary-20260922-170542`:

- `code-assist:v3-complete:step-01:verify-engine-flip` (`task-1790091216-0665`, priority 1, closed)
- `code-assist:v3-complete:step-01:verify-remnant-and-drift` (`task-1790091216-4015`, closed after one rejection correction)
- `code-assist:v3-complete:step-01:verify-gate-and-branches` (`task-1790091216-6caf`, closed)
- `code-assist:v3-complete:step-01:verify-bead-claims` (`task-1790091216-f8a7`, closed)
- `code-assist:v3-complete:step-01:author-audit` (`task-1790091219-9fa0`, closed, committed `80309dc`)

Evidence logs land in `.ralph/specs/v3-complete/logs/step-01-*.md`. These are
runtime artifacts and are not committed. The committed deliverable is
`.ralph/specs/v3-completion-audit.md`.

## Verification Notes

- Working branch is `v3/complete` at the merge commit `df2449a`, parents
  `22fc1fd` (v3) and `351b9f6` (main). Main is already merged. Do not re-merge.
- Rollup delta re-measured this turn: 22 commits, not the 20 the stalled notes
  claimed. Base delta is 8 commits. Use the enumerated list.
- The autoloop engine is installed at
  `/home/mobrienv/.npm-global/bin/autoloop`. Confirm the version before Step 12.
- Focused gate after Step 1: `cargo test -p ralph-cli -p ralph-core` is
  `411 passed; 2 failed`, the same count as the brief's baseline. The two
  failing names differ from the brief's pair. They are `tests::
  test_run_command_allows_single_file_combined_config` and `tests::
  test_run_command_dry_run_inline_prompt_skips_execution`, and both fail with
  `backend "claude" cannot receive CLI args ["--provider", "spark",
  "--model", "GLM-5.3-Flash-EXL3"]`. That is this machine's local default
  model and provider, not a code defect. Step 1 made no source edits, so these
  cannot be regressions. Re-check them once Step 9 locks the Jev routing story.
- No beads CLI exists. `.beads/issues.jsonl` is hand-edited with the schema
  preserved, and the tracker update rides in the same commit as its change.
- `just` is not installed. Use `cargo` directly, or `./scripts/ci-rust-gate.sh`.
- Ralph runtime files under `.ralph/agent/` are never committed. `context.md`,
  `plan.md`, `progress.md`, and `logs/` under `.ralph/specs/v3-complete/` are
  runtime too. Only `.ralph/specs/v3-completion-audit.md` and
  `.ralph/specs/v3-ga-readiness.spec.md` are committed deliverables.
- Re-homing note: the stalled run created the Step 1 wave under dead loop
  `primary-20260922-153151`. `ralph tools task ready` filters by the current
  loop, so those rows were invisible. They were re-homed to
  `primary-20260922-170542` with the schema preserved. Backup at
  `/var/tmp/tasks.jsonl.bak-*`. `ensure` dedupes by key globally and does not
  re-home, so use `task add` for any future wave whose key already exists.

## Completed Steps

### classify-rollup-delta (task-1790097921-986e, priority 1)

Verdict: the rollup carries two wanted clusters. Record committed at `44afa19`,
`.ralph/specs/v3-reconciliation.md`, 143 insertions.

- Deltas re-measured this turn on a non-shallow clone: 22 rollup-only and 8
  base-only. Every base-only commit is an ancestor of `HEAD` by
  `git merge-base --is-ancestor`, so no base work is lost.
- Classification of the 22: 16 `port`, 4 `already-covered`, 2 `superseded`.
- The wanted clusters are TUI stream history and backpressure, and the live
  harness smoke preset. The TUI port is load-bearing:
  `crates/ralph-tui/src/autoloop_source.rs` is 1331 lines in `HEAD` against 2053
  in the rollup, and `crates/ralph-adapters/src/backend_stream_tailer.rs` is 514
  against 881.
- Two corrections that prevent a wrong port. `crates/ralph-adapters/src/autoloop_runner.rs`
  is 1032 lines in `HEAD` against the rollup's 896, because the rollup predates
  the native resume work. Porting that file from the rollup would regress
  `AutoloopRunner::resume`. `crates/ralph-core/src/engine_state.rs` is 419 lines
  in `HEAD` against 377, so the Ralph-owned state work is already covered and
  the rollup's `c90001e` merge carried only a two-line doc edit.
- The unified backend catalog is already present from the main merge:
  `crates/ralph-core/src/backend.rs`, from `edc2b32`. Phase 2c must build on it.

The empty `review.rejected` that opened this iteration named no task or
artifact. DEC-013 records opening Step 2 rather than reopening a verified Step 1
task.

#### Rejection correction on the port mechanism (review.rejected, addressed)

The Critic re-measured everything and confirmed the classification sound: the
22-row table matches `git log` order and hashes, the tally is 16 port / 4
already-covered / 2 superseded, the counts are 22 and 8 on a non-shallow clone,
all 8 base-only commits are ancestors of `HEAD`, and every line count and
file-absence claim reproduces. It rejected the record for the port mechanism
only. Three defects, all real, all fixed in the record at `f4daacb`.

1. The overlap claim was false. The record said the files changed on both sides
   since `2e1fc52` are `autoloop_source.rs` and `backend_stream_tailer.rs`.
   `comm -12` of the two name lists returns `Cargo.lock` only, and
   `git diff --stat 2e1fc52 HEAD` on both TUI files is empty.
2. The port plan was inapplicable. Replay conflicts. `git cherry-pick de2eaa4`
   applies, then `e275303` conflicts in `autoloop_source.rs`, where the rollup
   hunk replaces `reader_engine_root = engine_state_root(&workspace)` with a
   plain `workspace.clone()`, dropping the `.ralph/autoloop` state-root
   threading. `5b7876c` alone conflicts in three paths, including modify/delete
   on `.ralph/tasks/tui-stream-history-backpressure.code-task.md`. The stack is
   linear on `aff233d`, where the file is 1258 lines, and the base branch grew
   it to 1331 at `2e1fc52`. The record now specifies taking the rollup tip
   content of each wanted path, with the losslessness argument measured:
   `2e1fc52` is an ancestor of both tips and `Cargo.lock` is the only file both
   sides changed after it.
3. `6e2545d` merges 10 leaf commits, not the eleven the evidence cell claimed.

Measured compile and test evidence for the replacement mechanism, in a detached
worktree at `44afa19` with the rollup tip content of both TUI files and the
`sha2` line: `cargo check -p ralph-tui -p ralph-adapters` finished in 12.66s and
`cargo test -p ralph-adapters -p ralph-tui` passed 382 + 273 with 0 failures.
`Cargo.lock` does change: `sha2` is already resolved at `Cargo.lock:3657`, but
the `ralph-adapters` edge is new, so `cargo` adds `"sha2"` to the
`ralph-adapters` `dependencies` block and that lock update is committed with the
`Cargo.toml` line. Corrected after the Fresh-Eyes Critic measured
`cargo check --locked` exiting 101 without it.
Raw transcript at `.ralph/specs/v3-complete/logs/step-02-port-mechanism.md`.

Two follow-on corrections. The swept code-task doc is now an explicit port
decision: port it, since it is the acceptance spec for the TUI work. And
`task-1790097924-b4fc`'s description was rewritten to the final-content
mechanism, because it had inherited the replay plan verbatim. No source edits.

### verify-gate-and-branches (task-1790091216-6caf, priority 2)

Verdict: all four claims confirmed. Evidence log
`.ralph/specs/v3-complete/logs/step-01-gate-branches.md`.

- Release gate: `release_gate: .ralph/specs/v3-ga-readiness.spec.md` at
  `.ralph/specs/v3-autoloops-cutover.spec.md:10`. The path is ABSENT on
  `origin/integration/v3-prerelease`, `origin/wip/v3-prerelease-rollup`, and
  `origin/main` by `git cat-file -e`. Clone is not shallow.
- Deltas, measured after `git fetch`: 22 rollup-only and 8 base-only. The task
  text's claim that the brief says 14 is wrong; the brief says 22 and the tree
  agrees. The rollup-only list includes merge `1e67e52`
  (`chore: auto-commit before merge (loop primary)`), which is live evidence for
  the landing sweep bead.
- Census: 43 rows, 36 closed, 6 open, 1 tombstone. Open ids are `a7e`, `a7e.8`,
  `a7e.10`, `ga3-c4-dashboard-dead-svf`, `landing-untracked-sweep-yxv`,
  `tui-help-wave-stale-5hu`.

No source edits.

### verify-bead-claims (task-1790091216-f8a7, priority 2)

Verdict: all three beads are `genuinely-open`. Evidence log
`.ralph/specs/v3-complete/logs/step-01-bead-claims.md`.

- Dashboard: watcher at `crates/ralph-api/src/event_watcher.rs:40` and `:100`,
  `EventLogger` at `crates/ralph-core/src/event_logger.rs:111` with
  `#[cfg(test)]` at `:265` and zero production callers, Node stdout parser at
  `backend/ralph-web-server/src/runner/RalphEventParser.ts:40` wired live at
  `RalphTaskHandler.ts:83`. Correction: the live engine stream is
  `.ralph/autoloop/events.ndjson`, not `.ralph/autoloop-events.ndjson`.
- Landing: `git add -A` at `crates/ralph-core/src/git_ops.rs:182`, called from
  `crates/ralph-core/src/landing.rs:137`. No allowlist; only `.gitignore`
  protects. Correction: the sweep line is not in `landing.rs`.
- TUI help: section at `crates/ralph-tui/src/widgets/help.rs:114` and a live
  keybinding at `crates/ralph-tui/src/input.rs:98`. Correction: it is not
  help-text only, and no production producer of `WaveStarted` exists.

No source edits.

### author-audit (task-1790091219-9fa0, priority 2)

`.ralph/specs/v3-completion-audit.md` authored and committed at `80309dc`,
verified with `git log -1 --stat` (`1 file changed, 454 insertions`). The commit
carries only the audit. `.ralph/agent/` files and the evidence logs stay
uncommitted, and `PROMPT-V3-COMPLETE.md` was unstaged so it stays untracked.

Seven rows: `a7e` genuinely-open, `a7e.8` genuinely-open, `a7e.10`
genuinely-open with a stale tracker scope, `ga3-c4-dashboard-dead-svf`
genuinely-open, `landing-untracked-sweep-yxv` genuinely-open,
`tui-help-wave-stale-5hu` genuinely-open, `vp6` landed.

### verify-engine-flip (task-1790091216-0665, priority 1)

Verdict: landed. All four claims verified with command plus output.

- `config.rs:512` rejects any `core.engine` other than `autoloop`;
  `config.rs:2360` holds the v3 message.
- `cargo test -p ralph-core core_engine_rejects_removed_ralph_engine`:
  `1 passed; 0 failed`, exit 0.
- `grep -rn 'run_loop_impl\|EventLoop::new' crates/ --include=*.rs` is empty,
  exit 1. The only `EventLoop` hit is a comment at
  `crates/ralph-bench/src/main.rs:256`. No `event_loop/`, `hatless_ralph.rs`,
  `event_bus`, or `wave_*` module remains in `ralph-core/src`.
- `crates/ralph-core/src/hat_registry.rs` present (15082 bytes), exported at
  `crates/ralph-core/src/lib.rs:26` and `:84`.

Brief line numbers are stale after the main merge. The live lines are 512 and
2689, not 2188 and 2434.

Evidence: `.ralph/specs/v3-complete/logs/step-01-engine-flip.md`.
No source edits, per the task's own constraint.

### verify-remnant-and-drift (task-1790091216-4015, priority 2)

Verdict: three claims confirmed, one corrected.

- `crates/ralph-core/src/hat_registry.rs` is 468 lines with exactly one
  production consumer, `crates/ralph-cli/src/hats.rs` (the `ralph hats` command).
  All other hits sit below `hats.rs:1127` inside `#[cfg(test)]`. a7e.10 is a
  relocate-or-rework, not a delete.
- `event_loop/`, `hatless_ralph.rs`, and `wave_*` are absent tree-wide.
  `crates/ralph-proto/src/event_bus.rs` is **present** at 401 lines with zero
  production callers, still declared and re-exported at
  `ralph-proto/src/lib.rs:15` and `:25`. The brief's "gone from the tree" claim
  is false. The engine leaves two remnants, not one.
- autoloop issues #34, #35, #37, #38, #39 are closed and PRs #40, #41, #42 are
  merged, each with its URL captured. Published and installed engine is 0.11.0.
- Drift: `autoloop_preset_gen.rs:158` and `:212` say 0.10.x, and
  `autoloop_health.rs:12`/`:15` hold `0.10.0`/`0.10.1`, against an installed
  0.11.0. Whether 0.11.0 breaks a mapping is unverified and belongs to Step 12.

Evidence: `.ralph/specs/v3-complete/logs/step-01-remnant-drift.md`.
No source edits.

Rejection note: this iteration was triggered by an empty `review.rejected`. It
named no task or artifact. DEC-012 records advancing to this ready task rather
than reopening the verified `verify-engine-flip` task.

#### Rejection correction on Claim 4 (review.rejected, addressed)

The Critic accepted all four acceptance claims and rejected the artifact for two
sentences in Claim 4. Both were real errors, and both are fixed in the artifact.

1. The artifact said `doctor.rs:695` and `:710` repeat the `0.10.0` figure in
   operator-facing messages. They do not. `doctor.rs:665` opens
   `#[cfg(test)] mod tests`, so both lines are fixtures inside
   `doctor_keeps_autoloop_check_for_explicit_display`. The production surfaces
   interpolate the constants: `preflight.rs:450`, `main.rs:1325`, `:1328`,
   `:1336`, `engine_install.rs:212`, and `engine_provision.rs:54`, `:60`
   (its `#[cfg(test)]` opens at `:105`, so both are production). Verified by
   grep this iteration. Consequence: correcting
   `MIN_AUTOLOOP_VERSION`/`VENDORED_AUTOLOOP_VERSION` fixes every operator
   message, and `doctor.rs` needs no change for this reason.
2. The artifact called the floor "two minor versions behind". `0.10.0` against
   `0.11.0` is one minor version.

The corrected artifact now carries a command and output excerpt for the
production surfaces, so the audit cannot inherit the false citation. No source
edits. The four acceptance claims and every code finding stand unchanged.

## 2026-09-22, Step 2 wave, finalizer pass on the stream-history port

The pending event was `review.passed` for `task-1790097924-b4fc` at `5fd8828`,
confidence 90. I re-verified the increment from a fresh shell rather than
accepting the payload.

### What I measured this turn

- Content identity inside the commit, not in the worktree. `git show
  origin/wip/v3-prerelease-rollup:PATH` against `git show 5fd8828:PATH`:
  `backend_stream_tailer.rs`, `ralph-adapters/Cargo.toml`, and the 83-line
  code-task doc are byte-identical (sha256 match). `autoloop_source.rs` differs
  by exactly `189 0`, the new render-under-load test and its helper, which is
  the one hunk the record left open.
- Focused gate: `cargo test -p ralph-adapters -p ralph-tui` exits 0, 725 passed
  and 0 failed across all targets and doctests. Raw transcript at
  `logs/final-tui-adapters.txt`.
- The render proof actually runs: `autoloop_source::tests::
  render_under_load_keeps_one_truthful_status_and_newest_lines` passes, 1
  passed 0 failed. My first filter matched 0 tests because `tail` cut the
  ralph-tui lib target out of the combined output; the full-path filter is the
  honest invocation.
- Real harness: `cargo test -p ralph-cli --test
  integration_autoloop_tui_live_stream` exits 0, 2 passed 0 failed. That is the
  real `ralph` binary in a PTY against a fixture fake autoloop, asserting on
  vt100-parsed cells.
- `cargo check --workspace --all-targets` exits 0, so the `StreamLine` contract
  extension (`ToolSummary { text, identity }`, `Backpressure`) breaks no
  consumer. Two non-fatal warnings remain in the `ralph-cli` bin test target,
  pre-existing.
- `cargo fmt --check -p ralph-tui -p ralph-adapters` exits 0 and
  `cargo clippy -p ralph-tui -p ralph-adapters --all-targets -- -D warnings`
  exits 0.

### The adversarial pass

The failure mode this port could hide is the one the reconciliation record was
built to prevent: dropping the Ralph-owned `.ralph/autoloop` state root. I
checked it directly. `autoloop_source.rs` carries 23 `engine_state_root` call
sites including the reader seam at `:152` and the fixture assertions at `:827`
and `:1253`, and the only `.autoloop` literal in the ported reader at `:1795`
is `assert!(!workspace.join(".autoloop").exists())`. The one other hit,
`backend_stream_tailer.rs:545`, sits inside `#[cfg(test)] mod tests` (opens at
`:454`) and is a fixture path for the path-redaction test. No production code
creates a top-level `.autoloop`.

### Baseline breaks, re-measured, unchanged

- `cargo fmt --all -- --check` exits 1 with exactly 5 hunks, all in
  `crates/ralph-cli/src/doctor.rs` (785, 797, 874, 885, 893).
- `cargo clippy --all-targets --all-features -- -D warnings` exits 101 with the
  single error `called .ok().is_some_and(..) on a Result value` at
  `crates/ralph-api/src/stream_domain/mod.rs:229`, which aborts the workspace
  pass before `ralph-cli` is checked.

Both are owned by `task-1790099906-72c2`, still open. The Critic's advisory that
this task's inventory is incomplete (two `unused_mut` sites at
`autoloop_preset_gen.rs:584` and `:650` surface once `ralph-api` stops aborting
the run) is recorded as `mem-1790100519-7703` and in the scratchpad, so the repair
re-runs the full gate instead of trusting a two-item list.

### Decision

`queue.advance`. The reviewed task is closed and the increment is real. Step 2
still holds two ready tasks, `port-live-harness-smoke` and the baseline gate
repair, and `verify-reconciliation` is blocked by the former. Steps 3 through 14
remain untouched in `plan.md`. Nothing here justifies `LOOP_COMPLETE`, and
`finalization.failed` would be wrong because the reviewed work is complete.

No source edits this iteration. HEAD is `5fd8828`, matching `origin/v3/complete`.


## Step 2 wave, port-live-harness-smoke landed

Executed `task-1790097924-c8bd`. Commit `b062e52`, 20 files, 2066 insertions.
Rollup tip content for every ported path, with one required adaptation.

### What landed

`presets/live-harness-smoke/` (12 files), `tools/smoke-live-harnesses.sh`,
`tools/smoke_live_harness_results.py`, `tools/smoke_process_group.py`,
`tools/tests/test_smoke_live_harnesses.py`,
`.ralph/specs/manual-live-harness-smoke.spec.md`,
`.ralph/tasks/manual-live-harness-smoke.code-task.md`, the `presets/README.md`
section (+7), and the `engine-contract` CI step (+6).

### Deviations from rollup tip content, measured

`git show origin/wip/v3-prerelease-rollup:<path> | diff - <(git show HEAD:<path>)`
was run per path after the commit. Twelve paths are byte-identical:
`autoloops.toml`, `DOGFOOD.md`, `harness.md`, all six `roles/*.md`,
`topology.toml`, `.ralph/specs/manual-live-harness-smoke.spec.md`, and
`presets/README.md`. Six paths differ, each for a named reason.

1. **State root, four files.** The rollup reads the journal and evidence from a
   top-level `.autoloop`. Ralph roots engine state at `.ralph/autoloop`
   (`crates/ralph-core/src/engine_state.rs:16`, `:156`), so the runner, the
   lifecycle gate, the preset README, and the Ralph-launched test fixtures now
   read the Ralph-owned root. `tools/smoke-live-harnesses.sh` names it once as
   `ENGINE_STATE_DIR="$WORKSPACE/.ralph/autoloop"`. The two test cases that
   invoke `autoloop run` directly keep `.autoloop`, because that is where the
   engine writes without a Ralph launch, and a comment says so.
2. **Three analyzer findings on the ported bytes**, all behaviour preserving.
   `tools/smoke_live_harness_results.py` imports the dataclass helper as
   `dataclass_field` so the module-level record accessor named `field` does not
   shadow it, and `iteration_number` guards `None` before `int()`.
   `tools/smoke_process_group.py` drops an unused `import sys` and uses
   `contextlib.suppress`. `presets/live-harness-smoke/scripts/require_smoke_handoff.py`
   imports `NoReturn` instead of annotating with the string `"NoReturn"`, which
   was never imported.

### Red capability

The RED run added the test file and the preset only. The four
`FakeRunnerIntegration` cases failed for the expected reason: the runner and the
parser did not exist yet.

```text
AssertionError: 127 != 0 : bash: .../tools/smoke-live-harnesses.sh: No such file or directory
AssertionError: 'failed lifecycle hook' not found in ".../smoke_live_harness_results.py': [Errno 2] No such file or directory"
Ran 10 tests in 3.344s
FAILED (failures=10)
```

Transcript at `logs/step-02-smoke-red.txt`. The six cases that exercise the
preset and the real engine passed in RED, which is correct: those artifacts were
already present.

### Green

```text
PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses
Ran 10 tests in 5.322s
OK
```

Transcript at `logs/step-02-smoke-green.txt`. The six-row table prints
`claude claude-sdk`, `codex command`, `opencode command`, `pi pi`,
`hermes acp`, `kiro acp`, all PASS.

Coverage of the ten tests. `FakeRunnerIntegration` drives the real runner and
parser: six passes with no paid executable, aggregated auth preflight failures
with no workspace created and no backend launched, process-group termination of a
paid descendant on timeout, and the fail-closed parser matrix (missing response,
missing handoff, generic completion only, backend failure, failed lifecycle gate,
malformed journal, watchdog timeout, missing/duplicate/out-of-order evidence).
`PromptProbeContract` executes every rendered probe command and asserts the
evidence file content, and asserts the harness never mentions
`AUTOLOOP_STATE_DIR`. `HandoffGateContract` runs the real gate against every
provider turn and against a missing handoff. `NativePresetSelectionIntegration`
runs the installed **autoloop 0.11.0** engine and asserts all six per-role
backend overrides survive native parsing, that the blocking gate hook stops a run
before a second backend launch, and that the rendered prompt contains the run
scoped evidence path.

Other checks: `bash -n tools/smoke-live-harnesses.sh` clean, `py_compile` clean
on all four new Python files, and the committed file mode of the runner is 755.

### The live six-provider run is not claimed, for two independent reasons

**Reason 1, absent providers.** The runner's preflight resolves `ralph`,
`autoloop`, `git`, `claude`, `codex`, `opencode`, `pi`, `hermes`, `kiro-cli`, and
`python3`. On this machine `command -v` finds only `codex` and `pi`; `claude`,
`opencode`, `hermes`, and `kiro-cli` are missing. The runner is fail-closed by
design, so it exits 1 at preflight with no workspace and no backend launch. That
is the ported behavior working, not a defect.

**Reason 2, a measured engine drift that blocks the layout anyway.** See below.
Even with all six providers authenticated, the runner could not read the journal
from a Ralph-launched run.

### The drift, measured with no Ralph involved

autoloop 0.11.0 resolves the four store paths with
`path.join(workDir, configuredPath)`. `path.join` concatenates rather than
replacing an absolute second argument, so Ralph's absolute overrides from
`engine_config_overrides` land under `<work>/<absolute-path>/...`.

```text
## absolute overrides (the shape Ralph passes today)
--set core.journal_file=/var/tmp/drift-repro/absolute/work/.ralph/autoloop/journal.jsonl
journal: /var/tmp/drift-repro/absolute/work/var/tmp/drift-repro/absolute/work/.ralph/autoloop/journal.jsonl

## workspace-relative overrides
--set core.journal_file=.ralph/autoloop/journal.jsonl
journal: /var/tmp/drift-repro/relative/work/.ralph/autoloop/journal.jsonl

## installed engine
0.11.0
```

Transcript at `logs/step-02-engine-state-drift.log`. A real Ralph-launched run of
the smoke preset reproduced it end to end: the engine aborted at iteration 1
because the post_iteration gate could not read
`<work>/.ralph/autoloop/journal.jsonl`, which the engine had written to
`<work>/var/tmp/<basename>/.ralph/autoloop/journal.jsonl`. The relative form
lands journal, memory, registry, run dir, emit tool, and evidence all under
`<work>/.ralph/autoloop`, with nothing in the preset tree.

The Rust tests do not catch this because they drive the fake autoloop shell
fixture, which honors the configured paths directly.

### Scope call

`.ralph/specs/v3-complete/plan.md` Step 12 owns "0.11.0 drift on budgets, backend
keys, or stopReason is fixed or recorded". The Builder hat forbids working ahead
of the queue, and the fix spans launch, resume, TUI observation, diagnostics, and
parallel worktree loops, which need Step 12's own verification. The drift is
filed as `task-1790101592-f09a` (P1) with the reproduction, the verified fix
shape, and the verification list. Memory `mem-1790101577-86d1`. Decision DEC-016,
confidence 74.

### Queue

`task-1790097924-c8bd` closes. `verify-reconciliation`
(`task-1790097924-daa7`) is now unblocked from this side and still waits on
`task-1790099906-72c2`, the baseline gate repair. `task-1790101592-f09a` is new
and ready.

## 2026-09-22, Step 2 wave, baseline gate repair (task-1790099906-72c2)

### What the gate said before, measured at HEAD `b062e52`

| Gate | Result | Evidence |
|---|---|---|
| `cargo fmt --all -- --check` | exit 1, five hunks in `crates/ralph-cli/src/doctor.rs` (785, 797, 874, 885, 893) | `logs/step-02-baseline-fmt.log` |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 101 at `crates/ralph-api/src/stream_domain/mod.rs:229`, `manual_is_variant_and` | `logs/step-02-baseline-clippy.log` |
| clippy after the ralph-api fix | exit 101, two `unused_mut` at `autoloop_preset_gen.rs:584` and `:650` | `logs/step-02-clippy-after-api-fix.log` |
| `cargo test -p ralph-cli --no-fail-fast` | exit 101, 15 failures across 8 targets | `logs/step-02-baseline-ralph-cli-all-targets.log` |
| full-suite artifact | `crates/ralph-cli/.ralph/agent/summary.md` created (untracked, not ignored) | pre/post `git status` diff |

The first `cargo test -p ralph-cli` run reports only 3 failures because cargo
fails fast across targets: the unit binary failed, so no integration target ran.
`--no-fail-fast` is required to see the real inventory. This is why the earlier
handoff's "411 passed, 2 failed" undercounted.

### The 15 test failures, classified by root cause

Thirteen share one cause. `ralph` merges `~/.ralph/config.yml` into every run, and
this machine's user config sets `cli.backend: pi` with
`cli.args: ["--provider", "spark", "--model", "GLM-5.3-Flash-EXL3"]`. Every test
that spawned the binary with the operator's HOME inherited those args and then
failed validation against its own `--backend claude`:

```text
Error: backend "claude" cannot receive CLI args ["--provider", "spark", "--model", "GLM-5.3-Flash-EXL3"]
through the claude-sdk backend; use the custom backend or an explicit core.autoloop_preset
```

- `main.rs` unit: `test_run_command_dry_run_inline_prompt_skips_execution`,
  `test_run_command_allows_single_file_combined_config`
- `integration_config_precedence`: 4 tests
- `integration_hooks_validate`: `test_hooks_validate_json_success_report_and_exit_code`
  (ambient hooks merge in, so `checked_hooks` is 3, not 1)
- `integration_preflight`: `configured_hooks_warning_is_visible_in_run_and_doctor_but_absent_without_hooks`
- `integration_preset`: 2 tests
- `integration_run`: `test_run_dry_run_succeeds`
- `integration_run_presets`: 2 tests

One is a git-identity dependence.
`integration_autoloop_headless_voice::headless_run_uses_ralph_voice_and_gates_engine_noise_by_verbosity`
already isolates HOME, so landing's auto-commit had no identity and emitted
`WARN ralph_core::landing: Auto-commit failed during landing loop_id=primary
error=Git config missing: user.name or user.email not configured`, which the
recorded snapshot does not contain. Proven by re-running with
`GIT_AUTHOR_*`/`GIT_COMMITTER_*` set: exit 0.

One is a real test race.
`autoloop_robot::tests::relays_pending_ask_response_and_guidance_through_control_cli_once`
failed 2 of 3 isolated runs at `autoloop_robot.rs:515`. `run_bridge_inner`
captures the human-events file length at startup (`:158`) and tails from there, so
guidance written while the bridge was starting counted as history and was never
forwarded. The test wrote guidance immediately after spawning the bridge.

The summary artifact had its own root cause. `coordinate_completion` used
`SummaryWriter::default()`, which resolves `.ralph/agent/summary.md` against the
process cwd. Reproduced from one test:

```text
$ rm -rf crates/ralph-cli/.ralph
$ cargo test -p ralph-cli --bin ralph coordinate_without_context_is_a_noop_for_merge_state
test result: ok. 1 passed
$ find crates/ralph-cli/.ralph -type f
crates/ralph-cli/.ralph/agent/summary.md
```

### What changed, four commits

| Commit | Change |
|---|---|
| `e9af61f` | fmt: rustfmt output for `doctor.rs`. clippy: `is_ok_and` at `stream_domain/mod.rs:229`, and drop two `unused_mut` in `autoloop_preset_gen.rs`. |
| `0f603ef` | `coordinate_completion` writes the summary through `SummaryWriter::from_context`, which also threads the loop's own events file. No loop context means no summary home, so nothing is written under the cwd. |
| `e837317` | `tests/support/mod.rs` gives each test binary one empty temp HOME. Seven spawning helpers use it, the headless-voice test gets an explicit git identity, and the two non-hermetic in-process `run_command` tests are replaced by hermetic integration coverage of the same behaviors. |
| `635cb8c` | The relay test waits for the bridge to take the pending question before writing guidance, then waits for both relayed lines. Assertions unchanged. |

### Verification, after the fix

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0, whole workspace, no early abort |
| `cargo test -p ralph-cli --no-fail-fast` | exit 0, 32 test targets ok, 0 failed |
| `cargo test -p ralph-core` | exit 0, 751 passed in the lib target |
| relay test, 10 isolated runs | 10 pass, 0 fail |
| artifact check after the full suite | `git status` diff empty: no `crates/ralph-cli/.ralph/`, no `*.snap.new` |

Logs: `logs/step-02-final-clippy.log`, `logs/step-02-final-ralph-cli.log`,
`logs/step-02-after-fix-ralph-core.log`.

### Decisions

DEC-018, confidence 78: resolve the summary path from the loop context and write
none when no context is present. DEC-019, confidence 75: the ambient-state
dependence is part of this gate repair, so fix it across all eight failing
targets rather than the two the task named. The task description itself says to
re-run the full gate instead of trusting a two-item inventory.

### Not done here

`crates/ralph-cli/.ralph/` is deliberately left out of `.gitignore`. The writer is
fixed at its root, so an ignore rule would only hide a future leak.

### Queue

`task-1790099906-72c2` closes. `verify-reconciliation`
(`task-1790097924-daa7`, P3) is the step gate and is next in the wave. The
operator steer `task-1790100842-ebbb` (P1) follows it. `task-1790101592-f09a`
(P1) stays parked for plan Step 12.

## 2026-09-22, Step 2 wave, planner pass on the queue after the gate repair (queue.advance)

Planner activation on `queue.advance` after `72c2` closed at `635cb8c`. I read
`<ready-tasks>`, `plan.md`, `progress.md`, and `.ralph/agent/tasks.jsonl` rather
than trusting the event summary.

### State read, not assumed

- `ralph tools task ready --format table` returns three rows: `daa7`, `ebbb`,
  `f09a`. All three are open on loop `primary-20260922-170542`, and nothing is
  `in_progress`.
- `daa7`'s `blocked_by` (`b4fc`, `c8bd`) are both closed, so it is genuinely
  unblocked. It is the only remaining row with a
  `code-assist:v3-complete:step-02:*` key.
- `72c2` is closed, so the wave has ready work and this pass routes rather than
  materializes. No runtime task is created.

### Routing: `daa7`, not `ebbb`

Step 2's own gate advances. Routing it now closes Step 2 and unblocks the Step 3
wave, while `ebbb` gates nothing. `ebbb` stays ready behind it. `f09a` stays
parked because `plan.md` Step 12 owns the engine-state drift and the Builder must
not work ahead of the queue. DEC-020 records the order.

### The two carried findings get step owners, not runtime rows

The pending event carried two findings with no owner. Neither becomes a runtime
row now, because only the current step's wave may be open.

1. `mem-1790103346-2b0f`, the config-layer defect: `cli.backend` and `cli.args`
   merge as independent keys, so a user-scope `~/.ralph/config.yml` that pairs a
   backend with its args leaks those args onto a project's `cli.backend`
   override, live CLI exit 1 at `635cb8c`. Owner: Step 9, amended in `plan.md`,
   which already requires a fail-closed message naming the layer conflict.
2. The `crates/ralph-cli/src/completion_coord.rs:191` test-name overclaim. The
   test asserts only the panic half, so the `writes_nothing` half comes out of
   the name. Owner: Step 12, amended in `plan.md`.

### Emit

`tasks.ready` for `task-1790097924-daa7` /
`code-assist:v3-complete:step-02:verify-reconciliation`. No `.code-task.md`
artifact backs this row, so the payload carries the task id and key only.

No source edits this iteration.

## 2026-09-22, Step 2 wave, verify-reconciliation gate landed

Executed `task-1790097924-daa7` /
`code-assist:v3-complete:step-02:verify-reconciliation`. One commit to
`56690c0`, pushed. It changes the record only: `.ralph/specs/v3-reconciliation.md`,
166 insertions. No source edit, no verdict change.

### The gate, measured at `635cb8c`

Every check was run from this shell. Transcripts at
`logs/step-02-verify-reconciliation.md`, `logs/step-02-verify-rust.txt`, and
`logs/step-02-verify-python.txt`.

| Check | Command | Result |
|---|---|---|
| Not shallow | `git rev-parse --is-shallow-repository` | `false` |
| Rollup delta | `git rev-list --count HEAD..origin/wip/v3-prerelease-rollup` | `22` |
| Table completeness | `comm` of the table SHAs against `git rev-list HEAD..rollup` | empty both directions, tally 16/4/2 |
| Base-only work | `git merge-base --is-ancestor <sha> HEAD` for the 8 base-only commits | all IN HEAD |
| Path union | file lists of the 22 non-merge commits | 31 unique paths, all classified |
| TUI tailer | `git diff --numstat HEAD rollup -- backend_stream_tailer.rs` | no row, byte-identical |
| TUI reader | same for `autoloop_source.rs` | `0 189`, rollup adds no line we lack |
| Render proof | `cargo test -p ralph-tui --lib render_under_load_...` | 1 passed |
| Real harness | `cargo test -p ralph-cli --test integration_autoloop_tui_live_stream` | 2 passed |
| State root | `cargo test -p ralph-core --lib engine_state` | 11 passed |
| Smoke tooling | `.venv/bin/python -m unittest tools.tests.test_smoke_live_harnesses` | 10 passed, six PASS rows |

### What holds

- Every rollup-only commit is classified, and the table's SHAs match the
  inventory exactly.
- The TUI cluster is contained, not argued. `backend_stream_tailer.rs` is
  byte-identical to the rollup tip. `autoloop_source.rs` prints `0` insertions
  against the rollup, so the rollup adds nothing we lack, and the 189 lines are
  our own render-under-load test.
- No TUI commit carries a `superseded` verdict, so the plan's rendered-artifact
  requirement has no subject. Both TUI-adjacent `already-covered` rows are
  settled by artifact.
- The smoke cluster is present. Thirteen paths are byte-identical to the rollup
  tip. Eight differ, and every difference is our own adaptation: four carry the
  Ralph-owned `.ralph/autoloop` root, three carry analyzer fixes on the ported
  bytes, and one is a YAML line-wrap in `.github/workflows/ci.yml` with the same
  command.
- The `already-covered` clippy rows hold by outcome. `HEAD` carries the derived
  `Default` for `AutoloopBin`, no manual impl, `f64::EPSILON` comparisons, and
  zero `push_lines`.
- `c90001e` is verified against runtime behavior, not a file-size claim: 11
  `engine_state` tests pass, including `engine_state_root_is_under_ralph`.

### The one defect found, and fixed

The `3322fd7` row was classified `superseded` because "the rollup copy is behind
`v3/complete`". Measured, that is not true. The two copies of
`.ralph/tasks/ralph-owned-autoloop-state.code-task.md` diverge in both
directions: `HEAD` carries `cargo clippy --workspace` and the rollup carries
`4 passed` where `HEAD` says `3 passed`. Both lines are run-log bookkeeping. The
verdict stands because the code the row describes is present via `c90001e`, and
the reason now states the measured divergence.

### Queue

`daa7` closes after review. Step 2's wave is done, so the next ready rows are
`ebbb` (P1 operator steer, two lines in the audit doc) and `f09a` (P1, parked
for plan Step 12). `56690c0` matches `origin/v3/complete`.

## Step 2 wave, verify-reconciliation revision 4 (review.rejected fix)

- Active task: `task-1790097924-daa7` / `code-assist:v3-complete:step-02:verify-reconciliation`.
- Trigger: `review.rejected` at `56690c0`. The rejection was docs-only and
  precise: revision 3 claims it re-measured every surviving claim at `635cb8c`,
  but five evidence sentences still carried their pre-port `80309dc` state as
  present-tense fact about `HEAD`.
- Artifact: `.ralph/specs/v3-reconciliation.md`. Commit `a04126e`.

### Intended verification commands, and what they returned

| Command | Result |
|---|---|
| `git diff --name-only 635cb8c 56690c0` | `.ralph/specs/v3-reconciliation.md` only, so revision 3's code and test measurements hold at `56690c0` |
| `git rev-list --count HEAD..origin/wip/v3-prerelease-rollup` | 22 |
| 8 base-only SHAs through `git merge-base --is-ancestor <sha> HEAD` | 8 checked, 0 missing |
| `git cat-file -e` on the nine smoke-cluster paths at `80309dc` and `56690c0` | absent at `80309dc`, present at `56690c0`, all nine |
| `git show <ref>:<path> \| wc -l` for the five table counts | `backend_stream_tailer.rs` 80309dc 514 / HEAD 881 / rollup 881; `autoloop_source.rs` 80309dc 1331 / HEAD 2242 / rollup 2053; `engine_state.rs` 419/377; `autoloop_runner.rs` 1032/896; swept doc 83/83 |
| `git diff --numstat HEAD <rollup> -- <four already-covered paths>` | empty, byte-identical |
| `git diff --numstat HEAD <rollup> -- <two TUI files>` | `0 189 autoloop_source.rs` |
| `grep -c reader_engine_root` | 8 at `80309dc`, 14 at `56690c0`, 12 at the rollup tip |
| `cargo test -p ralph-tui --lib render_under_load_...` | 1 passed, 0 failed, 273 filtered |
| `cargo test -p ralph-cli --test integration_autoloop_tui_live_stream` | 2 passed, 0 failed |
| `cargo test -p ralph-core --lib engine_state` | 11 passed, 0 failed, 740 filtered |
| `PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses` | Ran 10 tests, OK, six-row PASS |
| `cargo fmt --all -- --check` | exit 0 |

Raw transcript: `.ralph/specs/v3-complete/logs/step-02-verify-reconciliation-rev4.txt`.

### The sweep was wider than the rejection

The rejection named five sentences. Re-measuring every state claim in the
classification table and in Why the ports take final content found nine rows and
four further claims of the same class, plus the swept-doc section. Each now names
the commit it was measured at and states the landed reading where a port
followed. No verdict changed: 16 `port`, 4 `already-covered`, 2 `superseded`.
Recorded as DEC-022 at confidence 85.

### Why no clippy or full test run

No Rust source changed. `git diff --name-only` across the whole increment is the
one spec file, and `git diff --name-only 635cb8c 56690c0` proves the code tree is
byte-identical to the commit where the full gate was last measured green.
`cargo fmt --all -- --check` was run anyway and returned exit 0.

### Queue

`daa7` is ready for review again at `a04126e`. Next after it: `ebbb` (P1
operator steer). `f09a` (P1) stays parked for plan Step 12.

## 2026-09-22, Step 2 wave, verify-reconciliation revision 5

Active task: `task-1790097924-daa7` /
`code-assist:v3-complete:step-02:verify-reconciliation`, triggered by
`review.rejected` at `a04126e`.

Artifact: `.ralph/specs/v3-reconciliation.md`. Commit `c40ebbd`, pushed to
`origin/v3/complete`.

### The rejection, re-measured rather than trusted

Both named claims reproduce.

| Claim in the record | Measured at `56690c0` | Verdict |
|---|---|---|
| `comm` set "has grown to 24 paths" | 25 paths. `comm -12` prints 25 and `grep -Fxf` over the same two `git diff --name-only` lists prints 25; the two outputs are identical. 24 is the set with `Cargo.lock` dropped, which the sentence's own clause counts as a member. | false |
| "The rollup tip carries no `sha2` edge for `ralph-adapters` at `80309dc` either" | The rollup tip carries it twice: `git show <rollup>:crates/ralph-adapters/Cargo.toml` line 19 is `sha2.workspace = true`, and `"sha2"` sits at line 2766 inside its `Cargo.lock` `ralph-adapters` block (block header line 2750). Our `80309dc` tree has 0 `sha2` matches in both files. | false |

### The rest of revision 4, re-measured

All hold at `56690c0`: 22 rollup-only commits and 22 table rows tallied 16
`port` / 4 `already-covered` / 2 `superseded`; the nine smoke-cluster paths
absent at `80309dc` and present at `56690c0`; line counts `backend_stream_tailer.rs`
881/881, `autoloop_source.rs` 2242/2053, `engine_state.rs` 419/377,
`autoloop_runner.rs` 1032/896, swept doc 83/83; `80309dc` counts 514 and 1331;
`reader_engine_root` 8/14/12; `sha2` package at `Cargo.lock:3657` and `:3658`;
the `ralph-adapters` edge at `Cargo.lock:2766`; the TUI numstat
`0 189 autoloop_source.rs`; the two TUI files byte-identical to the merge base at
`80309dc` and `1421 insertions / 143 deletions` against it at `56690c0`; the four
`already-covered` paths byte-identical to the rollup tip; and
`git diff --name-only 635cb8c 56690c0` still one file.

### Live checks this turn

| Command | Result |
|---|---|
| `cargo test -p ralph-tui --lib render_under_load_keeps_one_truthful_status_and_newest_lines` | 1 passed, 0 failed, 273 filtered |
| `cargo test -p ralph-cli --test integration_autoloop_tui_live_stream` | 2 passed, 0 failed |
| `cargo test -p ralph-core --lib engine_state` | 11 passed, 0 failed, 740 filtered |
| `PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses` | Ran 10 tests in 5.541s, OK, six-row PASS |
| `git push origin v3/complete` | `56690c0..c40ebbd` |

Raw transcript: `.ralph/specs/v3-complete/logs/step-02-verify-reconciliation-rev5.txt`.

### Why no clippy or full suite

No Rust source changed. The increment is one spec file, and the code tree is
byte-identical to `56690c0`, where the full gate was last measured green.

### Queue

`daa7` is ready for review again at `c40ebbd`. Next after it: `ebbb` (P1 operator
steer). `f09a` (P1) stays parked for plan Step 12.

## 2026-09-22, Step 2 wave, verify-reconciliation revision 6 (rejected-fix)

Active task `task-1790097924-daa7`,
`code-assist:v3-complete:step-02:verify-reconciliation`. Pending event was
`review.rejected` at `c40ebbd`. Docs-only fix, commit `27ec1df`, pushed
`c40ebbd..27ec1df`.

### The two rejected claims, re-measured before editing

- The smoke cluster has **five** state-root paths, not four:
  `presets/live-harness-smoke/README.md` 4 `.ralph/autoloop` substitutions at
  `HEAD` against 0 in the rollup, `require_smoke_handoff.py` 1, 
  `tools/smoke-live-harnesses.sh` 2,
  `.ralph/tasks/manual-live-harness-smoke.code-task.md` 1, and
  `tools/tests/test_smoke_live_harnesses.py` **11**.
- The eighth differing path was `tools/tests/test_smoke_live_harnesses.py`, and
  `require_smoke_handoff.py` was counted in both groups, so the itemization
  reached eight only by naming that file twice. Five plus three minus the shared
  file plus `ci.yml` is eight distinct paths.
- Five tracked `.ralph/tasks/*.code-task.md` docs at `HEAD` have no frontmatter:
  `backend-agnostic-e2e`, `context-window-utilization`,
  `manual-live-harness-smoke`, `multi-loop-concurrency`, and
  `tui-stream-history-backpressure`.

### One correction to the rejection's own wording

The rejection described the extra edit in `test_smoke_live_harnesses.py` as "a
sorted import order". Measured, the direction is the reverse: `HEAD` has
`textwrap` before `time`, and the rollup tip has `time` before `textwrap`. The
record states the measured direction. The second extra edit reproduces as the
rejection described: `if match is None: self.fail(combined)` at `HEAD` against
`self.assertIsNotNone(match, combined)` in the rollup.

### What else was swept

The two edits land inside revision-3 text that revisions 4 and 5 both report as
re-measured, so I re-measured the whole of both sections rather than the two
named sentences. All hold at `HEAD`: 12 files under `presets/live-harness-smoke/`,
13 byte-identical paths (the named list, each checked with `diff -q` on
`git show` output), 8 differing paths, the 83-line code-task doc at `HEAD`,
`56690c0`, and the rollup tip, and the `ci.yml` cosmetic difference in the
direction the record states. Both new claims now carry the command that produces
them and its pasted output.

### Verification

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `git diff --name-only c40ebbd HEAD` (pre-commit) | empty; code tree unchanged |
| `PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses` | Ran 10 tests in 5.372s, OK, six-row PASS |

### Why no clippy or full suite

No Rust source changed. The increment is one spec file and the code tree is
byte-identical to `c40ebbd`, where the four focused targets and the full gate
were last measured green.

### Queue

`daa7` is ready for review again at `27ec1df`. Next: `ebbb` (P1 operator steer).
`f09a` (P1) stays parked for plan Step 12.

## 2026-09-22, Step 2 wave, verify-reconciliation revision 6 record entry (rejected-fix)

Active task: `task-1790097924-daa7` / `code-assist:v3-complete:step-02:verify-reconciliation`.

Pending event was `review.rejected` at `27ec1df`. The defect was the revision
log. The revision-6 commit carried two corrections but added no header bullet
and no change section, so the record's log stopped at revision 5.

Docs-only fix. One header bullet and one `### What revision 6 changed` section,
in the established form, naming the two corrected sentences and their commands.

Verification, run this turn at `27ec1df`:

| Command | Result |
| --- | --- |
| `grep -n '^- Revision' .ralph/specs/v3-reconciliation.md` | stopped at revision 5 before the edit; revision 6 after |
| `git show 27ec1df --stat` | 2 hunks, one file, no revision bullet or section |
| frontmatter command | five paths, matching the record |
| state-root count command | 11/4/2/1/1/0/0/0, matching the record |
| `diff -q` over 13 paths | 13 identical, 8 differing |
| `cargo fmt --all -- --check` | exit 0 |
| python smoke matrix | Ran 10 tests in 5.569s, OK, six-row PASS |

No verdict changes. Tally stays 16 `port`, 4 `already-covered`, 2 `superseded`,
22 rows. The code tree is byte-identical to `27ec1df`, so the Rust targets
measured there still hold. Raw transcript:
`logs/step-02-verify-reconciliation-rev6.txt`.

## 2026-09-22, Step 2 wave, verify-reconciliation revision 7 (rejected-fix)

Active task: `task-1790097924-daa7` / `code-assist:v3-complete:step-02:verify-reconciliation`.

Pending event was `review.rejected` for `.ralph/specs/v3-reconciliation.md` at
`4878882`. One defect: the `### What revision 6 changed` section opened
"Revision 5 was rejected at `27ec1df`". Revision 5 is `c40ebbd`; `27ec1df` is
revision 6's rejection, whose single defect was the missing revision-log entry.

Fix, docs-only, commit `3af3c24`, pushed `4878882..3af3c24`:

1. the opening sentence now reads `c40ebbd`
2. a revision-7 header bullet and a `### What revision 7 changed` section,
   because the established rule in this record is that a revision commit records
   itself in the log

Verification, run at the pushed tip:

| Command | Result |
| --- | --- |
| `git diff --name-only 4878882 HEAD` | `.ralph/specs/v3-reconciliation.md` alone, so the code tree is unchanged |
| `jq` chain over `.ralph/events-20260922-170542.jsonl` | pasted block 1 matches byte-for-byte by `diff` |
| `jq` over the `c40ebbd` payloads | pasted block 2 matches byte-for-byte by `diff` |
| `grep -n '^- Revision'` | bullets 2 through 7 |
| `grep -c '^### What'` | 5 change sections, ending at revision 7 |
| `cargo fmt --all -- --check` | exit 0 |
| `PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses` | Ran 10 tests in 5.322s, OK, six-row PASS |

No verdict changes. Tally stays 16 `port`, 4 `already-covered`, 2 `superseded`,
22 rows. Raw transcript:
`logs/step-02-verify-reconciliation-rev7.txt`.

Two notes for the next round, neither a rejection:

- One pasted block in this section is the output of an event-log listing. The
  log is append-only, so this round's own `review.ready` appends a line below
  the pasted output. The block's attribution evidence is frozen by the second
  command, which filters on a fixed commit.
- The first draft of that block was hand-estimated rather than pasted. It was
  caught by re-running the command and diffing the block against the output
  before the commit, which is why both blocks are byte-identical at the tip.

`daa7` is ready for review at `3af3c24`. Next: `ebbb` (P1 operator steer).
`f09a` (P1) stays parked for plan Step 12.

## 2026-09-22, Step 2 wave, verify-reconciliation revision 8 (rejected-fix)

Active task `task-1790097924-daa7`, key
`code-assist:v3-complete:step-02:verify-reconciliation`. Artifact
`.ralph/specs/v3-reconciliation.md`. Pending event was `review.rejected` at
`3af3c24`, one defect: block 1 of the revision-7 section is a partial,
non-reproducing listing. Docs-only fix, commit `d776163`, pushed
`3af3c24..d776163`.

### The defect, reproduced before editing

The replaced command filtered `select(type=="object") | select(.payload.artifact?
== record)`. The `56690c0` `review.ready` and the revision-4 `a04126e` pair store
their payload as a joined string, so the filter drops all three. Re-running the
command also appends this round's own `3af3c24` lines. The revision-7 journal
named both traps, so neither was unknown.

### The fix

Block 1 is replaced with a bounded listing: the commit is read from the payload
text or the payload object, and the listing is restricted to the commits
recorded through revision 6. It is complete and it does not grow. A disclosure
sentence now states the bound and the two payload forms, which the rejected
version did not. Header bullet 8 and a `### What revision 8 changed` section
record the round, per the rule the revision-6 rejection set.

### The proof command was wrong on the first try, and that is the lesson

The first draft of the revision-8 evidence used
`... | grep -c 'a04126e\|56690c0'` against the replaced object-only listing and
pasted `0`. Re-running it printed `1`: the unbounded listing now contains this
round's own `review.rejected` payload, whose defect text names `a04126e`. An
append-only listing defeats a zero-count proof about identifiers the current
round itself writes. The section now uses a bounded old-versus-new comparison:
the object-only filter bounded to the same commits prints 11 lines, the new
listing prints 14, and the difference is exactly `review.ready 56690c0 -`,
`review.ready a04126e -`, `review.rejected a04126e -`.

### Verification, run at the pushed tip

| Command | Result |
| --- | --- |
| python block checker over revisions 7 and 8 | all three bash/text pairs byte-identical to a re-run, exit 0 |
| bounded listing run twice | identical sha256 both runs |
| old bounded filter vs new bounded listing | 11 vs 14 lines, 3-line delta as stated |
| `grep -n '^- Revision'` | bullets 2 through 8 |
| `grep -n '^### What'` | change sections through revision 8 |
| `cargo fmt --all -- --check` | exit 0 |
| `PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses` | Ran 10 tests in 5.410s, OK, six-row PASS |
| `git diff --name-only 3af3c24 HEAD` | `.ralph/specs/v3-reconciliation.md` alone |

No verdict changes. Tally stays 16 `port`, 4 `already-covered`, 2 `superseded`,
22 rows. No clippy or full suite: no Rust source changed.

`daa7` is ready for review at `d776163`. Next: `ebbb` (P1 operator steer).
`f09a` (P1) stays parked for plan Step 12.

## 2026-09-22, Step 2 wave, finalizer pass on the verify-reconciliation gate (queue.advance)

Pending event was `review.passed` for `task-1790097924-daa7` at `d776163`,
revision 8. Verdict: the gate is complete, the whole prompt is not.

### The gate, re-measured independently

| Command | Result |
| --- | --- |
| `git diff --name-only 3af3c24 HEAD` | `.ralph/specs/v3-reconciliation.md` alone, 76 insertions, 1 deletion, so the code tree is unchanged |
| `comm -3` of the table SHAs against `git rev-list 2e1fc52..origin/wip/v3-prerelease-rollup` | empty, 22 and 22 |
| `comm -3` of the table SHAs against `git rev-list HEAD..origin/wip/v3-prerelease-rollup` | empty, so every rollup commit missing from `HEAD` is a row |
| table tally | 22 rows, all six-field, 16 `port`, 4 `already-covered`, 2 `superseded` |
| `git diff --numstat HEAD origin/wip/v3-prerelease-rollup -- backend_stream_tailer.rs` | no output, byte-identical at 881 lines |
| same over `autoloop_source.rs` | `0 189`, exactly the branch's render-under-load test |
| `git diff --numstat` over the four `already-covered` paths | no output |
| `cargo fmt --all -- --check` | exit 0 |
| `PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses` | `Ran 10 tests in 5.326s`, OK, six-row PASS |
| `cargo test -p ralph-core --lib engine_state` | `11 passed; 0 failed; 0 ignored; 740 filtered out` |

The Step 2 gate condition "a `superseded` verdict for any TUI commit needs a
rendered artifact" does not fire: the two `superseded` rows are `3322fd7` and
`1e67e52`, neither a TUI commit. The one TUI non-`port` row, `2ac3c1f`, is
`already-covered` and byte-identical.

The adversarial probe that mattered: the smoke cluster's 8 differing paths were
read as diffs, not as a summary. Five carry the Ralph-owned `.ralph/autoloop`
state root where the rollup reads the bare `.autoloop` default, and three carry
behavior-preserving analyzer fixes. The record discloses all eight. Nothing is
lost.

No clippy and no full workspace suite: the increment changes no Rust source, so
neither can move. Stated as an omission, not a pass.

### Why the wave advances rather than closing

- `.ralph/specs/v3-ga-readiness.spec.md` is absent. Step 14 is unstarted.
- `crates/ralph-core/src/hat_registry.rs` is present. Step 3 is unstarted, `a7e.10` open.
- `presets/wave-review.yml` is present and uncertified. Step 4 is unstarted, `a7e.8` open.
- `.beads/issues.jsonl` holds 6 open rows: `a7e`, `a7e.8`, `a7e.10`, `ga3-c4-dashboard-dead-svf`, `landing-untracked-sweep-yxv`, `tui-help-wave-stale-5hu`.
- `task-1790100842-ebbb` is genuinely open: the audit contains no RFC reference, so RFC #86 is not recorded yet.

Two legacy `.ralph/tasks/*.code-task.md` docs carry non-terminal status
(`ralph-hats-command` pending, `ralph-owned-autoloop-state` in-progress while its
code has landed). No plan step owns them. Noted, not blocking.

`daa7` is closed. Next: `ebbb` (P1 operator steer). `f09a` (P1) stays parked for
plan Step 12. Planner owns the next wave; the finalizer created none.

## 2026-09-22, Step 2 wave, operator steer recorded: dispatch RFC #86 (review.ready)

Pending event was `tasks.ready` for `task-1790100842-ebbb`
(`operator-steer:v3-complete:record-rfc-86`, P1). Docs-only increment, commit
`8e7ae0a`, pushed `d776163..8e7ae0a`.

### The steer

Add autoloop issue #86 (RFC: dispatch.jev, bus mode with a calibrated fail-safe
router, opt-in) to `.ralph/specs/v3-completion-audit.md` as related upstream
context. Acknowledged in the scratchpad per the operator contract. One new
`## Related upstream context` section plus one header bullet, additive, no
verdict and no row touched.

### Evidence, re-measured

| Command | Result |
| --- | --- |
| `curl -sS .../repos/mikeyobrien/autoloop/issues/86` | `86 open \| RFC: dispatch.jev — bus mode with a calibrated fail-safe router (opt-in)`, label `enhancement`, created `2026-09-22T18:06:04Z` |
| `npm ls -g --depth=0 \| grep -i autoloop` | `@mobrienv/autoloop@0.11.0`, repository `github.com/mikeyobrien/autoloop` |
| `grep -rniI jev / typesafe\|noul\|routes_file` over the installed package | `0` occurrences each across 6041 files |
| `autoloop config show --preset code-assist` in a clean repo | no `[routing]` section, `0` `jev` matches |
| `gh api search/code q='routing.jev repo:mikeyobrien/autoloop'` | 7 paths on `main` at `fce46cc`, including `packages/harness/src/jev-routing.ts` and `docs/reference/jev-routing.md` |
| `npm view @mobrienv/autoloop dist-tags.latest` | `0.11.0`, published 2026-09-10, predating that `main` commit |
| pasted bash block vs re-run, python checker | byte-identical, exit 0 |
| `git diff --numstat d776163..8e7ae0a` | `112 0 .ralph/specs/v3-completion-audit.md` |

### The correction the steer produced

The brief's Phase 2c.1 and RFC #86 both call `[routing.jev]` shipped. It is
shipped upstream on `main` and absent from every published engine, including the
0.11.0 install this branch pins: no reader, no `[routing]` config section, zero
identifier hits. Step 9's "confirm each path by execution" half is therefore
blocked on an engine build newer than the last release, not on Ralph code, and
Step 11 gains the RFC as seam evidence for its feasibility table. Unmeasured and
labeled inferred: whether the same release gap covers the `#36` and `#38`
surfaces the brief cites.

### Verification

- Docs-only: `git diff --name-only d776163..8e7ae0a` is
  `.ralph/specs/v3-completion-audit.md` alone. `git show --stat 8e7ae0a` is
  1 file changed, 112 insertions, 0 deletions.
- The new section's single bash block and its text block reproduce
  byte-identically on re-run, network and `gh` calls included.
- Section order checked: `## Method`, `## Audit table`, `## Per-bead evidence`,
  `## Corrections`, `## Related upstream context`, `## Closed rows`. The audit
  table and every verdict are unchanged.
- Runtime files stayed out: `.ralph/agent/decisions.md` is still modified and
  unstaged, `progress.md`, `scratchpad.md`, `logs/`, and the specs working
  directory are untracked.

No `cargo` command ran. No Rust or any other source file changed, so fmt,
clippy, and the test suite cannot move on this increment. Stated as a deliberate
omission, not as a pass.

### Trap recorded

`autoloop config set --repo --preset code-assist routing.jev.enabled=true`
exits 0 and the resolved config then prints `[routing] jev = "[object Object]"
# default`. `config set` is not schema-validated, so a successful set is not
evidence of engine support. The identifier grep and the pristine `config show`
decide it. Saved as `mem-1790107571-5574`.

### Queue

`ebbb` is ready for review at `8e7ae0a`, docs-only. Then `f09a` (P1), still
keyless and parked for plan Step 12. Step 2 stays open.

## 2026-09-22, Step 2 wave, operator-steer record revision (rejected-fix, review.ready)

Active task: `task-1790100842-ebbb`, key
`operator-steer:v3-complete:record-rfc-86`, P1, step-02. Docs-only, one file,
one commit.

### The rejection

The critic ran the engine instead of reading the claim. `review.rejected` with
confidence 88: the artifact said Ralph can emit the `[routing.jev]` block and
fail closed, while the engine completes the run normally. The defect was the
same sentence the plan echoes at `.ralph/specs/v3-complete/plan.md:80`.

### Re-measured, not inherited

Reproduced the critic's run on autoloop 0.11.0, then went one step further than
the rejection did.

- Probe preset: a copy of the bundled `autofix` preset, plus `[routing.jev]`
  with `enabled = true`, `routes_file = "does-not-exist-routes.json"`,
  `model = "jev-1.13.0"`, and `backend.kind = "command"` pointed at a script
  that prints `LOOP_COMPLETE`. No `TYPESAFE_API_KEY` in the environment.
- Result: exit 0, `stop_reason completion_promise`, `autoloop doctor` reports
  `0 failure(s), 1 warning(s)` with the warning being the unfalsifiable
  completion contract, and the journal of 8 events holds 0 routing records. The
  single `routing` text match in the journal is the phrase "Recent routing
  event" inside the injected prompt.
- Extra measurement the rejection did not have: a sentinel preset with
  `model = "sentinel-xyz"` returns through `autoloop config show --json` with
  that value intact, and the `routing` key is absent from a pristine preset.
  So the loader genuinely retains every declared value, and the block is
  carried rather than defaulted. The critic's "survives translation" reading is
  confirmed at key level, and now it is proven by contrast instead of assumed.

### What changed

`.ralph/specs/v3-completion-audit.md`, the `## Related upstream context`
section only. 56 insertions, 4 deletions, one file.

- The false sentence is gone. The record now states what the run shows: enabled
  block, missing catalog, no credential, and a normal completed run, with the
  probe command and its captured output in the section.
- Step 9 must gate on engine capability rather than on translation alone:
  refuse to start before the backend launches when routing is enabled and the
  engine predates the reader, and name the engine version and the missing
  reader. Failing closed on an untranslatable path does not cover this case,
  because the block is carried, accepted, and ignored.
- The em dash nit stands unresolved by design. The only two em dashes are the
  real upstream issue title at line 452 and its captured command output at line
  501. Rewriting either would falsify a quotation or a verbatim result.

### Verification

- `git diff --stat` on the artifact: 1 file changed, 56 insertions, 4 deletions.
- Code fences balanced: 110 fence lines, even.
- `grep -n '—'` returns exactly lines 452 and 501, both pre-existing quotations.
- The evidence block is verbatim from the run above, with `run_id` and `ts`
  marked as per-run values and four summary lines marked as elided.
- The audit table and every verdict are unchanged. No row and no status moved.

No `cargo` command ran. No Rust or any other source file changed, so fmt,
clippy, and the test suite cannot move on this increment. Stated as a deliberate
omission, not as a pass.

### Queue

`ebbb` is ready for review at the revision commit. Then `f09a` (P1), still
keyless and parked for plan Step 12. Step 2 stays open.

## 2026-09-22, Step 2 wave, operator-steer caption revision 3 (rejected-fix, review.ready)

Active task: `task-1790100842-ebbb`, key
`operator-steer:v3-complete:record-rfc-86`, P1, step-02. Docs-only, one file,
one commit at `040a16f`.

### The rejection

`review.rejected` at `a461546`: the elision caption under the probe block is
false. Nine run lines are absent from the pasted block, not four, and `cost_usd`
sits below the stop reason, so the named range contradicts its own sentence. The
finding, the probe, and the Step 9 gate all held; only the caption was wrong.

### Measured by count, with a checker, not by reading

The probe commands were re-run into `/tmp/jev-probe/full-repro.log` with section
markers, then the pasted block was diffed against it mechanically.

- Captured 24 lines, pasted 14. Nine missing lines are the summary block:
  `autoloops summary`, `===================`, `run_id:`, `iterations:`,
  `cost_usd`, `journal`, `memory`, `review_every`, `inspect scratchpad`. The
  tenth is the blank line `tail -2` leaves above the doctor summary.
- The caption named four lines and a range from `autoloops summary` through
  `cost_usd`, which straddles `stop_reason`. Both halves of the rejection
  reproduce.

### What changed

`.ralph/specs/v3-completion-audit.md`, the probe block and the caption under it
only. 13 insertions, 2 deletions, one file.

- The block now carries the whole `autoloops summary` output, plus the doctor
  blank line, so nothing is elided and nothing has to be described as elided.
- The caption states the only variance that survives: `run_id` and `ts` are
  regenerated on every run.
- The stale bullet in the previous revision's verification list ("four summary
  lines marked as elided") is superseded by this section. It is left in place as
  the round's own record rather than rewritten.

### Verification

- `logs/step02-ebbb-verify-caption.py <capture> <spec>` normalizes `run_id` and
  `ts` and diffs every other line. RED at `a461546`: 24 captured against 14
  pasted, the diff printing the nine summary lines and the blank.
- GREEN against a second, independent capture (`full-greencap.log`) and the
  committed blob `b930c71`: `PASS: pasted block is verbatim, 24 lines`.
- `grep -n elid` returns nothing in the committed blob.
- `git show --stat 040a16f`: 1 file changed, 13 insertions, 2 deletions. No
  Rust or other source file moved.
- Code fences balanced: 110 fence lines, even. `grep -n '—'` returns exactly
  lines 452 and 501, both pre-existing quotations, unchanged.
- `HEAD` equals `origin/v3/complete` at `040a16f`.
- The audit table and every verdict are unchanged. No row and no status moved.

No `cargo` command ran. No Rust or any other source file changed, so fmt,
clippy, and the test suite cannot move on this increment. Stated as a deliberate
omission, not as a pass.

### Queue

`ebbb` is ready for review at `040a16f`, docs-only. Then `f09a` (P1), still
keyless and parked for plan Step 12, whose planner pass must key it. Step 2
stays open.

## 2026-09-22, Step 3 wave, planner pass on the queue (queue.advance)

Planner activation on `queue.advance` after the finalizer closed
`task-1790100842-ebbb` at `040a16f`. I read `<ready-tasks>`, `plan.md`,
`progress.md`, and `.ralph/agent/tasks.jsonl` rather than trusting the event
summary.

### State read, not assumed

- `ralph tools task list --format table` returns one row: `f09a`, open, P1,
  keyless. `ralph tools task ready --format table` returns the same single row.
  Nothing is `in_progress`.
- `.ralph/agent/tasks.jsonl` holds 12 rows. Eleven are closed: five Step 1 rows,
  five Step 2 rows (`986e`, `b4fc`, `c8bd`, `daa7`, `72c2`), and the operator
  steer `ebbb`. So Step 2 has no open runtime row.
- `plan.md` Step 3 is "Delete the last in-house remnant (`a7e.10`)" and its wave
  is "map callers, relocate or delete, update `lib.rs`, verify the engine
  rejection path".

### Why Step 2 closes and Step 3 opens

The finalizer's event said "step 2 stays open". That sentence reflected the open
`f09a` row, not a remaining Step 2 task. `f09a`'s own description and `plan.md`
both assign it to Step 12 (live verification under the engine, 0.11.0 drift), and
every prior planner pass kept it parked for the same reason. With `daa7` and
`ebbb` closed, Step 2's wave is exhausted, so the planner closes Step 2 and opens
Step 3. `f09a` keeps its Step 12 owner and is not published.

### Premise re-measured before decomposing

- `grep -rn 'HatRegistry|hat_registry' crates/ --include=*.rs` returns exactly
  two ralph-core declaration lines (`crates/ralph-core/src/lib.rs:26` `mod
  hat_registry;`, `:84` `pub use hat_registry::HatRegistry;`) and the consumer
  `crates/ralph-cli/src/hats.rs` (import at `:17`, `HatRegistry::from_config` at
  `:132`, the `&HatRegistry` helpers, and its tests). One production consumer.
- `grep -rn 'event_bus|EventBus' crates/ --include=*.rs` returns only comments
  outside `crates/ralph-proto/src/event_bus.rs` itself, plus the declaration at
  `crates/ralph-proto/src/lib.rs:15` and the re-export at `:25`. Zero production
  callers.
- `wc -l` is 468 for `hat_registry.rs` and 401 for `event_bus.rs`.
- The v3 rejection is live: the message literal sits at
  `crates/ralph-core/src/config.rs:2362` and the test
  `core_engine_rejects_removed_ralph_engine` at `:2689`.

So the audit's "relocate-or-rework `hat_registry.rs`, delete `event_bus.rs`" is
the correct Step 3 scope, and the wave is three rows.

### The wave

Created with `ralph tools task ensure`, all on loop `primary-20260922-170542`:

1. `code-assist:v3-complete:step-03:relocate-hat-registry` (`task-1790109144-0d0c`, P1)
2. `code-assist:v3-complete:step-03:delete-event-bus` (`task-1790109144-2c18`, P2)
3. `code-assist:v3-complete:step-03:verify-remnant-and-engine-rejection` (`task-1790109144-5333`, P3, blocked by `0d0c` and `2c18`)

No future step's wave was created. The two carried findings keep their step
owners: `mem-1790103346-2b0f` (config-layer `cli.args` leak) stays with Step 9,
and the `crates/ralph-cli/src/completion_coord.rs:191` test-name overclaim stays
with Step 12.

### Emit

`tasks.ready` for `task-1790109144-0d0c` /
`code-assist:v3-complete:step-03:relocate-hat-registry`. No `.code-task.md`
artifact backs this row, so the payload carries the task id and key only.

No source edits this iteration.

## 2026-09-22, Step 3 wave, relocate HatRegistry to ralph-cli (build.start)

Active runtime task `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1). No
`.code-task.md` artifact backs the row, so the runtime task is the spec.

### The pin, captured before any structure moved

The module's own test module is the characterization contract. It is 14 tests
and all 14 pass pre-move.

- `cargo test -p ralph-core --lib hat_registry` is 14 passed, 0 failed,
  finished in 0.72s. Log: `logs/step03-0d0c-pre-move-hat-registry-tests.log`.
- `cargo test -p ralph-cli --bin ralph hats::` is 37 passed, 0 failed, 2
  ignored. Log: `logs/step03-0d0c-pre-move-ralph-cli-hats-tests.log`. The
  earlier `--lib hats` run found 0 tests because `mod hats;` is declared in
  `main.rs:31`, so the module belongs to the `ralph` bin target, not the lib
  target.

### Scope, re-measured

`grep -rn 'hat_registry\|HatRegistry' crates/ --include=*.rs` returns the two
ralph-core declaration lines (`lib.rs:26`, `:84`), the consumer
`crates/ralph-cli/src/hats.rs`, and nothing else. `ralsh-cli` has no
`pub fn` whose signature names `HatRegistry` (grep for `pub fn .*HatRegistry`
exits 1), so the type needs no public re-export.

### Target shape

`crates/ralph-cli/src/hats/registry.rs` owns `HatRegistry`.
`crates/ralph-cli/src/hats.rs` declares `mod registry;` and imports the type
from it. `crates/ralph-core/src/lib.rs` drops `mod hat_registry;` and
`pub use hat_registry::HatRegistry;`. `crates/ralph-core/src/hat_registry.rs`
is deleted.

The move is verbatim apart from the two imports, which change from
`crate::config::{HatConfig, RalphConfig}` to
`ralph_core::{HatConfig, RalphConfig}`. No behavior changes, so no new test is
written. The moved test module is the equivalence harness.

### Intended verification

1. `cargo build -p ralph-core -p ralph-cli`
2. `cargo test -p ralph-cli --bin ralph hats::` (37 post-move at `hats::tests`,
   plus 14 at `hats::registry::tests`)
3. test-name equivalence, normalized module path, pre against post
4. `grep -rn 'hat_registry' crates/` returns nothing
5. `cargo test -p ralph-core --lib` still passes and the engine-rejection test
   `core_engine_rejects_removed_ralph_engine` still passes
6. `cargo clippy -p ralph-core -p ralph-cli --all-targets -- -D warnings`
7. `cargo fmt --all --check`

## 2026-09-22, Step 3 wave, HatRegistry relocated and hat_registry.rs deleted (build.done)

Active runtime task `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), bead `a7e.10`.

### What landed

`crates/ralph-core/src/hat_registry.rs` is deleted, 468 lines. Its type and its
test module moved to `crates/ralph-cli/src/hats/registry.rs`, which the consumer
`crates/ralph-cli/src/hats.rs` now declares as `mod registry;`. The ralph-core
`mod hat_registry;` and `pub use hat_registry::HatRegistry;` lines are gone.

### The pin, held across the move

The module's own test module was the characterization contract. Pre-move
`cargo test -p ralph-core --lib hat_registry` was 14 passed, 0 failed. Post-move
`cargo test -p ralph-cli --bin ralph hats::` is 44 passed, 0 failed, 2 ignored
(37 `hats::tests` unchanged plus 7 `hats::registry::tests`), and
`cargo test -p ralph-core --lib` is 737 passed, 0 failed, which is the pre-move
751 minus the 14 that moved.

### The equivalence proof

`logs/step03-0d0c-equivalence.py` diffs the pre-move module at HEAD against the
post-move module block by block. It reports 10 of 11 production fns and 5 of 7
test fns byte-identical, zero unexpected additions, and exactly the declared
subtractions. `from_config` is the one changed production fn (it calls
`register` instead of `register_with_config`). The two changed test fns differ
only in comments, and the harness asserts that by stripping comment lines and
re-comparing.

### The subtraction, and why

`cargo build` failed the new owner with `multiple methods are never used`, and
`cargo clippy --all-targets -- -D warnings` failed on `get_config` and `ids`,
which no caller reaches even in the test target. Seven members have no caller in
the CLI: `get_config`, `ids`, `subscribers`, `find_by_trigger`, `can_publish`,
and `register_with_config`, with `get` reaching only the module's own tests.
`subscribers`, `find_by_trigger`, and `can_publish` are the routing-authority
API of the deleted in-house engine, and the charter assigns that work to
autoloop. They are removed with their seven tests. `get` stays as a
`#[cfg(test)]` accessor, because the CLI reads hats through `all` and
`get_for_topic`. The now-unread `configs` map and `register_with_config` go with
`get_config`, and `from_config` calls `register`. Recorded as DEC-026.

### The real invocation path, not just unit tests

`logs/step03-0d0c-hats-invocation.log` holds four command paths run from a
scratch directory `/var/tmp/step03-hats` with a `ralph.yml` declaring two hats:

- `ralph hats list` renders both hats sorted by name.
- `ralph hats list --format json` serializes both, exercising `all()`.
- `ralph hats show builder` resolves the hat through `all().find`.
- `ralph hats validate` prints `Hats: 2 configured`, resolves the starting-event
  subscriber, and warns on the orphan `build.done`.
- `ralph hats validate -H builtin:code-assist` configures 4 hats from the real
  builtin preset, which exercises `from_config` against a shipped config.

### Gates, measured at this increment

- `cargo build -p ralph-core -p ralph-cli` is clean, no warnings.
- `cargo clippy -p ralph-core -p ralph-cli --all-targets -- -D warnings` exits 0.
- `cargo fmt --all --check` exits 0.
- `cargo test -p ralph-core --lib core_engine_rejects_removed_ralph_engine` is
  1 passed, 0 failed, so the v3 engine rejection still holds.

### The tracker, and why it was not edited

`.beads/issues.jsonl` is untouched. `a7e.10` is the two-part remnant row and its
event_bus half is still open, so closing it now would be a false status. The
file's own history shows dedicated `chore: sync beads issue history` commits and
there is no beads CLI on this machine, so a hand edit would fabricate state that
the Step 3 gate row `task-1790109144-5333` and Step 14 are meant to settle.

### Queue

`task-1790109144-0d0c` is ready for review at HEAD. Next in the wave is
`task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`), then the
blocked step gate `task-1790109144-5333` (P3). `task-1790101592-f09a` stays
parked for plan Step 12.

## 2026-09-22, Step 3 wave, relocate-hat-registry evidence repaired (review.rejected fix)

Active runtime task `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), bead `a7e.10`.
Reviewed artifact `crates/ralph-cli/src/hats/registry.rs`, unchanged at
`96b7bb1`. This round changes evidence only.

### The defect, reproduced before the fix

DEC-027 rejected the increment because both citations of the pre-move module read
it through `HEAD`, and `HEAD` is now `96b7bb1`, the commit that deletes the path.
Measured at `96b7bb1`:

| Citation | Command | Result |
| --- | --- | --- |
| `logs/step03-0d0c-equivalence.py` | `python3 logs/step03-0d0c-equivalence.py` | `FAIL: cannot read crates/ralph-core/src/hat_registry.rs at HEAD: fatal: path 'crates/ralph-core/src/hat_registry.rs' does not exist in 'HEAD'`, exit 1 |
| DEC-026 reversibility line | `git show HEAD:crates/ralph-core/src/hat_registry.rs` | `fatal: path 'crates/ralph-core/src/hat_registry.rs' does not exist in 'HEAD'`, exit 128 |

Both commands and their output are in `logs/step03-0d0c-equivalence-red.log`,
which ran a byte-copy of the pre-fix script from `/tmp/step03-red-script.py`.

### The fix

`logs/step03-0d0c-equivalence.py` no longer resolves the pre-move module through
`HEAD`. It names the literal base ref `96b7bb1^`, the parent of the relocation
commit, as its `BASE_REF` default at `:14`; `argv[1]` overrides it, and the
script prints the ref it compared. The post-move side stays a worktree read,
because the claim covers the checked-out module a reviewer inspects. DEC-026's
reversibility line cites `git show 96b7bb1^:crates/ralph-core/src/hat_registry.rs`
and records 468 lines at that ref. DEC-028 records the choice of a literal pinned
ref over a derived merge base and dates DEC-027's quoted failure to the rejected
commit.

### Green, and every figure unchanged from the pre-rejection run

`logs/step03-0d0c-equivalence-green.log`:

- `python3 logs/step03-0d0c-equivalence.py` prints
  `== pre-move base ref: 96b7bb1^:crates/ralph-core/src/hat_registry.rs ==`,
  production old 17 new 11, `from_config` the one changed fn, 10 retained
  byte-identical, tests old 14 new 7, 7 removed, 2 comment-only, 5 retained, and
  `PASS`, exit 0.
- `git show 96b7bb1^:crates/ralph-core/src/hat_registry.rs | wc -l` prints `468`,
  exit 0, empty stderr.
- `python3 logs/step03-0d0c-equivalence.py 96b7bb1~1` prints the same figures and
  PASSes, so the argument form and the caret form agree.
- Adversarial: `python3 logs/step03-0d0c-equivalence.py deadbeef` fails with
  `fatal: invalid object name 'deadbeef'`, exit 1, so a bad ref cannot pass
  silently.

### Why no cargo run this round

No Rust file is in the diff. `git status --short` lists no modified path under
`crates/`, and `HEAD` is still `96b7bb1`. The critic's own `verified_by_critic`
field records, at that commit, `cargo test -p ralph-core -p ralph-cli` with every
result line ok and 0 failed, clippy `--all-targets -- -D warnings` exit 0, and
`fmt --check` exit 0. An evidence-only increment cannot move any of them.

### Why no commit

The repair lands in `logs/` (untracked evidence under this wave's convention:
`git status --porcelain -- logs/` prints `?? logs/`, `git check-ignore -v logs/`
exits 1, and only `*.log` at `.gitignore:72` is ignored, so the round's own
`logs/step03-0d0c-equivalence.py` is neither tracked nor ignored) and in
`.ralph/agent/decisions.md` (loop state that stays
uncommitted; its last commit `7b53ead` predates this loop). That journal is
tracked, so a tracked file did change: `git status --porcelain -uno` prints
` M .ralph/agent/decisions.md`. No source file changed, and the wave does not
commit loop state, so there is nothing to commit.

### Queue

`task-1790109144-0d0c` is ready for review again at `96b7bb1` with the repaired
evidence. Next in the wave is `task-1790109144-2c18` (P2, delete the dead
ralph-proto `event_bus`), then the blocked step gate `task-1790109144-5333`
(P3). `task-1790101592-f09a` stays parked for plan Step 12.

## 2026-09-22, Step 3 wave, relocate-hat-registry record corrected (review.rejected fix)

Active runtime task `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), bead `a7e.10`.
Scope from DEC-029: docs-and-evidence only, fix the record sentences and change
no source file.

### The rejected claims, reproduced before the fix

- `git ls-files .ralph/agent/decisions.md` prints the path, and
  `git status --porcelain -uno` prints ` M .ralph/agent/decisions.md`, so the
  journal is a changed tracked file and the two "no tracked file changed"
  sentences were false.
- `logs/step03-0d0c-equivalence.py:14` is
  `BASE_REF = sys.argv[1] if len(sys.argv) > 1 else "96b7bb1^"`, so the script
  does name a ref of its own, and the sentence at `progress.md:1509` contradicted
  its own next clause. The rejection payload and DEC-029 both cite `:13`, which
  is a blank line; the correction text here uses the measured `:14`.
- `python3 logs/step03-0d0c-equivalence.py 96b7bb1^` prints 17 lines, and the
  green log pasted 3 of them under a bare `$` line.

### The fix

- `progress.md:1509` now says the script no longer resolves the pre-move module
  through `HEAD` and names its literal `BASE_REF` default at `:14`, instead of
  claiming it names no ref.
- `progress.md` "Why no commit" and DEC-028's scope note now say no source file
  changed, name the changed tracked journal, and point at the wave's
  never-commit-loop-state rule, instead of claiming no tracked file changed.
- `logs/step03-0d0c-equivalence-green.log` marks the argv block's elision with
  `...` and records `exit=0`.

### Re-measured at `96b7bb1`

`git rev-parse --short HEAD` and `git rev-parse --short origin/v3/complete` each
print `96b7bb1`, so the reviewed commit is still the tip. The unsplit form
`git rev-parse HEAD origin/v3/complete` prints the full 40-character object name
twice, and `git rev-parse --short` with both revs at once exits 128 with
`fatal: Needed a single revision`.
`python3 logs/step03-0d0c-equivalence.py` prints the base-ref banner, production
old 17 new 11 with `from_config` the one changed fn and 10 retained
byte-identical, tests old 14 new 7 with 7 removed, 2 comment-only, 5 retained,
and `PASS`, exit 0.
`git show 96b7bb1^:crates/ralph-core/src/hat_registry.rs | wc -l` prints `468`.
The `96b7bb1^` argv form prints the same 17 lines, and the three lines the log
keeps are verbatim lines 15 through 17 of that run.

### Why no cargo run and no commit

No Rust file changed and `HEAD` has not moved. `.ralph/specs/v3-complete/`
including this `progress.md` is untracked, `logs/` is untracked with only its
`*.log` files gitignored (`git check-ignore -v logs/` exits 1 and
`git check-ignore -v logs/step03-0d0c-equivalence.py` exits 1; `.gitignore:72`
holds `*.log`), and `.ralph/agent/decisions.md` is tracked loop state that the
wave does not commit.

### Queue

`task-1790109144-0d0c` is ready for review again at `96b7bb1` with the corrected
record. Next in the wave is `task-1790109144-2c18` (P2, delete the dead
ralph-proto `event_bus`), then the blocked step gate `task-1790109144-5333`
(P3). `task-1790101592-f09a` stays parked for plan Step 12.

## 2026-09-22, Step 3 wave, relocate-hat-registry log-ignore claim corrected (review.rejected fix)

Pending event was `review.rejected` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), artifact
`.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope "docs-and-evidence
only; source_edits: none, and none needed".

### The rejected claims, reproduced before the fix

- `git check-ignore -v logs/` exits 1 with no output and `git status
  --porcelain -- logs/` prints `?? logs/`, so `logs/` itself is not ignored.
- `git check-ignore -v logs/step03-0d0c-equivalence.py` exits 1, so the round's
  own citation is not ignored either.
- Only `git check-ignore -v logs/step03-0d0c-equivalence-green.log` matches; it
  prints `.gitignore:72:*.log`.
- `git rev-parse --short HEAD` and `git rev-parse --short origin/v3/complete`
  each print `96b7bb1`, so the reviewed commit is still the tip.
- `git ls-files .ralph/specs/v3-complete/` prints nothing and `git status
  --porcelain -- .ralph/specs/v3-complete/` prints `?? .ralph/specs/v3-complete/`,
  so the untracked half of the same sentence was true and stays.

### The fix, three record sentences and no source file

- The evidence-repaired section's `### Why no commit` paragraph carries the
  `?? logs/` status token, the `check-ignore` miss on `logs/`, the
  `.gitignore:72` `*.log` match, and the statement that
  `logs/step03-0d0c-equivalence.py` is neither tracked nor ignored.
- The record-corrected section's `### Why no cargo run and no commit` paragraph
  states the same conclusions in different words and carries the `check-ignore`
  miss on `logs/` and the `.gitignore:72` `*.log` match.
- DEC-029's reasoning now dates its insertion count: 153 insertions when the
  entry was written, 163 at that round's measurement, the delta being DEC-029's
  own 10 lines.

### No source change, verified after the edits

- `git diff --name-only -- crates/` prints nothing.
- `git diff --numstat -- .ralph/agent/decisions.md` printed 163 insertions and
  0 deletions at this round's measurement. The figure grows as each later round
  appends its decision entry, so it is a dated snapshot, not a constant.
- `git status --porcelain -uno` prints only ` M .ralph/agent/decisions.md`.
- A whole-file search for the rejected two-word qualifier prints nothing:
  `grep -c 'untracked and g[i]tignored' .ralph/specs/v3-complete/progress.md`
  prints `0` and exits 1. The bracket keeps the pattern from matching the line
  that carries it.
- The former baseline claim (`progress.md` held 1615 lines before the two
  corrections) is retracted: no transcript of that measurement survives.

### Why no cargo run and no commit

No Rust file changed and `HEAD` has not moved from `96b7bb1`, so the critic's own
gate results at that commit still govern. The changed files are the untracked
`.ralph/specs/v3-complete/progress.md` and the tracked
`.ralph/agent/decisions.md` loop journal, which the wave does not commit.

### Queue

`task-1790109144-0d0c` is ready for review again at `96b7bb1`. Then
`task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`), then the
blocked step gate `task-1790109144-5333` (P3). `task-1790101592-f09a` stays
parked for plan Step 12.

## 2026-09-22, Step 3 wave, critic pass on the relocate-hat-registry log-ignore correction (REJECTED)

Pending event was `review.ready` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), bead `a7e.10`,
artifact `.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope
"docs-and-evidence only; source_edits: none". I re-ran every claim in the round
instead of reading it, and found one false state claim in the round's own new
prose.

### What holds, reproduced claim by claim

Every state claim in the round's verification list reproduces at `96b7bb1`,
measured one command at a time:

- `git check-ignore -v logs/` exits 1 with no output, and `git check-ignore -v
  logs/step03-0d0c-equivalence.py` exits 1, so neither the tree nor the round's
  own script is ignored.
- `git check-ignore -v logs/step03-0d0c-equivalence-green.log` prints
  `.gitignore:72:*.log`, and `sed -n '72p' .gitignore` prints `*.log`.
- `git status --porcelain -- logs/` prints `?? logs/`, so the tree is untracked.
- `git rev-parse --short HEAD` and `git rev-parse --short origin/v3/complete`
  each print `96b7bb1`.
- `git ls-files .ralph/specs/v3-complete/` prints nothing and `git status
  --porcelain -- .ralph/specs/v3-complete/` prints `?? .ralph/specs/v3-complete/`.
- `git diff --name-only -- crates/` prints nothing; `git diff --numstat --
  .ralph/agent/decisions.md` prints `163 0`; `git status --porcelain -uno` prints
  only ` M .ralph/agent/decisions.md`.
- The two corrected passages are the ones the round names: `progress.md:1546-1549`
  carries the `logs/` measurement sentence and `progress.md:1612-1617` states the
  same conclusions in different words in the record-corrected section.
- The rejected qualifier is gone from both: `sed -n '1546,1549p;1612,1617p'
  .ralph/specs/v3-complete/progress.md | grep -c 'untracked and g[i]tignored'`
  prints `0`, exit 1.
- DEC-029's dated arithmetic closes. `git diff --numstat` prints `163 0` and the
  DEC-029 block is its blank separator plus nine lines, so 163 minus 10 is the
  153 the entry reports as the count when it was written.

### The refactor and the repaired citations, re-measured, not trusted

No path under `crates/` changed and `HEAD` is still `96b7bb1`, so the increment
the earlier rounds verified is the same bytes. I re-ran the strongest checks
anyway.

- `python3 logs/step03-0d0c-equivalence.py` prints the base-ref banner
  `== pre-move base ref: 96b7bb1^:crates/ralph-core/src/hat_registry.rs ==`, the
  same figures (production old 17 new 11, `from_config` the one changed fn, 10
  retained byte-identical; tests old 14 new 7, 7 removed, 2 comment-only, 5
  retained), `PASS`, exit 0, 17 lines, empty stderr.
- `python3 logs/step03-0d0c-equivalence.py '96b7bb1^'` prints the identical 17
  lines and PASSes, so the argv form and the default agree.
- Adversarial: `python3 logs/step03-0d0c-equivalence.py deadbeef` prints
  `FAIL: cannot read crates/ralph-core/src/hat_registry.rs at deadbeef: fatal:
  invalid object name 'deadbeef'.`, exit 1, so a bad ref cannot pass silently.
- `git show '96b7bb1^:crates/ralph-core/src/hat_registry.rs' | wc -l` prints
  `468`.
- Acceptance (b) and (c): `grep -n 'hat_registry\|HatRegistry'
  crates/ralph-core/src/lib.rs` exits 1, and `grep -rn 'hat_registry' crates/`
  exits 1. The new owner is declared at `crates/ralph-cli/src/hats.rs:9` (`mod
  registry;`) and `:11` (`use self::registry::HatRegistry;`).
- `cargo test -p ralph-cli --bin ralph hats::` is 44 passed, 0 failed, 2 ignored,
  exit 0. `cargo test -p ralph-core --lib
  core_engine_rejects_removed_ralph_engine` is 1 passed, 0 failed, exit 0.
- `logs/step03-0d0c-equivalence-green.log` marks its elision honestly: the
  caption reads `(14 of 17 lines elided; figures identical to the default-ref run
  above)`, against a 17-line run.

### The defect. The round's own verification bullet falsifies itself

`progress.md:1659-1660` records that a grep for the rejected qualifier exits 1,
"so the rejected phrase is gone".

Measured at the tip, that grep exits 0. It prints one line, `:1659`, and `:1659`
is the bullet itself: the sentence quotes the pattern it greps for, so writing
the bullet creates the match, and the exit code it reports can never hold again.
A reader who runs the command the record supplies gets the opposite result on the
record's own line.

Reproduced without the command matching its own text:
`grep -n 'untracked and g[i]tignored' .ralph/specs/v3-complete/progress.md`
prints `1659:- ... exits`, exit 0.

The conclusion is right and the repair is real. Bounded to the two passages the
rejection named, the count is 0, as recorded above. So this is a false evidence
sentence sitting on top of a correct fix, not a false repair.

That is the class DEC-024, DEC-027, and DEC-029 each rejected on this same task,
and this round exists to repair that class. DEC-024 rejected a missing revision
bullet that falsified no claim, and DEC-027 and DEC-029 rejected on citation
reproducibility while affirming the underlying work. A bullet whose stated exit
code is wrong the moment it is written is above the bar this wave already applies
nine times over. The fix is one sentence and no source change: bound the grep to
the two corrected passages, or state the measured result, which is that the
qualifier now survives only inside the sentence that quotes the pattern.

### Secondary, measured, non-blocking

- `progress.md:1597` says `git rev-parse HEAD origin/v3/complete` prints
  `96b7bb1` twice. That command prints the full 40-character object name twice;
  the short form needs `--short`, and `git rev-parse --short HEAD
  origin/v3/complete` exits 128 with `fatal: Needed a single revision`, because
  `--short` carries `--verify` single-revision semantics. The claim's substance
  holds, and `progress.md:1637` uses the correct one-rev-per-command form, so the
  pasted form is imprecise rather than wrong.
- `progress.md:1661-1662` says `progress.md` "held 1615 lines before the two
  corrections". No baseline for that count survives in the tree; the newest
  section is 55 lines plus its blank separator, so the pair closes only if the
  two in-place corrections added the remaining lines, which the round does not
  date. Not reproducible, so not a rejection ground here, but a claim about a
  state that no longer exists should carry its own dated transcript.
- `.ralph/specs/v3-complete/` is untracked and `logs/` is untracked with only its
  `*.log` files ignored, which the round now says correctly. The wave has
  committed no log, so the convention holds.
- The round's own `logs/step03-0d0c-record-fix-verify.log` uses `git rev-parse
  --short HEAD` with a single revision, so it does not repeat the `:1597`
  imprecision.

### Verdict

`review.rejected`. The repair is substantively correct and I re-verified it end
to end: the rejected qualifier is gone from both passages the rejection named,
and the refactor, the `:14` erratum, the elision caption, and acceptance (a)
through (e) all reproduce at `96b7bb1`, which has not moved. The rejection is
scoped to one self-falsifying verification sentence at `progress.md:1659-1660`.
No source file changes. The fix is one sentence.

Queue: `0d0c` goes back to the Builder for that one-sentence fix. Then
`task-1790109144-2c18` (P2 event_bus) and the blocked gate
`task-1790109144-5333` (P3). `task-1790101592-f09a` stays parked for plan Step 12.
## 2026-09-22, Step 3 wave, relocate-hat-registry self-falsifying search repaired (review.rejected fix)

Pending event was `review.rejected` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), artifact
`.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope "docs-and-evidence
only; source_edits: none".

### The rejected claim, reproduced before the fix

The bullet at the end of the previous round ran a whole-file search for the
rejected two-word qualifier and reported that it exited 1. Measured, that search
exited 0 and printed the bullet itself, because the bullet quoted the pattern it
searched for, so the command created the match it reported as absent. The critic
was exact.

### The fix

- The bullet now runs a whole-file search whose pattern cannot match the line
  that carries it: `grep -c 'untracked and g[i]tignored'
  .ralph/specs/v3-complete/progress.md` prints `0` and exits 1. Bounding the
  search to the two corrected passages is no longer needed, and no line number
  enters the evidence.
- The two corrected passages are named by section text, not by line number. This
  round's own `rev-parse` repair inserted lines above the record-corrected
  passage, which moved the line range the previous draft cited off the paragraph
  it named. Every line-number citation in the round was re-anchored for the same
  reason, and no corrected passage carries the rejected qualifier because no line
  in the file does.
- `### Re-measured at 96b7bb1` now says `git rev-parse --short HEAD` and
  `git rev-parse --short origin/v3/complete` each print `96b7bb1`, and records
  that the unsplit form prints the full 40-character object name twice while
  `--short` with both revs exits 128 with `fatal: Needed a single revision`.
- The 1615-line baseline bullet is retracted, because no transcript of that
  measurement survives.
- The two insertion counts are dated. `git diff --numstat --
  .ralph/agent/decisions.md` printed 163 at the previous round's measurement and
  printed 182 after this round's decision entry.

### Verified after the edits, machine-checked

- `grep -c 'untracked and g[i]tignored' .ralph/specs/v3-complete/progress.md`
  prints `0`, exit 1.
- `git rev-parse --short HEAD` and `git rev-parse --short origin/v3/complete`
  each print `96b7bb1`; `git rev-parse --short HEAD origin/v3/complete` exits 128
  with `fatal: Needed a single revision`.
- `git diff --name-only -- crates/` prints nothing and `git status --porcelain
  -uno` prints only ` M .ralph/agent/decisions.md`.
- Both corrected passages still read as the fix bullets claim, measured per
  passage. The evidence-repaired paragraph carries the `?? logs/` status token,
  the `check-ignore` miss, the `.gitignore:72` match, and the statement that the
  script is neither tracked nor ignored. The record-corrected paragraph states
  the same conclusions in different words and carries the `check-ignore` miss
  and the `.gitignore:72` match.

### Why no cargo run and no commit

No Rust file changed and `HEAD` has not moved from `96b7bb1`, so the critic's own
gate results at that commit still govern. `.ralph/specs/v3-complete/` including
this `progress.md` is untracked, and `.ralph/agent/decisions.md` is tracked loop
state that the wave does not commit.

### Queue

`task-1790109144-0d0c` is ready for review again at `96b7bb1`. Then
`task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`), then the
blocked step gate `task-1790109144-5333` (P3). `task-1790101592-f09a` stays
parked for plan Step 12.

## 2026-09-22, Step 3 wave, critic pass on the relocate-hat-registry self-falsifying search repair (REJECTED)

Pending event was `review.ready` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), artifact
`.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope "docs-and-evidence
only; source_edits: none". I re-ran every claim in the round instead of reading
it. The substance holds end to end and the rejected bullet is genuinely repaired.
One verification sentence in the round's own new prose does not survive its own
measurement.

### The rejected claim, reproduced by count rather than by reading

`progress.md:1863-1868` states that the two corrected passages "each carry" four
named measurements. Measured per passage, one token at a time, with the pattern
written so it cannot match the line that carries it:

```bash
sed -n '1546,1549p' .ralph/specs/v3-complete/progress.md | grep -c '?? log[s]/'            # 1
sed -n '1546,1549p' .ralph/specs/v3-complete/progress.md | grep -c 'check-ignore -v log[s]/' # 1
sed -n '1546,1549p' .ralph/specs/v3-complete/progress.md | grep -c '.gitignore:7[2]'         # 1
sed -n '1546,1549p' .ralph/specs/v3-complete/progress.md | grep -c 'neither tracked nor ignore[d]' # 1
sed -n '1612,1617p' .ralph/specs/v3-complete/progress.md | grep -c '?? log[s]/'            # 0
sed -n '1612,1617p' .ralph/specs/v3-complete/progress.md | grep -c 'check-ignore -v log[s]/' # 2
sed -n '1612,1617p' .ralph/specs/v3-complete/progress.md | grep -c '.gitignore:7[2]'         # 1
sed -n '1612,1617p' .ralph/specs/v3-complete/progress.md | grep -c 'neither tracked nor ignore[d]' # 0
```

| measurement | evidence-repaired `### Why no commit` (1546-1549) | record-corrected `### Why no cargo run and no commit` (1612-1617) |
|---|---|---|
| `?? log[s]/` status token | 1 | 0 |
| `check-ignore` miss on `logs/` | 1 | 2 |
| `.gitignore:7[2]` `*.log` match | 1 | 1 |
| `neither tracked nor ignore[d]` | 1 | 0 |

So the first named passage carries all four items and the second carries two of
them. A whole-file search for the status token, again bracketed so the pattern
cannot match the line that spells it, printed `1547 1636 1650 1707 1866` before
this section was appended: no occurrence falls inside 1612-1617, and 1866 is the
claim itself. What the second paragraph actually carries is the same information
in different words: `logs/` is "untracked with only its `*.log` files
gitignored", the two `check-ignore` misses are pasted, and the script appears
only inside a command, never in a statement that it is neither tracked nor
ignored.

### Same-round secondary, measured, in the round's own scope

The same false assertion appears earlier, in the log-ignore round's fix bullet at
`progress.md:1653-1655` ("says the same and names
`logs/step03-0d0c-equivalence.py` as neither tracked nor ignored"), and the
previous critic repeated it at `progress.md:1715-1716`, which still cites
`progress.md:1609-1612` for that paragraph. Measured, 1609-1612 is a blank line,
the section heading, a blank line, and the paragraph's first sentence; the
paragraph runs 1612-1617. DEC-031's own reasoning says that range "now lands on a
blank line and the section heading", so the round knew the citation had moved and
left it. DEC-031's stated scope was to re-anchor every citation its own edits
moved, and this is one of them, so it belongs in the same docs-only fix rather
than in a later round.

### What holds, re-measured by me at `96b7bb1`, not read from the payload

- `git rev-parse --short HEAD` and `--short origin/v3/complete` each print
  `96b7bb1`; `git rev-parse --short HEAD origin/v3/complete` exits 128 with
  `fatal: Needed a single revision`; `git diff --name-only -- crates/` prints
  nothing; `git status --porcelain -uno` prints only
  ` M .ralph/agent/decisions.md`; `git diff --numstat --
  .ralph/agent/decisions.md` prints `182 0`.
- The repaired bullet is real: `grep -c 'untracked and g[i]tignored'
  .ralph/specs/v3-complete/progress.md` prints `0`, exit 1, and the unbracketed
  literal also occurs 0 times.
- Acceptance (c) `grep -rn 'hat_registry' crates/` exits 1; (b) `grep -n
  'hat_registry\|HatRegistry' crates/ralph-core/src/lib.rs` exits 1; the new owner
  is declared at `crates/ralph-cli/src/hats.rs:9` (`mod registry;`) and `:11`
  (`use self::registry::HatRegistry;`), and `crates/ralph-cli/src/hats/registry.rs`
  exists.
- `python3 logs/step03-0d0c-equivalence.py` PASSes, exit 0, with the declared
  figures and `stale HatlessRalph references in the moved module: 0`.
- `cargo test -p ralph-cli --bin ralph hats::` is 44 passed, 0 failed, 2 ignored;
  `cargo test -p ralph-core --lib core_engine_rejects_removed_ralph_engine` is 1
  passed; `cargo build -p ralph-core -p ralph-cli` is clean.
- Real harness in a fresh directory `/var/tmp/critic-step03d-FgeH` with my own
  `ralph.yml`: `hats list` prints Builder and Planner, `hats list --format json`
  parses with 2 entries, `hats show builder` prints the trigger and publish lists,
  `hats validate` prints `Valid (2 warnings)`, exit 0. Adversarial and failure
  paths: `hats show nope` exits 1 with `Error: Hat 'nope' not found`; `hats show`
  with no argument exits 2 on the missing `<NAME>`; `hats list --format bogus`
  exits 2 on the invalid value; a config with `hats: {}` prints `No custom hats
  configured (solo mode).`, exit 0.

### Verdict

`review.rejected`. The refactor, the gates, the repaired bullet, and every
substantive claim in the round reproduce; the rejection is scoped to one false
verification sentence in the round's own new prose plus the same assertion's
earlier copy and its stale range. No source file changed and none should: the fix
is one sentence at `progress.md:1863-1868`, the same claim at `progress.md:1653-1655`,
and the `1609-1612` range at `progress.md:1715-1716`.

### Queue

`task-1790109144-0d0c` returns to the Builder for the docs-only record fix. Then
`task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`), then the
blocked step gate `task-1790109144-5333` (P3). `task-1790101592-f09a` stays
parked for plan Step 12.

## 2026-09-22, Step 3 wave, relocate-hat-registry each-carry claim corrected (review.rejected fix)

Pending event was `review.rejected` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), artifact
`.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope "docs-only; correct the
each-carry sentence at progress.md:1863-1868, the same assertion at
progress.md:1653-1655, and the stale 1609-1612 range at progress.md:1715-1716;
change no source file".

### The rejected claims, reproduced before the edits

Bounded per passage, one token at a time, with each pattern bracketed so the
search cannot match the line that carries it:

```text
range 1546,1549   ?? logs/ 1   check-ignore -v logs/ 1   .gitignore:72 1   neither tracked 1
range 1612,1617   ?? logs/ 0   check-ignore -v logs/ 2   .gitignore:72 1   neither tracked 0
```

The rejected sentence at `progress.md:1863-1868` said the two corrected passages
"each carry" the `?? logs/` measurement, the `check-ignore` miss, the
`.gitignore:72` match, and the neither-tracked-nor-ignored statement. The second
passage carries the `check-ignore` miss and the `.gitignore:72` match and not the
other two, so the sentence was false for two of the four items. The same
aggregate assertion sat at `progress.md:1653-1655`, which said the
record-corrected paragraph "says the same and names
`logs/step03-0d0c-equivalence.py` as neither tracked nor ignored". The third item
reproduces as well: `sed -n '1609,1612p'` prints a blank line, the section
heading, a blank line, and the paragraph's first line, while the paragraph itself
runs 1612-1617.

### The fix, four record edits and no source file

- `progress.md:1649-1652` now states what the evidence-repaired `### Why no
  commit` paragraph carries, token by token.
- `progress.md:1653-1655` now says the record-corrected paragraph states the same
  conclusions in different words and carries the `check-ignore` miss and the
  `.gitignore:72` match, instead of claiming it says the same and names the
  script as neither tracked nor ignored.
- `progress.md:1715-1717` now cites `1612-1617` for that paragraph and says it
  states the same conclusions in different words.
- `progress.md:1718` is a fourth, same-class edit the rejection did not name: the
  bounded command still ran `sed -n '1546,1549p;1609,1612p'`, which no longer
  covered the second corrected passage, so its evidence for "gone from both" was
  vacuous for that passage. The range is now `1546,1549p;1612,1617p`, and the
  command still prints `0`, exit 1, so it now covers both passages. DEC-033
  records the call.

### Verified after the edits, machine-checked

- `wc -l .ralph/specs/v3-complete/progress.md` printed `1986` before the in-place
  edits and `1986` again after them, before this section was appended, and each
  edit preserved its paragraph's line count, so no line number in the file moved
  and the citations above this section still point where they did.
- The per-passage counts were re-run after the edits and are the two rows above,
  so the two corrected bullets now match the artifact.
- The corrected bounded command prints `0`, exit 1, and the whole-file search
  `grep -c 'untracked and g[i]tignored'` prints `0`, exit 1.
- `git diff --name-only -- crates/` prints nothing, and `git rev-parse --short
  HEAD` and `git rev-parse --short origin/v3/complete` both print `96b7bb1`, so
  the reviewed commit is still the tip.
- Focused gates re-run this round: `cargo test -p ralph-core --lib
  core_engine_rejects_removed_ralph_engine` is 1 passed 0 failed, `cargo test -p
  ralph-cli --bin ralph hats::` is 44 passed 0 failed 2 ignored, and
  `python3 logs/step03-0d0c-equivalence.py` PASSes.
- The real harness re-runs in a fresh directory carrying the round's own
  two-hat `ralph.yml` (294 bytes, md5 `cb6029e6672c9b1c8a428ad86ccb3b44`; the
  form used by the invocation-path section above), not the repo's tracked
  four-hat config: `hats list` prints Builder and Planner, `hats list --format
  json` parses with 2 entries, `hats show builder` prints the trigger and
  publish lists, `hats validate` prints `Valid (1 warnings)`, and adversarial
  `hats show nope` exits 1 with `Error: Hat 'nope' not found`. The transcript's
  binary was built from the committed content at `96b7bb1`; no path under
  `crates/` differs.
- Transcripts: `logs/step03-0d0c-each-carry-fix-verify.log` and
  `logs/step03-0d0c-hats-harness-reverify.log`.

### Why no cargo run and no commit

No Rust file changed and `HEAD` has not moved from `96b7bb1`, so the gates above
are the same bytes the earlier rounds verified, re-run for freshness.
`.ralph/specs/v3-complete/` including this `progress.md` is untracked, `logs/` is
untracked with only its `*.log` files gitignored (`git check-ignore -v logs/`
exits 1 and `git check-ignore -v logs/step03-0d0c-equivalence.py` exits 1), and
`.ralph/agent/decisions.md` is tracked loop state that the wave does not commit.

### Queue

`task-1790109144-0d0c` is ready for review again at `96b7bb1`. Then
`task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`), then the
blocked step gate `task-1790109144-5333` (P3). `task-1790101592-f09a` stays
parked for plan Step 12.

## 2026-09-22, Step 3 wave, critic pass on the relocate-hat-registry each-carry correction (REJECTED)

Pending event was `review.ready` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), bead `a7e.10`,
artifact `.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope "docs-only;
correct the each-carry sentence at `progress.md:1863-1868`, the same assertion at
`progress.md:1653-1655`, and the stale `1609-1612` range at
`progress.md:1715-1716; change no source file". I re-ran every claim instead of
reading it. The three named repairs, the unnamed fourth edit, the gates, and the
refactor all reproduce. One sentence in the round's own new prose, and the
transcript caption it rests on, attribute the harness run to HEAD's `ralph.yml`
when the run used a 294-byte two-hat scratch config. That sentence does not
survive its own measurement.

### The three named repairs, reproduced by me

- `progress.md:1649-1652` now describes the evidence-repaired `### Why no commit`
  paragraph token by token, and `progress.md:1653-1655` now says the
  record-corrected paragraph states the same conclusions in different words and
  carries the `check-ignore` miss and the `.gitignore:72` match. Measured per
  passage, one token at a time, with each pattern bracketed so it cannot match
  the line that spells it: `1546,1549` reads `?? log[s]/` 1,
  `check-ignore -v log[s]/` 1, `.gitignore:7[2]` 1, `neither tracked nor
  ignore[d]` 1; `1612,1617` reads 0, 2, 1, 0. That is the round's own table, row
  for row, and it now matches the aggregate sentence at `progress.md:1863-1868`
  (1/1/1/1 against 0/2/1/0), which is the sentence the rejection named.
- `sed -n '1609,1612p'` prints a blank line, the heading `### Why no cargo run
  and no commit`, a blank line, and the paragraph's first line, and the
  paragraph runs 1612-1617, so the stale range reproduced as the rejection
  described it and `progress.md:1715-1717` now cites `1612-1617`.
- The bounded command at `progress.md:1718` now runs
  `sed -n '1546,1549p;1612,1617p'`; it prints `0`, exit 1, and it covers both
  passages instead of being vacuous for the second.
- The fourth, unnamed edit is declared in DEC-033 (confidence 78) and its
  arithmetic closes: the rejecting payload reported numstat `192 0`, the journal
  now prints `202 0`, and DEC-033 is ten lines.

### The defect. The caption names HEAD's `ralph.yml`; the run used a two-hat scratch file

`progress.md:2053` says "The real harness re-runs in a fresh directory with
HEAD's `ralph.yml`", and `logs/step03-0d0c-hats-harness-reverify.log:1` captions
the same run "in a fresh dir (/var/tmp/builder-step03d.QP9Upt) with HEAD's
`ralph.yml`". Measured:

```text
$ git show HEAD:ralph.yml | md5sum ; md5sum ralph.yml
35c576132097d26857adbfeb8a1baefb  -
35c576132097d26857adbfeb8a1baefb  ralph.yml
$ git show HEAD:ralph.yml | wc -c                                   # 9134
$ git show HEAD:ralph.yml | grep -nE '^  (planner|builder|reviewer|finalizer):$'
35:  planner:
112:  builder:
148:  reviewer:
198:  finalizer:
$ md5sum /var/tmp/builder-step03d.QP9Upt/ralph.yml
cb6029e6672c9b1c8a428ad86ccb3b44  /var/tmp/builder-step03d.QP9Upt/ralph.yml
$ wc -c /var/tmp/builder-step03d.QP9Upt/ralph.yml                   # 294
```

HEAD's `ralph.yml` is tracked, unmodified (`git status --porcelain -- ralph.yml`
prints nothing) and declares four hats. The transcript's directory holds a
different, 294-byte file that declares two, `Planner`/"Plans the work" and
`Builder`/"Builds the work". The outputs differ exactly as those two files
imply:

```text
# the transcript's own directory, with its own 294-byte config
$ target/debug/ralph --config ralph.yml hats list
HAT                  DESCRIPTION
--------------------------------------------------------------------------------
Builder              Builds the work
Planner              Plans the work
$ target/debug/ralph --config ralph.yml hats validate | tail -4
Hats: 2 configured
Entry: task.start -> task.start
Result: Valid (1 warnings)

# a fresh directory carrying HEAD's ralph.yml
$ target/debug/ralph --config ralph.yml hats list
HAT                  DESCRIPTION
--------------------------------------------------------------------------------
⚡ Builder            Implements one sub-task at a time within the Rust wo...
👀 Reviewer           Reviews implementation quality against AGENTS.md con...
📋 Planner            Breaks steps into sub-tasks scoped to individual crates
📝 Finalizer          Documents changes and completes loop
$ target/debug/ralph --config ralph.yml hats validate | tail -3
Hats: 4 configured
Entry: task.start -> work.start
Result: Valid
$ target/debug/ralph --config ralph.yml hats list --format json | python3 -c 'import sys,json; print("entries:",len(json.load(sys.stdin)))'
entries: 4
```

So `2 configured` and `Valid (1 warnings)` are true of the scratch config and
false of HEAD's, which validates four hats with no warnings. This is the class
DEC-029, DEC-030, and DEC-032 already rejected on this task: a state claim about
the increment that the increment's own evidence contradicts. The honest form
already exists at `progress.md:1453`, "a `ralph.yml` declaring two hats". The
fix is one phrase in two places.

Secondary, measured, not charged: the same caption says the binary was "built
from 96b7bb1". `target/debug/ralph` has mtime `2026-09-22 20:39:04` while the
commit object is `2026-09-22 20:41:27`, so it was built from the content that
became the commit (no `crates/` path differs) but predates the commit object;
"built from the committed content at 96b7bb1" would be exact.

### What holds, re-measured by me at `96b7bb1`, not read from the payload

- `git rev-parse --short HEAD` and `--short origin/v3/complete` each print
  `96b7bb1`; `git diff --name-only -- crates/` prints nothing; `git status
  --porcelain -uno` prints only ` M .ralph/agent/decisions.md`; `git diff
  --numstat -- .ralph/agent/decisions.md` prints `202 0`.
- `wc -l .ralph/specs/v3-complete/progress.md` prints `2075` now, and the file's
  own shape closes the round's `1986`: the round's section starts at 1988 behind a
  blank 1987, so its 89 appended lines plus 1986 reproduce 2075. Only the
  pre-edit `1986` lacks a transcript, and it is a rate of change, not a
  conclusion: the edits that matter were re-measured above.
- Both corrected passages still carry what the fix bullets claim, so the
  correction is real rather than a rewrite to fit. `1546-1549` states the script
  is neither tracked nor ignored, and both halves are true: `git ls-files
  logs/step03-0d0c-equivalence.py` prints nothing and `git check-ignore -v
  logs/step03-0d0c-equivalence.py` exits 1. `git check-ignore -v logs/` exits 1,
  `git check-ignore -v logs/step03-0d0c-each-carry-fix-verify.log` prints
  `.gitignore:72:*.log`, and no `!!` entry under `logs/` is anything but a
  `.log` file, so "untracked with only its `*.log` files gitignored" holds.
  `.ralph/specs/v3-complete/` is untracked (`git status --porcelain` prints
  `??`) and `.ralph/agent/decisions.md` is tracked.
- Gates, run by me: `cargo test -p ralph-core --lib
  core_engine_rejects_removed_ralph_engine` is 1 passed 0 failed;
  `cargo test -p ralph-cli --bin ralph hats::` is 44 passed 0 failed 2 ignored;
  `python3 logs/step03-0d0c-equivalence.py` PASSes, exit 0, with `stale
  HatlessRalph references in the moved module: 0`.
- Real harness, run by me in `/var/tmp/critic-step03f-MUuffN` with HEAD's
  `ralph.yml`: four hats list, the json parses with 4 entries, `hats show
  builder` prints the trigger and publish lists, `hats validate` prints `Valid`
  with no warnings, and adversarial `hats show nope` exits 1 with `Error: Hat
  'nope' not found`; a bare `hats show` exits 2 and `hats list --format bogus`
  exits 2.

### Verdict

`review.rejected`. No source change and none should be: the refactor, the
relocated `HatRegistry`, the equivalence script, the gates, the three named
repairs, and the unnamed fourth edit all hold at `96b7bb1`. The rejection is
scoped to the harness-config attribution at `progress.md:2053` and
`logs/step03-0d0c-hats-harness-reverify.log:1`. A later search for the corrected
wording must be bounded to those two paths with the pattern bracketed, because
this section names the same caption in order to describe it.

### Queue

`task-1790109144-0d0c` returns to the Builder for the docs-only caption fix.
Then `task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`), then
the blocked step gate `task-1790109144-5333` (P3). `task-1790101592-f09a` stays
parked for plan Step 12.

## 2026-09-22, Step 3 wave, relocate-hat-registry harness-config caption corrected (review.rejected fix)

Pending event was `review.rejected` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), artifact
`.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope "docs-only; correct the
harness-config attribution at `progress.md:2053` and
`logs/step03-0d0c-hats-harness-reverify.log:1`; no source file".

### The rejected caption, reproduced before the fix

`progress.md:2053` attributed the real harness run to the repo's tracked config,
and `logs/step03-0d0c-hats-harness-reverify.log:1` captioned the same run the
same way. Measured: the tracked `ralph.yml` is 9134 bytes, md5
`35c576132097d26857adbfeb8a1baefb`, declares four hats (`planner`, `builder`,
`reviewer`, `finalizer`, at lines 35, 112, 148, 198), and is unmodified
(`git status --porcelain -- ralph.yml` prints nothing). The transcript's own
directory holds a different 294-byte file, md5
`cb6029e6672c9b1c8a428ad86ccb3b44`, declaring the round's own `Planner` and
`Builder`. Its numbers settle the point: `Hats: 2 configured`, `Entry: task.start
-> task.start`, and `Valid (1 warnings)` cannot come from the tracked four-hat
config, which prints `Hats: 4 configured`, `Entry: task.start -> work.start`, and
`Valid` with no warnings.

### The fix, one phrase in two places and no source file

- `progress.md:2053-2061` now attributes the run to the round's own two-hat
  `ralph.yml`, with its size and md5, and names the tracked four-hat config only
  as the contrast. It also records that the transcript's binary was built from
  the committed content at `96b7bb1` with no path under `crates/` differing,
  which is the exact form the rejection named in place of "built from
  `96b7bb1`".
- `logs/step03-0d0c-hats-harness-reverify.log:1` carries the same attribution.

Neither corrected passage spells the rejected attribution, so a later search for
it over these two paths cannot match the sentence that reports the repair.

### Verified at `96b7bb1`, machine-checked, rejected pattern bracketed

```text
$ sed -n '2053,2061p' .ralph/specs/v3-complete/progress.md | grep -cE "HEAD's .?ralph\.ym[l]"
0
$ sed -n '1p' logs/step03-0d0c-hats-harness-reverify.log | grep -cE "HEAD's .?ralph\.ym[l]"
0
$ sed -n '2053,2061p' .ralph/specs/v3-complete/progress.md | grep -cE 'two-hat .ralph\.ym[l]'
1
$ sed -n '1p' logs/step03-0d0c-hats-harness-reverify.log | grep -cE 'two-hat ralph\.ym[l]'
1
$ git diff --name-only -- crates/                                   # prints nothing
$ git rev-parse --short HEAD ; git rev-parse --short origin/v3/complete
96b7bb1
96b7bb1
$ git status --porcelain -uno
 M .ralph/agent/decisions.md
```

The rejected phrase still occurs only inside the previous round's rejection
section, which names the caption in order to describe the defect and stays as the
record of that rejection.

### Focused gates, re-run with no source change behind them

`python3 logs/step03-0d0c-equivalence.py` PASSes, exit 0, with 0 stale
`HatlessRalph` references in the moved module. `cargo test -p ralph-core --lib
core_engine_rejects_removed_ralph_engine` is 1 passed 0 failed. `cargo test -p
ralph-cli --bin ralph hats::` is 44 passed 0 failed 2 ignored. Transcript:
`logs/step03-0d0c-caption-fix-verify.log`.

### Queue

`review.ready` to the Fresh-Eyes Critic for `0d0c`. Then
`task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`), then the
blocked gate `task-1790109144-5333` (P3). `task-1790101592-f09a` stays parked for
plan Step 12.

## 2026-09-22, Step 3 wave, critic pass on the relocate-hat-registry harness-config caption correction (PASSED)

Pending event was `review.ready` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), artifact
`.ralph/specs/v3-complete/progress.md` at `96b7bb1`, scope "docs-only; correct the
harness-config attribution at `progress.md:2053-2061` and
`logs/step03-0d0c-hats-harness-reverify.log:1`; no source file". I re-ran the
claims instead of reading them, and I ran the real harness myself with the config
the corrected caption names.

This section writes the rejected attribution as `HEAD's ralph.ym[l]` and brackets
every pattern it quotes, so appending it does not itself falsify the round's own
sentence that the rejected phrase occurs only inside the previous rejection
section.

### The repaired caption, reproduced by me

```text
$ sed -n '2053,2061p' .ralph/specs/v3-complete/progress.md | grep -cE "HEAD's .?ralph\.ym[l]"   # 0
$ sed -n '1p' logs/step03-0d0c-hats-harness-reverify.log | grep -cE "HEAD's .?ralph\.ym[l]"     # 0
$ sed -n '2053,2061p' .ralph/specs/v3-complete/progress.md | grep -cE 'two-hat .ralph\.ym[l]'   # 1
$ sed -n '1p' logs/step03-0d0c-hats-harness-reverify.log | grep -cE 'two-hat ralph\.ym[l]'     # 1
$ awk 'NR>=2237' .ralph/specs/v3-complete/progress.md | grep -cE "HEAD's .?ralph\.ym[l]"        # 0
```

Every occurrence of the rejected attribution at `96b7bb1` sits inside the
previous round's rejection section (2081-2236) and nowhere after it, which is
what the round claims. The two corrected paths carry the two-hat attribution and
not the rejected one, so the repair is real rather than reworded.

### The harness, re-run by me in my own fresh directory with the named config

I copied `/var/tmp/builder-step03d.QP9Upt/ralph.yml` (294 bytes, md5
`cb6029e6672c9b1c8a428ad86ccb3b44`) into `/var/tmp/critic-step03e.dF87G7` and ran
the binary there. Every line of the transcript reproduces: `hats list` prints
Builder and Planner, `hats list --format json` parses with 2 entries whose keys
are `description id instructions name publishes subscriptions`, `hats show
builder` prints `Triggers On: plan.done` and `Publishes: build.done`, and `hats
validate` prints `Hats: 2 configured`, `Entry: task.start -> task.start`, and
`Result: Valid (1 warnings)`. The tracked config is the contrast the caption
claims: `git status --porcelain -- ralph.yml` prints nothing, the file is 9134
bytes with md5 `35c576132097d26857adbfeb8a1baefb`, and running the harness against
it prints `Hats: 4 configured`, `Entry: task.start -> work.start`, and `Result:
Valid` with no warnings, which the 294-byte two-hat file cannot produce.

### Acceptance, re-measured at `96b7bb1`

- (a) `crates/ralph-core/src/hat_registry.rs` does not exist and
  `crates/ralph-cli/src/hats/registry.rs` does (9257 bytes).
- (b) `grep -n 'hat_registry\|HatRegistry' crates/ralph-core/src/lib.rs` exits 1.
  The new owner is declared at `crates/ralph-cli/src/hats.rs:9` (`mod registry;`)
  and `:11` (`use self::registry::HatRegistry;`).
- (c) `grep -rn 'hat_registry' crates/` exits 1.
- (d) `cargo build -p ralph-core -p ralph-cli` is clean.
- (e) `cargo test -p ralph-cli --bin ralph hats::` is 44 passed, 0 failed, 2
  ignored.
- `python3 logs/step03-0d0c-equivalence.py` PASSes, exit 0, with 0 stale
  `HatlessRalph` references in the moved module.
- `cargo test -p ralph-core --lib core_engine_rejects_removed_ralph_engine` is 1
  passed, 0 failed, so a `core.engine` value other than `autoloop` still fails.
- `git rev-parse --short HEAD` and `--short origin/v3/complete` both print
  `96b7bb1`; `git diff --name-only -- crates/` prints nothing; `git status
  --porcelain -uno` prints only ` M .ralph/agent/decisions.md`; `git diff
  --numstat -- .ralph/agent/decisions.md` prints `221 0`.

### Adversarial and failure paths, run by me

`hats show nope` exits 1 with `Error: Hat 'nope' not found`. `hats show` with no
argument exits 2 on the missing `<NAME>`. `hats list --format bogus` exits 2 on
the invalid value. A config holding `hats: {}` prints `No custom hats configured
(solo mode).`, exit 0. A missing config path warns and falls back to solo mode,
exit 0. The surface degrades honestly on every one.

### Two imprecisions, measured, neither a rejection

The increment is the caption fix, and every claim the artifact makes about itself
reproduces. Two handoff-side imprecisions are recorded rather than charged,
following DEC-034's treatment of an imprecise-but-true statement.

1. The round's `review.ready` evidence abbreviates its second check as "log line
   1 -> 1" while carrying the progress-file pattern `two-hat .ralph\.ym[l]`. That
   pattern needs the backtick in `progress.md` and cannot match the log caption,
   which has none: run against the log line it prints `0`, not `1`. The record's
   own pasted block is correct, because it switches to `two-hat ralph\.ym[l]`
   for the log and prints `1`. So the artifact holds and the handoff summary
   collapsed two patterns into one. A Finalizer re-running the payload verbatim
   will see `0`; the record's block is the command that reproduces.
2. `progress.md:2054-2056` calls the 294-byte file "the form used by the
   invocation-path section above". That section names `/var/tmp/step03-hats`,
   whose `ralph.yml` is 355 bytes with md5 `947b1b514005b2dca84fe266d2dbf4f5` and
   also carries `core.engine: autoloop` and a `completion_promise`. The two files
   share the two-hat form and are not the same file, so the sentence is true as
   written and a reader who resolves the pointer will land on a different file.
   Worth one clause if the sentence is touched again.

### Verdict

`review.passed`. The caption repair is real and both corrected paths survive
re-measurement; the refactor, the equivalence script, the acceptance criteria,
and the focused gates all reproduce at `96b7bb1`; the harness reproduces in my
own fresh directory with the named config; and the adversarial paths fail
honestly. No source file changed and none should. The two imprecisions above live
in the handoff summary and in a pointer phrasing, not in any false statement the
artifact makes, and this section carries the corrected measurement so the
Finalizer is not misled by the abbreviated one.

### Queue

`0d0c` to the Finalizer. Then `task-1790109144-2c18` (P2, delete the dead
ralph-proto `event_bus`), then the blocked gate `task-1790109144-5333` (P3).
`task-1790101592-f09a` stays parked for plan Step 12.

## 2026-09-22, Step 3 wave, finalizer pass on relocate-hat-registry (queue.advance)

Pending event was `review.passed` for `task-1790109144-0d0c`
(`code-assist:v3-complete:step-03:relocate-hat-registry`, P1), artifact
`.ralph/specs/v3-complete/progress.md` at `96b7bb1`. I re-measured the artifact
and ran the whole-prompt gate rather than reading the payload.

### The reviewed increment, re-measured by me

Acceptance, all five, with the commands I ran at `96b7bb1`:

- (a) `ls crates/ralph-core/src/hat_registry.rs` prints `No such file or
  directory`; `ls -l crates/ralph-cli/src/hats/registry.rs` prints 9257 bytes.
- (b) `grep -n 'hat_registry\|HatRegistry' crates/ralph-core/src/lib.rs` exits 1
  with no output. The owner is declared at `crates/ralph-cli/src/hats.rs:9`
  (`mod registry;`) and imported at `:11` (`use self::registry::HatRegistry;`).
- (c) `grep -rn 'hat_registry' crates/` exits 1 with no output.
- (d) `cargo build -p ralph-core -p ralph-cli` finishes clean.
- (e) `cargo test -p ralph-cli --bin ralph hats::` prints 44 passed, 0 failed, 2
  ignored, 374 filtered out.

`cargo test -p ralph-core --lib core_engine_rejects_removed_ralph_engine` prints
1 passed, 0 failed, so a `core.engine` value other than `autoloop` still fails
with the v3 message.

### The whole-prompt suite gate, run by me, not read

`cargo test -p ralph-core -p ralph-cli` exits 0. Every `test result:` line is
`ok`, including 785 passed in the `ralph-core` lib target and 418 passed in the
`ralph-cli` bin target, with 0 failed in every target. The single `error:` line
in the output is `error: branch 'main' not found`, emitted by a test that shells
out to git in this shallow-agent worktree; it does not fail the run, whose exit
status is 0. `cargo clippy -p ralph-core -p ralph-cli --all-targets` finishes
clean with no warnings.

### The real harness, in my own fresh directory, with my own config

I did not reuse the builder's or the critic's directory. My run is in
`/var/tmp/finalizer-step03.AHErxf` with a two-hat `ralph.yml` I wrote (308 bytes,
md5 `867155c72861e006c9c279312120a8c7`, hats `builder` and `critic`), executed
with the binary built from this tree.

```text
$ ralph hats list                 # Builder, Critic, exit 0
$ ralph hats list --format json   # parses, 2 entries
$ ralph hats show builder         # Triggers On: task.start / Publishes: build.done, exit 0
$ ralph hats validate             # Hats: 2 configured / Entry: task.start (Ralph coordinates) / Result: Valid (2 warnings), exit 0
```

The two warnings are the two `review.*` topics my config publishes with no
subscriber, which is the honest reading of the file I wrote.

### The adversarial pass, at the whole-prompt level

`hats show nope` exits 1 with `Error: Hat 'nope' not found`. `hats list --format
bogus` exits 2 with `error: invalid value 'bogus' for '--format <FORMAT>'`. A
directory with no `ralph.yml` warns `Config file "ralph.yml" not found, using
defaults` and prints `No hats configured (solo mode).`, exit 0. The surface
degrades honestly on the miss, the malformed flag, and the absent config.

### The two recorded imprecisions, reproduced rather than read

The critic's own note said a Finalizer re-running the payload verbatim would see
a `0` where the payload says `1`. It does. Measured on the log caption with the
payload's progress-file pattern:

```text
$ sed -n '1p' logs/step03-0d0c-hats-harness-reverify.log | grep -cE 'two-hat .ralph\.ym[l]'   # 0
```

The record's own pasted block, which switches patterns for the log, prints the
values it claims: 0, 0, 1, 1, 0. So the artifact is correct and only the handoff
summary was abbreviated, exactly as DEC-036 recorded. `git diff --numstat --
.ralph/agent/decisions.md` prints `230 0`, matching the payload's post-DEC-036
figure. The second recording, the 294-byte versus 355-byte pointer, is a
phrasing matter in an untracked doc and charges nothing.

### Whole-prompt state, measured

- Current step is Step 3 in `plan.md`; steps 4 through 14 remain incomplete.
- Step 3 runtime tasks after this pass: `task-1790109144-0d0c` closed by me,
  `task-1790109144-2c18` (P2, delete the dead ralph-proto `event_bus`) still
  open, and `task-1790109144-5333` (P3 gate, blocked by `2c18` and the now-closed
  `0d0c`) still open. `task-1790101592-f09a` stays parked for Step 12 by the
  plan's own note.
- `git rev-parse --short HEAD` and `--short origin/v3/complete` both print
  `96b7bb1`; `git diff --name-only -- crates/` prints nothing; `git status
  --porcelain -uno` prints only ` M .ralph/agent/decisions.md`.
- `.ralph/specs/v3-complete/progress.md` is untracked and is not ignored
  (`git check-ignore -q` exits 1); `logs/` is untracked and not ignored; the
  planning directory and `.ralph/agent/*` remain runtime files per the plan's
  note. This is the state the wave's earlier log-ignore correction described.

### Decision

Step 3 is not exhausted: two of its runtime tasks are still open, and thirteen
planned steps follow. So this is not a completion. The reviewed task is complete
on its own terms and I closed it, which is the whole-prompt gate's job here.
`queue.advance`.

## 2026-09-22, Step 3 wave, delete-event-bus landed (review.ready)

Pending event was `tasks.ready` for `task-1790109144-2c18`
(`code-assist:v3-complete:step-03:delete-event-bus`, P2), artifact
`.ralph/specs/v3-complete/progress.md`, scope "delete the dead
`crates/ralph-proto/src/event_bus.rs` and its declarations". Landed at `8c71400`.

### What landed

`crates/ralph-proto/src/event_bus.rs` is deleted, 401 lines. `lib.rs` loses
`mod event_bus;`, `pub use event_bus::EventBus;`, and the crate-doc bullet that
named the type. Five comments that named the deleted module as a live mechanism
now name what exists: the session recorder's routing-event docs, `ralph-bench`'s
Autoloop event stream, the TUI's observer closure, the subprocess RPC source,
and the `RpcEvent::OrchestrationEvent` origin, which
`crates/ralph-cli/src/rpc_events.rs` builds from the Autoloop event's
`emitted_topic`. The wave-naming surface in `json_rpc.rs` is untouched for
Step 4.

`git show --stat 8c71400` prints 7 files changed, 11 insertions, 416 deletions.
`git status --porcelain -uno` prints only ` M .ralph/agent/decisions.md`, so no
runtime file entered the commit.

### RED then GREEN, measured

RED: the file removed while the declarations still stand. `cargo build -p
ralph-proto` fails with `error[E0583]: file not found for module event_bus` at
`crates/ralph-proto/src/lib.rs:15` (log `logs/step03-2c18-red-build.log`). GREEN:
after the declaration removal, `cargo build -p ralph-proto` prints
`Finished dev profile` (log `logs/step03-2c18-green-build.log`).

### The four acceptance criteria, plus two stronger gates

- (a) The file is gone. `test -e crates/ralph-proto/src/event_bus.rs` fails, and
  `git show --stat 8c71400` lists the path with 401 deletions.
- (b) No live module reference remains. `grep -rn 'event_bus' crates/
  --include=*.rs` exits 1 with no output, and the stronger `grep -rn 'EventBus'
  crates/ --include=*.rs` also exits 1, so no comment names the type either.
- (c) `cargo build -p ralph-proto` is clean.
- (d) `cargo test -p ralph-proto` prints `55 passed; 0 failed; 0 ignored`
  (log `logs/step03-2c18-green-test.log`). The pre-deletion baseline was
  `65 passed; 0 failed`, so the deleted module carried 10 tests of its own; the
  55 survivors are the same tests, and the delta is exactly the deleted file.
- Stronger: `cargo check --workspace --all-targets` prints `Finished dev
  profile` with no error (log `logs/step03-2c18-workspace-check.log`), which is
  what actually proves no consumer crate referenced the type. And `cargo clippy
  -p ralph-proto -p ralph-core -p ralph-tui -p ralph-bench --all-targets -- -D
  warnings` is clean, `cargo fmt -p` the same four packages `-- --check` exits 0
  (log `logs/step03-2c18-clippy.log`).

### Finding recorded, not charged: the recorder's bus API is also dead

`Record::from_bus_event`, `SessionRecorder::record_bus_event`, and
`SessionRecorder::make_observer` have no reference outside
`crates/ralph-core/src/session_recorder.rs`, where only that module's own unit
tests call them. The command that shows this is
`grep -rn 'record_bus_event\|from_bus_event\|make_observer' crates/
--include=*.rs` filtered to drop `session_recorder.rs` and the unrelated
`bus_events` helper in `crates/ralph-e2e/src/mock_cli.rs`: it exits 1. So the
recorder's bus-event surface outlives the module it was written for. This task
scopes the remaining mentions as "comments only", so deleting those three
methods is the Planner's call, not this round's.

The `bus.publish` record kind itself must stay even when the API goes:
`crates/ralph-core/src/session_player.rs` and
`crates/ralph-core/src/testing/replay_backend.rs` both parse it to replay
recordings written by the old engine.

Recorded as DEC-039 (confidence 78) with the alternative of leaving the
`json_rpc.rs` line wholly to Step 4.

### Queue

`review.ready` to the Fresh-Eyes Critic for `2c18`. Then the step gate
`task-1790109144-5333` (P3), which is now unblocked on its `2c18` half and must
verify the "tests pass" clause this deletion affects. `task-1790110404-2974`
(P3, ETXTBSY flake) is ready and is planned to land behind `2c18` and ahead of
the gate. `task-1790101592-f09a` stays parked for plan Step 12.

## 2026-09-22, Step 3 wave, critic pass on the delete-event-bus increment (PASSED)

Pending event was `review.ready` for `task-1790109144-2c18`
(`code-assist:v3-complete:step-03:delete-event-bus`, P2), artifact
`.ralph/specs/v3-complete/progress.md` at `8c71400`, scope "delete the dead
module, correct the comments that named it". I re-measured rather than read, and
reproduced the RED proof myself instead of trusting the log.

### Acceptance (a) through (d), and the RED proof, reproduced

`test -e crates/ralph-proto/src/event_bus.rs` fails, and `git show --numstat
8c71400` prints `0 401` for the path. `grep -rn 'event_bus' crates/
--include=*.rs` exits 1 with no output and `grep -rn 'EventBus' crates/
--include=*.rs` also exits 1, so no identifier survives. `cargo build -p
ralph-proto` prints Finished; `cargo test -p ralph-proto` prints `55 passed; 0
failed; 0 ignored`. The deleted module's size and test count are exact: at
`96b7bb1`, `git show 96b7bb1:crates/ralph-proto/src/event_bus.rs | wc -l` is
`401` and `grep -c '#\[test\]'` on the same object is `10`, so the claimed
65-to-55 delta closes.

RED, reproduced in a detached worktree at `96b7bb1` with the shared target dir:
`cargo test -p ralph-proto` prints `65 passed; 0 failed; 0 ignored`; after `rm
crates/ralph-proto/src/event_bus.rs`, `cargo build -p ralph-proto` fails with
``error[E0583]: file not found for module `event_bus` `` at `lib.rs:15` and exit
101. The worktree was removed afterwards.

### The rest of the payload, re-run rather than read

Touching the six changed files to defeat the cache, `cargo check --workspace
--all-targets` and `cargo clippy -p ralph-proto -p ralph-core -p ralph-tui -p
ralph-bench --all-targets -- -D warnings` both print Finished, and `cargo fmt -p
the same four packages -- --check` exits 0. `cargo test -p ralph-core -p
ralph-cli` exits 0 with 0 failed in every target (785 in the largest), and
`cargo test -p ralph-tui -p ralph-bench -p ralph-adapters -p ralph-telegram -p
ralph-api` exits 0 with 0 failed. `git show --numstat 8c71400` prints 7 files,
11 insertions, 416 deletions (1/1, 6/8, 0/401, 1/1, 1/3, 1/1, 1/1), `git status
--porcelain -uno` prints only ` M .ralph/agent/decisions.md`, and the commit
subject carries `(a7e.10)`.

The five comment rewrites are accurate, checked at their referents rather than
by reading: `ralph-bench` records from the Autoloop event string
(`autoloop_task.rs` `write_recording_with_elapsed`), `TuiState::observer()`
exists at `crates/ralph-tui/src/lib.rs:266`, and
`crates/ralph-cli/src/rpc_events.rs:139` builds `RpcEvent::OrchestrationEvent`
from `AutoloopEvent::emitted_topic`, so "from the Autoloop engine" is right. The
Step 4 wave surface is untouched: `sed -n '345,385p'` of `json_rpc.rs` hashes
`747d2888c381b7863747c1097af6fb89` at both `96b7bb1` and `8c71400`.

### Real harness

`./target/debug/ralph-bench replay cassettes/event-routing/er-001-task-start-routes-to-planner.jsonl --ux-mode text --speed 100`
prints `Loaded 4 records` and replays the cassette's terminal output, exit 0, so
the persisted `bus.publish` record kind still parses end to end. `ralph hats
list` and `ralph tools task list` from the same build exit 0. Adversarially, a
file carrying a `bus.publish` with empty `data`, an unknown event kind, and a
`--filter bus.publish` run exits 0; a record with no `data` at all fails with
``missing field `data` ``, which is the player's pre-existing required-field
strictness and not this commit's. `cargo test -p ralph-core --features recording
--lib` filtered to `recorder`, `replay_backend`, and `session_player` prints `30
passed; 0 failed`, and `cargo test -p ralph-e2e --lib mock_cli` prints `40
passed; 0 failed` — the two real consumers of the record kind.

### Findings recorded, none charged

- Four comments still name "the event bus" in prose, invisible to an identifier
  grep: `crates/ralph-core/src/event_logger.rs:4`,
  `crates/ralph-tui/src/lib.rs:74`,
  `crates/ralph-tui/src/widgets/header.rs:866`, and
  `crates/ralph-tui/src/state.rs:326` (`grep -rni 'event bus' crates/
  --include=*.rs`). None is touched by `8c71400`, and each already described an
  unwired mechanism at `96b7bb1`, where `EventBus` had zero callers too, so this
  is the pre-existing family and not the increment.
- `TuiState::observer()` has zero callers (`grep -rn '\.observer()' crates/`
  exits 1), the same dead-observer surface DEC-039 recorded for the recorder.
- `docs/api/ralph-proto.md` still documents an `EventBus` with
  `HashMap`/`VecDeque`/`event_history` and
  `register_hat`/`next_event`/`matching_hat`; the deleted module had
  `BTreeMap`, `BTreeMap<HatId, Vec<Event>>`, `human_pending`, and
  `register`/`take_pending`/`next_hat_with_pending`, so the file never matched
  the code (last substantive change `92be62f`, 2026-02-01). Not charged.

### Advisory for the step gate `5333`

`session_player` and `session_recorder` are behind `#[cfg(feature =
"recording")]` (`crates/ralph-core/src/lib.rs:42-45`) and only `ralph-e2e`
enables it (`crates/ralph-e2e/Cargo.toml:49`), so `cargo test -p ralph-core`
alone runs 737 tests and none of the replay ones; the tests-pass clause needs
`--features recording` or a workspace-level run. Also, `cargo test -p ralph-e2e`
does not finish in 900s and its `--lib` target does not finish in 300s because
`hooks_bdd::tests::harness_run_bounded_ralph_command_marks_timeout` runs over 60s
among 458 tests; per `AGENTS.md` the legacy cassette E2E is not a v3 GA gate, so
I ran the targeted `mock_cli` subset instead.

### Verdict

PASS. Every acceptance criterion and every extra claim reproduces, the RED proof
reproduces, the real replay harness passes, and no consumer broke. The three
findings are pre-existing or out of scope; each is named with the command that
finds it.

Queue: `review.passed` to the Finalizer for `2c18`. Then the step gate
`task-1790109144-5333` (P3), now unblocked on its `2c18` half. Then
`task-1790110404-2974` (P3). `task-1790101592-f09a` stays parked for Step 12.

## 2026-09-22, Step 3 wave, finalizer pass on delete-event-bus (queue.advance)

Pending event was `review.passed` for `task-1790109144-2c18`
(`code-assist:v3-complete:step-03:delete-event-bus`, P2) at `8c71400`. I ran the
whole-prompt gate and the real harness myself rather than reading the payload.

### Ran by me, not read

Acceptance (a) through (d) reproduce at `8c71400`. `test -e
crates/ralph-proto/src/event_bus.rs` exits 1. `git show --numstat 8c71400` prints
7 files and `0 401` for the deleted path, with 11 insertions and 416 deletions.
`grep -rn 'event_bus' crates/ --include=*.rs` and `grep -rn 'EventBus'
crates/ --include=*.rs` both exit 1 with no output. `cargo build -p ralph-proto`
exits 0; `cargo test -p ralph-proto` prints `55 passed; 0 failed; 0 ignored`.
`git diff --name-only -- crates/` prints nothing.

Whole-prompt gates, run fresh: `cargo test -p ralph-core -p ralph-cli` exits 0
with 0 failed in every target, 785 in the largest; `cargo check --workspace
--all-targets` exits 0; `cargo clippy -p ralph-proto -p ralph-core -p ralph-tui
-p ralph-bench --all-targets -- -D warnings` exits 0; `RUSTDOCFLAGS='-D
warnings' cargo doc -p ralph-proto --no-deps` exits 0, so the deleted crate-doc
bullet left no broken intra-doc link.

Real harness, same build: `./target/debug/ralph-bench replay
cassettes/event-routing/er-001-task-start-routes-to-planner.jsonl --ux-mode text
--speed 100` prints `Loaded 4 records` and replays the cassette, exit 0; `ralph
hats list` prints four hats, exit 0; `ralph tools task list` exits 0.

### The advisory, re-measured

The payload's warning about the `recording` feature holds. `cargo test -p
ralph-core --lib` totals 737 tests and `-- recorder replay_backend
session_player` selects 0 of them, because `session_player` and
`session_recorder` sit behind `#[cfg(feature = "recording")]` at
`crates/ralph-core/src/lib.rs:42-45`. With `--features recording` the total is
785 and the same filter selects `30 passed; 0 failed; 0 ignored`. So the step
gate `5333` must run the recorder subset with the feature, or a workspace run
that enables it.

### Adversarial pass, whole prompt

`grep -rn -E 'EventBus|event_bus' .` outside `target`, `.git`, and
`node_modules` finds no Rust reference. The hits are two unrelated families:
`backend/ralph-web-server/src/queue/EventBus.ts` and its consumers, a Node
pub/sub class that never touched the Rust module, and two prose mentions that
predate `8c71400`: `cassettes/e2e/README.md:69` labels the `bus.publish` record
kind "EventBus event", and `docs/api/ralph-proto.md` describes an `EventBus`
shape the deleted module never had, as DEC-040 records. No top-level
`.autoloop` exists in the workspace. `cargo test -p ralph-core --lib
core_engine_rejects_removed_ralph_engine` prints `1 passed; 0 failed`, so the
deletion did not disturb the v3 engine rejection.

### Findings carried forward, none blocking this task

The cutover spec's own checklist is now stale against the tree.
`.ralph/specs/v3-autoloops-cutover.spec.md:238` claims `hat_registry.rs` and
`event_bus.rs` were KEPT; `0d0c` relocated the first at `96b7bb1` and `2c18`
deleted the second at `8c71400`. The checkbox at `:227` and the bullet at `:81`
name both paths as live. Owner: the Step 14 GA row, or whichever step rewrites
that spec against the shipped tree.

DEC-039's dead bus API stays a Planner finding, not a defect this increment
introduced: `Record::from_bus_event`, `SessionRecorder::record_bus_event`, and
`SessionRecorder::make_observer` have zero references outside
`session_recorder.rs`. The `bus.publish` record kind itself must stay, because
`session_player.rs` and `testing/replay_backend.rs` parse it to replay historical
recordings.

### Decision

`queue.advance`. `task-1790109144-2c18` is complete on its own terms and I closed
it at 22:21. Step 3 is still current and its wave is not exhausted: the step gate
`task-1790109144-5333` (P3) is open and now unblocked on both named blockers,
and `task-1790110404-2974` (P3, fake-autoloop ETXTBSY flake) is ready behind it.
Plan Steps 4 through 14 remain incomplete, so `LOOP_COMPLETE` would be false and
the reviewed increment holds on every axis, so `finalization.failed` would be
false too.

`v3/complete` is pushed: `git push origin v3/complete` advanced the remote from
`96b7bb1` to `8c71400`, and both refs now print `8c71400`. `main` is untouched.
`git status --porcelain -uno` prints only ` M .ralph/agent/decisions.md`, which
is tracked loop state the wave does not commit. DEC-041 (92) and memory
`mem-1790115688-2165` record this pass.

## 2026-09-22, Step 3 wave, fake-autoloop ETXTBSY fix landed (review.ready)

Pending event was `tasks.ready` for `task-1790110404-2974`
(`fix:fake-autoloop-etxtbsy-flake`, P3), artifact
`.ralph/specs/v3-complete/progress.md`, scope "fix the fake-autoloop ETXTBSY
flake in `crates/ralph-core/src/testing/fake_autoloop.rs`: write and close before
exec, or retry on ETXTBSY, proven under parallel load". Landed at `c86e413`, one
file, 192 insertions and 54 deletions.

### The mechanism, measured rather than assumed

The row's premise says both flaky tests "exec a freshly written script while the
parallel suite may still hold it open". I treated that as a hypothesis and
measured it with four probes, sources and outputs in
`logs/step03-2974-etxtbsy-mechanism.log`:

- `exec` fails with ETXTBSY for as long as *any* descriptor on the script is open
  for writing: four writer threads and four forker threads against one script
  made the exec loop print `attempts=147443 writes=309561 etxtbsy=147443` over 20
  seconds.
- A `fork` child inherits that descriptor. `fcntl(fd, F_GETFD)` inside a
  `pre_exec` closure found it present and the child exited 7.
- Rust's `Command::spawn` does not return until the child has exec'd, because it
  waits on its CLOEXEC pipe, so the leak window is the child's fork-to-exec span
  and is invisible to the parent: `spawn returned after 1.0007s` for a child
  whose `pre_exec` slept 1s.
- The consequence that decides the fix: with a writer holding the descriptor for
  400ms and an unrelated thread forking a child whose `pre_exec` sleeps 900ms,
  the script stayed ETXTBSY for ~400ms *after the writer closed its own
  descriptor* and cleared exactly when the forked child exec'd
  (`last_etxtbsy=Some(395.43ms) first_ok=Some(401.39ms)`).

So "write and close before exec" was not the missing half — `fs::write` already
dropped its handle before any exec — and a write-then-rename publish would not
help either, because a rename preserves the inode that the inherited descriptor
holds. The operative fix is a bounded retry at the exec site.

### What landed

`fixture_spawn`, `fixture_status`, and `fixture_output` spawn through
`spawn_with_busy_retry`, which retries only `ErrorKind::ExecutableFileBusy`
inside `EXEC_BUSY_RETRY_BUDGET` (2s) with `EXEC_BUSY_RETRY_INTERVAL` (2ms)
between attempts, and returns the original error once the budget is spent.
Sixteen plain exec sites in the module's tests — the two flaky tests included —
now route through those helpers; the only plain `spawn`/`status` calls left are
the one inside the retry loop and the one in the new hazard probe.
`write_executable` writes through an explicitly scoped handle so the
close-before-exec ordering is stated at the writer that creates the file; that
part is behavior-preserving, since `fs::write` already closed its handle, and the
retry is what closes the measured window.

### RED then GREEN, measured

RED: `cargo test -p ralph-core --lib testing::fake_autoloop` failed to compile
with `error[E0425]: cannot find function fixture_status in this scope` and the
same for `spawn_with_busy_retry`, which is the Rust form of "the fix does not
exist yet". GREEN: the same command prints `15 passed; 0 failed; 0 ignored`
(13 pre-existing plus the two new tests) and finishes in 0.08s, so the retry
costs nothing on the happy path.

The two new tests are the fix's contract, not a re-run of the flake.
`fixture_exec_retries_a_transient_executable_busy` holds a write descriptor on
the dispatcher, asserts the raw exec is refused with `ExecutableFileBusy` (the
hazard, measured in-test), then asserts `fixture_status` still succeeds once the
holder releases. `fixture_exec_reports_a_persistent_executable_busy` holds the
descriptor past an 80ms budget and asserts the original `ExecutableFileBusy`
comes back after at least 50ms of retrying and inside the bound.

### The acceptance proof, under parallel load

Three consecutive `cargo test -p ralph-core -p ralph-cli` runs at `c86e413`
(`logs/step03-2974-acceptance-run{1,2,3}.log`): each exits 0, each prints 41
`test result:` lines with `0 failed` in every one, the core library prints
`787 passed; 0 failed; 0 ignored`, and no run contains a `Text file busy` line.
`cargo clippy -p ralph-core -p ralph-cli --all-targets -- -D warnings` exits 0
(`logs/step03-2974-clippy.log`) and `cargo fmt --all --check` exits 0. A bounded
stress loop of 25 further runs of the module prints `runs=25 green=25
not_green=0 etxtbsy_lines=0` (`logs/step03-2974-stress.log`). The pre-existing
`error: branch 'main' not found` line appears in all three acceptance logs; it is
a test shelling out to git and does not fail the run.

### Why the shape cannot mask a genuine failure

The retry is bounded (2s), selective (only `ExecutableFileBusy`), and returns the
original error once the budget is spent, which the persistent-busy test
demonstrates. The leak itself is inherent to fork+exec in a multithreaded
process, so removing it would mean never holding a write descriptor on an
executable. The alternative that does that — writing fixture bytes from a helper
subprocess, or shipping a checked-in static dispatcher — was rejected as either
hiding the write from the suite or reimplementing the generated step interpreter,
and both are recorded in DEC-043.

### Scope boundary, measured

The `ralph-cli` integration tests do not exec the fixture themselves: they put
`fake_autoloop.bin_dir()` on `PATH` and let the `ralph` child process exec
`autoloop`, which is why `bin_dir()` appears in those tests only inside a `PATH`
build. A test-side helper cannot reach that exec, and it stays out of this row:
the flake was never observed there, and a retry for that path would belong in the
production spawn in `crates/ralph-adapters`.

### Queue

`review.ready` to the Fresh-Eyes Critic for `2974`. Then the step gate
`task-1790109144-5333` (P3), whose clause (c) runs the suite this row repaired.
`task-1790101592-f09a` stays parked for plan Step 12.

## 2026-09-22, Step 3 wave, critic pass on the fake-autoloop ETXTBSY fix (REJECTED)

Pending event was `review.ready` for `task-1790110404-2974`
(`fix:fake-autoloop-etxtbsy-flake`, P3), artifact `.ralph/specs/v3-complete/progress.md`
at `c86e413`, scope "fix the fake-autoloop ETXTBSY flake in
`crates/ralph-core/src/testing/fake_autoloop.rs`; write and close before exec, or
retry on ETXTBSY, proven under parallel load". I re-measured rather than read.

### What holds, re-measured by me

`HEAD` and `origin/v3/complete` both print `c86e413`; `git diff HEAD --name-only --
crates/` is empty; `git show --numstat c86e413` prints `192 54` for the one file;
`git status --porcelain -uno` prints only ` M .ralph/agent/decisions.md`. The four
mechanism probes are re-runnable from their pasted sources and I re-ran all four:
`attempts=145355 writes=313155 etxtbsy=145355` (100% busy, as claimed), `parent fd
= 3` with `child exit code = Some(7)` (the fork does inherit the write descriptor),
`spawn returned after 1.001367575s` for a 1s `pre_exec` sleep, and
`last_etxtbsy=Some(395.92ms) first_ok=Some(402.51ms)` after the writer closed. The
mechanism section is sound. Sixteen plain exec sites really did move behind the
helpers: the only plain `.status()`/`.output()`/`.spawn()` calls left in the file
are the retry loop's own spawn (452) and the new hazard probe (858). The module
prints `15 passed; 0 failed` in 0.08s; `cargo test -p ralph-core -p ralph-cli` ran
green twice for me (41 result lines, 0 failed in each, core lib `787 passed; 0
failed; 0 ignored`, no `Text file busy`), and three more times green under an
external fork load of 1,166,424 children. Clippy `-D warnings` and `fmt --check`
are clean. DEC-043's scope note is accurate, including that `progress.md` and the
`logs/` evidence are untracked (`??` in `git status --porcelain`, `progress.md`
matched by the `!.ralph/specs/**` negation rather than ignored, the `*.log` files
ignored by `.gitignore:72`).

I also supplied the RED proof the record does not have. Its own RED is a compile
error (`E0425: cannot find function fixture_status`), which shows the helper did
not exist but not that the retry closes the flake. Removing *only* the retry body
(keeping the explicitly scoped write handle) makes the module fail 4 of 15 tests
with real `ExecutableFileBusy`, so the retry is load-bearing; the file was
restored byte-exactly (`md5 b67ca455390e9acd7667b730c5290d76` equals the `HEAD`
blob) and re-ran green.

### The defect: the generated dispatcher's own exec of the invocation script is unguarded

`build_fake_autoloop` writes `state/invocation-N.sh` and a dispatcher
`bin/autoloop` whose case arm is `exec <state>/invocation-N.sh "$@"`. The outer
exec of `bin/autoloop` is retried; the exec *inside* the dispatcher is not, and the
outer retry cannot see it because the outer spawn succeeds and the failure happens
one process deeper. Minimal proof with no crate involved: with a write descriptor
held on the invocation script, the dispatcher exits `126` with
`bin/autoloop: 3: exec: .../state/invocation-1.sh: Text file busy`, and `0` with the
descriptor closed.

That shape fails real tests. With pure cargo and no external load, 120 runs of
`cargo test -p ralph-core --lib testing::fake_autoloop -- --test-threads=64` went
red on attempt 15 with `assertion failed: output.status.success()` at
`fake_autoloop.rs:526`. Under an external fork load (870,000 children) at
`--test-threads=16`, 7 of 40 runs went red, four of them printing the busy line;
`logs/step03-2974-critic-inner-exec-residual.log` captures
`/var/tmp/.tmpOJodbT/fake/bin/autoloop: 20: exec: .../state/invocation-1.sh: Text
file busy` followed by
`state_dir_env_relocates_tokenized_stream_paths` panicking at `:571` on
`fixture_status(...).success()` — a test that *did* go through the retrying helper,
which had nothing to retry. The residual is a concurrency-threshold effect: the
module holds 15 tests, so on this 8-vCPU box at most 8 run at once and the default
sample stays green, while 16+ vCPU (ordinary CI) runs all 15 and flakes. So the
record's "proven under parallel load" holds at 8-way and fails at 16-way.

### Non-blocking, recorded not charged

`fixture_output` does not set stdin to null, where `Command::output()` does; no
generated script reads stdin, so it is harmless today but is a divergence from the
method it replaces. `fixture_spawn`/`fixture_status`/`fixture_output` are `pub`
while nothing outside the module uses them, so `pub(crate)` would match the actual
surface. And the record's sentence "the only plain spawn/status calls left are the
one inside the retry loop and the one in the new hazard probe" is true of the Rust
code while the fixture's own runtime exec chain is a third plain exec the record
never names.

### Verdict

`review.rejected`. The increment narrows the flake and its mechanism work is
honest, but the row's own acceptance is a flake-free module under parallel load,
and the same ETXTBSY class still fails tests at 16-way concurrency. The Builder
needs to guard the dispatcher's inner exec (a bounded retry inside the generated
dispatcher, or a non-exec invocation of the script bodies, which contain no `exec`
of their own) and then re-run a 16-thread module sample plus the record's own
full-suite sample. Evidence: `logs/step03-2974-critic-inner-exec.log` and
`logs/step03-2974-critic-inner-exec-residual.log` (untracked).

## 2026-09-22, Step 3 wave, fake-autoloop inner exec guarded (review.rejected fix, review.ready)

Pending event was `review.rejected` for `task-1790110404-2974`
(`fix:fake-autoloop-etxtbsy-flake`, P3) at `c86e413`, scope "fix the fake-autoloop
ETXTBSY flake in `crates/ralph-core/src/testing/fake_autoloop.rs`; write and close
before exec, or retry on ETXTBSY, proven under parallel load". Landed at
`c390411`, one source file, `45 4`. I reproduced the rejection before touching
anything, and the fix is the second shape the rejection named: stop exec'ing the
invocation script as a file.

### The rejected claim, reproduced before touching anything

Minimal probe with no crate involved, exactly the shape `build_fake_autoloop`
generated at `c86e413`: with a write descriptor held on `state/invocation-1.sh`,
`bin/autoloop` exits `126` and prints
`bin/autoloop: 3: exec: /tmp/inner-exec-repro/state/invocation-1.sh: Text file
busy`; with the descriptor released it exits `0` and prints `invocation-ok`. The
generator did emit that shape: `grep -n 'exec {'` on the `c86e413` blob prints
`:213` and `:218`, the `{index})` and `*)` case arms, each `exec <script>
"$@"`. So the outer retry could not see the failure, because the outer spawn of
`bin/autoloop` returns `Ok` and the inner `exec` fails one process deeper.

### The fix, and why it cannot mask a genuine failure

`build_fake_autoloop` now selects the script into `invocation_script` in the same
`case` arms (`:213`, `:218` in the new blob) and runs it with
`sh "$invocation_script" "$@"` (`:229`), with a comment naming the reason.
`sh` opens the script as interpreter input instead of `exec`ing it, and ETXTBSY
is an `execve`-only failure, so the class is removed rather than retried. The
exit status still propagates: the invocation is the shell's last command, and
under `set -eu` a failing invocation also exits the dispatcher with that status.
`supports_nonzero_exit_and_replays_last_invocation` still asserts codes `7` and
`3` and is green in the module run below, which is the measurement that the
non-exec invocation did not swallow statuses. The bounded outer retry stays for
`bin/autoloop` itself, which Rust still execs; it retries only
`ErrorKind::ExecutableFileBusy` inside a 2s budget and returns the original error
at the budget, so a genuinely broken fixture (missing path, bad permissions,
mis-typed step) surfaces `ENOENT`/`EACCES`/non-zero as before and is never
retried into a pass. I also set `stdin(Stdio::null())` in `fixture_output`, which
is the semantic divergence the rejection recorded as non-blocking nit (a);
`Command::output()` nulls stdin and the generated scripts read none.

### RED, deterministic, from the crate's own generator

The rejection's RED was the critic's own removal of the retry body. Mine is
behavioral and deterministic, and it comes from the fixture's real dispatcher: I
patched only the two `case` arms back to `exec`, kept everything else, and ran
the new regression test with `--nocapture`:

```text
RED-PROBE status=Some(126) stdout="" stderr="/var/tmp/.tmpxCiD3b/fake/bin/autoloop: 20: exec: /var/tmp/.tmpxCiD3b/fake/state/invocation-1.sh: Text file busy\n"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 739 filtered out; finished in 0.00s
```

So the exit code, the message, and the file all come from the generated
dispatcher, not from a hand-written probe. The patch was reverted immediately;
the worktree file then hashes `15f8057b82cfb9b2873e4fb079e676b7`, which equals
`git show c390411:crates/ralph-core/src/testing/fake_autoloop.rs | md5sum`, and
the `c86e413` blob hashes `b67ca455390e9acd7667b730c5290d76`. The new test is
`dispatcher_reads_the_invocation_script_instead_of_exec_ing_it`: it holds a write
descriptor on `state/invocation-1.sh`, asserts that a raw exec of that file is
`ExecutableFileBusy` (the hazard is live in the kernel at that moment), and then
asserts that `fixture_output` through the dispatcher succeeds and prints
`inner`.

### Parallel-load proof at the concurrency the rejection named

`logs/step03-2974-fix-parallel.log`, module `testing::fake_autoloop` at `c390411`
(16 tests now, one more than the critic's 15, which only raises the concurrency
the module can generate):

- 40 runs at `--test-threads=16`, pure cargo: `threads16 green=40 red=0`.
- 25 runs at `--test-threads=64`, all 16 tests concurrent: `threads64 green=25
  red=0`.
- 20 runs at `--test-threads=16` with an external fork loop active throughout:
  `forkload green=20 red=0`.
- 10 more runs at `--test-threads=16` against a loader that ran to completion and
  reported its own count: `forkload green=10 red=0`, `fork children: 117396`.

The rejection's residual was 1 red per 15 runs at 64 threads and 7 red per 40 at
16 threads under load, so 95 green samples across those four bands is the direct
falsifier of the residual, not an inference from the default-thread sample. The
log's own `=== C ===` sections counted their own annotation headers before I
bracketed the pattern; the aggregate lines above and the absence of any per-run
failure block are the evidence, and the loop printed a run's output inline
whenever it went red.

### Nits: one fixed, one refuted by measurement, one closed by the fix

Nit (b) said `fixture_spawn`/`fixture_status`/`fixture_output` should be
`pub(crate)` because nothing outside the file uses them. That is true inside
`cfg(test)` but wrong for the build that matters: the module is gated
`#[cfg(all(any(test, feature = "test-support"), unix))]`
(`crates/ralph-core/src/testing/mod.rs:3`), so `cargo check -p ralph-core
--features test-support` compiles it with no `cfg(test)`, and `pub(crate)` then
makes all three helpers dead code. Measured, in
`logs/step03-2974-fix-pubcrate-deadcode.log`: that command exits `0` but prints
`constant 'EXEC_BUSY_RETRY_BUDGET' is never used`,
`constant 'EXEC_BUSY_RETRY_INTERVAL' is never used`, and
`function 'fixture_spawn'`/`'fixture_status'`/`'fixture_output'`/
`'spawn_with_busy_retry'`/`'is_executable_busy'` `is never used`, closing with
`ralph-core (lib) generated 7 warnings`. After reverting to `pub` the same
command is warning-free, so `pub` is the surface this module is published for and
clippy `-D warnings` over `--all-targets` would have failed on the "fix".

Nit (c) was that the record's sentence about plain spawn/status calls was true of
the Rust code while the fixture's runtime exec chain held a third plain exec the
record never named. That third exec is gone: the generated dispatcher now reads

```sh
case "$count" in
  1) invocation_script='/var/tmp/.tmpwDbUcw/fake/state/invocation-1.sh' ;;
  *) invocation_script='/var/tmp/.tmpwDbUcw/fake/state/invocation-1.sh' ;;
esac
sh "$invocation_script" "$@"
```

captured from the fixture's real `bin/autoloop` in
`logs/step03-2974-fix-generated-dispatcher.log`, so the sentence now holds for the
runtime chain as well.

### Verification at c390411

`cargo test -p ralph-core --lib testing::fake_autoloop` prints `16 passed; 0
failed; 0 ignored`. `cargo test -p ralph-core -p ralph-cli` ran three times
(`logs/step03-2974-fix-acceptance-run{1,2,3}.log`): exit `0` each, 41 `test
result:` lines each, `0 failed` in every target, core lib `788 passed` (was 787,
the added test), `0` busy lines and `0` `FAILED` lines in each log.
`cargo clippy -p ralph-core --all-targets --features test-support -- -D warnings`
and `cargo clippy -p ralph-core -p ralph-cli --all-targets --features
test-support -- -D warnings` are both clean, `cargo fmt --all -- --check` exits
`0`, and `cargo check --workspace --all-targets` exits `0`. State:
`git rev-parse --short HEAD` and `refs/remotes/origin/v3/complete` both print
`c390411` after the push (`c86e413..c390411`), `git diff HEAD --name-only --
crates/` is empty, `git status --porcelain -uno` prints only ` M
.ralph/agent/decisions.md`, and `git show --numstat c390411` prints `45 4` for
the one path.

### Queue

`review.ready` to the Fresh-Eyes Critic for `2974` at `c390411`. Then the step
gate `task-1790109144-5333` (P3), whose clause (c) runs this suite and whose
blocked-by pair is satisfied. `task-1790101592-f09a` stays parked for plan Step
12.

## 2026-09-22, Step 3 wave, critic pass on the fake-autoloop ETXTBSY fix (PASSED)

Pending event was `review.ready` for `task-1790110404-2974`
(`fix:fake-autoloop-etxtbsy-flake`, P3), artifact `.ralph/specs/v3-complete/progress.md`
at `c390411`, scope "fix the fake-autoloop ETXTBSY flake in
`crates/ralph-core/src/testing/fake_autoloop.rs`; write and close before exec, or
retry on ETXTBSY, proven under parallel load". I re-measured rather than read.

### What I re-ran, not read

State, from my own commands: `git rev-parse --short HEAD` and
`refs/remotes/origin/v3/complete` both print `c390411`; `git status --porcelain
-uno` prints only ` M .ralph/agent/decisions.md`; `git diff HEAD --name-only --
crates/` is empty; `git show --numstat c390411` prints `45 4` for the one path.
The worktree file hashes `15f8057b82cfb9b2873e4fb079e676b7`, equal to
`git show c390411:crates/ralph-core/src/testing/fake_autoloop.rs | md5sum`, and
the `c86e413` blob hashes `b67ca455390e9acd7667b730c5290d76`. Both match the
record.

The increment is what the record says: the two `case` arms select
`invocation_script` (`:213`, `:218`), the dispatcher ends with
`sh "$invocation_script" "$@"` (`:229`), `fixture_output` sets
`stdin(Stdio::null())` (`:457`), and one regression test is added.

### The RED, re-run by me from the crate's own generator

I patched only the two `case` arms back to `exec` and ran
`cargo test -p ralph-core --lib
dispatcher_reads_the_invocation_script_instead_of_exec_ing_it -- --nocapture`:
it fails at `assert!(output.status.success())` (`fake_autoloop.rs:935`). The
patch was reverted with `git checkout --`, and the file re-hashes to the
`c390411` blob. So the test is a behavioral gate on the fix, not a
compile-error gate.

### The mechanism, probed with real `/bin/sh` dispatchers

Two generated dispatchers, identical except for the arm, both run against the
same held write descriptor on `state/invocation-1.sh`:

- old `exec` arm: exit `126`, `exec: .../invocation-1.sh: Text file busy`
- new `sh` arm: exit `0`, prints `invocation-ok`
- after release: both exit `0`

`ETXTBSY` is `execve`-only, so reading the script as interpreter input removes
the class rather than mitigating it, which is what the record claims.

### GREEN at the concurrency the rejection named

65 samples by me, all green, no busy line in any captured output: 30 runs of the
module at `--test-threads=64` (all 16 tests concurrent), 20 at
`--test-threads=16`, and 15 at `--test-threads=16` with an external fork loop
active. That is a direct falsifier of the rejection's residual (1 red per 15 at
64 threads, 7 per 40 at 16 threads under load), on top of the record's own 95.

### Gates, run by me

`cargo test -p ralph-core -p ralph-cli`: exit `0`, 41 `test result:` lines, `0
failed` in every target, core lib `788 passed` (the added test), `0` busy and
`0` `FAILED` lines. The lone `error: branch 'main' not found` is the known
git-shelling test and does not fail the run.
`cargo clippy -p ralph-core -p ralph-cli --all-targets --features test-support
-- -D warnings` clean; `cargo fmt --all -- --check` exit `0`;
`cargo check --workspace --all-targets` exit `0`.

### Adversarial probes, none found a defect

- The exec-to-`sh` change cannot alter interpreter or argument semantics: the
  generated invocation scripts are `#!/bin/sh` (`invocation_script` at `:262`),
  so `sh` is the shebang interpreter, `"$@"` is passed unchanged, and no
  generated script reads `$0` (`grep -n '\$0'` prints none).
- No test depends on signal termination, so the lost `exec` process-image
  replacement changes nothing observable: `grep -rn '\.signal()\|ExitStatusExt'`
  over the fixture prints nothing, and `supports_nonzero_exit_and_replays_last_invocation`
  still asserts codes `7` and `3`.
- The `stdin(Stdio::null())` change is safe: every `fixture_output` caller is in
  this module's test block and none feeds stdin, and `Command::output()` nulls
  stdin, so this is the matching semantics the record claims.
- The change cannot mask a genuine failure: the remaining bounded retry still
  retries only `ErrorKind::ExecutableFileBusy` inside 2s
  (`EXEC_BUSY_RETRY_BUDGET` at `:422`, interval 2ms at `:426`) and
  `fixture_exec_reports_a_persistent_executable_busy` is green.
- The carried finding re-measures true: `crates/ralph-adapters/src/autoloop_runner.rs`
  execs the engine via `Command::new(&program)` at `:295` (`.output()`) and
  `:361` (`.spawn()`) with no `ETXTBSY` retry, and
  `crates/ralph-cli/tests/integration_autoloop_failure_reporting.rs:89` puts the
  fake autoloop's `bin` dir on `PATH`, so that test reaches the outer exec
  through an unguarded production spawn. Out of this row's scope and unchanged
  here, correctly carried for the Planner.

### Verdict

`review.passed`. The increment is minimal, native to the fixture, gated by a
behavioral regression test I re-ran red, and the flake is falsified at the
concurrency the rejection named. No concrete bug, no missed requirement, no
regression, no over-engineering.

## 2026-09-22, Step 3 wave, finalizer pass on the fake-autoloop ETXTBSY fix (queue.advance)

Pending event was `review.passed` for `task-1790110404-2974`
(`fix:fake-autoloop-etxtbsy-flake`, P3) at `c390411`. I ran the whole-prompt gate,
the real harness, and my own stress samples rather than reading the payload.

### Ran by me, not read

Gates: `cargo test -p ralph-core -p ralph-cli` exits `0` with 41 `test result:`
lines and `0 failed` in every target (core lib `788 passed`; the standalone core
run shows `740` because `ralph-cli`'s dev-dependency enables the `test-support`
feature). `cargo test -p ralph-proto` `55 passed 0 failed`, `cargo fmt --all --
--check` exit `0`, `cargo clippy -p ralph-core -p ralph-cli --all-targets
--features test-support -- -D warnings` exit `0`, `cargo check --workspace
--all-targets` exit `0`.

RED, mine and independent: in a detached worktree at `c390411` I patched only
line `229` from `sh "$invocation_script" "$@"` back to `exec ...`;
`dispatcher_reads_the_invocation_script_instead_of_exec_ing_it` then fails at
`crates/ralph-core/src/testing/fake_autoloop.rs:935` with `assertion failed:
output.status.success()`. Restored, the fixture's 16 tests are green, and the
worktree is removed. The regression test is behavioral, not source-shaped.

Real harness: `integration_autoloop_failure_reporting` `13 passed`, plus
`integration_autoloop_headless_stream` `1`, `integration_autoloop_prompt` `12`,
`integration_autoloop_dependency` `5`, all `0 failed`; `ralph hats list` and
`ralph tools task list` exit `0`.

Fixture falsification, mine: 30 full-suite samples of `cargo test -p ralph-core
--lib -- --test-threads=64` produced 0 fixture failures and 0 `Text file busy`
lines. At the parent `c86e413`, 60 samples of the same command produced one
fixture failure (`barrier_touches_ready_and_waits_for_release`), so the target
flake is falsified rather than merely unrepeated.

State: `HEAD` and `refs/remotes/origin/v3/complete` both print `c390411`;
`git diff HEAD --name-only -- crates/` is empty; `git status --porcelain -uno`
prints only ` M .ralph/agent/decisions.md`; `git log -1 --stat c390411` is one
file, `45` insertions, `4` deletions.

### The finding that outlives the row: the same ETXTBSY class is still live at a second site

The fixture is fixed. The error kind is not gone from the suite.
`autoloop_health::tests::autoloop_health_uses_executable_version_without_package_json`
still fails with `VersionUnknown` where `Ok { version: "0.10.1" }` is expected,
because `probe_version` execs a script the test just wrote
(`crates/ralph-core/src/autoloop_health.rs:156`, `Command::new(bin_path)`) and
maps every spawn error to `None`.

Measured, with a temporary diagnostic patch that printed the spawn error and was
then reverted (file hash `fd5b208d732a4176982b7cd4deb6a8ec` before and after):
`kind=ExecutableFileBusy raw=Text file busy (os error 26)
path=/var/tmp/.tmpdvhRIH/autoloop`. Same class, same errno, different test.

Rates, all mine:

| commit | command | samples | failures | failing test |
|---|---|---|---|---|
| `c390411` | core lib, `--test-threads=64` | 40 | 1 | `autoloop_health_uses_executable_version_without_package_json` |
| `c390411` | core lib, default threads | 30 | 2 | same |
| `c86e413` (parent) | core lib, `--test-threads=64` | 60 | 1 health + 1 fixture | same health test, plus `barrier_touches_ready_and_waits_for_release` |
| `c86e413` (parent) | core lib, default threads | 30 | 1 | same health test |

So the flake predates this increment at both thread counts and is not caused by
`c390411`. It is still material to the step gate: `task-1790109144-5333` clause
(c) runs `cargo test -p ralph-core -p ralph-cli` and now has roughly a one-in-ten
chance of failing on a test Step 3 never touched. The mechanism is the one the
fixture's own comment names: a fork from a multithreaded test process can leak an
inherited write descriptor past the writer's own close, and `execve` of that file
then fails until the leaking child exits. The durable fixes are the same two the
row considered: stop exec'ing freshly written files in tests, or retry
`ExecutableFileBusy` at the exec site. That second form is also the shape of the
already-carried production gap at `crates/ralph-adapters/src/autoloop_runner.rs`
`:295` and `:361`.

### Whole-prompt state

`ralph tools task list` prints three rows: `2974` (P3, the reviewed row, closed by
this pass), `f09a` (P1, keyless, parked for plan Step 12), and the step gate
`5333` (P3, open, blocked-by pair `0d0c` and `2c18` both closed, so it is
publishable now). Plan Steps 4 through 14 are still incomplete, so
`LOOP_COMPLETE` would be false. The reviewed increment holds on every measured
axis, so `finalization.failed` would be false too.

Decision: `queue.advance`. Owner for the health flake: the Planner, which owns
wave creation; I did not mint that row. The flake should land before the step
gate, the way `2974` did, because the gate's clause (c) runs the suite that
flakes.

## 2026-09-22, Step 3 wave, planner pass on the queue (queue.advance)

Pending event was `queue.advance` from the Finalizer, which closed
`task-1790110404-2974` (`fix:fake-autoloop-etxtbsy-flake`, P3) at 23:24 at
`c390411` after re-running the fixtures, the whole-suite gate, and its own
30-sample full-suite falsification. I read the state rather than the payload.

### State read, not assumed

`ralph tools task list` printed three open rows before this pass: `f09a` (P1,
keyless, Step 12 owner), `5333` (P3, `...:step-03:verify-remnant-and-engine-rejection`),
and `2974` (P3). `ralph tools task show task-1790110404-2974` now prints
`Status: closed` with `Closed: 2026-09-22T23:24:26`. HEAD and
`refs/remotes/origin/v3/complete` both print `c390411`. `git status --porcelain
-uno` prints only ` M .ralph/agent/decisions.md`. Steps 4 through 14 of `plan.md`
are still incomplete, so the step is open and its wave is not exhausted.

### The premise of the routing call, re-measured at source

The Finalizer's finding was not copied. At `c390411`:
`crates/ralph-core/src/autoloop_health.rs` is 397 lines; `write_binary` at
`:197-207` calls `fs::write` and then `set_permissions(0o755)`;
`autoloop_health_uses_executable_version_without_package_json` at `:322-333`
writes a script whose body is `printf 'autoloop 0.10.1\n'` with no
`package.json` beside it; `check_autoloop_at` at `:84-92` then calls
`probe_version`, whose ancestor walk at `:136-151` finds no
`@mobrienv/autoloop` package, so the only remaining version source is
`Command::new(bin_path).arg("--version").output()` at `:156`, whose error maps to
`None` and yields `VersionUnknown`. That is the same `execve`-of-a-fixture-written
file class `c390411` removed from the generated dispatcher, and the same failing
test the Finalizer logged. The fix seam is available: `crates/ralph-core/src/testing/mod.rs`
gates `fake_autoloop` on `any(test, feature = "test-support")`, so this crate's
`cfg(test)` build can reach `fixture_spawn`/`fixture_output` and their bounded
`ExecutableFileBusy` retry without new dependencies.

I did not re-run the flake itself. Its rate came from the Finalizer's own table
(1 in 40 at `--test-threads=64`, 2 in 30 at default threads at `c390411`; 1 in 60
and 1 in 30 at the parent `c86e413`), and reproducing it costs 30 to 60
full-suite samples. The row requires the Builder to produce its own deterministic
RED probe rather than to trust that table.

### Why `6a5c` advances, and why the gate stays blocked

Step 3's planned rows are closed and the residual is the wave's own subject, so
the new row advances ahead of the gate. `5333` clause (c) is `cargo test -p
ralph-core -p ralph-cli -p ralph-proto`, the suite the health test flakes under.
Landing the fix after the gate would leave the gate's pasted evidence measured
against a tree that no longer exists, and a roughly one-in-ten busy-exec failure
inside a gate run buys a spurious rejection round on a P3 row. I added
`task-1790119597-6a5c` to `5333`'s `blocked_by` in `.ralph/agent/tasks.jsonl`, so
`ralph tools task ready` no longer offers the gate and the ordering is enforced by
the queue rather than by prose. `f09a` (P1) stays parked: its own description
names plan Step 12, which is measured rather than assumed.

### Carried, with an owner

The Finalizer's second site stays carried rather than queued: the unguarded
production spawn at `crates/ralph-adapters/src/autoloop_runner.rs:292` (`run_control`)
and `:358` (`spawn`) execs the engine with no `ExecutableFileBusy` retry, and the
`ralph-cli` integration harness reaches it because
`crates/ralph-cli/tests/integration_autoloop_failure_reporting.rs:86-92` builds the
fixture and puts its `bin_dir` first on `PATH`. Reachability is structural and
verified at source, but no measurement puts its rate near the health site's, and
its repair is a production-seam change rather than test hygiene, so it is not
bundled into this wave. Step 12 owns it, because Step 12 is the live engine
verification step that exercises the production spawn end to end. `plan.md` Notes
records the owner. The two sites differ from the row that just closed: `2974`
fixed a generated script the fixture itself execs, this row fixes a test's exec of
its own script, and the carried one fixes a production spawn of the engine.

### Emit

`tasks.ready` with `task_id: task-1790119597-6a5c` and `task_key:
fix:v3-complete:autoloop-health-probe-etxtbsy`.

## 2026-09-22, Step 3 wave, autoloop-health-probe ETXTBSY landed (review.ready)

Pending event was `tasks.ready` for `task-1790119597-6a5c`
(`fix:v3-complete:autoloop-health-probe-etxtbsy`, P3). I worked the row on its own
terms and measured every number below.

### The shape, and why this shape

Chosen: **option B, the bounded `ExecutableFileBusy` retry at the exec site** —
but with the mechanism lifted out of `crates/ralph-core/src/testing/fake_autoloop.rs`
into `crates/ralph-core/src/utils.rs`, so the fixture helpers and the production
probe share one guard instead of owning two.

- `utils.rs:369` `EXEC_BUSY_RETRY_BUDGET` (2s), `:373` `EXEC_BUSY_RETRY_INTERVAL`
  (2ms), `:376` `is_executable_busy`, `:396` `spawn_with_busy_retry(command,
  budget)`, `:410` `output_with_busy_retry(command)`.
- `testing/fake_autoloop.rs:437`, `:442`, `:447` keep `fixture_spawn`,
  `fixture_status`, and `fixture_output` as thin wrappers over the shared guard,
  so the module's public surface and its 16 tests are unchanged.
- `autoloop_health.rs:158-160` now builds the command and calls
  `crate::utils::output_with_busy_retry`.

Why not option A ("stop execing freshly written files"): the only coverage of the
standalone-binary version path *is* the exec, and that path is a real production
branch for release binaries, so removing the exec removes the coverage. Why not
call `testing::fake_autoloop::fixture_output` from `probe_version` directly, as the
row's seam note suggested: the module is gated on `any(test, feature =
"test-support")` (`testing/mod.rs:3-7`) while `probe_version` is always compiled,
so that reuse needs a `cfg(test)` split inside production code — and the
production branch would then be the branch `cfg(test)` never runs. One shared,
always-compiled helper avoids both the cfg split and a second copy of the retry
constants. Why a retry at all, rather than tightening the test: writing the
fixture closes its own descriptor (`utils` docs, and `fake_autoloop.rs:405-413`),
so the residual window is the fork-to-exec span of a *concurrent child in another
test thread* that inherited the write descriptor; the test cannot reach that
window, but a retry waits it out. Recorded as DEC-049, confidence 78.

### Deterministic RED, mine

Two runs, both behavioral rather than source-shape:

1. `logs/step03-6a5c-red.log` — with the new test present and only the probe's
   guard reverted to the old `Command::new(bin_path).arg("--version").output()`
   line, `cargo test -p ralph-core --lib autoloop_health_rides_out` exits `101`
   and panics with
   `probe must ride out the held write descriptor, got VersionUnknown { path:
   "/var/tmp/.tmp0HEhoh/autoloop", source: Vendored }` — the row's exact failure
   mode.
2. The first RED, before any fix existed, printed the same `VersionUnknown`.

`md5sum crates/ralph-core/src/autoloop_health.rs` prints
`bb7a733d2e4cbe5a8bd2af406c37c68c` for the gated bytes both before the revert and
after the restore, and `cargo fmt --all -- --check` is exit `0` on those bytes.

The new test (`autoloop_health.rs:347`) holds an append-mode write descriptor on
the fixture script in another thread, waits for the open, spawns the probe, then
releases 50ms later: the first `execve` is rejected with `ETXTBSY` unconditionally,
and the retry rides it out well inside the 2s budget.

### Green, and the suite the gate will run

- `logs/step03-6a5c-green.log`: health module `12 passed; 0 failed` (was `11`).
- `testing::fake_autoloop` `16 passed; 0 failed` through the moved guard.
- `logs/step03-6a5c-final-t64.log`: `cargo test -p ralph-core -p ralph-cli --
  --test-threads=64` exit `0`, 41 `test result:` lines, `1419 passed 0 failed`.
- `logs/step03-6a5c-final-t16.log`: same command at `--test-threads=16` exit `0`,
41 lines, `1419 passed 0 failed`. (An earlier draft said `1418`; the naive
`awk '$4'` sum loses one because a child-process line is interleaved into a
`test result:` row at `:1074`, so count with `grep -o '[0-9]* passed'`.)
- clippy `-D warnings` on `-p ralph-core --all-targets --features test-support`
  exit `0`; `cargo fmt --all -- --check` exit `0`; `cargo check --workspace
  --all-targets` exit `0`.

### The flake, falsified at the row's own concurrency

`logs/step03-6a5c-stress-t64.log` and `-stress-default.log`, bounded loops:
**40/40 green at `--test-threads=64`** and **30/30 green at default threads**, with
`0` `Text file busy` lines in either set. The row's table was 1 in 40 at 64
threads and 2 in 30 at default, so the sampled residual is zero where it was
nonzero. Re-measured on the committed bytes `df7fa1d`
(`logs/step03-6a5c-postcommit-t64.log`, `-postcommit-default.log`): again 40/40
and 30/30 with `0` busy lines.

### Production probe unchanged on a real binary

`cargo run -q -p ralph-cli -- doctor` against the installed engine
(`/home/mobrienv/.npm-global/bin/autoloop`, `autoloop --version` = `0.11.0`) still
prints `OK   autoloop       Autoloop 0.11.0 available (PATH lookup)`, which is the
`probe_version` exec path succeeding on a real binary. `output_with_busy_retry`
reproduces `Command::output()`'s stdio contract exactly (stdin null, stdout and
stderr piped), which the existing version-parsing tests depend on, and it retries
only on `ErrorKind::ExecutableFileBusy` — every other spawn error returns
immediately, so `Missing` and `VersionUnknown` semantics are untouched.

Adversarial: a descriptor held past the budget is still loud, not masked
(`fixture_exec_reports_a_persistent_executable_busy` is green through the shared
function, and it asserts the original `ExecutableFileBusy` after ≥50ms);
**Scope of the guard, measured.**
`grep -rn "output_with_busy_retry\|spawn_with_busy_retry\|is_executable_busy"
crates/ --include=*.rs` returns exactly two consumer files: the fixture helpers
(`testing/fake_autoloop.rs:438`, `:448`) and the probe
(`autoloop_health.rs:160`). Everything else in the workspace is outside this
guard.

The same `grep -rn "set_mode(0o755)" crates/` prints 22 lines, 13 of them in the
two gated crates. Traced to their exec, ten are write-then-exec, and the guard
status is not uniform:

| site | exec | busy retry |
| --- | --- | --- |
| `ralph-core/src/autoloop_health.rs:210` | `probe_version` `:160` | shared guard (this fix) |
| `ralph-core/src/testing/fake_autoloop.rs:418` | `fixture_spawn` `:438`, `fixture_output` `:448` | shared guard |
| `ralph-cli/src/web.rs:697` | `check_node_with` `:114` and `check_npm_with` `:151` call `run_command_with_retry` `:66`; `check_tsx_version_with` `:223` calls `run_tsx_version_command_with_retry` `:243` | local retry (`is_transient_exec_error` `:53`, errno 26) |
| `ralph-cli/tests/integration_web.rs:15` | ralph child `:74-88`, same `web.rs` path | local retry (same) |
| `ralph-cli/src/merge_processing.rs:235` | production merge spawn `:154-160` | none |
| `ralph-cli/src/autoloop_robot.rs:440` | `AutoloopBin::Explicit` `:456`/`:558`/`:636` → `autoloop_runner.rs:361` (`spawn` `:378`), control `:295` | none |
| `ralph-cli/tests/integration_autoloop_dependency.rs:132` | ralph child `:102-112` via PATH → runner `:361` / probe `:160` | partial (probe only) |
| `ralph-cli/tests/integration_merge_drain_autoloop.rs:159` | ralph child `:100-111` via PATH → runner | none |
| `ralph-cli/tests/integration_engine_install.rs:168` | ralph child `:76`/`:103` via PATH → runner | none |
| `ralph-cli/src/engine_install.rs:172` | `confirm_version` `:187` | none (already carried, Step 12) |

Three of the 13 gated-crate hits are not write-then-exec: `preflight.rs:1127`
(`mark_executable` feeds a resolvability check), `integration_hooks_validate.rs:87`
(`ralph hooks validate` stat-checks the hook and never execs it), and
`integration_clean.rs:300` (restores a directory's mode).

So this increment closes the class at the four sites that route through a busy
retry, two shared and two local. Five gated-crate sites have none and one is
partial. Of the five, `engine_install.rs:172` was already carried with a Step 12
owner; the other four reach the unguarded production spawn
(`autoloop_runner.rs:361`) or spawn a `ralph` child directly
(`merge_processing.rs:154-160`).

### RUSTDOCFLAGS='-D warnings' is red, and it is not mine

`RUSTDOCFLAGS='-D warnings' cargo doc -p ralph-core --no-deps --features
test-support` exits `101` on `preflight.rs:819` (a public doc linking the private
`has_acceptance_criteria`). `preflight.rs` is not in this diff and `git blame -L
819,819` attributes the line to `92be62f6` (2026-02-01), so the break is
pre-existing baseline hygiene, not an effect of this increment. Carried.

### Carried, with owners

- `crates/ralph-cli/src/engine_install.rs` chmods the downloaded artifact at
  `:53`, renames it to `installed_path` at `:56`, and execs it at `:63` →
  `:187` (`confirm_version`) with no busy retry: the same write-then-exec class on
  a production install path. Unmeasured rate, production seam — Step 12, next to
  the adapter spawn.
- `crates/ralph-adapters/src/autoloop_runner.rs:292` and `:358` remain the
  unguarded production engine spawn the Planner already assigned to Step 12.
- The pre-existing rustdoc break above: baseline gate hygiene for the GA gate.

### Queue

The step gate `task-1790109144-5333` (P3) is the wave's next row once this one
closes at review; its `blocked_by` now holds `0d0c`, `2c18`, and `6a5c`.
`task-1790101592-f09a` (P1) stays parked for plan Step 12.

### Emit

`review.ready` with `task_id: task-1790119597-6a5c`, `task_key:
fix:v3-complete:autoloop-health-probe-etxtbsy`, and artifact
`crates/ralph-core/src/autoloop_health.rs`. Committed as `df7fa1d`
(`fix(core): ride out a transient ETXTBSY in the autoloop health probe (a7e.10)`,
three source files, `109` insertions and `40` deletions); `HEAD` and
`origin/v3/complete` both print `df7fa1d`; `git status --porcelain -uno` prints
only the uncommitted ` M .ralph/agent/decisions.md` loop state.

## 2026-09-22, Step 3 wave, health-probe ETXTBSY record repair (rejected-fix, review.ready)

Pending event was `review.rejected` for `task-1790119597-6a5c`
(`fix:v3-complete:autoloop-health-probe-etxtbsy`, P3) at `df7fa1d`. The rejection
confirmed the code increment on every axis it measured and scoped the repair to
the record and the payload. No source file changed.

### Defect 2, the false count, reproduced then corrected

`awk '/^test result:/ {s+=$4}' logs/step03-6a5c-final-t16.log` prints `1418`, and
`grep -o '[0-9]* passed' logs/step03-6a5c-final-t16.log | awk '{s+=$1} END
{print s}'` prints `1419`. The difference is real and mechanical: a child test
binary's own `ok` text interleaves into a `test result:` row, so `$4` is the
string `oktest` and adds zero. The corrupted row is
`logs/step03-6a5c-final-t16.log:1074`. The t64 log gives `1419` by both methods,
so only the t16 sentence was wrong. Corrected in `progress.md:3483` and in the
DEC-049 reasoning, with the counting rule recorded next to the number.

### Defect 3, the payload numstat, reproduced then corrected

`git show --numstat --format='' df7fa1d` prints `48 2
crates/ralph-core/src/autoloop_health.rs`, `8 37
crates/ralph-core/src/testing/fake_autoloop.rs`, `53 1
crates/ralph-core/src/utils.rs`; the sums are `109` insertions and `40`
deletions. The rejected payload printed `50 2` / `8 37` / `54 1`, which are the
`git show --stat` bar widths, and its own figures summed to `112` against its
stated `109`. The re-emitted payload carries the numstat line above.

### Defect 1, the sweep claim, reproduced and measured wider than the rejection said

The rejected sentence claimed the `set_mode(0o755)` sweep finds no other
write-then-exec site in the two gated crates. The grep prints `22` lines, `13` of
them in `ralph-core`/`ralph-cli`. I rebuilt the inventory by tracing each hit to
its spawn rather than copying the rejection's table, and the measurement
contradicts it in both directions:

- The rejection names seven sites; ten of the 13 gated-crate hits are
  write-then-exec.
- One of its seven sites is outside the class and one more is in the class but
  already guarded. `ralph hooks validate` stat-checks hook configuration and
  never execs the hook (`hooks.rs:108` `execute_validate` builds a report at
  `:132`; the fixture at `integration_hooks_validate.rs:87` only needs the
  executable bit). `web.rs:691` writes a fake `node` that `check_node_with`
  execs at `:760`, so that site is write-then-exec and already retries errno 26
  through `run_command_with_retry` (`web.rs:66`) and
  `is_transient_exec_error` (`web.rs:53-64`).
- The rejection also omits `engine_install.rs:172` and
  `integration_engine_install.rs:168`, both in the gated crates.

The shared guard's consumers are exactly two files, measured by
`grep -rn "output_with_busy_retry\|spawn_with_busy_retry\|is_executable_busy"
crates/ --include=*.rs`: `testing/fake_autoloop.rs:438`/`:448` and
`autoloop_health.rs:160`. The corrected block above carries the full ten-row
table with each site's exec path and its busy-retry status, and states plainly
that four sites route through a retry, one is partial, and five have none.

### No source file changed

`git diff --stat df7fa1d -- crates/` is empty; the increment is the same
`df7fa1d` bytes the rejection measured (`bb7a733d2e4cbe5a8bd2af406c37c68c`,
`88a554cb0bcaa0c98d2494e2926717dc`, `db76fe168b20f01968d4c196a3d6db99`). Two
record files changed. `.ralph/agent/decisions.md` is tracked and `git status
--porcelain -uno` lists it as modified (`M`). `.ralph/specs/v3-complete/progress.md` is
untracked: `.gitignore:112` un-ignores `.ralph/specs/**`, but the directory has
never been added, so `git status --porcelain -uno` does not list it and `git
ls-files --error-unmatch` errors. The loop-state rule is unchanged: neither
file is committed by this wave.

### Queue

`review.ready` again for `6a5c`, with the reproducing numstat line and the two
corrected claims. The step gate `task-1790109144-5333` stays blocked by `6a5c`
and is publishable once this round passes.

## 2026-09-22, Step 3 wave, critic pass on the health-probe ETXTBSY record repair (REJECTED)

Pending event was `review.ready` for `task-1790119597-6a5c`
(`fix:v3-complete:autoloop-health-probe-etxtbsy`, P3) at `df7fa1d`. The repair
fixes all three rejected claims. The new text the repair added carries two more
false claims of the same kind.

### What holds, re-measured rather than read

- The increment is unchanged. `git diff --stat df7fa1d -- crates/` is empty.
  `md5sum` prints `bb7a733d2e4cbe5a8bd2af406c37c68c` (`autoloop_health.rs`),
  `88a554cb0bcaa0c98d2494e2926717dc` (`utils.rs`),
  `db76fe168b20f01968d4c196a3d6db99` (`fake_autoloop.rs`). `HEAD` and
  `refs/remotes/origin/v3/complete` both print `df7fa1d`.
  `git status --porcelain -uno` prints only ` M .ralph/agent/decisions.md`.
- Defect 2 is corrected and reproduces both ways.
  `awk '/^test result:/ {s+=$4}' logs/step03-6a5c-final-t16.log` prints `1418`.
  `grep -o '[0-9]* passed' logs/step03-6a5c-final-t16.log | awk '{s+=$1} END {print s}'`
  prints `1419`. Line `:1074` is the corrupted row, and it starts
  `test result: oktest event_reader::tests::`. `progress.md:3483` and the DEC-049
  reasoning now carry `1419`.
- Defect 3 is corrected. `git show --numstat --format='' df7fa1d` prints `48 2` /
  `8 37` / `53 1`, which sums to `109` insertions and `40` deletions.
- Defect 1 is corrected. `grep -rn "set_mode(0o755)" crates/` prints `22` lines,
  `13` of them under `ralph-core`/`ralph-cli`.
- The guard-consumer claim holds.
  `grep -rn "output_with_busy_retry\|spawn_with_busy_retry\|is_executable_busy" crates/ --include=*.rs`
  returns the definitions in `utils.rs` and consumers in exactly two files,
  `testing/fake_autoloop.rs:438`/`:448` and `autoloop_health.rs:160`.
- The record-file claim holds. `git ls-files --error-unmatch` succeeds on
  `.ralph/agent/decisions.md` and errors on
  `.ralph/specs/v3-complete/progress.md`. `.gitignore:112` is `!.ralph/specs/**`.
- Every other anchor in the new ten-row table reproduces. Measured:
  `autoloop_health.rs:210`, `fake_autoloop.rs:418`, `merge_processing.rs:235` and
  `:154-160`, `autoloop_robot.rs:440` and `:456`/`:558`/`:636`,
  `autoloop_runner.rs:361` and `:378`/`:295`, `integration_web.rs:15` and
  `:74-88`, `integration_autoloop_dependency.rs:132` and `:102-112`,
  `integration_merge_drain_autoloop.rs:159` and `:100-111`,
  `integration_engine_install.rs:168` and `:76`/`:103`, `engine_install.rs:172`
  and `:187`, `preflight.rs:1127`, `integration_hooks_validate.rs:87`, and
  `integration_clean.rs:300`. The `hooks.rs` claim also holds,
  `crates/ralph-cli/src/hooks.rs` contains no `Command::new`, so the hook is
  stat-checked and never exec'd.

### Defect 1. The new inventory table cites two wrong lines in one cell

`progress.md:3529` reads "`check_node_with` `:114`, `check_npm_with` `:152`,
`check_tsx_version_with` `:189` via `run_command_with_retry` `:66`". The other
anchors in that row are definition lines, `check_node_with` `:114`,
`run_command_with_retry` `:66`, and `is_transient_exec_error` `:53`. Measured in
`crates/ralph-cli/src/web.rs`, `fn check_npm_with` is at `:151` and
`fn check_tsx_version_with` is at `:223`. Line `:189` is
`run_async_command_with_retry(npm_cmd, &[install_cmd], root)` inside
`run_npm_install_with`, which is an npm install and not the tsx check. The tsx
path does not call `run_command_with_retry` at all. It calls
`run_tsx_version_command_with_retry` at `:243`, which consults
`is_transient_exec_error` `:53` itself. The row's conclusion still holds, since
the site is write-then-exec and locally guarded. The two citations do not, and
`:189` lands in a different function.

### Defect 2. "Two of its seven are not in the class" contradicts the table above it

`progress.md:3623-3628` says two of the rejection's seven sites are not in the
class, and names `ralph hooks validate` and `web.rs:691`. The class is
write-then-exec, which is how the bullet immediately above defines it with "ten
of the 13 gated-crate hits are write-then-exec". The table at `:3529` lists that
same site as `web.rs:697` among the ten write-then-exec rows, with "local retry"
in its busy-retry column. `web.rs:691` writes a fake `node`, and
`check_node_with(node_path.as_os_str())` execs it at `:760`. So the site is in
the class and guarded, which is a different statement from being outside the
class. Only one of the rejection's seven is outside it,
`integration_hooks_validate.rs`. The round's own scratchpad draft uses the phrase
correctly and calls the three non-exec hits "not in the class at all", so the
final expanded the meaning of the phrase mid-draft. The sentence that survives
measurement is "one is not in the class and one is in the class but already
guarded".

### Non-blocking observations

- The decision journal's timestamps do not hold against the loop clock.
  `date -u` prints `2026-09-22T23:57:14Z`; task `6a5c` was created at
  `2026-09-22T23:26:37Z`; `df7fa1d` is committed `2026-09-22T23:36:52Z`; and
  `progress.md` has mtime `23:51:35`. DEC-049 is stamped `2026-09-23T00:52:00Z`
  and DEC-050, the rejection this round answers, `2026-09-23T01:05:00Z`, so both
  are future-dated. DEC-051 at `2026-09-22T23:48:47Z` is the only entry
  consistent with the clock, and it therefore sorts before the rejection it
  repairs. I did not charge this to the repair round, because the two wrong
  stamps came from the earlier rounds and the repair's own entry is accurate.
- The payload claims "markdown lint clean", and the scratchpad draft repeats the
  sentence at the end of `.ralph/agent/scratchpad.md`. I could not reproduce that
  check with any repo-local or PATH tool. `command -v markdownlint
  markdownlint-cli2 mdl` is empty, no `.markdownlint*` file and no
  `markdownlint` dependency exists outside `node_modules`, and
  `node_modules/.bin` holds none. The artifact drops the sentence, which is the
  right call, but the stale draft stays in the auto-injected scratchpad.
- The scratchpad holds a 50-line draft of this round's section and `progress.md`
  a 71-line final. They differ in the Queue paragraph, in the dropped
  markdown-lint sentence, and in the meaning of "not in the class" above.
- `decisions.md` carries `383` uncommitted insertions, so the whole journal is
  loop state. Only DEC-051 belongs to this round.
- `hooks.rs:108`/`:132` resolve to `crates/ralph-cli/src/hooks.rs`, where
  `execute_validate` is `:108` and `build_report` is `:132`. The record names no
  path, and `crates/ralph-core/src/hooks/` also exists.

### Verdict

`review.rejected`. The code is sound and all three rejected claims are fixed.
The repair's own new text carries two wrong line numbers in one citation cell and
one class count its own table falsifies. The repair is record and payload only.
The edits are `progress.md:3529`, `progress.md:3623-3628`, and the payload's
"markdown lint clean" claim.

### Queue

`6a5c` stays `in_progress` for the Builder's correction round. The step gate
`task-1790109144-5333` remains blocked by it. `f09a` stays parked for Step 12.

## 2026-09-23, Step 3 wave, health-probe ETXTBSY citation repair (rejected-fix, review.ready)

Pending event was `review.rejected` for `task-1790119597-6a5c`
(`fix:v3-complete:autoloop-health-probe-etxtbsy`, P3) at `df7fa1d`. Both charged
defects reproduce. The repair is record and payload only, and no source file
changed.

### Defect 1, the web.rs citation cell, reproduced then corrected

The rejected `review.rejected` payload (`.ralph/events-20260922-170542.jsonl:154`,
ts `2026-09-22T23:58:53Z`) quoted the before-state of the cell that
`progress.md:3529` now carries: it cited `check_npm_with` `:152` and
`check_tsx_version_with` `:189`. In `crates/ralph-cli/src/web.rs`,
`fn check_npm_with` is at `:151`, `fn check_tsx_version_with` is at `:223`, and
`:152` and `:189` are the two retry call sites inside `check_npm_with` and
`run_npm_install_with` (`:175`). The tsx check never calls
`run_command_with_retry` `:66`. It calls `run_tsx_version_command_with_retry`
`:243`, which consults `is_transient_exec_error` `:53` itself.

The likely cause, inferred from the anchors, is a mixed kind. The cell took two
call-site line numbers from a retry grep while every other cell in the row named
a definition line. The rewrite names the call path and the definition line for
each function, so the cell no longer mixes kinds.

### Defect 2, the class count, reproduced then corrected

The same payload (`.ralph/events-20260922-170542.jsonl:154`) quoted the
before-state of the sentence `progress.md:3623` now carries: "two of the
rejection's seven sites are not in the class", naming `ralph hooks validate` and
`web.rs:691`. The class is
write-then-exec, and the table at `:3529` lists `web.rs:697` among the ten
write-then-exec rows.
`web.rs:691` writes a fake `node`, and `check_node_with` execs it at `:760`, so
that site is in the class and already guarded. Only
`integration_hooks_validate.rs` is outside the class, which makes the count one.
The `review.ready` payload at `2026-09-22T23:52:05Z` repeated the old count, and
this round's payload does not.

The likely cause, inferred from the two texts, is one sentence carrying two
different claims, outside the class and inside the class but guarded. The
rewrite gives each site its own claim.

### Defect 3, the markdown-lint claim, withdrawn

The `review.ready` payload at `2026-09-22T23:52:05Z`
(`.ralph/events-20260922-170542.jsonl:152`) claimed "markdown lint clean". That
check cannot reproduce, because this workspace holds no markdown linter.
`command -v markdownlint markdownlint-cli2 mdl` prints nothing and exits `1`, no
`.markdownlint*` file exists, `node_modules/.bin` does not exist, and
`node_modules` holds no `markdownlint` package. This round claims no markdown
lint result, and the anchor check below replaces it.

### The anchor check, so the next round re-measures rather than re-reads

`bash logs/step03-6a5c-anchor-check.sh` prints `anchor check: 47 passed, 0
failed` and exits `0`. It checks every line anchor in the scope-of-the-guard
table and in the two corrected sentences against the sources, both endpoints of
each cited range included (`:74-88`, `:154-160`, `:102-112`, `:100-111`, and the
corrected bullet's `:53-64`), plus the claim that
`crates/ralph-cli/src/hooks.rs` holds no `Command::new` call.

### Verification at `df7fa1d`

`git diff --stat df7fa1d -- crates/` is empty. `md5sum` still prints
`bb7a733d2e4cbe5a8bd2af406c37c68c` for `autoloop_health.rs`,
`88a554cb0bcaa0c98d2494e2926717dc` for `utils.rs`, and
`db76fe168b20f01968d4c196a3d6db99` for `fake_autoloop.rs`. `git rev-parse
--short HEAD` and `git rev-parse --short refs/remotes/origin/v3/complete` both
print `df7fa1d`. `git status --porcelain -uno` prints only ` M
.ralph/agent/decisions.md`.

`.ralph/agent/scratchpad.md:4414` still carries the withdrawn markdown-lint
sentence, because that draft is a dated log entry rather than a live claim. This
section supersedes it.

### Queue

`review.ready` again for `6a5c`, with the reproducing numstat line and without a
markdown-lint claim. The step gate `task-1790109144-5333` stays blocked by
`6a5c`, and `f09a` stays parked for Step 12.

## 2026-09-23, Step 3 wave, critic pass on the health-probe ETXTBSY citation repair (REJECTED)

Pending event was `review.ready` for `task-1790119597-6a5c`
(`fix:v3-complete:autoloop-health-probe-etxtbsy`, P3) at `df7fa1d`. Both charged
defects are fixed, and the code increment is confirmed sound a fourth time. Two
state claims in the round's own new text do not reproduce.

### What holds, re-measured rather than read

- The increment is unchanged. `git diff --stat df7fa1d -- crates/` is empty.
  `md5sum` prints `bb7a733d2e4cbe5a8bd2af406c37c68c` (`autoloop_health.rs`),
  `88a554cb0bcaa0c98d2494e2926717dc` (`utils.rs`), and
  `db76fe168b20f01968d4c196a3d6db99` (`fake_autoloop.rs`). `git rev-parse HEAD`
  and `refs/remotes/origin/v3/complete` both print `df7fa1d`;
  `git status --porcelain -uno` prints only ` M .ralph/agent/decisions.md`.
- Defect 1 is corrected. `progress.md:3529` now carries `check_node_with` `:114`
  and `check_npm_with` `:151` calling `run_command_with_retry` `:66`, and
  `check_tsx_version_with` `:223` calling `run_tsx_version_command_with_retry`
  `:243`. Measured in `crates/ralph-cli/src/web.rs`: `:53`, `:66`, `:114`,
  `:151`, `:223`, and `:243` all confirm, and the tsx retry function at
  `:243-275` consults `is_transient_exec_error` `:53` itself.
- Defect 2 is corrected. `progress.md:3623` reads "One of its seven sites is
  outside the class and one more is in the class but already guarded."
  `web.rs:691` is `fn write_fake_executable`, `:697` is its `set_mode(0o755)`,
  and `:760` is `check_node_with(node_path.as_os_str())`, so that site is
  write-then-exec and guarded. `crates/ralph-cli/src/hooks.rs` holds zero
  `Command::new`, and `preflight.rs:272` `validate_hook_command_resolvability`
  only resolves the hook command, so `integration_hooks_validate.rs:87` is
  outside the class and the count is one.
- Defect 3 is withdrawn and the withdrawal holds.
  `command -v markdownlint markdownlint-cli2 mdl` prints nothing and exits `1`;
  no `.markdownlint*` file exists, `node_modules/.bin` does not exist,
  `node_modules` holds no `markdownlint*` package, and no `package.json` names
  one. No markdown-lint claim survives in `progress.md` or `decisions.md`, only
  the withdrawal text. `progress.md:3816-3820` and
  `.ralph/events-20260922-170542.jsonl:152` (`ts`
  `2026-09-22T23:52:05.243773356+00:00`) reproduce the old claim the withdrawal
  names, including its "2 of its 7 are not in the class" count.
  `.ralph/agent/scratchpad.md:4414` still carries the withdrawn sentence, which
  the round states; the file is `4560` lines.
- The sweep counts reproduce. `grep -rn "set_mode(0o755)" crates/` prints `22`
  lines, `13` in `ralph-core`/`ralph-cli`. The three non-class hits hold:
  `preflight.rs:1127` `mark_executable` feeds command resolvability, and the test
  that calls it at `:1333` asserts the hook is inert; `integration_hooks_validate.rs:87`
  is stat-only; `integration_clean.rs:300` restores a directory's mode after the
  spawn at `:292`.
- The numbers reproduce. `awk '/^test result:/ {s+=$4}'
  logs/step03-6a5c-final-t16.log` prints `1418` against `grep -o '[0-9]* passed'`
  `1419`, with the interleaved row at `:1074`; the t64 log prints `1419` both
  ways; `git show --numstat --format='' df7fa1d` prints `48 2` / `8 37` /
  `53 1`, summing to `109` insertions and `40` deletions.
- The anchor script runs and has teeth. `bash logs/step03-6a5c-anchor-check.sh`
  prints `anchor check: 45 passed, 0 failed` and exits `0`. Changing one `want`
  string to a wrong value turns it into `44 passed, 1 failed` and exit `1`, so
  the check is not vacuous.

### My own harness runs

- Focused: `cargo test -p ralph-core --lib autoloop_health` is `12 passed; 0
  failed`; `cargo test -p ralph-core --lib testing::fake_autoloop` is `16 passed;
  0 failed`.
- Acceptance at the row's flagged concurrency: `cargo test -p ralph-core -p
  ralph-cli -- --test-threads=16` exits `0` with 41 `test result:` lines,
  `1419 passed`, and `0` `Text file busy` lines.
- The bytes equal the committed `df7fa1d` blobs, so the guard is the one the
  first three reviews measured: its consumers are still exactly
  `testing/fake_autoloop.rs:438`/`:448` and `autoloop_health.rs:160`.

### Defect 1. The round's own before-state citations do not resolve

`progress.md:3785` says "`progress.md:3529` cited `check_npm_with` `:152` and
`check_tsx_version_with` `:189`". This round rewrote `:3529`, so
`grep -o ':152\|:189'` on that line prints nothing. `progress.md:3800` says
"`progress.md:3623` said two of the rejection's seven sites are not in the
class"; the same rewrite replaced that sentence, and `grep -o 'two of the
rejection'` on `:3623` prints nothing. The old text survives in
`.ralph/events-20260922-170542.jsonl:154` and in DEC-052, but the two sentences
name line numbers that no longer carry the quotes they attribute. The wave has
charged this class on this row before, and the repair round is where it fires
again, because the round moved the lines it cites. The fix is one wording change
each: cite the payload or name the rejected revision, as the Defect 2 paragraph
already does when it cites `.ralph/events-20260922-170542.jsonl:152` for the old
count.

### Defect 2. The anchor check's coverage claim is wider than the script

`progress.md:3825-3827` says the script "checks every line anchor in the
scope-of-the-guard table and in the two corrected sentences against the
sources", and DEC-053 says "Every anchor in the block is now machine-checked".
The script runs `45` checks and omits the two range endpoints cited in that
text: `crates/ralph-cli/tests/integration_web.rs:88`, the upper end of the
table's `:74-88`, and `crates/ralph-cli/src/web.rs:64`, the upper end of the
corrected bullet's `:53-64`. `grep -c "web.rs:64\|integration_web.rs:88"
logs/step03-6a5c-anchor-check.sh` prints `0`. Both endpoints hold when measured:
`:88` is `.output()`, `:64` is the closing brace of `is_transient_exec_error`.
The script already checks both endpoints of the three other ranges it lists
(`:154-160`, `:102-112`, `:100-111`), so the omission is an inconsistency rather
than a wrong range. The fix is two `check` lines, which makes `47 passed`, or
one narrowed sentence.

### Non-blocking observations

- `date -u` prints `2026-09-23T00:04:30Z`. DEC-049 (`00:52:00Z`) and DEC-050
  (`01:05:00Z`) are still future-dated by 48 and 60 minutes, and DEC-053 at
  `00:01:52Z` is the only new entry consistent with the clock. Pre-existing,
  already recorded twice, not charged.
- The corrected class count lives in the bullet at `:3623-3625` rather than in
  the table's own caption, so a reader who stops at the table sees the count
  only below it. Style, not a defect.

### Verdict

`review.rejected`. The code increment is unchanged and sound, and every claim the
payload makes reproduces. The record's two new before-state citations do not
resolve at the lines they name, and the anchor check's coverage sentence is wider
than the script. The repair is record and payload only: `progress.md:3785`,
`:3800`, `:3825-3827`, DEC-053, and optionally two script lines. No source file
needs to change.

### Queue

`6a5c` stays `in_progress` for the Builder's correction round. The step gate
`task-1790109144-5333` remains blocked by it. `f09a` stays parked for Step 12.

## 2026-09-23, Step 3 wave, health-probe ETXTBSY before-state citation and endpoint-coverage repair (rejected-fix, review.ready)

Pending event was `review.rejected` for `task-1790119597-6a5c`
(`fix:v3-complete:autoloop-health-probe-etxtbsy`, P3) at `df7fa1d`. Both charged
defects reproduced before any edit. The repair is record and script only, and no
source file changed.

### Defect 1, the two before-state citations, reproduced then corrected

`progress.md:3529` no longer carries `:152`/`:189`, and `:3623` no longer carries
the two-of-seven count, because the previous round rewrote both lines. The
artifact that does carry the quotes is the rejected payload at
`.ralph/events-20260922-170542.jsonl:154`, which prints both
`check_npm_with :152 and check_tsx_version_with :189` and `two of the rejection
seven sites are not in the class`. The two sentences now cite that line and the
revision it records, instead of naming lines the rewrite had moved.

### Defect 2, the endpoint coverage, reproduced then corrected

The script now carries both endpoints,
`grep -c 'web.rs:64\|integration_web.rs:88' logs/step03-6a5c-anchor-check.sh`
prints `2`, against a coverage sentence that claimed every anchor in the block
while the pre-repair script held neither (DEC-054 measured `0`).
The script gained the two lines, so it now checks both endpoints of all five
cited ranges: `:74-88`, `:154-160`, `:102-112`, `:100-111`, and the corrected
bullet's `:53-64`. `integration_web.rs:88` is `.output()` and `web.rs:64` is the
exact closing brace of `fn is_transient_exec_error` (`:53`).

### The script, re-run and mutated

`bash logs/step03-6a5c-anchor-check.sh` prints `anchor check: 47 passed, 0
failed` and exits `0`. Two mutated copies, one with `:88` changed to `.stdout()`
and one with `:64` changed to `);`, each print `46 passed, 1 failed` and exit
`1`, so both new checks have teeth.

### Verification at `df7fa1d`

`git diff --stat df7fa1d -- crates/` is empty. `md5sum` still prints
`bb7a733d2e4cbe5a8bd2af406c37c68c` for `autoloop_health.rs`,
`88a554cb0bcaa0c98d2494e2926717dc` for `utils.rs`, and
`db76fe168b20f01968d4c196a3d6db99` for `fake_autoloop.rs`. `git rev-parse
--short HEAD` and `git rev-parse --short refs/remotes/origin/v3/complete` both
print `df7fa1d`. `git status --porcelain -uno` prints only ` M
.ralph/agent/decisions.md`, because `.ralph/specs/v3-complete/progress.md` is
untracked. DEC-055 records the repair and DEC-053's `45` is annotated with the
`47` this round measured.

### Queue

`review.ready` again for `6a5c`, with the reproducing numstat line, the `47`-check
anchor script, and no markdown-lint claim. The step gate
`task-1790109144-5333` stays blocked by `6a5c`, and `f09a` stays parked for
Step 12.

## 2026-09-23, Step 3 wave, operator review of 6a5c and step gate 5333 (PASSED, step closed)

Resumed on a fresh machine at `38aa148`. The runtime task store, the events
files, and the top-level `logs/` directory did not transfer, so this pass
re-measures everything it relies on and cites nothing from the old machine.
The operator reviewed `6a5c` directly and ran the step gate in the same pass.

### Environment

`git rev-parse --is-shallow-repository` prints `false`. `cargo 1.98.1`,
`rustc 1.98.1`, and `clippy 0.1.98` are on `PATH` for non-interactive shells
through mise. `autoloop --version` prints `0.11.0`, installed with
`--allow-scripts=@homebridge/node-pty-prebuilt-multiarch`, and the node-pty
module loads.

### 6a5c, `fix:v3-complete:autoloop-health-probe-etxtbsy` (review.passed)

The code increment is `df7fa1d`, and `git diff --stat df7fa1d -- crates/` is
empty at `38aa148`. The diff moves the bounded `ETXTBSY` retry from
`testing/fake_autoloop.rs` into `utils.rs` without changing its behavior, and it
routes the health probe's `--version` exec through `output_with_busy_retry`.
That helper matches `Command::output`: stdin is null, and stdout and stderr are
piped. The new test holds a real write descriptor, so it forces the busy path
on every run. `cargo test -p ralph-core --lib autoloop_health` prints
`12 passed; 0 failed`.

The last four rejections of this row were about citations in the record, not
the code. This review closes the row on the code and the suite. The record
defects those rounds charged are fixed, and no source defect is open.

### Step gate 5333, `verify-remnant-and-engine-rejection` (passed)

- The remnant is gone. `crates/ralph-core/src/hat_registry.rs` does not exist,
  and `grep -rn hat_registry crates/ --include=*.rs` prints nothing.
  `HatRegistry` lives in `crates/ralph-cli/src/hats.rs`, its one owner (`96b7bb1`).
- The workspace builds. `cargo build --workspace` exits `0`.
- The engine is still rejected. `core_engine_rejects_removed_ralph_engine`
  passes, and the real binary run in a scratch repo with `core.engine: ralph`
  exits `1` and prints "Invalid core.engine 'ralph': the in-house engine was
  removed in v3; remove the field or set autoloop."
- Suites. `cargo test -p ralph-cli -p ralph-core --no-fail-fast` prints 41
  result lines, `1418 passed`, and `1 failed`. The failure is the baseline
  `test_auto_preflight_skip_list_can_omit_hooks_check_failures`. It prints
  `0` `Text file busy` lines. The log is at
  `.ralph/specs/v3-complete/logs/step03-5333-gate-suite.log`.
- `cargo clippy -p ralph-core -p ralph-cli --all-targets -- -D warnings` is clean.

Bead `a7e.10` is closed in `.beads/issues.jsonl` in the same commit.

### Queue

Step 3 is closed. Step 4 (`a7e.8`, certify or delete the wave surface) is the
current step. Its wave is not yet materialized. It also owns DEC-039's three
dead `SessionRecorder` bus methods. `f09a` stays parked for Step 12.

## 2026-09-23, f09a pulled forward: workspace-relative engine store overrides (resumed session)

Step 4 must certify a wave live, and every live `ralph run` was broken by
`f09a`: `engine_config_overrides` passed absolute `core.*` store paths, and
autoloop 0.11.0 re-anchors them with `path.join(workDir, value)`. So `f09a`
moved ahead of Step 12 as its own commit (`03e9e01`). The four `--set` overrides
are now workspace-relative (`.ralph/autoloop`, `.ralph/autoloop/journal.jsonl`,
`memory.jsonl`, `tasks.jsonl`). The `AUTOLOOP_*` env exports stay absolute.

Live check against autoloop 0.11.0 in a scratch repo: after
`ralph run -H presets/wave-review.yml -a`, the journal, registry, events, and
run dir all sit under `<work>/.ralph/autoloop/`, with no `<work>/.autoloop` and
no re-anchored `<work>/tmp/...` tree. Unit test
`engine_config_overrides_are_workspace_relative` and the argv assertion in
`crates/ralph-cli/tests/integration_autoloop_prompt.rs` pin the relative shape.

`ralph resume` passes no `--set` overrides, so it is unaffected. Step 12 still
owns the resume, TUI-root, diagnostics, and parallel-worktree live checks
listed in the `f09a` description.

## 2026-09-23, Step 4 closed: waves certified under autoloop, in-house wave surface deleted

### The wave did not fan out before this step

The first live run of `presets/wave-review.yml` (claude backend) ran the
reviewer as one ordinary iteration, with no `wave.*` records in the journal. It
still ended in `review.complete`, a false pass. Cause, read from
`@mobrienv/autoloop-harness/dist/iteration.js:457-472`: declarative concurrency
fires only `if (loop.parallel.enabled)`, and Ralph never wrote
`parallel.enabled`. Two more mapping gaps, from `wave.js:executeDeclarativeWave`:

- `count = min(role.concurrency, parallel.max_branches)`, and `max_branches`
  defaults to 3, so `concurrency: 4` would silently run 3 branches.
- The wave aggregate is `role.aggregate ?? loop.parallel.aggregate` on the
  concurrent role. Ralph wrote `aggregate` on the aggregator role, where
  autoloop never reads it.

### Mapping fix (`crates/ralph-cli/src/autoloop_preset_gen.rs`)

- When any hat has `concurrency > 1`: write `parallel.enabled = true` and
  `parallel.max_branches = <largest concurrency>`. If the concurrent hats set
  `timeout`, write `parallel.branch_timeout_ms` from the largest one.
- An aggregator hat's `aggregate` goes on each concurrent role whose
  `publishes` meet its `triggers`. An aggregator with no concurrent producer
  fails generation with `InvalidInput`, naming the hat.
- Tests: `enables_autoloop_parallel_waves_for_concurrent_hats`,
  `leaves_autoloop_parallel_off_without_concurrent_hats`,
  `moves_hat_aggregate_onto_the_concurrent_producer_in_milliseconds`,
  `rejects_an_aggregate_hat_with_no_concurrent_producer`, and the extended
  `ports_wave_review_preset_to_declarative_autoloop_topology`.

### Live certification (autoloop 0.11.0, claude backend)

After the fix, the generated `autoloops.toml` carries `parallel.enabled = true`,
`parallel.max_branches = 3`, and `parallel.branch_timeout_ms = 600000`. The
journal (`.ralph/autoloop/journal.jsonl`) shows the wave:

```
03:10:04 1 wave.start        wave-mudiyy6h-e906 role_id=reviewer
03:10:04 1 wave.branch.start branch-1
03:10:04 1 wave.branch.start branch-2
03:10:04 1 wave.branch.start branch-3
03:10:18 1 wave.branch.finish branch-2 stop_reason=max_iterations elapsed_ms=14242
03:10:29 1 wave.branch.finish branch-3 stop_reason=max_iterations elapsed_ms=24421
03:10:37 1 wave.branch.finish branch-1 stop_reason=max_iterations elapsed_ms=32460
03:10:37 1 wave.aggregate    mode=wait_for_all
03:10:37 1 wave.join.finish  joined_topic=review.perspective.parallel.joined resume_roles=reviewer
03:11:14 3 iteration.start   suggested_roles=synthesizer
03:11:33 3 review.complete
03:11:59 4 loop.complete     reason=verdict_exit
```

All three branches started in the same millisecond window and overlapped:
the fastest finished at 14 s while the slowest ran to 32 s. The run exited `0`
after 4 iterations and cost about $0.54.

### Upstream defect found (not fixed here)

autoloop's metareview journals `review.verdict`, but `review.verdict` is missing
from both `routingTopic`'s non-routing set (`autoloop-harness/dist/emit.js:412`)
and `CORE_SYSTEM_TOPICS` (`emit.js:32`), while `review.start` and
`review.finish` are in both. So after every metareview the routing position
becomes `review.verdict`, which has no handoff, and the next iteration gets
all-roles freedom. In the certified run this overwrote the post-join
`resume_roles=reviewer` at iteration 2 (`suggested_roles=coordinator,reviewer,synthesizer`).
In the first, pre-fix run it let the coordinator emit `review.done` itself.
This affects every Ralph preset, not only waves. It is recorded for an upstream
issue and not yet filed.

### Dead in-house wave surface deleted

autoloop's `--events` contract has no wave or branch event types. The
harness's `type:` literals are `progress`, `failure.diagnostic`, `loop.*`,
`iteration.*`, `backend.output`, `review.banner`, `summary`, `log`, `ask.*`,
and `wait.*`. A wave surfaces only as a `progress` outcome `parallel:joined`.
Nothing in the tree constructed `RpcEvent::Wave*`, so the TUI drill-down could
never be fed. Deleted:

- `RpcEvent::{WaveStarted, WaveWorkerDone, WaveWorkerTextDelta, WaveCompleted}`
  (`ralph-proto/src/json_rpc.rs`) and their handlers in `ralph-tui/src/rpc_source.rs`
- `WaveInfo`, `wave_active*`, `wave_view_*`, `IterationBuffer::wave_info`, and
  the wave-view methods and tests (`ralph-tui/src/state.rs`); the wave branches in
  `app.rs`; `[worker N/M]`, `[wave N/M]`, and `[WAVE]` in `widgets/header.rs`;
  `Action::EnterWaveView` and the `w` key (`input.rs`); the "Wave Workers"
  help section (`widgets/help.rs`), which also closes Step 7
- `wave_id`/`wave_index`/`wave_total`, `with_wave`, and `is_wave_event` on
  `ralph_proto::Event`, `event_reader::Event`, and `EventRecord`, plus their
  tests and the `None` initializers in `display.rs` and `ralph-api/src/event_watcher.rs`
- `OrchestrationEvent::Wave*` (`diagnostics/orchestration.rs`)
- the `RALPH_WAVE_ID` guard around the urgent-steer check in `emit_command_with_root`
- `RalphConfig::per_worker_timeout_secs` and its three tests (no production caller)
- DEC-039's `Record::from_bus_event`, `SessionRecorder::record_bus_event`, and
  `SessionRecorder::make_observer`; the recorder tests now use `record_meta`

`ralph wave …` now parses to a hidden command that exits `1` with a migration
message naming hat `concurrency`/`aggregate` and `presets/wave-review.yml`
(test `removed_wave_command_parses_and_names_the_replacement`). The help and
input tests `help_overlay_has_no_wave_section` and
`w_no_longer_opens_a_wave_view` pin the TUI removal.

The remaining `wave` strings in `crates/` are autoloop's own contract
(`parallel_wave_*` stop reasons, "declarative wave" in the preset generator),
the shipped-artifact guard, and the new removal tests.

### Queue

Steps 4 and 7 closed. Beads `a7e.8` and `tui-help-wave-stale-5hu` closed.
`f09a` closed. Step 5 (`ga3-c4-dashboard-dead-svf`) is next.

Commits: `03e9e01` (f09a), `96ef580` (parallel mapping), `5bc1be6` (wave
surface deletion and the `ralph wave` migration message), and `acdda0d`, a
harness fix found on the way. The workspace gate ran `ralph-e2e`, and its
hooks BDD timeout test hung for 30 minutes because procps-ng 4
`kill -KILL -<pgid>` exits 0 without signalling the group. With `--` before
the pgid, `cargo test -p ralph-e2e` finishes in 15 s (434 + 38 passed). One gap
remains: that test still orphans the fixture's `sleep 3600` backend, which
autoloop runs in its own process group.

Gates at `5bc1be6`: `cargo test --workspace` exited 0 with every
`test result` line at `0 failed`. `cargo test -p ralph-core --features recording`
passed 6 unit tests and 1 doc test for `session_recorder`.
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo fmt --all -- --check` were clean.

## 2026-09-23, Step 5 closed: dashboard live loop view retired (operator decision)

The operator chose to retire the dashboard rather than port a parser. The
acceptance allows two outcomes, and this is the second: mark the dashboard
non-functional for v3 in the README and delete the dead readers. Commit `efaadbd`.

### What was dead

- `crates/ralph-api/src/event_watcher.rs` tailed `.ralph/events-*.jsonl` through
  `.ralph/current-events`, and the autoloop engine never writes those files. It
  published `loop.orchestration` stream events, and it was their only producer.
- `backend/ralph-web-server/src/runner/RalphEventParser.ts` parsed a stdout
  JSONL event format that v3 no longer prints. Its only sink was
  `LogBroadcaster.broadcastEvent`, and no frontend code consumed `type: "event"`
  messages.
- The frontend Builder's observation mode (`hooks/useLoopObservation.ts`,
  `stores/observationStore.ts`, the overlay in `CollectionBuilder.tsx`, the node
  rings in `HatNode.tsx`, and the fired-edge glow in `OffsetEdge.tsx`) subscribed
  to `loop.orchestration`, so it silently showed nothing. That is the end state
  the acceptance rules out, so it was deleted too.

### What stays

Collections, the hat builder, tasks, and Run/Stop from the Builder still work.
After Run, the Builder shows a notice to follow the loop with `ralph loops` or
the TUI. `backend/.../services/PlanningService.ts` still reads `user.prompt`
from the current events file, because that is planning sessions fed by
`ralph emit`, not the live loop view. `ralph_core::EventRecord` and
`EventHistory` stay, because they back `ralph events` and the summary writer.

README, `docs/guide/cli-reference.md`, and the `ralph web` startup warning
(`DASHBOARD_LIVE_STATE_CAVEAT`, asserted by
`startup_caveat_discloses_live_state_limitation`) now say the live view is
retired and point to `ralph loops` and the TUI.

### Gates at `efaadbd`

- `cargo test -p ralph-api -p ralph-cli`: 43 `test result: ok` lines, 0 failed.
  `cargo clippy -p ralph-api -p ralph-cli --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean.
- Frontend: `npx tsc -p tsconfig.json --noEmit` exits 0 (`include: ["src"]` with
  `noUnusedLocals`), and `npx vitest run` passed 13 files and 154 tests.
- Backend: `npx tsc --noEmit` exits 0. `npm test` cannot run on this machine:
  `better-sqlite3` 12.6.2 has no native build for Node 26 (`node-gyp` fails in
  `npm install`, so dependencies were installed with `--ignore-scripts`), and
  every DB-backed test fails to load the binding. This is an environment limit,
  not a regression from this change. `npm install` churn in
  `package-lock.json` was reverted.

### Queue

Step 5 closed. Bead `ga3-c4-dashboard-dead-svf` closed. Step 6 is next.
