# Gastown & Beads Research: Parallel Agent Coordination

Research into Steve Yegge's open-source projects for multi-agent coding workflows.

## Sources

- [Beads GitHub](https://github.com/steveyegge/beads)
- [Gastown GitHub](https://github.com/steveyegge/gastown)
- [Beads Documentation](https://steveyegge.github.io/beads/)
- [Beads AGENT_INSTRUCTIONS.md](https://github.com/steveyegge/beads/blob/main/AGENT_INSTRUCTIONS.md)
- [Beads Village MCP](https://github.com/LNS2905/mcp-beads-village)
- [Gastown Analysis](https://www.wal.sh/research/gastown.html)
- [Blog: Beads Memory for Coding Agents](https://paddo.dev/blog/beads-memory-for-coding-agents/)

---

## Beads: The Foundation

Beads is a git-backed issue tracker designed for AI coding agents. It solves the "50 First Dates" problem—agents lose memory between sessions.

### Core Concepts

| Concept | Description |
|---------|-------------|
| **Bead** | A work item with hash-based ID (e.g., `bd-a1b2`), priority, dependencies |
| **JSONL Storage** | Issues stored in `.beads/` as JSONL, version-controlled in git |
| **SQLite Cache** | Local cache for fast queries, synced from JSONL source of truth |
| **Dependency Graph** | Tasks can block/be blocked by other tasks |

### Key Commands

```bash
bd ready          # Show tasks with no open blockers (actionable work)
bd list --json    # Programmatic access for agents
bd close <id>     # Mark work complete
bd sync           # Force immediate flush/commit/push
```

### Agent Workflow Loop

```
1. bd ready → get highest-priority unblocked task
2. Work on task
3. bd close <id> when done
4. Repeat until bd ready returns empty
```

---

## "Land the Plane" Pattern

**The critical end-of-session ritual.** This is the most important concept for Ralph.

### Trigger

User says: "Let's land the plane"

### Mandatory Sequence

1. **File remaining work** as new issues for follow-up
2. **Run quality gates** (if code changes):
   - `make lint` / `make test`
   - File P0 issues if gates fail
3. **Update issues** — close finished work, refresh status
4. **PUSH TO REMOTE** (non-negotiable):
   ```bash
   git pull --rebase
   # resolve conflicts if needed
   bd sync
   git push
   git status  # verify "up to date with origin/main"
   ```
5. **Clean git state**: `git stash clear`, `git remote prune origin`
6. **Generate handoff prompt** for next session

### Critical Rules

> "The plane is NOT landed until `git push` succeeds."

- Never stop before `git push` — leaves work stranded locally
- Never say "ready to push when you are!" — the agent must push
- Without `bd sync`, changes sit in 30-second debounce window
- Always verify clean git state at end

### Handoff Prompt Format

```
Continue work on bd-X: [issue title].
[Brief context about what's done and what's next]
```

---

## Gastown: Multi-Agent Orchestration

Gastown builds on Beads to coordinate multiple parallel agents.

### Architecture

```
Town (~/gt/)           → Workspace root
├── Mayor              → AI coordinator (Claude Code instance)
├── Rig (project)      → Container for git project + agents
│   ├── Polecats       → Worker agents
│   ├── Witness        → Monitors workers, handles lifecycle
│   └── Hooks          → Persistent state storage
└── Refinery           → Merge queue processor
```

### Key Components

| Component | Purpose |
|-----------|---------|
| **Mayor** | Primary AI coordinator with full workspace context |
| **Convoy** | Bundle of beads assigned to an agent |
| **Polecat** | Worker agent operating in isolated git worktree |
| **Sling** | Assign work to specific agent |
| **Hooks** | Git-backed persistent state (survives crashes/restarts) |

### Workflow (MEOW Pattern)

1. Brief the Mayor with requirements
2. Mayor analyzes and creates convoy structure
3. Issues distributed to spawned agents
4. Progress monitored via convoy status
5. Results summarized and merged

### Scaling

Without Gastown: chaotic 4-10 agents
With Gastown: comfortable 20-30 agents

---

## Beads Village: File Coordination

Community extension for multi-agent file locking.

### Coordination State Directories

```
.beads/        → Task/issue storage
.mail/         → Agent messaging
.reservations/ → File lock tracking
```

### Agent Lifecycle

```
init() → claim() → reserve() → [work] → done() → RESTART
```

| Step | Purpose |
|------|---------|
| `init()` | Agent joins workspace |
| `claim()` | Get next available task |
| `reserve()` | Lock files before editing |
| `done()` | Complete task, release locks |

### File Locking

Agents call `reserve()` before editing files, preventing merge conflicts.
Files tracked in `.reservations/` directory (git-tracked).

---

## Key Design Principles

### 1. Git Is The Coordination Layer

- No external services required
- Works offline
- Agents already understand git
- State survives crashes via git-backed files

### 2. Explicit Completion, Not Inference

- `bd close <id>` explicitly marks completion
- `bd ready` returns empty when all work done
- No magic completion detection

### 3. Session Boundaries Are Hard

The "land the plane" pattern acknowledges:
- Context is lost between sessions
- Handoff must be explicit
- Git push is the only real checkpoint

### 4. Dependencies Enable Parallelism

- `bd ready` only shows unblocked work
- Multiple agents can grab different tracks
- Hash-based IDs prevent merge collisions

---

## Relevance to Ralph

### What Ralph Already Has

- Git worktrees for parallel loops ✓
- Merge queue for worktree results ✓
- Tasks system (`.ralph/agent/tasks.jsonl`) ✓
- Memories for cross-session context ✓

### Potential Improvements

1. **Explicit "Land the Plane" Command**
   - `ralph land` or special phrase detection
   - Mandatory: sync tasks, push, clean state, generate handoff

2. **Task Dependencies**
   - Add `blockedBy` / `blocks` fields
   - `ralph tasks ready` shows only unblocked work

3. **File Reservations**
   - Track which loop owns which files
   - Prevent merge conflicts before they happen

4. **Handoff Prompt Generation**
   - On loop completion, generate ready-to-paste prompt
   - Include: what's done, what's next, context needed

5. **Convoy-like Bundling**
   - Group related tasks for parallel execution
   - Assign bundles to worktree loops

---

## Summary: What "Land the Plane" Really Means

It's a **forced checkpoint** that guarantees:

1. All work is captured (issues filed for incomplete items)
2. Quality gates passed (or failures tracked)
3. State is pushed to remote (the only durable checkpoint)
4. Git is clean (no dangling state)
5. Next session can start cold with a handoff prompt

The key insight: **agents are unreliable across session boundaries**. The "land the plane" pattern treats this as a first-class concern rather than hoping context survives.

For Ralph, this means loop completion should be a well-defined ritual, not just "LOOP_COMPLETE" detection.
