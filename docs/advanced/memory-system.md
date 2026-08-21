# Memory System

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive memory system documentation.

## Overview

Ralph's memory system provides persistent learning across orchestration sessions, stored in `.ralph/agent/memories.md`.

## Memory Types

- **Codebase Patterns** - Discovered conventions and patterns
- **Architectural Decisions** - Design choices and rationale
- **Recurring Solutions** - Common problem-solving approaches
- **Project Context** - Domain-specific knowledge

## Configuration

```yaml
memories:
  enabled: true  # Default
  inject: auto
  budget: 2000   # Maximum tokens to inject; 0 means unlimited
```

The storage path is fixed at `.ralph/agent/memories.md`; `memories` has no
configurable `path` key. The other supported key is `filter`, which controls
which memories are injected.

## See Also

- [Memories & Tasks](../concepts/memories-and-tasks.md) - Core concepts
- [Task System](task-system.md) - Runtime task tracking
- [Configuration](../guide/configuration.md) - Full configuration reference
