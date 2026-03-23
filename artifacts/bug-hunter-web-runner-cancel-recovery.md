# Scope

Scoped lane `web-runner-cancel-recovery`, limited to:

- `backend/ralph-web-server/src/services/TaskBridge.ts`
- `backend/ralph-web-server/src/runner/ProcessSupervisor.ts`
- `backend/ralph-web-server/src/runner/RalphRunner.ts`
- `backend/ralph-web-server/src/runner/FileOutputStreamer.ts`
- `backend/ralph-web-server/src/queue/PersistentTaskQueueService.ts`
- `backend/ralph-web-server/src/queue/Dispatcher.ts`
- `backend/ralph-web-server/src/repositories/QueuedTaskRepository.ts`

# Findings

## P1: Reconnected detached runs never get finalized after a server restart

- Impacted files:
  - `backend/ralph-web-server/src/services/TaskBridge.ts`
  - `backend/ralph-web-server/src/runner/ProcessSupervisor.ts`
  - `backend/ralph-web-server/src/runner/RalphRunner.ts`
- Why it is a bug:
  - A detached `ralph` process that survives a web-server restart is reattached only for log streaming. No replacement exit/status monitor updates the database when that process later exits.
- Exact evidence:
  - `ProcessSupervisor` writes terminal status only from the original in-process `child.on("exit")` callback: `backend/ralph-web-server/src/runner/ProcessSupervisor.ts:96-115`.
  - `RalphRunner` has its own exit polling only while the original runner instance is alive: `backend/ralph-web-server/src/runner/RalphRunner.ts:261-275`.
  - After restart, `TaskBridge.reconnectRunningTasks()` only calls `outputStreamer.stream(...)` and increments `reconnectedCount`; it does not start any new completion watcher: `backend/ralph-web-server/src/services/TaskBridge.ts:625-681`.
- Triggering scenario:
  - Start a long-running task from the web server.
  - Restart the web server while the detached `ralph` process keeps running.
  - After the process later exits, the DB task remains `running` indefinitely because no path transitions it to `closed` or `failed`.
- Likely impact:
  - Zombie-running tasks, blocked retries/cancels, misleading UI state, and permanent DB/queue divergence after restart.
- Recommended fix direction:
  - On reconnect, start a fresh status/exit monitor equivalent to the original runner's polling, or continuously reconcile `status.json` for reattached tasks until terminal.
- Confidence:
  - High.
- Whether current tests cover it:
  - No evidence of a restart-then-finish regression test in the inspected files.

## P1: `task.cancelled` is published but never mirrored into the task database

- Impacted files:
  - `backend/ralph-web-server/src/services/TaskBridge.ts`
  - `backend/ralph-web-server/src/queue/Dispatcher.ts`
- Why it is a bug:
  - The dispatcher supports canceling both pending and running queue entries and publishes `task.cancelled`, but `TaskBridge` subscribes only to `task.started`, `task.completed`, `task.failed`, and `task.timeout`. Cancelled queue entries therefore do not update the DB task row.
- Exact evidence:
  - `TaskBridge.subscribeToEvents()` subscribes to started/completed/failed/timeout only: `backend/ralph-web-server/src/services/TaskBridge.ts:242-274`.
  - `enqueueTask()` writes DB status `pending`: `backend/ralph-web-server/src/services/TaskBridge.ts:476-499`.
  - `Dispatcher.cancelTask()` cancels pending queue entries and publishes `task.cancelled`: `backend/ralph-web-server/src/queue/Dispatcher.ts:223-245`.
  - `executeTask()` also publishes `task.cancelled` when a running task aborts: `backend/ralph-web-server/src/queue/Dispatcher.ts:499-544`.
- Triggering scenario:
  - Cancel a pending task through the queue path, or abort a running task through the dispatcher path.
  - The queue entry becomes cancelled, but the DB row remains `pending` or `running`.
  - `retryTask()` later refuses to retry because it only accepts `failed`: `backend/ralph-web-server/src/services/TaskBridge.ts:565-591`.
- Likely impact:
  - Tasks become stuck in non-terminal DB states even though execution has already been cancelled.
- Recommended fix direction:
  - Subscribe to `task.cancelled` in `TaskBridge` and transition DB rows into an explicit cancelled or failed terminal state that retry logic understands.
- Confidence:
  - High.
- Whether current tests cover it:
  - No explicit `task.cancelled` mirroring coverage was visible in the inspected files.

# No-Finding Coverage Notes

- `backend/ralph-web-server/src/repositories/QueuedTaskRepository.ts`
  - Checked CRUD/state update paths for persisted queue rows.
  - No stronger defect found there beyond downstream consumers mishandling cancellation/recovery.
- `backend/ralph-web-server/src/runner/FileOutputStreamer.ts`
  - Checked live tailing and resume API shape.
  - A resume-offset weakness exists, but in the inspected tree it appears unused outside tests, so I did not elevate it as a material runtime bug in this report.
- `backend/ralph-web-server/src/queue/PersistentTaskQueueService.ts`
  - Checked hydrate/recover paths.
  - No separate crash-recovery defect confirmed beyond the missing DB sync/finalization issues above.

# Remaining Blind Spots

- I did not validate whether HTTP handlers ever expose dispatcher pending-cancel functionality directly.
- I did not inspect the surrounding frontend to see how these stale DB states surface in the UI.

# Recommended Next Search

- Validate `FileOutputStreamer` resume correctness, especially stdout/stderr offset handling around `backend/ralph-web-server/src/runner/FileOutputStreamer.ts:144-189`.
- Inspect whether the synchronous wait loop in `backend/ralph-web-server/src/runner/ProcessSupervisor.ts:233-256` can stall the server during cancellation.
