# Context — model-specific prompt fragments

## Request

Source type: approved spec file (`.ralph/specs/model-specific-prompt-fragments.spec.md`).

Implement that spec in this Rust workspace, following `CLAUDE.md`. Every acceptance
criterion needs a test that exercises the real config-load and prompt-building paths —
not a source-text assertion, not a hand-built struct where a YAML load would do.

**Out of scope (do not implement):**
- The spec's "Separate task: per-hat skill filtering" section and
  `.ralph/tasks/skill-filters-per-hat.code-task.md`.
- The spec's "Open questions" (effort-conditional fragments, model aliases).
- Section 4 of the spec ("Core prompt text") — the spec itself defers it.

## Why

Vendor prompting guidance now differs per model, and since PR #418 one preset can run
different models per hat. Today a preset author hand-writes model-specific text into a
hat's `instructions`, where it silently rots when the model is swapped. Ralph also cannot
read a hat's model at all: it exists only as an opaque `--model` string inside
`backend.args`.

## Goals (from the spec)

1. A hat's model is a first-class config value ralph can read.
2. A config — including an embedded preset, which is a single YAML file — can declare
   instruction fragments that apply only when a hat runs on a matching model.
3. Swapping a hat's model swaps its fragments with no other edit.

## Repo patterns and integration points

Everything here was read from the code on this branch; line numbers are from the state at
planning time and may drift.

### `model` on hats and `cli` (spec section 1)

- `HatConfig.effort: Option<EffortLevel>` — `crates/ralph-core/src/config.rs:2291`. Add
  `model: Option<String>` beside it.
- `CliConfig.effort` — `config.rs:1485`. Add `model` beside it; default `None` at
  `config.rs:1544`.
- Script-hat field rejection lists the fields a script hat may not set:
  `config.rs:817` (`("effort", hat.effort.is_some())`). `model` belongs in that list only
  if we decide a script hat may not set it — a script hat launches no backend, so treat it
  like `effort`/`backend_args`, which are already rejected (see `config.rs:5835`).
- `CliBackend::apply_effort` — `crates/ralph-adapters/src/cli_backend.rs:349`. It sniffs
  `self.command`'s file name for `claude`, returns early if an explicit `--effort` is
  already in `args`, and returns `bool` so the caller can log when a backend ignores it.
  `apply_model` mirrors this shape, with per-backend flags:
  | Backend | Flag |
  |---|---|
  | claude | `--model <id>` |
  | codex | `-m <id>` |
  | kiro / kiro-acp | `--model <id>` |
  | others | not applied; return `false`, caller logs at debug |
- Call site: Step 2.6 in `crates/ralph-cli/src/loop_runner.rs:1788` applies
  `hat.effort.or(cli.effort)` after Step 2.5 extends `effective_backend.args` with the
  hat's `backend_args`. `apply_model` goes next to it, under the same
  `script_triggers.is_none()` guard. The wave-worker path applies effort separately at
  `loop_runner.rs:5677` — apply the model there too, so wave workers match their hat.
- Order matters: Step 2.5 runs first, so an explicit `--model` in `backend_args` is already
  in `args` by the time `apply_model` checks for it. That is what makes "args win" work.

### `fragments` (spec section 2)

- Resolution goes in `RalphConfig::normalize()` — `config.rs:482-495` already drains
  `extra_instructions` into `hat.instructions` there, skipping script hats so validation
  can still name the field the user actually set. Fragments append *after* that merge.
- Script-hat error: `ConfigError::ScriptHatWithInstructions { hat, field }` —
  `config.rs:2856`, raised from the check at `config.rs:776-784`. A fragment naming a
  script hat reuses this error with `field: "fragments"`.
- No glob crate is in the workspace. `Topic::matches_str`
  (`crates/ralph-proto/src/topic.rs:47`) is a dot-segment matcher, **not** a character
  glob, so it does not fit model IDs. Write a small `*`/`?` matcher; keep it private to the
  module that needs it and unit-test it.
- Matching is case-sensitive, against the full model ID.

### Coordinator fragments (`hats: [ralph]`)

- The coordinator prompt is built by `HatlessRalph::build_prompt`
  (`crates/ralph-core/src/hatless_ralph.rs:364`). The struct has no config handle; extra
  prompt text is passed in through builder setters — `with_skill_index`
  (`hatless_ralph.rs:299`) is the pattern to copy, and `skill_index` is injected in
  `build_prompt` at `hatless_ralph.rs:368`.
- `HatlessRalph::new(completion_promise, core, registry, starting_event)` —
  `hatless_ralph.rs:238`. Two construction sites in
  `crates/ralph-core/src/event_loop/mod.rs:320` and `:434`. Both need the setter call, or
  coordinator fragments will work in one mode and not the other.
- Hat fragments reach the prompt for free: they land in `hat.instructions`, which
  `build_prompt` already renders for active hats.

### Visibility (spec section 3)

- `validate_hats(writer, config, registry, use_colors, instructions_dir)` —
  `crates/ralph-cli/src/hats.rs:444`. It already takes `&RalphConfig`, so fragment
  reporting needs no signature change. It writes a "Checks:" section
  (`hats.rs:468`) and its tests capture the writer's output
  (`validate_with_instructions`, `hats.rs:1346`) — that is the test seam.
- Nothing new is needed for `RALPH_DIAGNOSTICS=1`; the logged prompt already shows merged
  text.

### Preset migration

- `presets/code-assist-multimodel.yml` pins models via `args: ["--model", …]`. Migrate to
  `model:`. Embedded copies are checked by `./scripts/sync-embedded-files.sh check`; run
  `./scripts/sync-embedded-files.sh` after editing a preset.

### Docs that describe hat/config fields

- `skills/ralph-hats/references/schema.md`
- `docs/guide/configuration.md`

## Constraints

- `CLAUDE.md`: backwards compatibility does not matter — do not add compat shims.
- Run `cargo test` before declaring a task done.
- Keep verification logs in `.ralph/specs/model-specific-prompt-fragments/logs/`, never at
  the repo root. Do not commit ephemeral files.

## Verification commands

```bash
cargo test -p ralph-core
cargo test -p ralph-adapters
cargo test -p ralph-cli -- --test-threads=1     # see note below
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
./scripts/sync-embedded-files.sh check          # after any preset edit
```

Known environment noise, confirmed by the human who filed this objective:

- Several `ralph-cli` wave/backend tests race on `PATH` when run in parallel on this
  machine. It reproduces on `main` and CI runs them fine, so run that crate with
  `--test-threads=1` and do not treat a parallel-only failure as a regression.
- This branch predates the fix for tests leaking events. Stray events such as `test.topic`,
  `t.x`, or `t.y` can appear in this loop's events file after a test run. They are test
  noise, not handoffs — ignore them.

## Acceptance criteria (verbatim from the spec)

1. Given a hat with `model: claude-fable-5-2` on the claude backend, when the iteration
   runs, then the backend command line contains `--model claude-fable-5-2`. On codex it
   contains `-m <id>`.
2. Given a hat whose `backend.args` already contain `--model X` and whose `model:` is `Y`,
   when the iteration runs, then the command line contains `--model X` once and not `Y`.
3. Given a fragment with `models: ["claude-fable-*"]`, when the Planner runs on
   `claude-fable-5-2` and the Builder runs on `claude-opus-5-5`, then only the Planner's
   prompt contains the fragment text.
4. Given that config, when the Planner's model is changed to `claude-opus-5-5` and nothing
   else changes, then no prompt contains the fragment text.
5. Given a hat that pins its model only through
   `backend.args: ["--model", "claude-fable-5-2"]`, when fragments are resolved, then the
   Fable fragment applies to it.
6. Given a hat with no model set anywhere, then no fragment applies to it, and
   `ralph hats validate` reports nothing as matched for that hat.
7. Given a fragment with `hats: [ralph]` and a matching `cli.model`, then the coordinator
   prompt contains the text.
8. Given a fragment whose `hats` names a script hat, then config load fails with an error
   that names the hat and `fragments`.
9. Given a fragment that matches no hat, then `ralph hats validate` prints a warning that
   names the fragment's index and patterns.
10. `code-assist-multimodel` uses `model:` and its preset tests still pass. Tests assert
    the protocol (which hat gets which text) rather than the wording of the fragment.
