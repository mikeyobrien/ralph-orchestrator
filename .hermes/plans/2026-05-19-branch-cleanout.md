# Ralph Orchestrator Branch Cleanout

## Goal
Safely prune stale local branches/worktrees in `/Users/rook/projects/ralph-orchestrator`, biased toward deletion of old experiment/autoresearch branches while preserving active/open-PR work.

## Steps
1. Fetch/prune remotes and capture current git/worktree state.
2. Classify branches by merged status, age, worktree attachment, and GitHub PR state.
3. Delete clearly safe branches first: merged/local-only stale experiments with no active worktree/open PR.
4. Investigate ambiguous recent/unmerged branches before destructive deletion.
5. Verify final branch/worktree state and report deleted vs kept.

## Guardrails
- Do not force-delete branches attached to active worktrees without removing the worktree first.
- Do not delete open PR/release/hotfix branches without explicit evidence they are stale.
- Prefer cleanup over branch-by-branch questions when Git/GitHub evidence is clear.
