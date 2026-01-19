# Migration Guide: v2.0 Hatless Ralph

This guide helps you migrate from v1.x to v2.0, which introduces the "Hatless Ralph" architecture.

## What Changed

**v1.x**: Ralph wore hats (planner, builder) that were hardcoded into the orchestrator.

**v2.0**: Ralph is a constant coordinator. Hats are optional and configurable. Ralph handles all events by default.

## Breaking Changes

1. **No default hats**: Empty config = solo Ralph mode (no hats)
2. **JSONL events**: Events written to `.agent/events.jsonl` instead of XML in output
3. **Per-hat backends**: Each hat can specify its own backend
4. **Planner removed**: No automatic planner hat

## Migration Steps

### Solo Mode (No Hats)

**Before (v1.x):**
```yaml
cli:
  backend: claude
```

**After (v2.0):**
```yaml
cli:
  backend: claude
# No hats section = Ralph handles everything
```

Ralph receives all prompts directly and writes events to `.agent/events.jsonl`.

### Multi-Hat Mode

**Before (v1.x):**
```yaml
cli:
  backend: claude
# Planner and builder hats were automatic
```

**After (v2.0):**
```yaml
cli:
  backend: claude

hats:
  builder:
    name: "Builder"
    description: "Builds code from tasks"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    backend: claude
    default_publishes: "build.done"
    instructions: |
      You're building. Pick ONE task from scratchpad.
```

### Per-Hat Backends

**New in v2.0**: Each hat can use a different backend.

```yaml
cli:
  backend: claude  # Default for Ralph

hats:
  builder:
    name: "Builder"
    backend: gemini  # This hat uses Gemini
    triggers: ["build.task"]

  reviewer:
    name: "Reviewer"
    backend:
      type: kiro
      agent: codex  # Kiro with custom agent
    triggers: ["review.request"]
```

### Default Publishes

**New in v2.0**: Hats can specify a fallback event if they forget to write one.

```yaml
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    default_publishes: "build.done"
```

If the builder completes without writing any events, Ralph automatically injects `build.done`.

## Event Format

**Before (v1.x)**: XML events in agent output
```xml
<event topic="build.done">
tests: pass
</event>
```

**After (v2.0)**: JSONL in `.agent/events.jsonl`
```json
{"topic":"build.done","payload":"tests: pass","ts":"2026-01-14T19:30:00Z"}
```

Agents write events using this format. Ralph reads from the file.

## Common Configurations

### Feature Development (Multi-Hat)

```yaml
cli:
  backend: claude

hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    backend: claude
    default_publishes: "build.done"

  tester:
    name: "Tester"
    triggers: ["test.request"]
    publishes: ["test.pass", "test.fail"]
    backend: gemini
```

### Research (Solo Mode)

```yaml
cli:
  backend: claude
# No hats - Ralph does everything
```

### Mixed Backends

```yaml
cli:
  backend: claude

hats:
  fast-tasks:
    name: "Fast Tasks"
    backend: gemini
    triggers: ["quick.task"]

  complex-tasks:
    name: "Complex Tasks"
    backend: claude
    triggers: ["complex.task"]
```

## Validation

Test your config:
```bash
ralph validate ralph.yml
```

## Rollback

If you need to rollback to v1.x behavior, use a preset:
```bash
ralph run --preset feature
```

Presets provide curated multi-hat configurations.
