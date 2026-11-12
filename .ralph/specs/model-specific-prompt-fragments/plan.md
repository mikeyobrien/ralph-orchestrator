# Plan — model-specific prompt fragments

Three slices. Each is one user-facing behavior with its tests, built in order; the second
needs the `model` field from the first, and the third needs fragment resolution from the
second.

Acceptance-criterion numbers (AC1–AC10) refer to the list at the bottom of `context.md`.

---

## Step 1 — A hat's model is config, and it reaches the backend command line

Task key: `code-assist:model-specific-prompt-fragments:step-01:hat-model-field`

**Demo when done:** a config sets `model: claude-fable-5-2` on one hat and
`model: claude-opus-5-5` on `cli`, and the spawned command line for each hat carries the
right `--model` — with a model pinned in `backend_args` still winning.

Covers AC1, AC2, and the `model:` half of AC10.

Scope:
- `model: Option<String>` on `HatConfig` and `CliConfig`, beside `effort`.
- `CliBackend::apply_model`, mirroring `apply_effort`: claude `--model <id>`,
  codex `-m <id>`, kiro and kiro-acp `--model <id>`, every other backend returns `false`.
  An explicit flag already in `args` wins and still reports applied, as `apply_effort`
  does.
- Wire it at Step 2.6 of `loop_runner.rs` (`hat.model` else `cli.model`), and in the
  wave-worker backend path, so a wave worker runs the same model as its hat.
- Decide and implement script-hat handling for `model` consistently with `effort`
  (a script hat launches no backend). Record the choice in `.ralph/agent/decisions.md` if
  it is anything other than "reject it exactly like `effort`".
- Migrate `presets/code-assist-multimodel.yml` from `args: ["--model", …]` to `model:`,
  then run `./scripts/sync-embedded-files.sh` and check it with
  `./scripts/sync-embedded-files.sh check`.
- Update `skills/ralph-hats/references/schema.md` and `docs/guide/configuration.md`.

Test obligation — exercise real paths, not hand-built structs:
- Load YAML through the real config loader and assert the parsed `model` on hat and `cli`.
- Assert on the **resolved backend's `args`**, which is what becomes the command line:
  one test for claude (`--model <id>`), one for codex (`-m <id>`), one for a backend with
  no model control (args unchanged, no panic).
- AC2 as its own test: `backend_args: ["--model", "X"]` plus `model: "Y"` yields exactly
  one `--model`, its value is `X`, and `Y` appears nowhere in the args.
- Preset test: `code-assist-multimodel` loads and each hat's effective model is the
  expected ID. Assert the ID, not the file's prose.

---

## Step 2 — Fragments resolve per hat at config load

Task key: `code-assist:model-specific-prompt-fragments:step-02:fragment-resolution`

**Demo when done:** a config with one `fragments` entry for `claude-fable-*`; after load,
the Fable-backed hat's `instructions` end with the fragment text and the Opus-backed hat's
do not. Change the model, reload, and the text moves.

Covers AC3, AC4, AC5, the "no fragment applies" half of AC6, and AC8.

Scope:
- Top-level `fragments: Vec<FragmentConfig>` on `RalphConfig`. Fields: `models: Vec<String>`
  (required), `hats: Vec<String>` (optional; default every hat plus the coordinator),
  `text: String`.
- A small case-sensitive `*`/`?` glob matcher over the full model ID, unit-tested
  (prefix, suffix, interior, `?`, no-wildcard exact, non-match). The existing
  `Topic::matches_str` is a dot-segment matcher and must not be reused for this.
- Effective-model resolution for matching, in this order: `hat.model`, else `cli.model`,
  else a `--model`/`-m` value found in the hat's resolved backend args, else `cli` args.
  A hat with no model found this way matches nothing.
- Append matching fragment text to the hat's `instructions` in `normalize()`, after the
  `extra_instructions` merge, in the order fragments appear in config.
- Script hats: a fragment whose `hats` names a script hat fails config load with
  `ConfigError::ScriptHatWithInstructions`-style error naming the hat and `fragments`.
  Fragments never apply to script hats even when `hats` is omitted.
- Store the resolved coordinator (`ralph`) fragments somewhere Step 3 can read them;
  wiring them into the prompt is Step 3's job.
- Document `fragments` in `docs/guide/configuration.md` and
  `skills/ralph-hats/references/schema.md`, including the YAML-anchor usage from the spec.

Test obligation — all through the real config-load path:
- Two-hat config (Fable Planner, Opus Builder), one `claude-fable-*` fragment: only the
  Planner's instructions contain the text (AC3).
- Same config with the Planner switched to `claude-opus-5-5`: no hat's instructions contain
  the text (AC4).
- Hat pinning its model only via `backend_args: ["--model", "claude-fable-5-2"]` gets the
  fragment (AC5).
- Hat with no model anywhere gets no fragment (AC6, config half).
- `hats:` naming a script hat → load fails, and the error message contains both the hat ID
  and `fragments` (AC8).
- Omitted `hats:` applies to every non-script hat with a matching model.
- Two matching fragments append in config order.

---

## Step 3 — Coordinator fragments and `ralph hats validate` reporting

Task key: `code-assist:model-specific-prompt-fragments:step-03:coordinator-and-validate`

**Demo when done:** a fragment with `hats: [ralph]` and a matching `cli.model` shows up in
the coordinator prompt, and `ralph hats validate` prints which fragments apply to which
hats plus a warning for one that matches nothing.

Covers AC7, the `hats validate` half of AC6, AC9, and the "preset tests still pass" half of
AC10.

Scope:
- Carry resolved coordinator fragments into `HatlessRalph` through a builder setter in the
  shape of `with_skill_index`, and render them in `build_prompt`. Call the setter at **both**
  `event_loop/mod.rs` construction sites.
- `validate_hats` gains a fragments report: for each hat, which fragment indices apply
  (and nothing listed for a hat with no model), plus a warning for any fragment that
  matches no hat, naming the fragment's index and its `models` patterns.
- Confirm `code-assist-multimodel`'s existing preset tests still pass with `model:`.

Test obligation:
- Build a coordinator prompt through `HatlessRalph::build_prompt` from a loaded config with
  `hats: [ralph]` and a matching `cli.model`; assert the text is present. A paired case with
  a non-matching `cli.model` asserts it is absent (AC7).
- A hat fragment does **not** leak into the coordinator prompt, and a `ralph` fragment does
  not leak into a hat's instructions.
- `validate_hats` output, captured through the writer seam the existing tests use:
  a matching fragment is reported against the right hat; a hat with no model has nothing
  reported as matched (AC6); an unmatched fragment produces a warning containing its index
  and its model patterns (AC9).
- Whole-workspace run of the verification commands in `context.md`, with logs written under
  `.ralph/specs/model-specific-prompt-fragments/logs/`.

---

## Plan Changes

_None yet. The Planner appends here when a built slice shows the remaining plan is wrong._
