# Scope

Follow-up wave on the task-contract family discovered in wave 1.

Inspected:

- `frontend/ralph-web/src/pages/TaskDetailPage.tsx`
- `frontend/ralph-web/src/components/tasks/TaskDetailHeader.tsx`
- `frontend/ralph-web/src/components/tasks/TaskThread.tsx`
- `crates/ralph-api/src/task_domain.rs`

# Findings

## P1: Running a task can crash the task detail page on the next refresh because `pending` is a real backend state but not a supported detail-page status

- Impacted files:
  - `crates/ralph-api/src/task_domain.rs`
  - `frontend/ralph-web/src/pages/TaskDetailPage.tsx`
  - `frontend/ralph-web/src/components/tasks/TaskDetailHeader.tsx`
- Why it is a bug:
  - The API explicitly transitions tasks into `pending` on `task.run`, but the detail page narrows status to a union that excludes `pending` and passes it into a header that dereferences `STATUS_MAP[status]` without guarding unknown values.
- Exact evidence:
  - `task.run` queues tasks by setting `task.status = "pending"`: `crates/ralph-api/src/task_domain.rs:460-472`.
  - `TaskDetailPage` force-casts `task.status` to `"open" | "running" | "completed" | "closed" | "failed"`: `frontend/ralph-web/src/pages/TaskDetailPage.tsx:195-201`.
  - `TaskDetailHeader` defines `TaskStatus` without `pending` and immediately dereferences `STATUS_MAP[status]`: `frontend/ralph-web/src/components/tasks/TaskDetailHeader.tsx:14-15`, `frontend/ralph-web/src/components/tasks/TaskDetailHeader.tsx:64-67`, `frontend/ralph-web/src/components/tasks/TaskDetailHeader.tsx:104-106`.
- Triggering scenario:
  - Open a task detail page for an `open` task.
  - Trigger `task.run`.
  - After the page refetches or reloads and the backend returns `pending`, the header receives an unsupported status and tries to read `statusConfig.icon` from `undefined`.
- Likely impact:
  - User-visible crash or blank detail view immediately after starting a task.
- Recommended fix direction:
  - Add `pending` to the detail-page status contract and render it explicitly, or normalize backend `pending` to a supported UI state before rendering.
- Confidence:
  - High.
- Whether current tests cover it:
  - No. The requested files show explicit pending support in `TaskThread`, but no equivalent coverage or guard exists in the detail-header path.

# No-Finding Coverage Notes

- `frontend/ralph-web/src/components/tasks/TaskThread.tsx`
  - Checked adjacent task-list rendering.
  - The list/thread UI does define a `pending` status config: `frontend/ralph-web/src/components/tasks/TaskThread.tsx:93-98`.
- No additional P0-P2 issue was confirmed in this follow-up beyond the unsupported detail-page status family.

# Remaining Blind Spots

- I did not inspect every task-detail child component beyond the header path.
- I did not run frontend tests in this pass.

# Recommended Next Search

- Search the rest of the task-detail component stack for other unsupported backend states such as `cancelled` or archived terminal variants.
