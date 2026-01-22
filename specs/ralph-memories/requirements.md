# Ralph Memories — Requirements

## Overview

A persistent learning system that allows Ralph to accumulate wisdom across sessions. Unlike the scratchpad (working memory cleared each iteration), memories are long-term knowledge that persists via git and can be authored by both humans and agents.

## Problem Statement

Ralph's scratchpad is ephemeral within a session. When Ralph solves a problem or learns a codebase pattern, that knowledge is lost when the session ends. There's no mechanism for **accumulated wisdom** — learnings that compound across many sessions.

## Core Requirements

### R1: Memory Storage

| Requirement | Decision |
|-------------|----------|
| **Location** | `.agent/memories.md` (per-project) |
| **Format** | Structured Markdown (human-readable, directly injectable) |
| **Scope** | Per-project only |
| **Git Integration** | Memories are git-tracked, enabling team collaboration |

**Rationale**: Markdown enables human authoring and editing while remaining parseable. Direct injection into context requires no transformation.

### R2: Memory Structure

```markdown
## Patterns

### mem-1705751400-a3f8
> This codebase uses dependency injection for all services.
> Each service receives its dependencies via constructor.
<!-- tags: architecture, di | created: 2025-01-20 -->
```

| Element | Required | Description |
|---------|----------|-------------|
| Section header (`## Type`) | ✅ | Groups memories by type |
| Memory ID (`### mem-...`) | ✅ | Unique identifier: `mem-{unix_timestamp}-{4_hex_chars}` |
| Content (`> ...`) | ✅ | Blockquote format, supports multi-line |
| Metadata (`<!-- ... -->`) | ❌ | HTML comment with tags and created date |

### R3: Memory Types

| Type | Section Header | Purpose | Example |
|------|----------------|---------|---------|
| `pattern` | `## Patterns` | How this codebase does things | "Uses barrel exports in each module" |
| `decision` | `## Decisions` | Why something was chosen | "Chose Zod over Yup for runtime perf" |
| `fix` | `## Fixes` | Solution to recurring problem | "ECONNREFUSED = run `docker-compose up`" |
| `context` | `## Context` | Project-specific knowledge | "Main API is in /services/core" |

### R4: CLI Commands

All commands under `ralph memory` namespace:

| Command | Description | Example |
|---------|-------------|---------|
| `ralph memory add "<content>"` | Create a new memory | `ralph memory add "Use barrel exports"` |
| `ralph memory search [query]` | Search memories by content/tags | `ralph memory search "authentication"` |
| `ralph memory list` | List all memories | `ralph memory list --last 10` |
| `ralph memory show <id>` | Show single memory | `ralph memory show mem-1705751400-a3f8` |
| `ralph memory delete <id>` | Delete a memory by ID | `ralph memory delete mem-1705751400-a3f8` |
| `ralph memory prime` | Output memories for context injection | `ralph memory prime --budget 2000` |
| `ralph memory init` | Initialize memories file | `ralph memory init` |

#### R4.1: `ralph memory add`

```
ralph memory add "<content>" [OPTIONS]

OPTIONS:
    -t, --type <TYPE>      Memory type [default: pattern]
                           [values: pattern, decision, fix, context]
    --tags <TAGS>          Comma-separated tags
    --format <FORMAT>      Output format [default: table]
                           [values: table, json, quiet]
```

**Behavior**:
1. Generate unique ID using timestamp + random hex
2. Find appropriate section in `.agent/memories.md`
3. Append memory block to section
4. Print success message with memory ID

#### R4.2: `ralph memory search`

```
ralph memory search [QUERY] [OPTIONS]

ARGS:
    [QUERY]    Optional search query (fuzzy match on content/tags)

OPTIONS:
    -t, --type <TYPE>      Filter by memory type
    --tags <TAGS>          Filter by tags (comma-separated)
    --all                  Show all memories (no limit)
    --format <FORMAT>      Output format [default: table]
                           [values: table, json, markdown]
```

**Behavior**:
1. Parse memories from `.agent/memories.md`
2. If query provided: filter by fuzzy match on content and tags
3. Apply type/tag filters if specified
4. Return top 5 matches by default (or all with `--all`)
5. Display in requested format

#### R4.3: `ralph memory delete`

```
ralph memory delete <ID>

ARGS:
    <ID>    Memory ID to delete (e.g., mem-1705751400-a3f8)
```

**Behavior**:
1. Parse memories from `.agent/memories.md`
2. Find memory with matching ID
3. If not found: error with "Memory not found", exit code 1
4. Remove memory block from file
5. Print success message

#### R4.4: `ralph memory list`

```
ralph memory list [OPTIONS]

OPTIONS:
    --last <N>             Show only last N memories
    -t, --type <TYPE>      Filter by memory type
    --format <FORMAT>      Output format [default: table]
                           [values: table, json, markdown]
```

#### R4.5: `ralph memory prime`

```
ralph memory prime [OPTIONS]

OPTIONS:
    --budget <TOKENS>      Maximum tokens to include
    -t, --type <TYPES>     Filter by types (comma-separated)
    --tags <TAGS>          Filter by tags (comma-separated)
    --recent <DAYS>        Only memories from last N days
    --format <FORMAT>      Output format [default: markdown]
                           [values: markdown, json]
```

**Behavior**:
1. Parse memories from `.agent/memories.md`
2. Apply filters (type, tags, recent)
3. If budget specified, truncate to fit token limit
4. Output as raw markdown (suitable for direct context injection)

### R5: Context Injection Strategy

| Requirement | Decision |
|-------------|----------|
| **Injection Control** | Orchestrator-controlled via config |
| **Injection Modes** | `auto` (inject at iteration start), `manual` (agent runs search), `none` |
| **Budget** | Configurable token limit for auto-injection |
| **Skill Injection** | Auto-inject "how to use memories" skill when enabled |

**Configuration**:
```yaml
# ralph.yml
memories:
  enabled: true
  inject: auto           # auto | manual | none
  budget: 2000           # max tokens (0 = unlimited)
  skill_injection: true  # inject usage skill
  filter:
    types: []            # empty = all
    tags: []             # empty = all
    recent: 0            # 0 = no time limit
```

**Rationale**: Ralph owns context construction, so injection is a first-class feature. The `auto` mode respects "Fresh Context Is Reliability" by injecting a curated, budget-limited subset rather than everything.

### R6: Skill Auto-Injection

When `memories.skill_injection: true`, Ralph injects a skill that teaches agents:
1. How memories are structured
2. When to create new memories
3. How to search for specific memories
4. Best practices for memory content

## Output Formats

| Format | Use Case |
|--------|----------|
| `table` | Human-readable (default for interactive) |
| `json` | Programmatic consumption |
| `markdown` | Direct context injection (prime command) |
| `quiet` | ID-only output for scripting |

## Success Criteria

1. **CRUD Operations Work**: add, search, list, show, delete commands function correctly
2. **Human Authoring**: Memories can be edited directly in markdown
3. **Search Returns Relevant Results**: fuzzy matching finds memories by content/tag substring
4. **Git Integration**: memories.md can be committed and shared across team
5. **Orchestrator Injection**: auto mode injects memories at iteration start
6. **Skill Teaching**: agents learn how to use memory system via injected skill
7. **Budget Respected**: auto-injection stays within configured token limit
