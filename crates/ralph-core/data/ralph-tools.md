---
name: ralph-tools
description: Shared tool commands for interact, skill, and output format reference during Ralph orchestration
metadata:
  internal: true
---

# Ralph Tools

Quick reference for shared `ralph tools` commands used during orchestration.

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

## Wave Commands

Dispatch multiple events as a wave for parallel hat execution:

```bash
ralph wave emit <topic> --payloads "item1" "item2" "item3"
```

Each payload becomes a separate event tagged with a shared `wave_id`. The loop runner
detects wave events and spawns parallel backend instances (up to the hat's `concurrency` limit).

**When to use:** When a hat's configuration has `concurrency > 1`, use wave dispatch
to send N items for parallel processing instead of N sequential iterations.

**Constraints:**
- Cannot be used inside a wave worker (`RALPH_WAVE_WORKER=1` blocks it)
- Wave workers emit results via `ralph emit`, not `ralph wave emit`

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

Write tasks and memories with the CLI tools rather than echo/cat, which bypass the stores' formatting.

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

**Leave out of memories:**
- Session-specific state (use tasks instead)
- Obvious/universal practices
- Temporary workarounds and one-off failures

Every memory is injected into every later iteration, and later iterations follow
memories closely. Write each one with its scope and reason ("When X, do Y,
because Z"), not as an absolute rule ("never retry"). If a memory turns out to be
wrong, stale, or narrower than it claims, fix or delete it with
`ralph tools memory delete <mem-id>`.

### Failure Capture (Generic Rule)

When a command fails, a dependency is missing, or you are blocked:
1. **Solved it, and it could recur?** Record a fix memory that says how to recognize the problem and what fixed it.
2. **Not solved this iteration?** Open or reopen a task so the next iteration picks it up. A failure you have not solved is work, not a lesson, so it goes in a task rather than a memory.

```bash
ralph tools memory add \
  "When <command> fails with <error>, <fix>, because <cause>" \
  -t fix --tags tooling,error-handling

ralph tools task ensure "Fix: <short description>" --key fix:<short-key> -p 2
```

### Discover Available Tags

Before searching or adding, check what tags already exist:

```bash
ralph tools memory list
grep -o 'tags: [^|]*' .ralph/agent/memories.md | sort -u
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

Make reversible decisions yourself, and record consequential ones in
`.ralph/agent/decisions.md` so a later iteration or a human can see why. Follow
the template at the top of the file and keep IDs sequential (DEC-001, DEC-002, ...).
When you are unsure, prefer the reversible, narrower option that follows existing
patterns, and say so in the entry.

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

Write tasks and memories with the CLI tools rather than echo/cat, which bypass the stores' formatting.
