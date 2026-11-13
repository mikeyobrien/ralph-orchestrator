---
status: approved
created: 2026-11-12
related:
  - https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5-2
  - https://github.com/mikeyobrien/ralph-orchestrator/pull/418
---

# Model-specific prompt fragments

## Problem

Model vendors now publish prompting guidance per model, and the advice differs between models. Some examples from the Claude Fable 5.2 guide:

- The sentence "You are the owner and long-term maintainer of this codebase, and its design is up to you." raised consolidation of duplicated code from under 10% of runs to about 75%. It also raised output tokens by 70–80% and caused changes nobody asked for.
- The "Communicating clearly" block makes final reports easier to read for people who did not watch the run.
- The "Finishing work" block reduces early stops, mostly at `low` and `medium` effort. Ralph's core prompt already carries a version of it (`FINISHING_WORK` in `crates/ralph-core/src/hatless_ralph.rs`) for every model.
- Instructions written for earlier models can now cost more or have side effects. Examples are lines that tell the model to think less, absolute "never" rules, and requests to write out reasoning (which can trigger the `reasoning_extraction` refusal).

Ralph has no way to say "add this text when the hat runs on model X". Since #418, one preset can run different models per hat (`code-assist-multimodel`: Planner on `claude-fable-5-2`, the other hats on `claude-opus-5-5`). Today a preset author must put model-specific text into one hat's `instructions` by hand. The text then stays there when someone swaps the model, which is the failure the Fable guide warns about.

Ralph also does not know which model a hat runs. The model exists only as an opaque `--model` string inside `backend.args`.

## Goals

1. A hat's model is a first-class config value that ralph can read.
2. A config (including an embedded preset, which is a single YAML file) can declare instruction fragments that apply only when a hat runs on a matching model.
3. Swapping a hat's model swaps its fragments with no other edit.

## Non-goals

- Shipping a library of vendor guidance inside ralph. Presets own their text.
- Detecting the model at runtime from backend output. The prompt is built before the backend starts.
- Effort-conditional fragments. Possible later; see Open questions.

## Design

### 1. `model` field on hats and `cli`

Add `model: Option<String>` to `HatConfig` and `CliConfig`, beside `effort`. A hat's effective model is `hat.model`, else `cli.model`.

Map the model to the backend's own flag at the same point where effort is applied (Step 2.6 in `crates/ralph-cli/src/loop_runner.rs`, through a new `CliBackend::apply_model` next to `apply_effort`):

| Backend | Flag |
|---|---|
| claude | `--model <id>` |
| codex | `-m <id>` |
| kiro / kiro-acp | `--model <id>` (already supported by `kiro_acp_with_options`) |
| others | not applied; log at debug, as for effort |

An explicit model flag already present in `backend.args` wins over `model:`, as `--effort` does today. For model matching (section 2), ralph uses `model:` when set, else a `--model`/`-m` value found in the hat's resolved backend args. So existing configs that pin the model through `args` still match fragments.

Migrate `presets/code-assist-multimodel.yml` from `args: ["--model", …]` to `model:`.

### 2. `fragments` in config

Add a top-level `fragments` list. Each entry has:

```yaml
fragments:
  - models: ["claude-fable-5-2*"]   # glob patterns; required
    hats: [builder]                 # optional; default: every hat, plus the coordinator
    text: |
      You are the owner and long-term maintainer of this codebase, and its design is up to you.
```

- `models` matches the hat's effective model with glob patterns (`*`, `?`). Matching is case-sensitive, on the full ID.
- `hats` names hat IDs. The reserved ID `ralph` targets the coordinator prompt (the hatless prompt that runs when no hat subscribes to a topic).
- A fragment that matches is appended to the hat's instructions, after `extra_instructions`, in the order the fragments appear in config.
- A hat with no known model matches no fragment.
- Fragments do not apply to script hats (`run:`). A fragment whose `hats` names a script hat is a config error, as `extra_instructions` on a script hat is today (`ConfigError::ScriptHatWithInstructions`).

Resolution happens in `normalize()` in `crates/ralph-core/src/config.rs`, beside the existing `extra_instructions` merge. Hat models are static config, so no runtime state is needed.

YAML anchors let one block serve several fragments:

```yaml
_fable_reports: &fable_reports |
  # Communicating clearly
  Write for an intelligent but low-context reader. ...

fragments:
  - models: ["claude-fable-*"]
    hats: [finalizer, critic]
    text: *fable_reports
```

### 3. Visibility

- `ralph hats validate` reports which fragments apply to which hats, and warns when a fragment matches no hat (a typo in a model glob or hat ID).
- With `RALPH_DIAGNOSTICS=1`, the logged prompt already shows the merged text; nothing new is needed.

### 4. Core prompt text (follow-up, not in this change)

`FINISHING_WORK` and other model-tuned text in `hatless_ralph.rs` could move into fragments that a preset opts into. This change does not move them. It makes that move possible and measurable.

## Rejected alternative: a `models:` filter on skills

Skills already support `hats:` and `backends:` filters (`SkillOverride`, `crates/ralph-core/src/skill_registry.rs`). A `models:` filter there would be the smallest diff. Two reasons against it:

1. An embedded preset is one YAML file and cannot ship a skill directory.
2. The existing skill filters do not work per hat at runtime. The skill index is built once with `hat_id = None` (`event_loop/mod.rs` around line 314). Auto-inject skills are also selected with `None` (around line 1659). The backend filter compares against the global `cli.backend`, not the hat's own backend. A `models:` filter would inherit the same problem.

The second point is a separate defect. It is tracked as its own task (below) and is not a dependency of this design.

## Acceptance criteria

- Given a hat with `model: claude-fable-5-2` on the claude backend, when the iteration runs, then the backend command line contains `--model claude-fable-5-2`. On codex it contains `-m <id>`.
- Given a hat whose `backend.args` already contain `--model X` and whose `model:` is `Y`, when the iteration runs, then the command line contains `--model X` once and not `Y`.
- Given a fragment with `models: ["claude-fable-*"]`, when the Planner runs on `claude-fable-5-2` and the Builder runs on `claude-opus-5-5`, then only the Planner's prompt contains the fragment text.
- Given that config, when the Planner's model is changed to `claude-opus-5-5` and nothing else changes, then no prompt contains the fragment text.
- Given a hat that pins its model only through `backend.args: ["--model", "claude-fable-5-2"]`, when fragments are resolved, then the Fable fragment applies to it.
- Given a hat with no model set anywhere, then no fragment applies to it, and `ralph hats validate` reports nothing as matched for that hat.
- Given a fragment with `hats: [ralph]` and a matching `cli.model`, then the coordinator prompt contains the text.
- Given a fragment whose `hats` names a script hat, then config load fails with an error that names the hat and `fragments`.
- Given a fragment that matches no hat, then `ralph hats validate` prints a warning that names the fragment's index and patterns.
- `code-assist-multimodel` uses `model:` and its preset tests still pass. Tests assert the protocol (which hat gets which text) rather than the wording of the fragment.

## Separate task: per-hat skill filtering

File as `.ralph/tasks/skill-filters-per-hat.code-task.md`:

- Build the skill index per iteration for the active hat, not once with `None`.
- Select auto-inject skills for the active hat.
- Filter by the hat's effective backend, not `cli.backend`.
- Regression test: a skill with `hats: [builder]` is absent from the Critic's prompt; a skill with `backends: [codex]` is absent from a Claude-backed hat's prompt in a mixed-backend config.

## Open questions

1. **Effort-conditional fragments.** The Fable guide recommends "Finishing work" mainly at `low` and `medium` effort. An optional `effort: [low, medium]` key on a fragment would cover that. Proposal: leave it out until a preset needs it.
2. **Model aliases.** Claude Code accepts aliases such as `opus`, which do not match `claude-opus-*`. Proposal: document that fragments match the literal configured string, and recommend full IDs in presets.
