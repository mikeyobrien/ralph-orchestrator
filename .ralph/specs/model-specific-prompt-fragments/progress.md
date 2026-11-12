# Progress — model-specific prompt fragments

## Verification Notes

_Builder and Critic append what they ran and what it showed. Keep command output under
`.ralph/specs/model-specific-prompt-fragments/logs/`, not at the repo root._

Baseline reminders for this branch (from the objective, not yet re-verified here):
- Run `cargo test -p ralph-cli -- --test-threads=1`; parallel-only failures in that crate's
  wave/backend tests are a known PATH race that reproduces on `main`.
- Stray `test.topic` / `t.x` / `t.y` events after a test run are test leakage on this
  branch, not handoffs.

### Step 1 — hat model field (Builder)

Logs: `logs/step01-*.log`.

- `cargo fmt --all --check` → clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo test -p ralph-core` → all pass (1019 lib tests + integration/smoke).
- `cargo test -p ralph-adapters` → all pass (335 lib), incl. 4 new `apply_model` tests.
- `cargo test -p ralph-cli --no-fail-fast -- --test-threads=1` → 25 test binaries ok; one
  failure, `integration_gitignore::ralph_gitignore_tracks_committed_artifacts_not_runtime_state`
  (asserts `.ralph/agent/memories.md` is tracked). Pre-existing: HEAD commit `df75a81` changed
  `.gitignore` to ignore that file, and this step does not touch `.gitignore`. Filed as runtime
  task `fix:gitignore-memories-test`.
- `./scripts/sync-embedded-files.sh check` → in sync after `./scripts/sync-embedded-files.sh`.
- Real binary smoke (`ralph run --no-tui`, fake `claude` on PATH recording argv, temp repo under
  `/var/tmp`): hat `model: claude-fable-5-2` + `cli.model: claude-opus-5-5` → argv has
  `--model claude-fable-5-2` only; adding `backend_args: ["--model","X"]` → `--model X` once, no
  Fable/Opus; hat with no `model` → `--model claude-opus-5-5`. `--effort high` still applied.

New tests (all load YAML through `RalphConfig::parse_yaml` + `normalize` and assert on
`build_command` argv from `resolve_hat_backend`, the function the loop calls):
- `loop_runner::tests::test_hat_model_reaches_claude_and_codex_command_lines` (AC1)
- `loop_runner::tests::test_explicit_model_flag_in_hat_args_wins_over_model_field` (AC2; both
  `backend.args` and `backend_args`)
- `loop_runner::tests::test_model_on_backend_without_model_flag_leaves_args_unchanged`
- `loop_runner::tests::test_no_model_anywhere_adds_no_model_flag`
- `loop_runner::tests::test_wave_worker_launches_with_hat_model` (real `execute_wave`, captured argv)
- `presets::tests::test_code_assist_multimodel_matches_code_assist_events_with_per_hat_backends`
  now asserts each hat's `model` and its resolved `--model` (AC10, model half)
- `config::tests::test_script_hat_ignored_fields_warn` covers `model`

## Completed Tasks

_Builder and Critic append one entry per closed task: task key, what landed, and the
verification that backed closing it._
