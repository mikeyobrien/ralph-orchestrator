# Progress — loop hygiene fixes

## Verification Notes

Builder and Critic: append verification results here. Record the command, the outcome, and the log path
under `logs/`. Note the red run (test failing before the fix) as well as the green one — the spec requires
each fix to have a test that fails beforehand.

Reminder: `cargo test -p ralph-cli` must run with `-- --test-threads=1`. Parallel-only failures in the
wave/backend tests are a pre-existing PATH race that also reproduces on `main`; they are not regressions.

### Step 01 — test event isolation (task-1794517658-4cd5)

Regression check: `scripts/check-test-event-isolation.sh` points `RALPH_EVENTS_FILE` and
`RALPH_WORKSPACE_ROOT` at a fresh sentinel dir under `$TMPDIR` (outside the workspace), runs
`cargo test -p ralph-core`, `-p ralph-adapters`, `-p ralph-cli -- --test-threads=1`, and fails if
anything under the sentinel changed.

- **Red** (before fix): `logs/step01-red-sentinel.log`, exit 1. Two leaks:
  1. `{"topic":"test.topic","payload":"test payload"}` appended to the sentinel events file, from
     `integration_events_isolation.rs::test_ralph_emit_creates_timestamped_events_file` (spawned
     `ralph emit` inheriting `RALPH_EVENTS_FILE`).
  2. `<RALPH_WORKSPACE_ROOT>/.ralph/agent/scratchpad.md` created with `### HUMAN GUIDANCE` entries,
     from ralph-core `event_loop/tests.rs` guidance tests: `CoreConfig::default()` reads
     `RALPH_WORKSPACE_ROOT`, so these tests appended to the live loop's scratchpad (and, without the
     env, to `crates/ralph-core/.ralph/agent/scratchpad.md`, which is one of defect 3's artifacts).
  - `t.x` / `t.y` were **not** produced by any of the three suites. No source exists in the tree or
    in `git log -S` history, so they were most likely emitted by hand by an agent during the dogfood
    run. Nothing to fix in tests for them; the sentinel script would catch a future source.
- **Red for new tests** (fix reverted: helper strip disabled + one direct spawn restored):
  `logs/step01-red-guard.log`, both tests in `integration_env_isolation_guard.rs` fail.
- **Green**: `logs/step01-green-sentinel.log`, exit 0, "OK: sentinel events file and workspace root
  unchanged"; all three suites pass (ralph-cli single-threaded). Live loop events file line count
  unchanged (3) across all runs.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass (`logs/step01-clippy.log`).
- `cargo fmt --all --check`: pass.

For step 02: test runs still create `crates/ralph-{cli,core}/.ralph/diagnostics/<ts>/` when the
caller has `RALPH_DIAGNOSTICS=1` (in-process tests read it with cwd = crate dir). Not an events leak,
so left for the nested-`.ralph/` half of defect 3. I deleted the ones my runs created.

#### Critic review — step 01 (passed)

- `scripts/check-test-event-isolation.sh` (all three suites, ralph-cli single-threaded): sentinel unchanged
  (`logs/step01-critic-sentinel.log`). Script exit 1 only because ralph-adapters had one failure,
  `acp_executor::tests::test_terminal_output_remains_available_while_kill_is_in_progress` (timing; code
  untouched by this change); `cargo test -p ralph-adapters` rerun: 331/331 pass
  (`logs/step01-critic-adapters-rerun.log`).
- Live loop events file after all runs: 5 lines, all real loop events (build.start, handoffs,
  iteration.summary); no `test.topic`.
- Adversarial: added a probe test file with a direct `Command::new(env!("CARGO_BIN_EXE_ralph"))`; the
  guard failed naming it (`logs/step01-critic-guard-probe.log`), passed again once removed.
- clippy `-D warnings`: pass (`logs/step01-critic-clippy.log`); `cargo fmt --all --check`: pass.
- Runs recreated `crates/ralph-{cli,core}/.ralph/diagnostics/` (inherited `RALPH_DIAGNOSTICS`); deleted
  them. Step 02 owns this.

### Step 02 — auto-commit artifacts (task-1794517669-7ea9)

Mechanism (DEC-015): `auto_commit_changes` now runs `git add --update` plus a literal-pathspec `git add` of
untracked, non-ignored files that `is_run_artifact` doesn't reject (nested `.ralph/`, root `logs/`, and the
workspace `.ralph/` except specs-minus-logs, `*.code-task.md`, memories.md, decisions.md). `.gitignore`
gains `crates/**/.ralph/`, `/logs/`, `.ralph/specs/**/logs/`.

- **Red** (before fix): `logs/step02-red-auto-commit.log`, 3 new git_ops tests fail; the HEAD commit
  contained every artifact (nested diagnostics, crate scratchpad, `logs/**`, `.ralph/events-*`,
  `.ralph/loop.lock`, spec `logs/`). `logs/step02-red-gitignore.log`: new
  `ralph_gitignore_ignores_run_artifacts_outside_root_ralph_dir` fails on
  `crates/ralph-core/.ralph/diagnostics/...`.
- **Green**: `logs/step02-green-auto-commit.log` (git_ops 23/23), `logs/step02-green-gitignore.log` (2/2).
- Companion test `test_auto_commit_still_commits_intended_new_files_alongside_artifacts`: new source
  files (including `src/logs/mod.rs`, a path with spaces, a file named `logs*`), memories/decisions, a
  spec, a code-task, and an edit to a *tracked* `logs/README.md` all land in the commit.
- Adversarial: with `--literal-pathspecs` removed, the file named `logs*` globs in `logs/report.txt` and
  `logs/nested/run-1.txt` and the companion test fails (`logs/step02-adversarial-glob-without-literal.log`).
- Full suites: `cargo test -p ralph-core` and `-p ralph-adapters`: 1424 passed, 0 failed
  (`logs/step02-test-ralph-{core,adapters}.log`); `cargo test -p ralph-cli -- --test-threads=1`: 671
  passed, 0 failed (`logs/step02-test-ralph-cli.log`).
- After the runs, `crates/ralph-{cli,core}/.ralph/` exist again (inherited `RALPH_DIAGNOSTICS=1`) and
  `git status --ignored` shows them as `!!`, ignored. Live events file: 7 lines before and after, no
  `test.topic`.
- clippy `-D warnings`: pass (`logs/step02-clippy.log`); `cargo fmt --all --check`: pass.
- Note: this spec's `logs/` is now ignored, so verification logs no longer show in `git status`. They're
  on disk for review. The one already-tracked `.ralph/specs/tui-refactor/logs/test.log` stays tracked.

#### Critic review — step 02 (passed)

- Red confirmed from `logs/step02-red-auto-commit.log`: under `git add -A` the HEAD commit held every
  artifact; the companion test's red diff shows only artifacts as the extra files.
- `cargo test -p ralph-core`, `-p ralph-adapters`, `-p ralph-cli -- --test-threads=1`: all pass, 0 failed
  (`logs/step02-critic-test-ralph-{core,adapters,cli}.log`). Live events file 9 lines before and after.
- Adversarial probe (temporary integration test against `ralph_core::auto_commit_changes`, removed
  afterwards; `logs/step02-critic-probe.log`): repo with no `.gitignore`, a copy of the real
  `crates/ralph-core/.ralph/diagnostics/` the suites just created, 2000 `logs/**` files, 20000 new source
  files, `docs/.ralph/agent/scratchpad.md`, a non-UTF-8 filename, a filename containing a newline, a
  tracked edit and a tracked deletion. The commit held the edit, the deletion, the new sources and both odd
  filenames; every artifact stayed untracked. No pipe stall at 20k pathspecs.
- `git check-ignore -v`: nested crate `.ralph/`, `/logs/`, spec `logs/` ignored; `plan.md`,
  `agent/memories.md`, `agent/decisions.md` not ignored.
- clippy `-D warnings`: pass (`logs/step02-critic-clippy.log`); `cargo fmt --all --check`: pass.

### Step 03 — headless Claude metrics (task-1794517680-b2c6)

Mechanism (DEC-016): `ClaudeSessionMetrics` in `crates/ralph-adapters/src/claude_stream.rs` now does the
per-turn usage aggregation (peak input/cache, summed output) for both executors. `CliExecutor` parses
StreamJson lines with it and returns `session_result` for Claude, with metrics only: raw lines still go to the
writer, text still goes through `normalize_cli_output_for_parsing`, and `success` stays exit-code based. The
headless branch (`loop_runner.rs`) reads metrics from `session_result` on its own and sets
`context_tokens` to the peak input, as the PTY path does. `context_window` is still
`context_window_for_backend` (step 04).

Two corrections to the plan's premises, both fixed here:
- `performance.jsonl` was empty because nothing in production called `DiagnosticsCollector::log_performance`,
  on any path. `context_tokens = 0` had nothing to do with it. Added `EventLoop::log_iteration_performance`
  (iteration_duration + token_count per iteration), called next to `record_iteration_tokens`.
- The `--no-tui` footer printed `cum` before `add_cost`, so it lagged one iteration (`cost=+$0.29 (cum $0.00)`
  on iteration 1). `add_cost` now runs before the footer and still before `process_output`'s termination checks.

Test fixture: `crates/ralph-cli/tests/fixtures/claude_headless_iteration.jsonl`, a recorded-shape stream
(system, two assistant turns with usage, a tool_result, and a `result` line with `total_cost_usd: 0.2934`
and `modelUsage.<model>.contextWindow: 1000000` for step 04 to reuse). The integration tests run the real
`claude` backend definition with `command:` pointed at a script that replays it, under `--autonomous`, so
they go through `CliExecutor` with no API call.

- **Red** (before fix): `logs/step03-red-headless-metrics.log`: summary was `cost_usd: 0.0`, all tokens 0,
  `context_tokens: 0`; the max_cost run kept launching the backend past iteration 1.
  `logs/step03-red-cli-executor.log`: `test_execute_claude_stream_reports_session_metrics` fails, `session_result` is
  `None`. `logs/step03-red-footer-performance.log`: footer `cum` assertion fails and `performance.jsonl` has
  no `token_count` entry.
- **Green**: `logs/step03-green-headless-metrics.log` (3/3):
  `headless_claude_iteration_summary_carries_stream_cost_and_tokens` (both summaries carry 0.2934, 2 turns,
  input 22208, output 610, cache 22000/4100, context_tokens 22208; footer `cum $0.29` then `cum $0.59`;
  summary.md `Est. cost: $0.59`), `headless_claude_max_cost_terminates_after_first_iteration` (exit 2, one
  launch, `max_cost` termination), `headless_claude_iteration_tokens_reach_performance_diagnostics`.
  Adapter tests: `test_execute_claude_stream_reports_session_metrics`,
  `test_execute_claude_stream_without_result_has_no_session_metrics`, and two
  `claude_stream::tests::session_metrics_*` tests (the PTY aggregation test moved there with the struct; the
  PTY fixture test `test_fixture_driven_peak_not_sum` still covers `dispatch_stream_event` end to end).
- Real CLI probe (`logs/step03-cli-probe.log`, before the footer/performance fix): summaries carry the stream
  values; this is what showed `performance.jsonl` empty and the footer lag.
- Adversarial (`logs/step03-adversarial-error-session.log`): `is_error: true` result and exit 1, with
  `max_cost_usd: 0.5`. Cost still counts (an errored session still spent it): the loop stops with `max_cost`
  after iteration 2 (`cum $0.59`).
- Full suites: `cargo test -p ralph-core` 1062 passed, `-p ralph-adapters` 365 passed,
  `-p ralph-cli -- --test-threads=1` 674 passed, 0 failed (`logs/step03-test-ralph-{core,adapters,cli}.log`).
  Live events file 11 lines before and after, no `test.topic`.
- clippy `-D warnings`: pass (`logs/step03-clippy.log`); `cargo fmt --all --check`: pass. No preset or
  `crates/ralph-core/data/` change, so no embedded-files sync.

### Step 04 — stream context window (task-1794517688-dcbe)

Mechanism (DEC-017): `ClaudeStreamEvent::Result` now parses `modelUsage` (a lenient deserializer, so a
malformed map can't fail the line that also carries cost). `ClaudeSessionMetrics` remembers the init line's
model and reports that model's `contextWindow`, else the largest reported, else the caller's fallback.
The PTY path carries it on the new `PtyExecutionResult.context_window`; the headless path on
`SessionResult.context_window` (CliExecutor still passes fallback `0`).
`loop_runner::reported_or_configured_context_window` prefers a reported window (> 0) over
`context_window_for_backend` in the headless branch and both PTY/ACP builders. Precedence: stream >
`context_window_tokens` override > backend default.

- Red (`logs/step04-red-claude-stream.log`): `session_metrics_take_context_window_from_result_model_usage`
  got 200000 instead of 1000000; `session_metrics_prefer_the_session_models_context_window` got the fallback 123.
- Red (`logs/step04-red-headless-context-window.log`): `headless_claude_iteration_summary_carries_stream_cost_and_tokens`
  fails at the new assertion: `iteration.summary.context_window` is `Number(200000)`, expected 1000000
  (real `ralph run --autonomous` binary replaying `fixtures/claude_headless_iteration.jsonl`).
- Red (`logs/step04-red-malformed-model-usage.log`): adversarial `modelUsage: null` (also `[]`, `{"m":null}`,
  string and negative windows) made the whole result line unparseable, which would drop its cost.
  Fixed with the lenient deserializer.
- Green (`logs/step04-green-context-window.log`, `logs/step04-green-malformed-model-usage.log`): claude_stream
  tests, the new PTY test `test_stream_reported_context_window_reaches_pty_result`, the loop_runner test
  `test_stream_reported_context_window_beats_configured_one`, and all 3 integration_headless_metrics tests pass.
- Mutation probe (`logs/step04-mutation-probe.log`): (A) `build_result` dropping the session window fails
  the PTY test (0 vs 1000000); (B) loop_runner ignoring the reported window fails the headless
  integration test (200000 vs 1000000).
- Full suites: `cargo test -p ralph-core` 1062 passed, `-p ralph-adapters` 370 passed,
  `-p ralph-cli -- --test-threads=1` 675 passed, 0 failed (`logs/step04-test-ralph-{core,adapters,cli}.log`).
  Live events file has no `test.topic`/`t.x`/`t.y` lines afterwards.
- clippy `-D warnings`: pass after replacing one `Default::default()` (`logs/step04-clippy.log`);
  `cargo fmt --all --check`: pass. No preset or `crates/ralph-core/data/` change, so no embedded-files sync.

#### Critic review — step 04 (passed)

- `cargo test -p ralph-core` 1062 passed, `-p ralph-adapters` 370 passed, `-p ralph-cli -- --test-threads=1` 675
  passed, 0 failed (`logs/step04-critic-test-ralph-{core,adapters,cli}.log`). clippy `-D warnings` pass
  (`logs/step04-critic-clippy.log`), `cargo fmt --all --check` pass. Live events file 17 lines, no
  `test.topic`/`t.x`/`t.y`.
- Real shape check: a live `claude -p --output-format stream-json --verbose --model claude-haiku-5-5` run
  (`logs/step04-critic-real-claude-stream.jsonl`) has init `model: claude-haiku-5-5` and result
  `modelUsage.claude-haiku-5-5.contextWindow: 1000000`, matching what the parser reads.
- Real binary (`logs/step04-critic-probe.{sh,log}`): that stream replayed through `ralph run --autonomous`
  gives `context_window: 1000000` with and without `context_window_tokens: 128000`; with `modelUsage` removed
  it gives 200000 (default) or 128000 (override); `{"m":null}` gives 200000 and keeps cost; an init model with
  no matching key (`claude-haiku-5-5[1m]`) still gives 1000000 through the max fallback.
- Independent mutation (`logs/step04-critic-mutation.log`): forcing `reported_context_window` to `None` fails
  both claude_stream window tests, the PTY test, and the headless integration test (200000 vs 1000000).
- PTY/ACP: the executor fallback is the same `context_window_for_backend` value the loop runner passes as
  `configured`, so a PTY result that reports only the fallback can't outrank an override.

## Follow-ups

- (minor) `ralph_command_does_not_pass_loop_env_to_the_child` proves nothing when the test process has
  no ambient `RALPH_EVENTS_FILE` (CI). It passes there even if the helper stops stripping. Fix: re-exec the
  test binary with the var set, or factor the strip into a function over an explicit env list.
- (minor) The source-scan guard matches only the two literal spawn spellings. `let bin =
  env!("CARGO_BIN_EXE_ralph"); Command::new(bin)` gets past it.
- (minor) The sentinel check is a shell script outside `cargo test`/CI, so it only catches leaks when
  someone runs it.
- (minor, pre-existing) `acp_executor` kill-timing test flaked once under the full sequential run.
- (minor, step 02) Test runs still *create* `crates/*/.ralph/diagnostics/` on disk when the caller has
  `RALPH_DIAGNOSTICS=1`; they are now ignored and excluded from the auto-commit, but not prevented.
- (minor, step 02) `is_run_artifact` drops *any* untracked file under a nested `.ralph/`, so a monorepo
  subproject that keeps its own Ralph specs (`svc/.ralph/specs/new.md`) won't get a new spec
  auto-committed. Tracked edits still commit. Narrow the nested rule to runtime paths if that comes up.
- (minor, pre-existing, not from step 02) In worktree loops, `.ralph/agent/memories.md` is a symlink over a
  tracked file, so `git add --update` commits a typechange to a symlink, as `git add -A` did before.
  Untracked symlinked `.ralph/specs` is now excluded, which is an improvement.

- (minor, step 03, pre-existing) `ContentBlock` has no catch-all variant, so an assistant line with a
  `thinking` block (or any new block type) fails to parse whole and its `usage` is skipped, on both the PTY
  and the headless paths. Claude repeats a message's usage on each content-block line, so peaks usually
  survive, but a `#[serde(other)]` variant would make it robust.
- (minor, step 03) Copilot headless runs (`CopilotStreamJson`) still report zero metrics through
  `CliExecutor`. The spec only covers Claude; `copilot_stream.rs` already builds a `SessionResult` for the PTY.

- (minor, step 04) Pi-family and Copilot results still set `context_window` to the configured ceiling;
  only Claude streams report a real window. ACP has no stream window to report.
- (minor, step 03) A Claude stream cut off before its `result` line (timeout kill, crash) reports $0 and
  zero tokens, even though assistant turns with `usage` were seen. Spent cost is unknown without the result
  line, so repeated timeouts never count toward `max_cost_usd`. Critic probe: `step03-critic-probe.log` (trunc).

## Completed Tasks

- Step 01: test event isolation (task-1794517658-4cd5): passed Critic review 2026-11-12
- Step 02: auto-commit artifacts (task-1794517669-7ea9): passed Critic review 2026-11-12
- Step 03: headless Claude metrics (task-1794517680-b2c6): passed Critic review 2026-11-12
- Step 04: stream context window (task-1794517688-dcbe): passed Critic review 2026-11-12
