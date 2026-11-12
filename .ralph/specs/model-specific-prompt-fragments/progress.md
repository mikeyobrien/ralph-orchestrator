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

### Step 1 — hat model field (Critic)

Logs: `logs/critic-step1-*.log`. Re-ran independently against `ba6e22b`:
- fmt, clippy (`-D warnings`), `sync-embedded-files.sh check` → clean.
- `cargo test -p ralph-core` (1019 lib) and `-p ralph-adapters` (335 lib) → all pass.
- `cargo test -p ralph-cli --no-fail-fast -- --test-threads=1` → 25 binaries ok; only
  `integration_gitignore` fails (pre-existing, task `fix:gitignore-memories-test`). Note: without
  `--no-fail-fast` cargo stops at that binary and skips the rest.
- Real binary (`ralph run --no-tui`, fake `claude`/`codex` on PATH, `/var/tmp/critic-model.*`):
  Planner hat `model: claude-fable-5-2` + `backend_args: ["--model","X"]` → argv `--model X` only;
  codex hat `model: gpt-x` → `exec --yolo -m gpt-x`; hatless with `cli.model` →
  `--model claude-opus-5-5`; unknown hat backend still rejected at config validation.

### Step 2 — fragment resolution (Builder)

Logs: `logs/step02-*.log`.

- `cargo fmt --all --check` → clean. `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `./scripts/sync-embedded-files.sh check` → in sync (no preset touched).
- `cargo test -p ralph-core` → all pass (incl. new `tests/prompt_fragments.rs`, 18 tests, and
  `prompt_fragments::tests` glob/arg-scan/defect unit tests). `cargo test -p ralph-adapters` → all pass.
- `cargo test -p ralph-cli --no-fail-fast -- --test-threads=1` → 25 binaries ok; only
  `integration_gitignore` fails (pre-existing, task `fix:gitignore-memories-test`).
- Mutation check: disabling `apply_fragments` fails 12/18 integration tests; dropping the
  `_fragments_applied` guard fails the round-trip test.
- Real binary (`logs/step02-smoke.log`, harness `/var/tmp/frag-smoke-run.sh`, fake `claude` capturing
  argv + stdin): Planner on `claude-fable-5-2` → prompt has the fragment once plus `PLANNER BASE`;
  Planner on `claude-opus-5-5` → no fragment. Adversarial: fragment naming a script hat, `hat:` typo,
  and `models: []` each fail `ralph run` with exit 1 and a message naming the hat / field / `fragments[0]`.

What landed:
- `crates/ralph-core/src/prompt_fragments.rs`: `FragmentConfig` (`deny_unknown_fields`), `*`/`?`
  matcher, `model_in_args` (`--model`, `--model=`, `-m`; dangling flag = unknown model).
- `RalphConfig`: `fragments`, `_fragments_applied` marker; `normalize()` appends after
  `extra_instructions` in config order, skipping script hats; `hat_model`, `coordinator_model`,
  `fragment_indices_for_hat`, `coordinator_fragment_indices`, `coordinator_fragments` (for step 3).
- `validate()`: `ConfigError::ScriptHatInFragment { hat, index }` (AC8) and
  `ConfigError::InvalidFragment { index, reason }` (empty models/pattern/text/hats, `ralph` hat clash).
- `preflight.rs`: hats files and builtin presets carry `fragments:` (appended after core fragments)
  instead of rejecting / silently dropping them.
- Docs: `docs/guide/configuration.md` (## Fragments), `skills/ralph-hats/references/schema.md`.
- Decisions DEC-014 (args flag wins for matching, mirroring the command line) and DEC-015
  (round-trip marker; overlay append).

For step 3: coordinator text is `config.coordinator_fragments()`; per-hat report is
`config.fragment_indices_for_hat(id)`; unknown IDs in `hats:` are not errors yet — they should show
up as the "matches no hat" warning (AC9). `COORDINATOR_FRAGMENT_TARGET` is exported from ralph-core.

## Completed Tasks

_Builder and Critic append one entry per closed task: task key, what landed, and the
verification that backed closing it._

- `code-assist:model-specific-prompt-fragments:step-01:hat-model-field` (task-1794522125-9033) —
  `model:` on hats and `cli`, `CliBackend::apply_model` (claude/kiro `--model`, codex `-m`,
  explicit flag in args wins), shared `resolve_hat_backend` for iterations and wave workers,
  preset migrated, docs updated. Closed by Critic after the verification above.

## Follow-ups

- **minor (error handling):** a wave whose hat backend is invalid now logs only
  `Wave execution failed error=aborting wave (fail-closed)` — `handle_wave_events` logs with
  `%e`, which prints the outermost anyhow context only, and the hat name moved into the inner
  context (`Invalid backend for hat '…'`). Before this change the outer message named the hat.
  Fix: log `{e:#}` in `handle_wave_events` (also surfaces the root cause), or keep the hat id in
  the outer context. Outcome is still correct (fails closed, no worker spawned).
- **minor:** `model: ""` is accepted and becomes `--model ""` on the command line. Consider a
  `validate()` error/warning for an empty model string on hats and `cli`.
