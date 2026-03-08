# Worktree System Research

## Overview

Existing parallel loop infrastructure uses git worktrees for full filesystem isolation. Designed for independent loops, not intra-loop parallelism.

## Creation & Removal

**File:** `crates/ralph-core/src/worktree.rs`

- `create_worktree(repo_root, loop_id, config)` — creates `.worktrees/{loop_id}/`, runs `git worktree add -b ralph/{loop_id}`
- `remove_worktree(repo_root, path)` — `git worktree remove --force`, deletes branch, prunes refs
- `sync_working_directory_to_worktree()` — copies untracked + unstaged modified files, preserves symlinks

## Isolation Strategy

**Isolated per worktree:**
- `.ralph/events.jsonl` — local event log
- `.ralph/agent/tasks.jsonl` — local task tracking
- `.ralph/agent/scratchpad.md` — local scratchpad
- `.ralph/diagnostics/` — local diagnostics

**Shared via symlinks** (`crates/ralph-core/src/loop_context.rs:558-575`):
- `.ralph/agent/memories.md` → main repo
- `.ralph/specs/` → main repo
- `.ralph/tasks/` → main repo

**Shared files (single copy):**
- `.ralph/loop.lock` — primary loop coordination
- `.ralph/loops.json` — loop registry
- `.ralph/merge-queue.jsonl` — merge queue

## Coordination Primitives

### Loop Lock (`crates/ralph-core/src/loop_lock.rs`)
- `flock()` on `.ralph/loop.lock`
- Non-blocking acquisition
- Metadata: PID, start time, prompt

### Loop Registry (`crates/ralph-core/src/loop_registry.rs`)
- `.ralph/loops.json`
- Tracks: ID, PID, start time, prompt, worktree_path
- File locking via `flock()` for concurrent access
- Stale/zombie detection (PID alive but worktree gone)

### Merge Queue (`crates/ralph-core/src/merge_queue.rs`)
- Event-sourced: Queued → Merging → Merged | NeedsReview | Discarded
- Append-only JSONL with `flock()` for exclusive writes
- FIFO ordering for pending merges

## Reusable Infrastructure for Wave Isolation

| Component | Reusability |
|-----------|-------------|
| `create_worktree()` | Wrap to customize paths (e.g., `.worktrees/wave-{id}/`) |
| `remove_worktree()` | Works as-is |
| `sync_working_directory_to_worktree()` | Direct reuse |
| `setup_worktree_symlinks()` | Reusable, may need wave-specific additions |
| `flock()` pattern | Standard across all coordination |

## Adaptation Needs

- Current system assumes **one primary loop per repo** (loop.lock)
- Registry keyed by PID — waves need per-wave-instance keys
- Branch naming `ralph/{loop_id}` — may need `ralph/wave/{wave-id}/{index}`
- Worktree creation overhead is real — should only be opt-in for write-heavy waves
