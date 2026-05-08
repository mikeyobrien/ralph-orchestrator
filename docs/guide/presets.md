# Hat Collections

Built-in hat collections are now intentionally small. Ralph ships a core working set of defaults and documents broader workflow ideas as examples instead of treating every pattern as a supported builtin.

## Quick Start

```bash
ralph init --backend claude
ralph init --list-presets

ralph run -c ralph.yml -H builtin:code-assist -p "Add user authentication"
```

## Supported Builtins

| Collection | Hats | Best for | Notes |
|---|---|---|---|
| `code-assist` | `planner`, `builder`, `critic`, `finalizer` | Default implementation work | Recommended default; adds fresh-eyes review and a final completion gate |
| `debug` | `investigator`, `tester`, `fixer`, `verifier` | Root-cause debugging | Strong on repro and fix verification |
| `research` | `researcher`, `synthesizer` | Read-only analysis | No code changes |
| `review` | `reviewer`, `analyzer` | Adversarial code review | No code changes |
| `pdd-to-code-assist` | multi-stage design + build pipeline | Idea to code | Advanced and fun, but slower and less predictable |

## Internal Presets

Ralph also keeps a few internal/testing presets available without advertising them in the normal list:

- `merge-loop`
- `hatless-baseline`

## Recommended Workflow

- Use `code-assist` for most implementation tasks.
- Use `debug`, `research`, or `review` when you need a specialized mode.
- Use `pdd-to-code-assist` when you specifically want an end-to-end exploratory workflow and are comfortable paying for extra iterations.

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
| `wave-review` | `presets/wave-review.yml` | `coordinator`, `reviewer` (x3), `synthesizer` | `review.start` | `LOOP_COMPLETE` | Specialized parallel code review (wave-enabled) |

## Why The Builtin Set Is Small

Every builtin preset becomes product surface area:

- It must be documented.
- It must be tested and kept working.
- It must appear coherent in API and CLI listings.

Ralph now prefers a small supported set plus documentation examples for more experimental or niche orchestration patterns.

## Examples Instead Of Builtins

Historical workflow ideas such as spec-driven development, red-team review, mob programming, and fresh-eyes loops are now examples rather than shipped builtins. See:

- [Examples Index](../examples/index.md)
- [Spec-Driven Development Example](../examples/spec-driven.md)
- [Multi-Hat Workflow](../examples/multi-hat.md)

## Usage Examples

```bash
# Default implementation workflow
ralph run -c ralph.yml -H builtin:code-assist -p "Add OAuth login"

# Debugging
ralph run -c ralph.yml -H builtin:debug -p "Investigate why login fails on mobile"

# Research
ralph run -c ralph.yml -H builtin:research -p "Map the authentication architecture"

# Review
ralph run -c ralph.yml -H builtin:review -p "Review the changes in src/api/"

# Advanced/fun workflow
ralph run -c ralph.yml -H builtin:pdd-to-code-assist -p "Build a rate limiter"
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

### 4) Scatter-Gather (Waves)
One hat dispatches, parallel workers execute, an aggregator synthesizes.

Example: `wave-review`

See [Agent Waves](../advanced/agent-waves.md) for details.

### 5) Extended End-to-End Orchestration
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

## TOML-Shape Presets (multi-file directory)

Ralph also loads the multi-file TOML preset format originally defined by [`@mobrienv/autoloop`](https://github.com/mikeyobrien/autoloop) — a directory containing `autoloops.toml`, `topology.toml`, `harness.md`, and per-role prompt files under `roles/`. This is treated as a first-class preset format alongside YAML; use `-H <name>` or `-H <path>` identically.

```bash
# Discoverable via the resolver (see "Preset Resolution" below)
ralph run -H autocode -p "Add OAuth login"

# Or point at the directory directly
ralph run -H /path/to/autocode -p "Add OAuth login"
```

**Authoring a TOML preset** — follow the autoloop authoring guide:

- **[Creating presets](https://mikeyobrien.github.io/autoloop/guides/creating-presets)** — directory layout, `autoloops.toml` + `topology.toml` fields, role prompt conventions, harness rules, fail-closed patterns.
- **[Bundled preset examples](https://github.com/mikeyobrien/autoloop/tree/main/packages/presets/presets)** — 15+ reference implementations (autocode, autofix, autoreview, autodebug, autospec, etc.) you can copy as a starting point.

The TOML shape is useful when each role needs its own prompt file, a separate harness, or more structure than a single YAML permits. Both shapes compile to the same internal representation, so nothing about the rest of ralph changes — the backend, CLI, event flow, and completion semantics work identically.

## Preset Resolution

`-H <name>` resolves a bare preset name against these paths in order (first hit wins):

1. `./presets/<name>(.yml|.yaml|/)` — project-local
2. `$XDG_CONFIG_HOME/ralph/presets/<name>/`
3. `$HOME/.config/ralph/presets/<name>/` — user, canonical
4. `$HOME/.config/autoloop/presets/<name>/` — shared with the autoloop CLI
5. `$RALPH_PRESETS_DIR/<name>/` — explicit override
6. `$AUTOLOOP_PRESETS_DIR/<name>/` — back-compat fallback

Run `ralph hats list-presets` to see everything discoverable on your system (both YAML and TOML shapes in one table).

## Source of Truth and Sync

- Canonical preset files: `presets/*.yml`
- Embedded CLI mirror: `crates/ralph-cli/presets/*.yml`
- Sync script: `./scripts/sync-embedded-files.sh`
