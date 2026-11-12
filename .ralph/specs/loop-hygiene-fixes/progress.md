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

## Completed Tasks

- Step 01: test event isolation (task-1794517658-4cd5): passed Critic review 2026-11-12
