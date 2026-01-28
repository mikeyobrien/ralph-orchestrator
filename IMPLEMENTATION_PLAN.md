## Specs Referenced
- `specs/loop-management-merge-queue.spec.md`
- `specs/ralph-web-loops.spec.md`

## Stage 1: Spec & Scope Confirmation
**Goal**: Capture the merge-queue reliability fixes in a clear, reviewable spec.
**Success Criteria**: Spec approved with agreed goals/non-goals and test plan.
**Tests**: None (documentation only).
**Status**: Complete

## Stage 2: Merge Queue State Transitions + Exclusive Merge Spawn
**Goal**: Ensure merge loops run in-place and update merge queue state on start/finish.
**Success Criteria**:
- Merge loops use `--exclusive` and do not spawn worktrees.
- Queue entries move to `merging` on merge-loop start and to `merged`/`needs-review` on completion.
- Merge loops never enqueue themselves.
- `ralph loops` UX improvements applied (summary header, age column, actionable hints, default hides `merged`/`discarded` unless `--all`).
- Merge commits use conventional commit style (`merge(ralph): ...`) with diff-based summary.
**Tests**: `cargo test -p ralph-cli`, `cargo test -p ralph-core`.
**Status**: Complete

## Stage 3: Task-Centric Loop Visibility (Frontend)
**Goal**: Surface loop/merge status inside Tasks UI and add a merge-queue panel (no /loops page).
**Success Criteria**:
- Task threads show loop badge + detail/actions when loop mapping exists.
- Merge queue panel shows non-task loops and supports process/prune actions.
- tRPC `loops` router exists with list + actions (process/prune/retry/discard/stop/merge).
- UI auto-refreshes and supports manual refresh + show-all toggle (merge queue panel).
- Non-git workspaces supported in UI (workspace path shown, git-only actions/labels hidden).
**Tests**: Manual UI smoke checks; API sanity calls via dev server.
**Status**: Not Started

**Wireframe (ASCII)**

Tasks page (top)
```
┌────────────────────────────────────────────────────────────────────────────┐
│ Tasks                                             [notifications] [refresh] │
│ Manage and monitor your Ralph tasks                                         │
├────────────────────────────────────────────────────────────────────────────┤
│ Merge Queue (panel)                  [Process queue] [Prune stale] [Show all]│
│ • Running 1 • Queued 3 • Merging 1 • Needs-review 2 • Orphan 1              │
│ ───────────────────────────────────────────────────────────────────────────│
│ LOOP-ID   STATUS        AGE   LOCATION        ACTIONS                       │
│ e918      queued        3m    .worktrees/...  [merge] [discard]              │
│ 010840    needs-review  1h    .worktrees/...  [retry] [discard]              │
└────────────────────────────────────────────────────────────────────────────┘

Task list (collapsed row)
```
```
┌────────────────────────────────────────────────────────────────────────────┐
│ › ●  Task title…                         [Loop: queued] [Closed] 26m ago     │
└────────────────────────────────────────────────────────────────────────────┘
```

Task row (expanded)
```
┌────────────────────────────────────────────────────────────────────────────┐
│ ˅ ●  Task title…                         [Loop: queued] [Closed] 26m ago     │
│   Created: 1/26/2026 10:33 PM   Completed: 19m 14s                           │
│   Loop: ralph-20260127-043327-e918  Status: queued  Age: 3m                  │
│   Merge: —  Worktree: .worktrees/ralph-20260127-043327-e918                  │
│   Actions: [merge] [retry] [discard] [stop]                                  │
│                                                                              │
│   Execution Summary                                                         │
│   …                                                                          │
│                                                                              │
│   Logs…                                                                      │
└────────────────────────────────────────────────────────────────────────────┘
```

**Placement notes**
- Loop badge appears inline with status badge in collapsed header.
- Loop detail block sits in expanded metadata section, above Execution Summary.
- Merge Queue panel sits between archive controls and task list.

## Stage 4: Verification & Cleanup
**Goal**: Validate behavior and keep repo clean.
**Success Criteria**:
- Smoke tests run.
- Python tests run in a `.venv`.
- No new lint/format issues.
- `ralph loops` UX verified (summary header, age column, hints, and `--all` reveals `merged`/`discarded`).
- Snapshot check added/updated for `ralph loops` output (default + `--all`).
- Merge commit message format verified against conventional style.
**Tests**: `cargo test -p ralph-core smoke_runner`, python test command (TBD after discovery).
**Status**: Not Started
