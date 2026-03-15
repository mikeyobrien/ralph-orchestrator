# Changelog: Factory Git Status Panel

## 2026-03-16

### Added

- `git.status` read-only RPC method in `ralph-api` — runs `git status --porcelain` + `git branch --show-current` in workspace root, returns `{ branch, files: [{ status, path }], clean }`.
- `dispatch_git` handler in `crates/ralph-api/src/runtime/dispatch.rs` following existing dispatch pattern (`dispatch_board`, `dispatch_worker`).
- `git.status` registered in `KNOWN_METHODS` (not `MUTATING_METHODS`).
- `git.status` added to `rpc-v1-schema.json` methodName enum.
- `GitStatusPanel` React component in `frontend/ralph-web/src/components/factory/GitStatusPanel.tsx` — displays current branch, file change list with status badges, clean/dirty indicator.
- `trpc.factory.gitStatus` query wired in `frontend/ralph-web/src/trpc.ts` with 10s polling interval.
- GitStatusPanel integrated into `FactoryPage.tsx` between FactoryStats and Workers sections.
- Integration test `git_status_returns_branch_files_and_clean` in `crates/ralph-api/tests/rpc_v1_task_loop.rs` covering clean and dirty repo states.

### Crates Affected

- `ralph-api` — new `git.status` RPC method, dispatch handler, schema update, integration test
- `frontend/ralph-web` — new GitStatusPanel component, trpc query, FactoryPage layout

### Suggested AGENTS.md Updates

- **Git command pattern**: `dispatch_git` follows the same prefix-routing pattern as `dispatch_board` and `dispatch_worker`. Future `git.*` methods should be added to this dispatcher.
- **Porcelain parsing**: Git porcelain output is parsed with fixed 2-char status prefix + path. This is safe for ASCII output but would need adjustment for paths with special characters if git's `-z` flag is used.
- **Frontend polling convention**: Heavy operations (git status, build status) use 10s `refetchInterval`; lightweight queries (task list, worker heartbeat) use shorter intervals.
