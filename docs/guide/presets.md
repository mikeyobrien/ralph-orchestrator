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
