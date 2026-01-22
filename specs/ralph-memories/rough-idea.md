# Ralph Memories — Rough Idea

## Origin

Based on analysis of the [beads](https://github.com/steveyegge/beads) project, a context management system for AI agents.

## Problem Statement

Ralph's scratchpad is ephemeral within a session and persists via git, but there's no mechanism for **accumulated wisdom** — learnings that compound across many sessions. When Ralph solves a problem or learns a codebase pattern, that knowledge is lost when the session ends.

## Core Concept

A minimal persistent learning system that allows Ralph to:
1. **Remember** explicit learnings from sessions
2. **Recall** relevant memories when needed
3. **Forget** outdated or incorrect memories

## Design Principles

1. **YAGNI over feature-parity** — Take only what Ralph needs from Beads
2. **Scratchpad is working memory; memories are long-term** — Don't conflate them
3. **Human + Agent authoring** — Both can create and edit memories
4. **Orchestrator-controlled injection** — Ralph owns context, controls what gets injected

---

## Open Questions — RESOLVED

### Q1: Where should memories be stored?

**Decision:** `.agent/memories.md` (per-project)

**Rationale:** Aligns with existing `.agent/` pattern (scratchpad, events). Single file at root is simpler than nested directory.

### Q2: What format should be used?

**Decision:** Structured Markdown

**Rationale:**
- Human-readable and editable
- Can be injected into context without transformation
- Git diffs are meaningful
- Parseable with simple regex

### Q3: Should memories auto-inject into context?

**Decision:** Orchestrator-controlled via config (`inject: auto | manual | none`)

**Rationale:** Ralph owns context construction, so injection is a first-class feature. Budget limits prevent context bloat. Respects "Fresh Context Is Reliability" by injecting curated subset.

### Q4: What triggers memory creation?

**Decision:** Explicit command only (`ralph memory add`)

**Rationale:** Keeps control with user/agent. No magic. Session-end prompts deferred to future consideration.

### Q5: How do agents learn to use the memory system?

**Decision:** Skill auto-injection when memories enabled

**Rationale:** Agent needs to know how to use the system. Injecting a "how to use memories" skill teaches proper usage without manual documentation.

### Q6: CLI structure?

**Decision:** Namespaced under `ralph memory` with subcommands

**Rationale:**
- Groups related commands logically
- Matches CLI patterns in other tools (git, docker)
- Leaves room for future memory-related commands
- More discoverable than flat commands

---

## Final Design Summary

| Aspect | Decision |
|--------|----------|
| Storage | `.agent/memories.md` |
| Format | Structured Markdown |
| CLI | `ralph memory {add,search,list,show,delete,prime,init}` |
| Injection | Orchestrator-controlled, budget-aware |
| Agent guidance | Auto-injected skill |
| Authoring | Human + Agent |

See `design.md` for full specification.
