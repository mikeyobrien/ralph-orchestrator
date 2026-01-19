# Hat System Guide

Ralph's hat system enables role-based agent coordination through events. Each hat represents a specialized persona that activates on specific events and publishes events when done.

## What Are Hats?

Hats are specialized agent personas that coordinate through events. Each hat has:

| Component | Description |
|-----------|-------------|
| **name** | Display name (supports emoji) |
| **description** | Purpose description for hat selection |
| **triggers** | Events that activate this hat |
| **publishes** | Events this hat can emit |
| **instructions** | Custom prompt for the hat's role |

When an event is published, Ralph routes it to the hat that triggers on that event. The hat does its work and publishes another event, potentially triggering a different hat. This continues until the completion promise is output.

## When to Use Hats

### Solo Mode (No Hats)

Best for: Simple tasks, exploration, one-shot operations

```yaml
# ralph.yml - no hats section
cli:
  backend: claude

event_loop:
  prompt_file: PROMPT.md
  completion_promise: "LOOP_COMPLETE"
```

In solo mode, Ralph handles everything directly without role switching. This is the simplest configuration.

### Multi-Hat Mode

Best for: Complex workflows, TDD, code review pipelines, multi-perspective analysis

```yaml
cli:
  backend: claude

event_loop:
  completion_promise: "LOOP_COMPLETE"
  starting_event: "task.start"

hats:
  planner:
    name: "Planner"
    description: "Breaks down tasks and coordinates work"
    triggers: ["task.start", "build.done"]
    publishes: ["build.task"]
    instructions: |
      Plan the implementation and assign tasks...

  builder:
    name: "Builder"
    description: "Implements code based on plans"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    instructions: |
      Implement one task at a time...
```

## Quick Start with Presets

Ralph ships with 23 pre-configured hat collections for common workflows:

```bash
# List available presets
ralph init --list-presets

# Use a preset
ralph init --preset tdd-red-green
ralph run -p "Implement a binary search function"

# Use preset with different backend
ralph init --preset spec-driven --backend gemini
```

See the [Preset Reference](../reference/presets.md) for complete documentation of all presets.

## Hat Configuration Reference

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Human-readable name (supports emoji) |
| `description` | string | Purpose description - used for hat selection |
| `triggers` | string[] | Events that activate this hat |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `publishes` | string[] | `[]` | Events this hat can emit |
| `instructions` | string | `""` | Custom prompt instructions for this hat |
| `backend` | string/object | inherits | Override backend for this hat |
| `default_publishes` | string | null | Auto-emit if hat completes without publishing |

### Example Hat Definition

```yaml
hats:
  reviewer:
    name: "Code Reviewer"
    description: "Reviews implementation for quality. Does NOT modify code."
    triggers: ["review.request"]
    publishes: ["review.approved", "review.changes_requested"]
    default_publishes: "review.approved"
    instructions: |
      ## REVIEWER MODE

      Review the most recent implementation for quality.

      ### Checklist
      - [ ] Correctness: Does it match requirements?
      - [ ] Tests: Are there tests? Edge cases covered?
      - [ ] Patterns: Follows existing codebase patterns?

      ### If Approved
      Publish `<event topic="review.approved">`

      ### If Changes Needed
      Publish `<event topic="review.changes_requested">` with feedback
```

## Event Flow Design

### Event Naming Convention

Use `category.action` format for events:

| Category | Example Events |
|----------|----------------|
| build | `build.task`, `build.done`, `build.blocked` |
| review | `review.request`, `review.approved`, `review.changes_requested` |
| test | `test.written`, `test.passing`, `test.failing` |
| spec | `spec.ready`, `spec.approved`, `spec.rejected` |

### Reserved Events

These events have special meaning and are handled by Ralph:

| Event | Owner | Purpose |
|-------|-------|---------|
| `task.start` | Ralph | Published at loop start |
| `task.resume` | Ralph | Published when resuming interrupted session |

If no hat triggers on these events, Ralph handles them as the universal fallback.

### Designing a Workflow

1. **Identify roles**: What distinct responsibilities exist?
2. **Map events**: What triggers each role? What does it produce?
3. **Ensure coverage**: Every published event should have a subscriber (or Ralph catches it)
4. **Avoid conflicts**: Each trigger can only belong to one hat

### Example: TDD Workflow

```
task.start or refactor.done → [Test Writer] → test.written
                                                    ↓
                                            [Implementer] → test.passing
                                                    ↓
                                            [Refactorer] → cycle.complete
                                                  or ↓
                                               refactor.done → [Test Writer]
```

In YAML:

```yaml
event_loop:
  starting_event: "tdd.start"

hats:
  test_writer:
    name: "Test Writer"
    description: "Writes FAILING tests first. Never implements."
    triggers: ["tdd.start", "refactor.done"]
    publishes: ["test.written"]

  implementer:
    name: "Implementer"
    description: "Makes failing test pass with MINIMAL code."
    triggers: ["test.written"]
    publishes: ["test.passing"]

  refactorer:
    name: "Refactorer"
    description: "Cleans up code while keeping tests green."
    triggers: ["test.passing"]
    publishes: ["refactor.done", "cycle.complete"]
    default_publishes: "cycle.complete"
```

## Per-Hat Backend Configuration

Each hat can use a different AI backend, enabling heterogeneous agent teams.

### Named Backend

Use a simple string to reference a known backend:

```yaml
hats:
  builder:
    backend: gemini  # Use Gemini for this hat
    triggers: ["build.task"]
    publishes: ["build.done"]
```

### Kiro with Custom Agent

Kiro supports multiple agents via the `agent` field:

```yaml
hats:
  reviewer:
    backend:
      type: kiro
      agent: codex  # Use Codex agent via Kiro
    triggers: ["review.request"]
    publishes: ["review.done"]
```

### Custom Command

Run any CLI tool as a backend:

```yaml
hats:
  specialist:
    backend:
      command: ./my-custom-agent
      args: ["--mode", "review"]
    triggers: ["custom.trigger"]
    publishes: ["custom.done"]
```

## The default_publishes Fallback

If a hat completes without writing an event, `default_publishes` auto-emits:

```yaml
hats:
  refactorer:
    triggers: ["test.passing"]
    publishes: ["refactor.done", "cycle.complete"]
    default_publishes: "cycle.complete"  # Emitted if hat forgets
```

This prevents workflows from getting stuck when a hat doesn't explicitly publish an event.

## Validation Rules

Ralph validates hat configurations at startup:

| Rule | Error | Solution |
|------|-------|----------|
| Unique triggers | "Ambiguous routing for trigger 'X'" | Each trigger can only belong to one hat |
| Reserved triggers | "Reserved trigger 'task.start' used by hat 'X'" | Use different event names |

### Valid Configurations (No Longer Errors)

With the Hatless Ralph architecture, these are now valid:

| Configuration | Why Valid |
|--------------|-----------|
| Empty hats (`hats: {}`) | Ralph runs in solo mode |
| No entry point | Ralph handles `task.start` as universal fallback |
| Orphan events | Orphaned events fall through to Ralph |
| Unreachable hats | Wasteful but not fatal |

## Troubleshooting

### Hat Never Activates

**Symptoms**: A hat you defined never runs.

**Causes**:
- Trigger spelling doesn't match published event exactly
- No hat publishes the triggering event
- Hat is unreachable in the event flow

**Solutions**:
```bash
# Check event history
ralph events

# Look for the specific topic
ralph events --topic your.event
```

### Workflow Gets Stuck

**Symptoms**: Ralph stops iterating but task isn't complete.

**Causes**:
- Hat doesn't publish an event
- Published event has no subscriber and Ralph can't complete

**Solutions**:
- Add `default_publishes` to hats as a safety net
- Check for typos in event names
- Ensure event flow has a path to completion promise

### Ambiguous Routing Error

**Error**: `Ambiguous routing for trigger 'X'. Both 'hat1' and 'hat2' trigger on 'X'.`

**Cause**: Two hats claim the same trigger event.

**Solution**: Each trigger can only belong to ONE hat. Either:
- Split the trigger into separate events
- Merge the hats into one
- Redesign the event flow

### Event Not Found

**Symptoms**: `ralph events --topic X` shows nothing.

**Causes**:
- Hat didn't publish the event
- Event name has a typo

**Solutions**:
- Add explicit event publishing in hat instructions
- Double-check event name spelling across all hats

## Advanced Patterns

### Self-Routing

A hat can trigger on events it also publishes. This is allowed and NOT considered ambiguous routing:

```yaml
hats:
  iterator:
    triggers: ["work.item", "work.next"]
    publishes: ["work.next", "work.complete"]
    # Can publish work.next to trigger itself again
```

### Critic-Actor Pattern

One hat produces work, another validates it:

```yaml
hats:
  builder:
    triggers: ["build.task"]
    publishes: ["build.done"]

  reviewer:
    triggers: ["build.done"]
    publishes: ["review.approved", "review.rejected"]

  # Ralph catches review.approved/rejected and coordinates next steps
```

### Multiple Entry Points

Different events can trigger different workflows:

```yaml
hats:
  feature_starter:
    triggers: ["feature.start"]
    publishes: ["design.task"]

  bug_starter:
    triggers: ["bug.start"]
    publishes: ["investigate.task"]
```

## Next Steps

- [Preset Reference](../reference/presets.md) — All 23 pre-configured workflows
- [Configuration Guide](configuration.md) — Full YAML schema reference
- [Event Loop Specification](../../specs/event-loop.spec.md) — Technical details
