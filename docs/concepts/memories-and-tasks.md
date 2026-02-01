# Memories & Tasks

Ralph uses two complementary systems for persistent state: memories for cross-session learning, and tasks for runtime work tracking.

## Overview

| System | Storage | Purpose |
|--------|---------|---------|
| **Memories** | `.ralph/agent/memories.md` | Accumulated wisdom across sessions |
| **Tasks** | `.ralph/agent/tasks.jsonl` | Runtime work items |

Both are enabled by default and work together to replace the legacy scratchpad.

## Memories

Memories persist learning across sessions. They capture patterns, decisions, fixes, and context that Ralph should remember.

### Memory Types

| Type | Use For |
|------|---------|
| `pattern` | Codebase conventions discovered |
| `decision` | Architectural choices and rationale |
| `fix` | Solutions to recurring problems |
| `context` | Project-specific knowledge |

### Creating Memories

```bash
# Pattern: discovered convention
ralph tools memory add "All API handlers return Result<Json<T>, AppError>" \
  -t pattern --tags api,error-handling

# Decision: architectural choice
ralph tools memory add "Chose JSONL over SQLite: simpler, git-friendly" \
  -t decision --tags storage,architecture

# Fix: recurring problem solution
ralph tools memory add "cargo test hangs: kill orphan postgres" \
  -t fix --tags testing,postgres

# Context: project knowledge
ralph tools memory add "The /legacy folder is deprecated, use /v2" \
  -t context --tags api,migration
```

### Searching Memories

```bash
# Broad search
ralph tools memory search "api"

# Filter by type
ralph tools memory search -t fix "error"

# Filter by tags
ralph tools memory search --tags api,auth

# List all memories
ralph tools memory list

# List recent fixes
ralph tools memory list -t fix --last 10
```

### Memory Injection

Memories are automatically injected at the start of each iteration:

```yaml
memories:
  enabled: true
  inject: auto      # auto, manual, or none
  budget: 2000      # Max tokens to inject
  filter:
    types: []       # Filter by type (empty = all)
    tags: []        # Filter by tags (empty = all)
    recent: 0       # Days limit (0 = no limit)
```

### Memory Best Practices

1. **Be specific** — "Uses barrel exports" not "Has good patterns"
2. **Include why** — "Chose X because Y" not just "Uses X"
3. **One concept per memory** — Split complex learnings
4. **Tag consistently** — Reuse existing tags

## Tasks

Tasks track runtime work items during orchestration.

### Creating Tasks

```bash
# Basic task
ralph tools task add "Implement user authentication"

# With priority (1-5, 1 = highest)
ralph tools task add "Fix critical bug" -p 1

# With dependency
ralph tools task add "Deploy to production" --blocked-by setup-infra
```

### Managing Tasks

```bash
# List all tasks
ralph tools task list

# List unblocked tasks only
ralph tools task ready

# Close a completed task
ralph tools task close task-123
```

### Task Workflow

1. Ralph creates tasks from the prompt/plan
2. Tasks are worked in priority order
3. Dependencies are respected (blocked tasks wait)
4. Completed tasks are closed
5. Loop ends when no tasks remain

### Task Closure Rules

Tasks must only be closed when:

1. Implementation is actually complete
2. Tests pass
3. Build succeeds (if applicable)
4. Evidence of completion exists

```bash
# Good: Close with evidence
cargo test  # passes
ralph tools task close task-123

# Bad: Close without verification
ralph tools task close task-123  # No tests run!
```

## Memories vs Tasks

| Aspect | Memories | Tasks |
|--------|----------|-------|
| **Persistence** | Cross-session | Single session |
| **Purpose** | Learning | Work tracking |
| **When created** | When something is learned | When work is identified |
| **When removed** | Rarely | When completed |

## Legacy Scratchpad Mode

To disable memories and tasks (legacy mode):

```yaml
memories:
  enabled: false
tasks:
  enabled: false
```

In this mode, `.agent/scratchpad.md` is used for all state.

## File Formats

### memories.md

```markdown
# Memories

## Patterns

### mem-1737372000-a1b2
> All API handlers return Result<Json<T>, AppError>
<!-- tags: api, error-handling | created: 2024-01-20 -->

## Decisions

### mem-1737372100-c3d4
> Chose JSONL over SQLite for simplicity
<!-- tags: storage | created: 2024-01-20 -->
```

### tasks.jsonl

```json
{"id":"task-001","title":"Implement auth","priority":2,"status":"open","created":"2024-01-20T10:00:00Z"}
{"id":"task-002","title":"Add tests","priority":3,"status":"open","blocked_by":["task-001"],"created":"2024-01-20T10:01:00Z"}
```

## Integration with Hats

Hats can use memories and tasks:

```yaml
hats:
  builder:
    triggers: ["task.start"]
    instructions: |
      1. Check memories for relevant patterns
      2. Pick a task from `ralph tools task ready`
      3. Implement the task
      4. Record learnings as memories
      5. Close the task with `ralph tools task close <id>`
```

## Next Steps

- Learn about [Backpressure](backpressure.md) for quality gates
- See [Configuration](../guide/configuration.md) for full options
- Explore the [Memory System](../advanced/memory-system.md) in depth
