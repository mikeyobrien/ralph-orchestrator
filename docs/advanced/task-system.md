# Task System

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive task system documentation.

## Overview

Ralph's task system provides runtime work tracking through `.agent/tasks.jsonl`, replacing the legacy scratchpad mechanism.

!!! info "Two task systems"
    Ralph has **two distinct task systems**:

    1. **Runtime tasks** (`.ralph/agent/tasks.jsonl`) — Lightweight work items managed via `ralph tools task` during orchestration runs. Statuses: `open`, `in_progress`, `closed`, `failed`. Described on this page.
    2. **Board tasks** (`.ralph/api/tasks-v1.json`) — The RPC v1 control-plane task board managed via the `task.*` RPC methods. Canonical board states: `backlog`, `ready`, `in_progress`, `in_review`, `blocked`, `done`, `cancelled`. See the [ralph-api README](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/crates/ralph-api/README.md) for the full board-state contract.

    These systems serve different purposes: runtime tasks track agent work within a single loop iteration, while board tasks represent the shared project board visible to all workers and the web dashboard.

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
ralph task list              # Show current tasks
ralph task add "description" # Add new task
ralph task complete <id>     # Mark task complete
```

## See Also

- [Memories & Tasks](../concepts/memories-and-tasks.md) - Core concepts
- [Memory System](memory-system.md) - Persistent learning
- [CLI Reference](../guide/cli-reference.md) - Full CLI documentation
