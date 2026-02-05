# Concepts

Understanding Hats's core concepts will help you use it effectively.

## Overview

Hats is built around a few key ideas:

1. **[The Hats Wiggum Technique](hats-wiggum-technique.md)** — Continuous iteration until success
2. **[The Six Tenets](tenets.md)** — Guiding principles for orchestration
3. **[Hats & Events](hats-and-events.md)** — Specialized personas coordinating through typed events
4. **[Coordination Patterns](coordination-patterns.md)** — Multi-agent workflow architectures
5. **[Memories & Tasks](memories-and-tasks.md)** — Persistent learning and runtime work tracking
6. **[Backpressure](backpressure.md)** — Quality gates that reject incomplete work

## The Core Philosophy

> "The orchestrator is a thin coordination layer, not a platform. Hats is smart; let Hats do the work."

Hats is intentionally simple. Rather than building complex features into the orchestrator, Hats:

- **Trusts the agent** to do the actual work
- **Provides structure** through hats and events
- **Enforces quality** through backpressure gates
- **Maintains state** through files on disk

## Traditional vs Hat-Based Mode

Hats supports two orchestration styles:

### Traditional Mode

A simple loop that runs until completion:

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
```

The agent iterates until it outputs `LOOP_COMPLETE` or hits limits.

### Hat-Based Mode

Specialized personas coordinate through events:

```yaml
cli:
  backend: "claude"

event_loop:
  starting_event: "task.start"
  completion_promise: "LOOP_COMPLETE"

hats:
  planner:
    triggers: ["task.start"]
    publishes: ["plan.ready"]
    instructions: "Create a plan..."

  builder:
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    instructions: "Implement the plan..."
```

Events flow between hats, each contributing to the task.

## Key Concepts Summary

| Concept | Description |
|---------|-------------|
| **Iteration** | One cycle of the orchestration loop |
| **Completion Promise** | Signal that ends the loop (default: `LOOP_COMPLETE`) |
| **Hat** | Specialized Hats persona with specific triggers and behaviors |
| **Event** | Typed message that triggers hats and carries state |
| **Backpressure** | Quality gate (tests, lint, typecheck) that rejects bad work |
| **Memory** | Persistent learning stored in `.hats/agent/memories.md` |
| **Task** | Runtime work item stored in `.hats/agent/tasks.jsonl` |

## Next Steps

- Understand the [Hats Wiggum Technique](hats-wiggum-technique.md)
- Learn the [Six Tenets](tenets.md) that guide Hats's design
- Master [Hats & Events](hats-and-events.md) for complex workflows
