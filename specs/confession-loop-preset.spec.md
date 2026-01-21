# Spec: Confession Loop Preset (Confidence-Aware Completion)

## Context

Upstream issue: mikeyobrien/ralph-orchestrator#74 proposes a "Confession" phase: a structured self-assessment that gates loop completion based on honesty and a numeric confidence score.

The maintainer comment on the issue notes this can be implemented today via hat instructions and does not require orchestrator changes. This spec follows that direction: keep the orchestrator thin and provide a preset that demonstrates the pattern.

## Goal

Add an embedded preset named `confession-loop` that:

- Adds a dedicated self-assessment ("Confession") phase after implementation.
- Produces a numeric confidence score (0-100).
- Uses a confidence threshold (default 80) to decide whether to continue iterating or finish.
- Avoids any new orchestrator logic (configuration/preset only).

## Non-Goals

- Do not add new core fields or termination logic to `ralph-core`.
- Do not add new dependencies or modify lockfiles.
- Do not require live API calls for tests.

## User Workflow

1. User runs `ralph init --preset confession-loop`.
2. User writes `PROMPT.md` describing the task and runs `ralph run`.
3. The loop executes implementation, then confession, then a handler that either:
   - republishes a new build task when confidence is below threshold or issues exist, or
   - emits `LOOP_COMPLETE` only when confidence meets the threshold and there are no issues.

## Preset Design

### Hats

- `builder`
  - Trigger: `build.task`
  - Publishes: `build.done`, `build.blocked`
  - Requirement: maintain an "Internal Monologue" section in `.agent/scratchpad.md` while working.

- `confessor`
  - Trigger: `build.done`
  - Publishes: `confession.clean`, `confession.issues_found`
  - Requirement: append `## Confession` section to scratchpad, including:
    - Objective assessment (Met? + evidence)
    - Uncertainties and shortcuts
    - Single easiest issue to verify (one concrete command or check)
    - Confidence score (0-100)
  - Decision: if any issues OR confidence < 80 -> `confession.issues_found`, else `confession.clean`.

- `confession_handler`
  - Trigger: `confession.clean`, `confession.issues_found`
  - Publishes: `build.task`, `escalate.human`
  - Behavior:
    - If issues found or confidence < 80: publish `build.task` with specific follow-up actions (or `escalate.human` for major issues).
    - If clean and confidence >= 80: emit the completion promise (`LOOP_COMPLETE`) and avoid inventing new tasks.

### Confidence Threshold

- Default threshold: 80.
- Represented purely in the preset instructions (no new config fields).

## Acceptance Criteria

1. `ralph init --list-presets` includes `confession-loop`.
2. `ralph init --preset confession-loop` succeeds and writes `ralph.yml`.
3. The preset is valid YAML (covered by existing preset YAML validation tests).
4. The preset exists in both:
   - `presets/confession-loop.yml`
   - `crates/ralph-cli/presets/confession-loop.yml`
5. `crates/ralph-cli/src/presets.rs` embeds `confession-loop` and preset-count tests are updated.
6. Deterministic CLI integration tests cover the user workflow of listing presets and initializing from the preset.

