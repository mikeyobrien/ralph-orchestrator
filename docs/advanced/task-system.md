# Task System

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive task system documentation.

## Overview

Hats's task system provides runtime work tracking through `.agent/tasks.jsonl`, replacing the legacy scratchpad mechanism.

## Task Lifecycle

1. **Created** - Task added to the queue
2. **In Progress** - Agent actively working
3. **Completed** - Task finished successfully
4. **Blocked** - Awaiting dependency or input

## Configuration

```yaml
tasks:
  enabled: true  # Default
  path: .agent/tasks.jsonl
```

## CLI Commands

```bash
hats task list              # Show current tasks
hats task add "description" # Add new task
hats task complete <id>     # Mark task complete
```

## See Also

- [Memories & Tasks](../concepts/memories-and-tasks.md) - Core concepts
- [Memory System](memory-system.md) - Persistent learning
- [CLI Reference](../guide/cli-reference.md) - Full CLI documentation
