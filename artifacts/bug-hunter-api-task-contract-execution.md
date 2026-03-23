# Scope

Scoped lane `api-task-contract-execution`, limited to:

- `frontend/ralph-web/src/components/tasks/TaskInput.tsx`
- `frontend/ralph-web/src/pages/TaskDetailPage.tsx`
- `frontend/ralph-web/src/trpc.ts`
- `frontend/ralph-web/src/hooks/useTaskWebSocket.ts`
- `crates/ralph-api/src/task_domain.rs`
- `crates/ralph-api/src/runtime/dispatch.rs`
- `crates/ralph-api/src/stream_domain/rpc_side_effects.rs`
- `crates/ralph-api/README.md`

# Findings

## P2: Task detail page stays stale after `run` / `retry` / `cancel`

- Impacted files:
  - `frontend/ralph-web/src/pages/TaskDetailPage.tsx`
  - `frontend/ralph-web/src/trpc.ts`
  - `crates/ralph-api/src/runtime/dispatch.rs`
  - `crates/ralph-api/src/stream_domain/rpc_side_effects.rs`
- Why it is a bug:
  - The backend returns updated task records and emits `task.status.changed`, but the detail page does not invalidate `task.get` after `run`, `retry`, or `cancel`, and it does not subscribe to the stream hook that would apply the pushed status updates.
- Exact evidence:
  - `TaskDetailPage` creates `runMutation`, `retryMutation`, and `cancelMutation` with no `onSuccess` invalidation for `task.get`: `frontend/ralph-web/src/pages/TaskDetailPage.tsx:75-109`.
  - The page only invalidates loop list for merge retry, not task detail state: `frontend/ralph-web/src/pages/TaskDetailPage.tsx:85-90`.
  - `task.run`, `task.retry`, and `task.cancel` all return updated task payloads from the API runtime: `crates/ralph-api/src/runtime/dispatch.rs:113-133`.
  - The runtime also emits `task.status.changed` for those methods: `crates/ralph-api/src/stream_domain/rpc_side_effects.rs:20-28`.
- Triggering scenario:
  - Open a task detail page.
  - Click `Run`, `Retry`, or `Cancel`.
  - The mutation succeeds, but the page continues showing the old status until a manual refresh or navigation cycle.
- Likely impact:
  - Users make decisions from stale state, including retrying, leaving the page, or assuming the action failed.
- Recommended fix direction:
  - Either invalidate `task.get` in each mutation success handler, or wire `useTaskWebSocket` into `TaskDetailPage` so `task.status.changed` updates local state.
- Confidence:
  - High.
- Whether current tests cover it:
  - No direct coverage in the inspected files. The lane found no evidence that `TaskDetailPage` tests assert post-mutation state refresh.

## P2: The dashboard preset selector is a no-op for task creation

- Impacted files:
  - `frontend/ralph-web/src/components/tasks/TaskInput.tsx`
  - `frontend/ralph-web/src/trpc.ts`
  - `crates/ralph-api/src/task_domain.rs`
- Why it is a bug:
  - The UI exposes preset selection as if task creation will honor it, but the client strips the field before the RPC call and the API task schema has nowhere to store or apply it.
- Exact evidence:
  - `TaskInput` includes `preset: selectedPreset` when creating a task: `frontend/ralph-web/src/components/tasks/TaskInput.tsx:88-99`.
  - The RPC client explicitly removes `preset` from `task.create` before sending the request: `frontend/ralph-web/src/trpc.ts:221-226`.
  - `TaskCreateParams` in `ralph-api` has no `preset` field at all: `crates/ralph-api/src/task_domain.rs:21-29`.
- Triggering scenario:
  - Select a non-default preset in the task input.
  - Submit a task and expect that preset to influence execution.
  - The created task is indistinguishable from a default task because the selected preset never leaves the browser.
- Likely impact:
  - Users believe they launched a task under a specific preset when the backend always uses the default task path.
- Recommended fix direction:
  - Either remove preset selection from this API path until supported, or extend the RPC contract and task domain so `preset` is accepted, persisted, and applied.
- Confidence:
  - High.
- Whether current tests cover it:
  - The UI has preset-selection tests, but the inspected implementation intentionally drops the field, so there is no end-to-end assertion that preset reaches the backend.

# No-Finding Coverage Notes

- `crates/ralph-api/src/runtime/dispatch.rs`
  - Checked task-method dispatch correctness and response shapes for `task.create`, `task.run`, `task.retry`, `task.cancel`, and `task.status`.
  - No separate bug found in the dispatch switch itself.
- `crates/ralph-api/src/task_domain.rs`
  - Checked task persistence and state transitions for create/run/retry/cancel/status.
  - No separate serialization or malformed-input defect found in the inspected path beyond missing preset support.
- `crates/ralph-api/src/stream_domain/rpc_side_effects.rs`
  - Checked event emission for task mutations.
  - Side-effect emission exists and matches the intended task-status topic family.
- `frontend/ralph-web/src/hooks/useTaskWebSocket.ts`
  - Checked payload parsing and status-event handling.
  - No parsing bug found in-scope; the problem is that the detail page does not use this hook.

# Remaining Blind Spots

- This lane did not inspect the broader task-detail component stack outside the requested files.
- This lane did not inspect task-to-loop linking beyond the files above.

# Recommended Next Search

- Inspect the task-detail component stack just outside this lane to see whether live task status is already available but dropped before it reaches the page.
- Inspect task-to-loop linkage because the detail page expects `task.loopId` while this task contract does not supply it in the inspected path.
