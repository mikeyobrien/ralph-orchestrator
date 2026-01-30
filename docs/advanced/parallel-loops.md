# Parallel Loops

Ralph supports running multiple orchestration loops in parallel using git worktrees for filesystem isolation. This enables working on multiple tasks simultaneously without conflicts.

## How It Works

When you start a Ralph loop:

1. **First loop** acquires `.ralph/loop.lock` and runs in-place (the primary loop)
2. **Additional loops** automatically spawn into `.worktrees/<loop-id>/`
3. **Each loop** has isolated events, tasks, and scratchpad
4. **Memories are shared** — symlinked back to the main repo's `.agent/memories.md`
5. **On completion**, worktree loops automatically spawn a merge-ralph to integrate changes

```
┌─────────────────────────────────────────────────────────────────────┐
│  Terminal 1                    │  Terminal 2                       │
│  ralph run -p "Add auth"       │  ralph run -p "Add logging"       │
│  [acquires lock, runs in-place]│  [spawns to worktree]             │
│           ↓                    │           ↓                       │
│     Primary loop               │  .worktrees/ralph-20250124-a3f2/  │
│           ↓                    │           ↓                       │
│     LOOP_COMPLETE              │     LOOP_COMPLETE → auto-merge    │
└─────────────────────────────────────────────────────────────────────┘
```

## Usage

```bash
# First loop acquires lock, runs in-place
ralph run -p "Add authentication"

# In another terminal — automatically spawns to worktree
ralph run -p "Add logging"

# Check running loops
ralph loops

# View logs from a specific loop
ralph loops logs <loop-id>
ralph loops logs <loop-id> --follow  # Real-time streaming

# Force sequential execution (wait for lock)
ralph run --exclusive -p "Task that needs main workspace"

# Skip auto-merge (keep worktree for manual handling)
ralph run --no-auto-merge -p "Experimental feature"
```

## Loop States

| State | Description |
|-------|-------------|
| `running` | Loop is actively executing |
| `queued` | Completed, waiting for merge |
| `merging` | Merge operation in progress |
| `merged` | Successfully merged to main |
| `needs-review` | Merge failed, requires manual resolution |
| `crashed` | Process died unexpectedly |
| `orphan` | Worktree exists but not tracked |
| `discarded` | Explicitly abandoned by user |

## File Structure

```
project/
├── .ralph/
│   ├── loop.lock          # Primary loop indicator
│   ├── loops.json         # Loop registry
│   ├── merge-queue.jsonl  # Merge event log
│   └── events.jsonl       # Primary loop events
├── .agent/
│   └── memories.md        # Shared across all loops
└── .worktrees/
    └── ralph-20250124-a3f2/
        ├── .ralph/events.jsonl    # Loop-isolated
        ├── .agent/
        │   ├── memories.md → ../../.agent/memories.md  # Symlink
        │   └── scratchpad.md      # Loop-isolated
        └── [project files]
```

## Managing Loops

```bash
# List all loops with status
ralph loops list

# View loop output
ralph loops logs <id>              # Full output
ralph loops logs <id> --follow     # Stream real-time

# View event history
ralph loops history <id>           # Formatted table
ralph loops history <id> --json    # Raw JSONL

# Show changes from merge-base
ralph loops diff <id>              # Full diff
ralph loops diff <id> --stat       # Summary only

# Open shell in worktree
ralph loops attach <id>

# Re-run merge for failed loop
ralph loops retry <id>

# Stop a running loop
ralph loops stop <id>              # SIGTERM
ralph loops stop <id> --force      # SIGKILL

# Abandon loop and cleanup
ralph loops discard <id>           # With confirmation
ralph loops discard <id> -y        # Skip confirmation

# Clean up stale loops (crashed processes)
ralph loops prune
```

## Auto-Merge Workflow

When a worktree loop completes, it queues itself for merge. The primary loop processes this queue when it finishes:

```
┌──────────────────────────────────────────────────────────────────────┐
│  Worktree Loop                         Primary Loop                  │
│  ─────────────                         ─────────────                 │
│  LOOP_COMPLETE                                                       │
│       ↓                                                              │
│  Queue for merge ─────────────────────→ [continues working]         │
│       ↓                                       ↓                      │
│  Exit cleanly                          LOOP_COMPLETE                 │
│                                              ↓                       │
│                                        Process merge queue           │
│                                              ↓                       │
│                                        Spawn merge-ralph             │
└──────────────────────────────────────────────────────────────────────┘
```

The merge-ralph process uses a **hat collection** with specialized roles:

| Hat | Trigger | Purpose |
|-----|---------|---------|
| `merger` | `merge.start` | Performs `git merge`, runs tests |
| `resolver` | `conflict.detected` | Resolves merge conflicts by understanding intent |
| `tester` | `conflict.resolved` | Verifies tests pass after conflict resolution |
| `cleaner` | `merge.done` | Removes worktree and branch |
| `failure_handler` | `*failed`, `unresolvable` | Marks loop for manual review |

The workflow handles conflicts intelligently:
1. **No conflicts**: Merge → Run tests → Clean up → Done
2. **With conflicts**: Detect → AI resolves → Run tests → Clean up → Done
3. **Unresolvable**: Abort → Mark for review → Keep worktree for manual fix

## Conflict Resolution

When merge conflicts occur, the AI resolver:

1. Examines conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`)
2. Understands the **intent** of both sides (not just the code)
3. Resolves by preserving both intents when possible
4. Prefers the loop's changes when directly contradictory (newer work)

**Conflicts marked `needs-review`:**
- Major architectural changes on both sides
- Complex refactoring that can't be automatically reconciled
- Business logic contradictions requiring human judgment

To manually resolve:
```bash
# Enter the worktree
ralph loops attach <loop-id>

# Fix the issue, commit
git add . && git commit -m "Manual conflict resolution"

# Retry the merge
ralph loops retry <loop-id>

# Or discard if unneeded
ralph loops discard <loop-id>
```

## Best Practices

**When to use parallel loops:**
- Independent features with minimal file overlap
- Bug fixes while feature work continues
- Documentation updates parallel to code changes
- Test additions that don't conflict with active development

**When to use `--exclusive` (sequential):**
- Large refactoring touching many files
- Database migrations or schema changes
- Tasks that modify shared configuration files
- Work that depends on changes from another in-progress loop

**Tips for reducing conflicts:**
- Keep loops focused on distinct areas of the codebase
- Use separate files when adding new features
- Avoid modifying the same functions in parallel loops
- Let one loop complete before starting conflicting work

## Troubleshooting

### Loop stuck in `queued` state

```bash
# Check if primary loop is still running
ralph loops

# If primary finished but merge didn't start, manually trigger
ralph loops retry <loop-id>
```

### Merge keeps failing

```bash
# View merge-ralph logs
ralph loops logs <loop-id>

# Check what changes conflict
ralph loops diff <loop-id>

# Manually resolve in worktree
ralph loops attach <loop-id>
```

### Orphaned worktrees

```bash
# List and clean up orphans
ralph loops prune

# Force cleanup of specific worktree
git worktree remove .worktrees/<loop-id> --force
git branch -D ralph/<loop-id>
```

### Lock file issues

```bash
# Check who holds the lock
cat .ralph/loop.lock

# If process is dead, remove stale lock
rm .ralph/loop.lock
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RALPH_MERGE_LOOP_ID` | Set by auto-merge to identify which loop to merge |
| `RALPH_DIAGNOSTICS=1` | Enable detailed diagnostic logging |
| `RALPH_VERBOSE=1` | Verbose output mode |
