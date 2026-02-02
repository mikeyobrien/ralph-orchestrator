---
name: ralph-tools
description: Use when managing runtime tasks or memories during Ralph orchestration runs
---

# Ralph Tools

Quick reference for `ralph tools task` and `ralph tools memory` commands used during orchestration.

## Two Task Systems

| System | Command | Purpose | Storage |
|--------|---------|---------|---------|
| **Runtime tasks** | `ralph tools task` | Track work items during runs | `.agent/tasks.jsonl` |
| **Code tasks** | `ralph task` | Implementation planning | `tasks/*.code-task.md` |

This skill covers **runtime tasks**. For code tasks, see `/code-task-generator`.

## Task Commands

```bash
ralph tools task add "Title" -p 2 -d "description" --blocked-by id1,id2
ralph tools task list [--status open|in_progress|closed] [--format table|json|quiet]
ralph tools task ready                    # Show unblocked tasks
ralph tools task close <task-id>
ralph tools task show <task-id>
```

**Task ID format:** `task-{timestamp}-{4hex}` (e.g., `task-1737372000-a1b2`)

**Priority:** 1-5 (1 = highest, default 3)

### Task Rules
- One task = one testable unit of work (completable in 1-2 iterations)
- Break large features into smaller tasks BEFORE starting implementation
- On your first iteration, check `ralph tools task ready` — prior iterations may have created tasks
- ONLY close tasks after verification (tests pass, build succeeds)

### First thing every iteration
```bash
ralph tools task ready    # What's open? Pick one. Don't create duplicates.
```

## Interact Commands

```bash
ralph tools interact progress "message"
```

Send a non-blocking progress update via the configured RObot (Telegram).

## Skill Commands

```bash
ralph tools skill list
ralph tools skill load <name>
```

List available skills or load a specific skill by name.

## Memory Commands

```bash
ralph tools memory add "content" -t pattern --tags tag1,tag2
ralph tools memory list [-t type] [--tags tags]
ralph tools memory search "query" [-t type] [--tags tags]
ralph tools memory prime --budget 2000    # Output for context injection
ralph tools memory show <mem-id>
ralph tools memory delete <mem-id>
```

**Memory types:**

| Type | Flag | Use For |
|------|------|---------|
| pattern | `-t pattern` | "Uses barrel exports", "API routes use kebab-case" |
| decision | `-t decision` | "Chose Postgres over SQLite for concurrent writes" |
| fix | `-t fix` | "ECONNREFUSED on :5432 means run docker-compose up" |
| context | `-t context` | "ralph-core is shared lib, ralph-cli is binary" |

**Memory ID format:** `mem-{timestamp}-{4hex}` (e.g., `mem-1737372000-a1b2`)

**NEVER use echo/cat to write tasks or memories** — always use CLI tools.

### When to Search Memories

**Search BEFORE starting work when:**
- Entering unfamiliar code area → `ralph tools memory search "area-name"`
- Encountering an error → `ralph tools memory search -t fix "error message"`
- Making architectural decisions → `ralph tools memory search -t decision "topic"`
- Something feels familiar → there might be a memory about it

**Search strategies:**
- Start broad, narrow with filters: `search "api"` → `search -t pattern --tags api`
- Check fixes first for errors: `search -t fix "ECONNREFUSED"`
- Review decisions before changing architecture: `search -t decision`

### When to Create Memories

**Create a memory when:**
- You discover how this codebase does things (pattern)
- You make or learn why an architectural choice was made (decision)
- You solve a problem that might recur (fix)
- You learn project-specific knowledge others need (context)
- Any non-zero command, missing dependency/skill, or blocked step (fix + task if unresolved)

**Do NOT create memories for:**
- Session-specific state (use tasks instead)
- Obvious/universal practices
- Temporary workarounds

### Failure Capture (Generic Rule)

If any command fails (non-zero exit), or you hit a missing dependency/skill, or you are blocked:
1. **Record a fix memory** with the exact command, error, and intended fix.
2. **Open a task** if it won't be resolved in the same iteration.

```bash
ralph tools memory add \
  "failure: cmd=<command>, exit=<code>, error=<message>, next=<intended fix>" \
  -t fix --tags tooling,error-handling

ralph tools task add "Fix: <short description>" -p 2
```

### Discover Available Tags

Before searching or adding, check what tags already exist:

```bash
ralph tools memory list
grep -o 'tags: [^|]*' .agent/memories.md | sort -u
```

Reuse existing tags for consistency. Common tag patterns:
- Component names: `api`, `auth`, `database`, `cli`
- Concerns: `testing`, `performance`, `error-handling`
- Tools: `docker`, `postgres`, `redis`

### Memory Best Practices

1. **Be specific**: "Uses barrel exports in each module" not "Has good patterns"
2. **Include why**: "Chose X because Y" not just "Uses X"
3. **One concept per memory**: Split complex learnings
4. **Tag consistently**: Reuse existing tags when possible

## Decision Journal

Use `.ralph/agent/decisions.md` to capture consequential decisions and their
confidence scores. Follow the template at the top of the file and keep IDs
sequential (DEC-001, DEC-002, ...).

Confidence thresholds:
- **>80**: Proceed autonomously.
- **50-80**: Proceed, but document the decision in `.ralph/agent/decisions.md`.
- **<50**: Choose the safest default and document the decision in `.ralph/agent/decisions.md`.

Template fields:
- Decision
- Chosen Option
- Confidence (0-100)
- Alternatives Considered
- Reasoning
- Reversibility
- Timestamp (UTC ISO 8601)

## Output Formats

All commands support `--format`:
- `table` (default) - Human-readable
- `json` - Machine-parseable
- `quiet` - IDs only (for scripting)
- `markdown` - Memory prime only

## Common Workflows

### Track dependent work
```bash
ralph tools task add "Setup auth" -p 1
# Returns: task-1737372000-a1b2

ralph tools task add "Add user routes" --blocked-by task-1737372000-a1b2
ralph tools task ready  # Only shows unblocked tasks
```

### Store a discovery
```bash
ralph tools memory add "Parser requires snake_case keys" -t pattern --tags config,yaml
```

### Find relevant memories
```bash
ralph tools memory search "config" --tags yaml
ralph tools memory prime --budget 1000 -t pattern  # For injection
```

### Memory examples
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
