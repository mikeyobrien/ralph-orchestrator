# bug-hunter-web-task-execution-persistence

## Coverage summary

Scope inspected:
- `backend/ralph-web-server/src/services/TaskBridge.ts`
- `backend/ralph-web-server/src/runner/ProcessSupervisor.ts`
- `backend/ralph-web-server/src/runner/FileOutputStreamer.ts`
- `backend/ralph-web-server/src/runner/RalphTaskHandler.ts`
- `backend/ralph-web-server/src/queue/EventBus.ts`
- `backend/ralph-web-server/src/queue/TaskQueueService.ts`
- `backend/ralph-web-server/src/queue/TaskState.ts`
- `backend/ralph-web-server/src/queue/PersistentTaskQueueService.ts`
- `backend/ralph-web-server/src/queue/Dispatcher.ts`
- `backend/ralph-web-server/src/repositories/QueuedTaskRepository.ts`
- `backend/ralph-web-server/src/db/connection.ts`
- `backend/ralph-web-server/src/db/schema.ts`
- `backend/ralph-web-server/src/db/testUtils.ts`

Tests inspected for encoded assumptions:
- `backend/ralph-web-server/src/services/TaskBridge.test.ts`
- `backend/ralph-web-server/src/runner/ProcessSupervisor.test.ts`
- `backend/ralph-web-server/src/runner/FileOutputStreamer.test.ts`
- `backend/ralph-web-server/src/queue/PersistentTaskQueueService.test.ts`
- `backend/ralph-web-server/src/queue/Dispatcher.test.ts`

Bug classes checked:
- persistence and restart recovery
- crash consistency and partial failure
- cancellation and duplicate/incorrect terminal transitions
- queue/task-table divergence
- log streaming correctness and output loss
- malformed state/config assumptions at the DB/queue boundary
- tests that encode the same wrong assumptions

Highest-risk paths are exhausted. I stopped after the restart/cancellation/logging surfaces converged on the same small set of material defects.

## Findings

### P1: Pending tasks lose DB lifecycle updates after restart because TaskBridge correlation is in-memory only

Severity: `P1`  
Confidence: `High`

Why this is a bug:
- `TaskBridge` updates `tasks.status` only when it can map `queuedTaskId -> dbTaskId` through `taskIdMap`.
- That map is populated only at enqueue time in memory and is never rebuilt on startup.
- After restart, hydrated pending queue entries still execute, but `task.started` / `task.completed` / `task.failed` are ignored, so the user-facing task row stays stuck in `pending`.

Exact evidence:
- Event handlers bail out if `taskIdMap` has no entry: `TaskBridge.handleTaskStarted()` at `backend/ralph-web-server/src/services/TaskBridge.ts:275-290`, `handleTaskCompleted()` at `backend/ralph-web-server/src/services/TaskBridge.ts:298-359`, `handleTaskFailed()` at `backend/ralph-web-server/src/services/TaskBridge.ts:365-382`, `handleTaskTimeout()` at `backend/ralph-web-server/src/services/TaskBridge.ts:388-405`.
- The only place that populates the map is enqueue time: `TaskBridge.enqueueTask()` at `backend/ralph-web-server/src/services/TaskBridge.ts:476-499`.
- Startup restores pending queue rows with `taskQueue.hydrate()` but does not rebuild `taskIdMap`: `backend/ralph-web-server/src/serve.ts:172-185`.
- Queue persistence throws away the DB task link entirely by writing `dbTaskId: null` for every queued task row: `PersistentTaskQueueService.enqueue()` at `backend/ralph-web-server/src/queue/PersistentTaskQueueService.ts:46-58`.

Minimal triggering scenario:
1. Enqueue a DB task through `TaskBridge.enqueueTask()`.
2. Restart the server before the dispatcher starts or finishes the queued work.
3. On restart, `PersistentTaskQueueService.hydrate()` restores the pending queue row.
4. Dispatcher executes it and publishes lifecycle events.
5. `TaskBridge` ignores those events because `taskIdMap` is empty, leaving `tasks.status='pending'` and stale timestamps/error fields.

Impact:
- User-facing task rows can remain permanently `pending` even though the queued process already ran to completion or failure.
- Retry/cancel UX becomes inconsistent because the DB task and persisted queue row disagree about reality.
- Any restart during queue backlog processing can silently corrupt task history.

Fix direction:
- Persist the DB task linkage in `queued_tasks.db_task_id` and restore `taskIdMap` from durable state on startup.
- Alternatively, stop relying on `taskIdMap` entirely and resolve DB task rows from `queued_tasks.db_task_id` or `tasks.queued_task_id` inside the event handlers.

Current test coverage:
- `PersistentTaskQueueService.test.ts` covers hydrate/recover in isolation but never verifies DB-task correlation after restart: `backend/ralph-web-server/src/queue/PersistentTaskQueueService.test.ts:68-145`.
- `TaskBridge.test.ts` has no restart-path test for `taskIdMap` reconstruction or lifecycle updates after hydration: `backend/ralph-web-server/src/services/TaskBridge.test.ts:31-260`.

### P1: Running tasks are not actually recovered across restart; alive processes become orphaned and dead ones leave `queued_tasks` stuck in `running`

Severity: `P1`  
Confidence: `High`

Why this is a bug:
- The live-process restart path only resumes file tailing; it does not recreate a watcher that will ever publish completion/failure or update queue state when the detached child exits later.
- The dead-process restart path updates only the `tasks` table, not the persisted queue row.
- `PersistentTaskQueueService.recoverCrashed()` exists specifically for running queue recovery, but startup never calls it.

Exact evidence:
- `ProcessSupervisor.spawn()` writes terminal `status.json` only from `child.on("exit")` in the original server process: `backend/ralph-web-server/src/runner/ProcessSupervisor.ts:100-117`.
- `ProcessSupervisor.reconnect()` only returns `{ pid, taskDir }`; it does not attach any exit monitoring in the new process: `backend/ralph-web-server/src/runner/ProcessSupervisor.ts:127-165`.
- `TaskBridge.reconnectRunningTasks()` alive branch only calls `outputStreamer.stream(...)` and publishes `task.output`; it never monitors liveness, publishes `task.completed` / `task.failed`, or updates queue state: `backend/ralph-web-server/src/services/TaskBridge.ts:625-652`.
- The dead branch marks only the `tasks` row failed; it never updates `queued_tasks`: `backend/ralph-web-server/src/services/TaskBridge.ts:653-677`.
- Startup calls `taskQueue.hydrate()` and `taskBridge.reconnectRunningTasks()`, but never calls `taskQueue.recoverCrashed()`: `backend/ralph-web-server/src/serve.ts:172-185`.
- `PersistentTaskQueueService.recoverCrashed()` is the only code that marks running queue rows failed in durable storage: `backend/ralph-web-server/src/queue/PersistentTaskQueueService.ts:107-119`.

Minimal triggering scenarios:
1. Alive-process orphaning:
   1. Start a long-running task.
   2. Restart the web server while the detached child keeps running.
   3. Startup reconnects and resumes log tailing.
   4. When the child later exits, no current process converts that exit into `task.completed` / `task.failed`, so the DB task and queue row can remain `running` forever.
2. Dead-process queue divergence:
   1. Start a task and let the server crash/restart after the child has already died.
   2. `reconnectRunningTasks()` marks the `tasks` row failed.
   3. The corresponding `queued_tasks.state` remains `running`, because startup never invokes `recoverCrashed()` and the dead branch never updates the queue repository.

Impact:
- Running tasks can become permanently orphaned after restart.
- Recovery reports can claim success (`reconnected`) while the system has no path to terminalize the queue row later.
- Queue persistence stops being authoritative because dead rows accumulate in `running`.

Fix direction:
- Decide on one restart strategy and make it end-to-end:
  - Either rehydrate running tasks into a restart-aware runner/dispatcher that can publish terminal events.
  - Or treat all pre-restart running tasks as crashed and call `recoverCrashed()` before exposing the service as healthy.
- If reconnecting live detached processes is required, add an explicit completion watcher in the new process and update both `tasks` and `queued_tasks` from that watcher.

Current test coverage:
- `PersistentTaskQueueService.test.ts` proves `recoverCrashed()` works in isolation but there is no startup integration test showing it is actually used: `backend/ralph-web-server/src/queue/PersistentTaskQueueService.test.ts:101-126`.
- `ProcessSupervisor.test.ts` only checks immediate reconnect/status cases while the original parent process is still alive; it never simulates restart semantics: `backend/ralph-web-server/src/runner/ProcessSupervisor.test.ts:48-96`.
- `TaskBridge.test.ts` covers `recoverStuckTasks()` but not `reconnectRunningTasks()`: `backend/ralph-web-server/src/services/TaskBridge.test.ts:160-214`.

### P2: User cancellation is recorded as failure in `tasks` but success/completion in the queue path

Severity: `P2`  
Confidence: `High`

Why this is a bug:
- `TaskBridge.cancelTask()` kills the detached process directly and immediately marks the DB task as failed/cancelled.
- `RalphRunner` reports `SIGTERM`/`SIGKILL` exits as `RunnerState.CANCELLED`.
- `RalphTaskHandler` throws only on `FAILED`, so a cancelled run returns normally to `Dispatcher`.
- `Dispatcher` treats a normal return as success, marks the queue task completed, publishes `task.completed`, and increments `successCount`.

Exact evidence:
- Immediate DB update on cancel: `backend/ralph-web-server/src/services/TaskBridge.ts:705-744`.
- `RalphRunner.handleExit()` maps `SIGTERM`/`SIGKILL` to `RunnerState.CANCELLED`: `backend/ralph-web-server/src/runner/RalphRunner.ts:350-378`.
- `RalphTaskHandler` throws only when `result.state === RunnerState.FAILED`: `backend/ralph-web-server/src/runner/RalphTaskHandler.ts:104-120`.
- `Dispatcher.executeTask()` marks any normal handler return as completed and publishes `task.completed`: `backend/ralph-web-server/src/queue/Dispatcher.ts:455-490`.
- Dispatcher already has a real cancellation path, but `TaskBridge.cancelTask()` bypasses it: `backend/ralph-web-server/src/queue/Dispatcher.ts:223-245` and `backend/ralph-web-server/src/queue/Dispatcher.ts:497-545`.

Minimal triggering scenario:
1. Start a running `ralph.run` task through the dispatcher.
2. Call the web cancel mutation, which delegates to `TaskBridge.cancelTask()`.
3. The detached process is stopped and the DB task becomes `failed` with "Task cancelled by user".
4. The runner exits with `RunnerState.CANCELLED`, but the handler returns successfully.
5. Dispatcher marks the persisted queue row `completed` and emits `task.completed`.

Impact:
- `tasks` and `queued_tasks` disagree after user cancellation.
- Cancellation inflates success metrics and can trigger downstream "completed" logic incorrectly.
- Because `TaskBridge.cancelTask()` deletes the in-memory correlation mapping first, the later `task.completed` event is ignored, hiding the inconsistency instead of fixing it.

Fix direction:
- Route cancellation through `Dispatcher.cancelTask()` so queue state, events, and process termination stay in one control path.
- If `TaskBridge` must stop the process directly, then `RalphTaskHandler` must treat `RunnerState.CANCELLED` as a cancellation/error signal instead of success.
- Add durable queue updates for cancelled state; today `queued_tasks` typing excludes `cancelled`.

Current test coverage:
- `TaskBridge.test.ts` only asserts the immediate DB-side behavior of `cancelTask()` with mocked supervisors; it never runs a real dispatcher/runner cancellation flow: `backend/ralph-web-server/src/services/TaskBridge.test.ts:31-157`.
- `Dispatcher.test.ts` proves dispatcher-native cancellation works, but that is not the path used by the web cancel endpoint: `backend/ralph-web-server/src/queue/Dispatcher.test.ts` (inspected separately).

### P2: Reconnected log streaming publishes to an unused event channel, so live logs after restart are dropped instead of reaching WebSocket clients or durable log storage

Severity: `P2`  
Confidence: `High`

Why this is a bug:
- Normal execution uses `LogBroadcaster.broadcast()` to fan logs to clients and persist them through `TaskLogRepository`.
- Restart reconnection bypasses `LogBroadcaster` and emits `task.output` on `EventBus`.
- There is no subscriber for `task.output` in the scoped server code, so resumed logs after restart are neither broadcast nor persisted.

Exact evidence:
- Normal path persists/broadcasts log lines through `LogBroadcaster`: `backend/ralph-web-server/src/runner/RalphTaskHandler.ts:87-94`.
- Reconnect path publishes only `task.output` events: `backend/ralph-web-server/src/services/TaskBridge.ts:642-650`.
- `rg` over `backend/ralph-web-server/src` found the only `task.output` publish at `TaskBridge.ts:645`; there is no matching `subscribe("task.output", ...)` consumer in the scoped code.
- `LogBroadcaster.broadcast()` is where persistence happens through `TaskLogRepository.append()`: `backend/ralph-web-server/src/api/LogBroadcaster.ts:196-205` and `backend/ralph-web-server/src/api/LogBroadcaster.ts:277-287` (inspected to confirm the missing bridge).

Minimal triggering scenario:
1. Start a long-running task with a WebSocket client subscribed.
2. Restart the server while the detached child keeps running.
3. `reconnectRunningTasks()` resumes file watching.
4. New log lines are published as `task.output` events only.
5. No WebSocket subscriber receives them, and no backlog is persisted for later viewers.

Impact:
- Users lose live task output after server restart even if the underlying task survives.
- Completed-task backlog becomes incomplete because post-restart lines were never stored in `task_logs`.

Fix direction:
- Reuse the same `LogBroadcaster` path on reconnect that the normal runner path uses.
- If `EventBus` is intended as the reconnect bridge, add an explicit subscriber that forwards `task.output` into `LogBroadcaster.broadcast()`.

Current test coverage:
- `FileOutputStreamer.test.ts` only checks local file watching and stdout-only resume behavior; it never covers restart integration or WebSocket persistence: `backend/ralph-web-server/src/runner/FileOutputStreamer.test.ts:12-140`.
- No test in the scoped suite asserts that `reconnectRunningTasks()` delivers logs to `LogBroadcaster` or `task_logs`.

## Explicit no-finding coverage notes

- `QueuedTaskRepository.ts`, `db/schema.ts`, and `db/connection.ts` look mechanically sound for basic CRUD/table creation. I did not find an additional standalone P0-P2 defect there beyond the higher-level misuse already captured above: the schema and repository support `queued_tasks`, but startup/control-flow code fails to keep that table authoritative.
- `TaskQueueService.ts`, `TaskState.ts`, and `EventBus.ts` look internally coherent on their own. The material problems come from how the web task bridge bypasses or fails to use the queue/dispatcher cancellation and recovery mechanisms, not from the state machine or pub/sub primitives themselves.
- `ProcessSupervisor.stop()` and `QueuedTaskRepository` CRUD methods did not show another independent high-severity defect after the restart/cancellation issues above were accounted for.

## Remaining blind spots

- I did not inspect frontend consumers of task/log state, only the backend boundary listed in scope.
- I did not run a full end-to-end restart harness; the findings are code-backed and cross-checked against startup/tests, but not demonstrated through a live multi-process integration test in this lane.
- `FileOutputStreamer` has additional correctness issues around shared resume offsets for stdout/stderr (`backend/ralph-web-server/src/runner/FileOutputStreamer.ts:36-45`, `:144-188`), but I am not promoting that to a separate finding because the restart path currently fails earlier by not wiring resumed logs anywhere useful.

## Completion rationale

This lane covered the highest-risk persistence boundary end-to-end:
- enqueue -> persistent queue -> dispatcher -> runner -> detached process
- startup recovery for pending and running tasks
- cancellation and terminal-state publication
- live log delivery and persisted backlog after restart

The defects above explain the main correctness failures in this subsystem:
- pending work loses DB correlation after restart
- running work is not actually recovered
- cancellation diverges between DB and queue state
- resumed logs after restart are dropped

After those were established, the remaining scoped files mostly reduced to helper CRUD/state-machine code or tests with missing integration coverage rather than additional independent P0-P2 bugs.
