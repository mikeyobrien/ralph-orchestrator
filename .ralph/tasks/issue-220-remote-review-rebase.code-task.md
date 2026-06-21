---
status: completed
created: 2026-06-21
started: 2026-06-21
completed: 2026-06-21
---
# Task: Issue #220 Remote Review and Rebase Workflow

## Description
Add first-class CLI support for a remote review workflow for completed Ralph worktree loops without merging them into the base branch.

## Scope
- Add a `ralph loops publish-review` command that pushes a loop branch to a remote and writes a local review summary artifact.
- Add a `ralph loops rebase` command that rebases one loop branch, or all queued/needs-review/manual Ralph worktree branches, onto a selected base branch without merging.
- Reuse existing loop resolution, worktree branch naming, registry, and merge queue conventions.
- Keep behavior CLI-level and avoid broad orchestration or merge queue refactors.
- Update CLI help/docs for the new workflow.

## Verification
- Integration tests with temporary git repositories, remotes, and worktrees.
- Focused `ralph-cli` tests for new loop commands.
- `cargo test -p ralph-core smoke_runner`.
- `cargo test` if feasible.
