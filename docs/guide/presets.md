# Hat Collections (Built-ins)

Built-in hat collections are pre-configured workflows for common development patterns.

Use them with `-H/--hats` alongside a core config loaded via `-c/--config`.

## Quick Start (Current CLI)

```bash
# 1) Create core config once
ralph init --backend claude

# 2) List built-in collections
ralph init --list-presets

# 3) Run with a built-in collection
ralph run -c ralph.yml -H builtin:feature
```

> `ralph init --preset <name>` is removed.
> Use `ralph run -c ralph.yml -H builtin:<name>`.

## Core vs Hats Responsibilities

- `-c/--config` (core): backend, paths, guardrails, memories/tasks/skills, runtime defaults
- `-H/--hats` (collection): hats, events, and workflow event-loop settings

Single-file combined configs remain supported (`-c` file may include `hats`/`events`).

When `-H/--hats` is provided, it takes precedence:
- `hats`/`events` from `-H` replace `hats`/`events` from `-c`
- `event_loop` values from `-H` override matching `event_loop` keys from `-c`
- `-c core.*=...` overrides are still applied last

## Available Built-in Collections

`ralph init --list-presets` lists these collections.

| Collection | Canonical source | Hats | Start event | Completion | Best for |
|---|---|---|---|---|---|
| `bugfix` | `presets/bugfix.yml` | `reproducer`, `fixer`, `verifier`, `committer` | `repro.start` | `LOOP_COMPLETE` (default) | Reproduce/fix/verify/commit bug workflow |
| `code-assist` | `presets/code-assist.yml` | `planner`, `builder`, `validator`, `committer` | `build.start` | `LOOP_COMPLETE` | TDD implementation from specs/tasks/descriptions |
| `debug` | `presets/debug.yml` | `investigator`, `tester`, `fixer`, `verifier` | `debug.start` | `DEBUG_COMPLETE` | Root-cause debugging and hypothesis testing |
| `deploy` | `presets/deploy.yml` | `builder`, `deployer`, `verifier` | `task.start` (default) | `LOOP_COMPLETE` | Deployment and release workflows |
| `docs` | `presets/docs.yml` | `writer`, `reviewer` | `task.start` (default) | `DOCS_COMPLETE` | Documentation writing and review |
| `feature` | `presets/feature.yml` | `builder`, `reviewer` | `task.start` (default) | `LOOP_COMPLETE` | Feature development with integrated review |
| `fresh-eyes` | `presets/fresh-eyes.yml` | `builder`, `fresh_eyes_auditor`, `fresh_eyes_gatekeeper` | `fresh_eyes.start` | `LOOP_COMPLETE` | Enforced repeated skeptical self-review passes |
| `gap-analysis` | `presets/gap-analysis.yml` | `analyzer`, `verifier`, `reporter` | `gap.start` | `GAP_ANALYSIS_COMPLETE` | Spec-vs-implementation auditing |
| `hatless-baseline` | `presets/hatless-baseline.yml` | _(none)_ | `task.start` | `LOOP_COMPLETE` | Baseline no-hat behavior for comparison |
| `merge-loop` | `crates/ralph-cli/presets/merge-loop.yml` | `merger`, `resolver`, `tester`, `cleaner`, `failure_handler` | `merge.start` | `MERGE_COMPLETE` | Internal merge/worktree automation |
| `pdd-to-code-assist` | `presets/pdd-to-code-assist.yml` | `inquisitor`, `architect`, `design_critic`, `explorer`, `planner`, `task_writer`, `builder`, `validator`, `committer` | `design.start` | `LOOP_COMPLETE` | Full idea → plan → implementation pipeline |
| `pr-review` | `presets/pr-review.yml` | `correctness_reviewer`, `security_reviewer`, `architecture_reviewer`, `synthesizer` | `task.start` (default) | `LOOP_COMPLETE` | Multi-perspective PR review |
| `refactor` | `presets/refactor.yml` | `refactorer`, `verifier` | `task.start` (default) | `REFACTOR_COMPLETE` | Incremental, verified refactoring |
| `research` | `presets/research.yml` | `researcher`, `synthesizer` | `research.start` | `RESEARCH_COMPLETE` | Exploration and analysis without code changes |
| `review` | `presets/review.yml` | `reviewer`, `analyzer` | `review.start` | `REVIEW_COMPLETE` | Review-only workflow |
| `spec-driven` | `presets/spec-driven.yml` | `spec_writer`, `spec_reviewer`, `implementer`, `verifier` | `spec.start` | `LOOP_COMPLETE` (default) | Specification-driven implementation |

Notes:
- If a collection omits `event_loop.completion_promise`, Ralph defaults to `LOOP_COMPLETE`.
- `merge-loop` is primarily internal and used by merge queue/worktree flows.

## Usage Examples

```bash
# Feature work
ralph run -c ralph.yml -H builtin:feature -p "Add user authentication"

# Debug workflow
ralph run -c ralph.yml -H builtin:debug -p "Investigate intermittent timeout"

# Spec-driven workflow
ralph run -c ralph.yml -H builtin:spec-driven -p "Build a rate limiter"

# Research workflow (analysis without code changes)
ralph run -c ralph.yml -H builtin:research -p "Map auth architecture"

# Use a local hats file instead of a built-in
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml
```

## Common Workflow Patterns

Ralph built-ins usually follow one of these shapes:

### 1) Linear Pipeline
A fixed sequence of specialist hats.

Examples: `feature`, `bugfix`, `deploy`, `docs`

### 2) Critic / Actor Loop
One hat proposes, another critiques/validates, then iterates.

Examples: `spec-driven`, `review`, `fresh-eyes`

### 3) Multi-Reviewer + Synthesis
Parallel perspectives merged into one result.

Example: `pr-review`

### 4) Extended End-to-End Orchestration
Large multi-stage pipelines from idea through implementation.

Example: `pdd-to-code-assist`

## Split Config vs Single-File Config

Recommended:
- Keep core/runtime config in `ralph.yml`
- Select workflow via `-H builtin:<name>`

Backward-compatible single-file mode (still supported):

```bash
# Uses one combined preset file as the main config
ralph run -c presets/feature.yml -p "Add OAuth login"
```

## Creating Your Own Hat Collection

Create a hats file with hats-related sections:

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

  reviewer:
    name: "Reviewer"
    triggers: ["build.done"]
    publishes: ["LOOP_COMPLETE"]
    instructions: |
      Review the change, request fixes if needed, and close when done.
```

Run it:

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml
```

## Source of Truth and Sync

- Canonical preset files: `presets/*.yml`
- Embedded CLI mirror: `crates/ralph-cli/presets/*.yml`
- Sync script: `./scripts/sync-embedded-files.sh`
