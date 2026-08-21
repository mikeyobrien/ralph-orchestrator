# Task System

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive task system documentation.

## Overview

Ralph's task system provides runtime work tracking through
`.ralph/agent/tasks.jsonl`. It is independent of `core.scratchpad` and does
not replace it.

## Task Lifecycle

1. **Created** - Task added to the queue
2. **In Progress** - Agent actively working
3. **Completed** - Task finished successfully
4. **Blocked** - Awaiting dependency or input

## Configuration

```yaml
tasks:
  enabled: true  # Default
  path: .ralph/agent/tasks.jsonl
```

Under v3, these are Ralph coordination records. Autoloop's canonical task
store uses a different format and is the only task store in the engine's
completion gate; Ralph may only warn about its own open records after autoloop
completes.

## CLI Commands

```bash
ralph tools task list              # Show current tasks
ralph tools task add "description" # Add a runtime task
ralph tools task close <id>        # Close a completed task
```

## See Also

- [Memories & Tasks](../concepts/memories-and-tasks.md) - Core concepts
- [Memory System](memory-system.md) - Persistent learning
- [CLI Reference](../guide/cli-reference.md) - Full CLI documentation
