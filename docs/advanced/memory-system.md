# Memory System

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive memory system documentation.

## Overview

Hats's memory system provides persistent learning across orchestration sessions, stored in `.agent/memories.md`.

## Memory Types

- **Codebase Patterns** - Discovered conventions and patterns
- **Architectural Decisions** - Design choices and rationale
- **Recurring Solutions** - Common problem-solving approaches
- **Project Context** - Domain-specific knowledge

## Configuration

```yaml
memories:
  enabled: true  # Default
  path: .agent/memories.md
```

## See Also

- [Memories & Tasks](../concepts/memories-and-tasks.md) - Core concepts
- [Task System](task-system.md) - Runtime task tracking
- [Configuration](../guide/configuration.md) - Full configuration reference
