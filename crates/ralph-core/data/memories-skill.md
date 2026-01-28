---
name: ralph-memories
description: Use when discovering codebase patterns, making architectural decisions, solving recurring problems, or learning project-specific context that should persist across sessions
---

# Ralph Memories

Persistent learning system for accumulated wisdom across sessions. Storage: `.agent/memories.md`.

## When to Search Memories

**Search BEFORE starting work when:**
- Entering unfamiliar code area → `ralph tools memory search "area-name"`
- Encountering an error → `ralph tools memory search -t fix "error message"`
- Making architectural decisions → `ralph tools memory search -t decision "topic"`
- Something feels familiar → there might be a memory about it

**Search strategies:**
- Start broad, narrow with filters: `search "api"` → `search -t pattern --tags api`
- Check fixes first for errors: `search -t fix "ECONNREFUSED"`
- Review decisions before changing architecture: `search -t decision`

## When to Create Memories

**Create a memory when:**
- You discover how this codebase does things (pattern)
- You make or learn why an architectural choice was made (decision)
- You solve a problem that might recur (fix)
- You learn project-specific knowledge others need (context)

**Do NOT create memories for:**
- Session-specific state (use tasks instead)
- Obvious/universal practices
- Temporary workarounds

## Memory Types

| Type | Flag | Use For |
|------|------|---------|
| pattern | `-t pattern` | "Uses barrel exports", "API routes use kebab-case" |
| decision | `-t decision` | "Chose Postgres over SQLite for concurrent writes" |
| fix | `-t fix` | "ECONNREFUSED on :5432 means run docker-compose up" |
| context | `-t context` | "ralph-core is shared lib, ralph-cli is binary" |

## Discover Available Tags

Before searching or adding, check what tags already exist:

```bash
# See all memories with their tags
ralph tools memory list

# Extract unique tags (grep the file directly)
grep -o 'tags: [^|]*' .agent/memories.md | sort -u
```

Reuse existing tags for consistency. Common tag patterns:
- Component names: `api`, `auth`, `database`, `cli`
- Concerns: `testing`, `performance`, `error-handling`
- Tools: `docker`, `postgres`, `redis`

## Quick Reference

```bash
# Add memory (creates file if needed)
ralph tools memory add "content" -t pattern --tags tag1,tag2

# Search (start broad, narrow with filters)
ralph tools memory search "query"
ralph tools memory search -t fix "error message"
ralph tools memory search --tags api,auth

# List and show
ralph tools memory list
ralph tools memory list -t fix --last 10
ralph tools memory show mem-1737372000-a1b2

# Delete
ralph tools memory delete mem-1737372000-a1b2

# Prime for context injection
ralph tools memory prime --budget 2000
ralph tools memory prime --tags api,auth    # Prime specific tags only
ralph tools memory prime --recent 7         # Only last 7 days
```

## Best Practices

1. **Be specific**: "Uses barrel exports in each module" not "Has good patterns"
2. **Include why**: "Chose X because Y" not just "Uses X"
3. **One concept per memory**: Split complex learnings
4. **Tag consistently**: Reuse existing tags when possible

## Examples

```bash
# Pattern: discovered codebase convention
ralph tools memory add "All API handlers return Result<Json<T>, AppError>" -t pattern --tags api,error-handling

# Decision: learned why something was chosen
ralph tools memory add "Chose JSONL over SQLite: simpler, git-friendly, append-only" -t decision --tags storage,architecture

# Fix: solved a recurring problem
ralph tools memory add "cargo test hangs: kill orphan postgres from previous run" -t fix --tags testing,postgres

# Context: project-specific knowledge
ralph tools memory add "The /legacy folder is deprecated, use /v2 endpoints" -t context --tags api,migration
```
