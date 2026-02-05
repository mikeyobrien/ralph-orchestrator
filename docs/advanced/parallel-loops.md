# Parallel Loops

Hats supports running multiple orchestration loops in parallel using git worktrees for filesystem isolation. This enables working on multiple tasks simultaneously without conflicts.

## How It Works

When you start a Hats loop:

1. **First loop** acquires `.hats/loop.lock` and runs in-place (the primary loop)
2. **Additional loops** automatically spawn into `.worktrees/<loop-id>/`
3. **Each loop** has isolated events, tasks, and scratchpad
4. **Memories are shared** — symlinked back to the main repo's `.agent/memories.md`
5. **On completion**, worktree loops automatically spawn a merge-hats to integrate changes

```
┌─────────────────────────────────────────────────────────────────────┐
│  Terminal 1                    │  Terminal 2                       │
│  hats run -p "Add auth"       │  hats run -p "Add logging"       │
│  [acquires lock, runs in-place]│  [spawns to worktree]             │
│           ↓                    │           ↓                       │
│     Primary loop               │  .worktrees/hats-20250124-a3f2/  │
│           ↓                    │           ↓                       │
│     LOOP_COMPLETE              │     LOOP_COMPLETE → auto-merge    │
└─────────────────────────────────────────────────────────────────────┘
```

## Usage

```bash
# First loop acquires lock, runs in-place
hats run -p "Add authentication"

# In another terminal — automatically spawns to worktree
hats run -p "Add logging"

# Check running loops
hats loops

# View logs from a specific loop
hats loops logs <loop-id>
hats loops logs <loop-id> --follow  # Real-time streaming

# Force sequential execution (wait for lock)
hats run --exclusive -p "Task that needs main workspace"

# Skip auto-merge (keep worktree for manual handling)
hats run --no-auto-merge -p "Experimental feature"
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
├── .hats/
│   ├── loop.lock          # Primary loop indicator
│   ├── loops.json         # Loop registry
│   ├── merge-queue.jsonl  # Merge event log
│   └── events.jsonl       # Primary loop events
├── .agent/
│   └── memories.md        # Shared across all loops
└── .worktrees/
    └── hats-20250124-a3f2/
        ├── .hats/events.jsonl    # Loop-isolated
        ├── .agent/
        │   ├── memories.md → ../../.agent/memories.md  # Symlink
        │   └── scratchpad.md      # Loop-isolated
        └── [project files]
```

## Managing Loops

```bash
# List all loops with status
hats loops list

# View loop output
hats loops logs <id>              # Full output
hats loops logs <id> --follow     # Stream real-time

# View event history
hats loops history <id>           # Formatted table
hats loops history <id> --json    # Raw JSONL

# Show changes from merge-base
hats loops diff <id>              # Full diff
hats loops diff <id> --stat       # Summary only

# Open shell in worktree
hats loops attach <id>

# Re-run merge for failed loop
hats loops retry <id>

# Stop a running loop
hats loops stop <id>              # SIGTERM
hats loops stop <id> --force      # SIGKILL

# Abandon loop and cleanup
hats loops discard <id>           # With confirmation
hats loops discard <id> -y        # Skip confirmation

# Clean up stale loops (crashed processes)
hats loops prune
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
│                                        Spawn merge-hats             │
└──────────────────────────────────────────────────────────────────────┘
```

The merge-hats process uses a **hat collection** with specialized roles:

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
hats loops attach <loop-id>

# Fix the issue, commit
git add . && git commit -m "Manual conflict resolution"

# Retry the merge
hats loops retry <loop-id>

# Or discard if unneeded
hats loops discard <loop-id>
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
hats loops

# If primary finished but merge didn't start, manually trigger
hats loops retry <loop-id>
```

### Merge keeps failing

```bash
# View merge-hats logs
hats loops logs <loop-id>

# Check what changes conflict
hats loops diff <loop-id>

# Manually resolve in worktree
hats loops attach <loop-id>
```

### Orphaned worktrees

```bash
# List and clean up orphans
hats loops prune

# Force cleanup of specific worktree
git worktree remove .worktrees/<loop-id> --force
git branch -D hats/<loop-id>
```

### Lock file issues

```bash
# Check who holds the lock
cat .hats/loop.lock

# If process is dead, remove stale lock
rm .hats/loop.lock
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `HATS_MERGE_LOOP_ID` | Set by auto-merge to identify which loop to merge |
| `HATS_DIAGNOSTICS=1` | Enable detailed diagnostic logging |
| `HATS_VERBOSE=1` | Verbose output mode |
