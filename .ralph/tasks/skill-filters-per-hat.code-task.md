# Task: Apply skill `hats` and `backends` filters per hat

## Description
Skills can be scoped with `hats:` and `backends:` (frontmatter or `skills.overrides`), but at runtime the scoping does not take effect per hat. Make the skill index and auto-injected skills reflect the hat that is running and that hat's own backend.

## Background
- The skill index is built once, with `hat_id = None`, when the event loop is constructed (`crates/ralph-core/src/event_loop/mod.rs`, around line 314). `is_visible` treats `None` as "show everything", so every hat sees every skill.
- Auto-inject skills are selected with `auto_inject_skills(None)` (around line 1659), with the same effect.
- The registry's `active_backend` is `cli.backend`. A hat with its own `backend:` is filtered as if it ran on the global backend, so `backends: [codex]` skills appear in Claude-backed hats of a mixed config (for example `code-assist-multimodel` with a Codex hat).

Found while designing `.ralph/specs/model-specific-prompt-fragments.spec.md`.

## Technical Requirements
1. Select skill-index entries and auto-inject skills for the active hat on each iteration.
2. Filter `backends:` against the hat's effective backend (hat `backend`, else `cli.backend`).
3. In solo mode (no hats), keep today's behavior.

## Acceptance Criteria
1. **Hat filter.** Given a skill with `hats: [builder]` and a config with Builder and Critic hats, when the Critic's prompt is built, then the skill is absent from its index and auto-inject block; when the Builder's prompt is built, it is present.
2. **Per-hat backend.** Given `cli.backend: claude`, a hat with `backend: codex`, and a skill with `backends: [codex]`, when prompts are built, then only the Codex hat's prompt lists the skill.
3. **Solo mode.** Given no hats, a skill with `hats: [builder]` still appears, as today.
4. Tests exercise prompt building through the event loop, not `is_visible` alone.

## Metadata
- **Complexity**: Medium
- **Labels**: skills, prompts, bug
