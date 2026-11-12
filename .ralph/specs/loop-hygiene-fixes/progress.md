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

## Completed Tasks

- Step 01: test event isolation (pending review)
