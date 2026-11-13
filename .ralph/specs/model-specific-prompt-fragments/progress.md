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

### Step 2 — fragment resolution (Critic)

Logs: `logs/critic-step2-*.log`, `logs/critic-step2-smoke-run.out`. Re-ran independently against `0d16336`:
- fmt, clippy (`-D warnings`), `sync-embedded-files.sh check` → clean.
- `cargo test -p ralph-core` and `-p ralph-adapters` → all pass (incl. 18 `tests/prompt_fragments.rs`).
- `cargo test -p ralph-cli --no-fail-fast -- --test-threads=1` → only `integration_gitignore` fails
  (pre-existing, task `fix:gitignore-memories-test`).
- Real binary (`ralph run --no-tui -H hats.yml`, fake `claude` on PATH, `/var/tmp/critic-frag.*`): core
  config `cli.model: claude-opus-5-5` + core fragment `hats: [planner]`; hats file Planner pinned only via
  `backend_args: ["--model","claude-fable-5-2"]` plus two hats-file fragments (fable/all, opus/builder).
  Planner argv `--model claude-fable-5-2`; prompt has `PLANNER BASE`, `CORE-FABLE-FRAG`, `HATS-FABLE-FRAG`
  in that order, no `HATS-OPUS-FRAG` (AC5 + overlay append end to end). Adversarial: hats-file fragment naming
  a script hat → load fails naming `advance` and `fragments`; `fragments:` as a mapping in a hats file →
  `hats 'fragments' must be a list when provided`.
- Reviewed DEC-014 (args flag wins for matching): consistent with `resolve_hat_backend`, all ACs hold. Accepted.
- Checked double-`normalize()` paths (`start_loop`, bot daemon temp-config round trip): marker keeps it to
  one application; daemon reload passes no hats source, so the marker never suppresses fresh hats.

### Step 3 — coordinator fragments + `hats validate` (Builder)

Logs: `logs/step03-*.log`.

- `cargo fmt --all --check` → clean. `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `./scripts/sync-embedded-files.sh check` → in sync (no preset touched).
- `cargo test -p ralph-core` → all pass (`tests/prompt_fragments.rs` now 22 tests).
- `cargo test -p ralph-adapters` → first run: `pty_executor::tests::test_run_observe_large_stdin_backend_does_not_deadlock`
  failed with an empty panic body (`logs/step03-adapters.log`); untouched crate, re-run all 335 pass
  (`logs/step03-adapters-rerun.log`). Treat as a timing flake under load.
- `cargo test -p ralph-cli --no-fail-fast -- --test-threads=1` → 25 binaries ok; only `integration_gitignore` fails
  (pre-existing, task `fix:gitignore-memories-test`).
  `presets::tests::test_code_assist_multimodel_matches_code_assist_events_with_per_hat_backends` passes (AC10 preset half).
- Mutation checks: neutralizing either `with_coordinator_fragments` call in `event_loop/mod.rs` fails exactly one
  distinct test (`..._on_a_matching_cli_model` for the loop-context constructor,
  `..._without_a_loop_context` for `with_diagnostics`); dropping the `config.validate()` check fails
  `config_that_run_rejects_fails_validate`.
- Real binary, coordinator (`logs/step03-coord-smoke.log`, harness `/var/tmp/step3-coord-smoke.sh`, fake `claude` from
  `/var/tmp/frag-smoke-bin`): `cli.model: claude-fable-5-2`, `task.start` with no hat → coordinator prompt has
  `SMOKE-COORD-FRAGMENT` once, planner fragment absent; `cli.model: claude-opus-5-5` → absent; solo mode → present.
- Real binary, validate (`logs/step03-validate-smoke.log`): report lists each hat + `ralph (coordinator)` with model
  and `fragments[i]`, script hat shown as `(script hat) none`, typo glob warns with index, patterns, and hats.
  Adversarial: a fragment naming a script hat used to print `Result: Valid` (only a "matches no hat" warning) though
  `ralph run` rejects it → now `[err] Config is invalid: fragments[1].hats names script hat 'advance'…`, exit 1 (DEC-016).

New tests:
- `tests/prompt_fragments.rs`: `coordinator_fragment_reaches_the_coordinator_prompt_on_a_matching_cli_model` (AC7, also
  asserts a hat fragment is absent), `coordinator_fragment_is_absent_on_a_non_matching_cli_model` (AC7 pair),
  `coordinator_fragment_does_not_leak_into_an_active_hat_prompt`, `coordinator_fragment_reaches_the_prompt_without_a_loop_context`.
  All build through a real `EventLoop` → `HatlessRalph::build_prompt`.
- `hats::fragment_report_tests` (YAML → parse/normalize/validate → `HatRegistry::from_config` → `validate_hats` into a
  buffer): right-hat reporting, no-model hat lists nothing (AC6), unmatched warning with index + patterns (AC9),
  unknown hat ID warns, no `fragments:` → no report, solo mode reports the coordinator, run-rejected config fails.

What landed: `HatlessRalph::with_coordinator_fragments` (rendered after the workflow section, only when no hat is
active), wired at both `EventLoop` constructors; `validate_hats` split (`check_topology`), plus `fragment_targets`,
`check_unmatched_fragments`, `print_fragment_report`, and a `config.validate()` check. Docs: configuration.md
("The coordinator", "Checking what applies"), ralph-hats schema.md.

### fix:gitignore-memories-test (task-1794522885-98f8, DEC-017)

`integration_gitignore.rs` moved `.ralph/agent/memories.md` from the "visible to git" list to the ignored list,
matching df75a81 (human commit on this branch; `git ls-files .ralph/agent` now tracks only `decisions.md`).
- Before: `cargo test -p ralph-cli --test integration_gitignore` fails at `:82` "memories.md should be visible to git"
  (`logs/gitignore-before.log`).
- After: same test passes (`logs/gitignore-after.log`); `cargo fmt --all --check` clean; `cargo clippy -p ralph-cli
  --all-targets --all-features -- -D warnings` clean (`logs/gitignore-clippy.log`).
- `cargo test -p ralph-cli --no-fail-fast -- --test-threads=1`: exit 0, all 26 test binaries ok, no failures
  (`logs/gitignore-cli-full.log`). The ralph-cli suite is fully green for the first time on this branch.

## Completed Tasks

_Builder and Critic append one entry per closed task: task key, what landed, and the
verification that backed closing it._

- `code-assist:model-specific-prompt-fragments:step-01:hat-model-field` (task-1794522125-9033) —
  `model:` on hats and `cli`, `CliBackend::apply_model` (claude/kiro `--model`, codex `-m`,
  explicit flag in args wins), shared `resolve_hat_backend` for iterations and wave workers,
  preset migrated, docs updated. Closed by Critic after the verification above.
- `code-assist:model-specific-prompt-fragments:step-02:fragment-resolution` (task-1794522135-27a0) —
  top-level `fragments:` resolved in `normalize()` (glob matcher, effective model mirrors the command line,
  config-order append after `extra_instructions`, script hats skipped/rejected, malformed fragments rejected),
  coordinator fragments exposed for step 3, hats files/presets carry fragments, docs. Closed by Critic after
  the Step 2 verification above.

- `code-assist:model-specific-prompt-fragments:step-03:coordinator-and-validate` (task-1794522145-2453) —
  `HatlessRalph::with_coordinator_fragments` set at both `EventLoop` constructors, rendered only on coordinator
  iterations; `ralph hats validate` Fragments report, unmatched-fragment warnings, and a `config.validate()` error
  check (DEC-016). Closed by Critic: fmt/clippy/core/adapters/sync pass; ralph-cli single-thread `--no-fail-fast`
  fails only the known `integration_gitignore` (logs `logs/critic3-*.log`). Real binary: `hats validate` on a mixed
  config reports per-hat/coordinator indices and warns `fragments[2] ... (models: ["claud-*"])`; a fragment naming a
  script hat prints `[err]` and exits 1, matching `ralph run`'s rejection.

## Follow-ups

- **minor (error handling):** a wave whose hat backend is invalid now logs only
  `Wave execution failed error=aborting wave (fail-closed)` — `handle_wave_events` logs with
  `%e`, which prints the outermost anyhow context only, and the hat name moved into the inner
  context (`Invalid backend for hat '…'`). Before this change the outer message named the hat.
  Fix: log `{e:#}` in `handle_wave_events` (also surfaces the root cause), or keep the hat id in
  the outer context. Outcome is still correct (fails closed, no worker spawned).
- **minor:** `model: ""` is accepted and becomes `--model ""` on the command line. Consider a
  `validate()` error/warning for an empty model string on hats and `cli`.
- **minor (error handling, step 2):** fragment errors index the *merged* list, so a hats file's first
  fragment is reported as `fragments[1]` when the core config has one fragment, and the message does not
  name the source file. Repro: core `fragments:` with one entry, `-H hats.yml` whose `fragments[0]` names a
  script hat → `fragments[1].hats names script hat 'advance'`. Outcome is correct (load fails, hat named).
- **minor (step 2):** `model_in_args` treats `-m <id>` as pinned on every backend, but `apply_model` only
  honors `-m` for codex. On claude, `backend_args: ["-m","X"]` + `model: Y` runs `--model Y` while fragments
  match `X`. Unrealistic input (claude has no `-m` model flag).
- **step 3 note:** docs already say `hats: [ralph]` reaches the coordinator prompt; step 3 makes that true.
- **minor (step 2 matching, found in step 3):** `hat_model` falls back to `cli.model` for a hat on a *different* backend
  (e.g. `backend: gemini` under `cli: {backend: claude, model: claude-opus-5-5}`), so `hats validate` reports and
  fragments match `claude-opus-5-5` for a gemini hat, while `apply_model` never passes it to gemini. Spec §1 says
  "hat.model, else cli.model" literally, so this follows the spec; consider only inheriting `cli.model` when the hat
  uses the global backend.
- **minor (step 3, validate):** a fragment naming a script hat gets both the `[err]` load error and a redundant
  `[warn] fragments[0] matches no hat` line. Harmless; could skip the warning when `config.validate()` failed.
- **follow-up for the human (step 1 behavior, spec-literal):** the `cli.model` fallback above also reaches the
  command line, not only matching: `resolve_hat_backend` passes `cli.model` to a hat with its own `backend:`, so
  `cli: {backend: claude, model: claude-fable-5-2}` + a `backend: codex` hat with no `model:` spawns codex with
  `-m claude-fable-5-2`. Seen via `hats validate` (critic hat reported as `claude-fable-5-2`). Fix if wanted: inherit
  `cli.model` only when the hat uses the global backend, in both `resolve_hat_backend` and `RalphConfig::hat_model`.

### Finalizer

Logs: `logs/final-*.log`. Against `de2ce38`: fmt, clippy (`-D warnings`), sync check, `ralph-core`, `ralph-adapters` pass;
`ralph-cli --no-fail-fast -- --test-threads=1` fails only the pre-existing `integration_gitignore`.
Real binary, full loop (`/var/tmp/final-frag-run.sh`, fake `claude` that hands off planner → builder): fable Planner gets the
anchored fragment, opus Builder does not, coordinator gets the `hats: [ralph]` fragment on fable `cli.model` (AC3, AC7, anchors);
switching to opus removes every fragment (AC4); each argv has exactly one `--model`. `hats validate` reports per target and warns
with index + patterns (AC9). Adversarial: fragment naming a script hat → `ralph run` exit 1 naming the hat and `fragments` (AC8);
no-model hat with `models: ["*"]` → `(no model) none` (AC6). All ACs met; LOOP_COMPLETE.
