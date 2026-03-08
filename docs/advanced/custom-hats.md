# Creating Custom Hats

## Overview

Custom hats allow you to extend Ralph's orchestration capabilities by defining specialized behavioral modes for AI agents.

## Defining a Hat Inline

The simplest approach — define hats directly in your preset or config file:

```yaml
hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done", "build.blocked"]
    default_publishes: "build.done"
    max_activations: 5
    instructions: |
      Implement the task using TDD.
```

## Importing Hats from Files

When the same hat is used across multiple presets, define it once in a standalone file and import it.

### Imported hat file format

An imported hat file is a plain YAML mapping with the same fields as an inline hat definition:

```yaml
# shared-hats/builder.yml
name: "Builder"
description: "TDD builder — one task, one commit"
triggers: ["build.start"]
publishes: ["build.done", "build.blocked"]
default_publishes: "build.done"
max_activations: 5
instructions: |
  ## BUILDER MODE
  Implement the task using TDD.
  Run tests before emitting build.done.
```

No fields are required in the imported file — you can provide them via overrides in the consuming preset.

### Using imports in a preset

Reference the file with `import:` and optionally override any fields:

```yaml
hats:
  builder:
    import: ./shared-hats/builder.yml

  builder-strict:
    import: ./shared-hats/builder.yml
    max_activations: 1
    publishes: ["build.done"]  # replaces the full list from the import

  reviewer:
    name: "Reviewer"
    triggers: ["build.done"]
    publishes: ["LOOP_COMPLETE"]
    instructions: |
      Review the change and close when satisfied.
```

Imported and inline hats can be mixed freely in the same file.

### Path resolution

Paths are resolved relative to the importing file's directory:

```
project/
├── ralph.yml                        # -c ralph.yml
├── presets/
│   └── my-workflow.yml              # -H presets/my-workflow.yml
└── shared-hats/
    └── builder.yml
```

From `presets/my-workflow.yml`, use `import: ../shared-hats/builder.yml`.

Absolute paths also work: `import: /home/team/shared-hats/builder.yml`.

### Override semantics

Any field specified alongside `import:` **fully replaces** the corresponding field from the imported file. There is no deep merging.

```yaml
# Imported file has: publishes: ["build.done", "build.blocked"]

hats:
  builder:
    import: ./shared-hats/builder.yml
    publishes: ["build.done"]
    # Result: publishes is ["build.done"] — not a union of both lists
```

Fields not specified locally are inherited from the imported file.

### Restrictions

- **No transitive imports**: An imported file cannot itself contain an `import:` directive.
- **No events in imported files**: Event metadata (`events:`) belongs in the consuming preset, not in shared hat files.
- **Embedded presets**: Builtin presets (`-H builtin:<name>`) resolve `import:` paths from a compiled-in shared hat library. Only paths matching `shared-hats/*.yml` are available.

### Split config resolution

When using `-c ralph.yml -H hats.yml`, each file resolves its own imports independently, relative to its own directory.

```bash
ralph run -c ralph.yml -H ./presets/my-hats.yml
# ralph.yml resolves imports relative to .
# my-hats.yml resolves imports relative to ./presets/
```

## Hat Fields Reference

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | Yes | Display name |
| `description` | string | No | Purpose description |
| `triggers` | list | Recommended | Event subscription patterns (defaults to empty) |
| `publishes` | list | Recommended | Allowed event types (defaults to empty) |
| `default_publishes` | string | No | Default event if none explicit |
| `max_activations` | integer | No | Limit how many times this hat activates |
| `backend` | string | No | Backend override for this hat |
| `backend_args` | list | No | Extra CLI arguments for this hat's backend |
| `instructions` | string | Recommended | Prompt injected when hat is active (defaults to empty) |
| `extra_instructions` | list | No | Additional instruction fragments appended |
| `disallowed_tools` | list | No | Tools this hat cannot use |
| `import` | string | No | Path to an imported hat file |

## See Also

- [Hats & Events](../concepts/hats-and-events.md) - Core concepts
- [Presets](../guide/presets.md) - Using built-in hat collections
- [Configuration](../guide/configuration.md) - Full configuration reference
