# Hat Collections (Built-ins)

Built-in hat collections are pre-configured workflows for common development patterns.

## Quick Usage

```bash
# 1) Create core config once
ralph init --backend claude

# 2) List built-in hat collections
ralph init --list-presets

# 3) Run with a collection
ralph run -c ralph.yml -H builtin:feature
```

> `ralph init --preset <name>` is removed. Presets are now used via `-H/--hats`.

## Available Built-in Collections

- `feature` — general feature development
- `code-assist` — TDD implementation workflow
- `spec-driven` — specification-first workflow
- `refactor` — incremental refactoring workflow
- `pdd-to-code-assist` — full idea-to-code pipeline
- `bugfix` — reproduce/fix/verify/commit flow
- `debug` — hypothesis-driven debugging
- `review` — review-only workflow
- `pr-review` — multi-perspective PR review
- `fresh-eyes` — repeated skeptical review passes
- `gap-analysis` — spec-vs-implementation audit
- `docs` — documentation workflow
- `research` — exploration/analysis workflow
- `deploy` — deployment workflow
- `hatless-baseline` — no-hat baseline
- `merge-loop` — internal merge-loop workflow

## Examples

```bash
# Feature work
ralph run -c ralph.yml -H builtin:feature -p "Add user authentication"

# Debug workflow
ralph run -c ralph.yml -H builtin:debug -p "Investigate intermittent timeout"

# Use a local hats file
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml
```

## Core vs Hats Responsibilities

- `-c/--config` (core): backend, paths, guardrails, memories/tasks/skills, runtime defaults
- `-H/--hats` (collection): hats, events, and workflow event-loop settings

Single-file combined configs remain supported (`-c` file may include `hats`/`events`).

When `-H/--hats` is provided, it takes precedence:
- `hats`/`events` from `-H` replace `hats`/`events` from `-c`
- `event_loop` values from `-H` override matching `event_loop` keys from `-c`

For maintainability, prefer keeping core/runtime settings in `-c` and workflow hats in `-H`.

## Creating Your Own Hat Collection

Create a hats file with only hats-related sections:

```yaml
event_loop:
  starting_event: "build.start"
  completion_promise: "LOOP_COMPLETE"

hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done"]
    instructions: |
      Implement the requested change and verify it.
```

Run it:

```bash
ralph run -c ralph.yml -H hats/my-workflow.yml
```
