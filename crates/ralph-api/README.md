# ralph-api

Rust-native bootstrap runtime for the RPC v1 control plane.

## What this crate provides (bootstrap scope)

- HTTP RPC endpoint: `POST /rpc/v1`
- WebSocket stream endpoint: `GET /rpc/v1/stream` (keepalive skeleton)
- Metadata endpoints:
  - `GET /health`
  - `GET /rpc/v1/capabilities`
- Protocol runtime for canonical RPC v1 envelopes
- Shared error envelope mapping (`INVALID_REQUEST`, `METHOD_NOT_FOUND`, etc.)
- Auth abstraction:
  - `trusted_local`
  - `token` mode hook
- Idempotency primitives for mutating methods with in-memory store
- Implemented methods:
  - `system.health`
  - `system.version`
  - `system.capabilities`
  - Full `task.*` family (`list/get/ready/create/update/close/archive/unarchive/delete/clear/retry/cancel`)
  - Full `loop.*` family (`list/status/process/prune/retry/discard/stop/merge/merge_button_state/trigger_merge_task`)
  - Full `planning.*` family (`list/get/start/respond/resume/delete/get_artifact`)
  - Full `config.*` family (`get/update`)
  - Full `preset.*` family (`list`)
  - Full `collection.*` family (`list/get/create/update/delete/import/export`)
  - Full `worker.*` family (`list/get/register/deregister/heartbeat/claim_next/reclaim_expired`)

Persistence notes:
- `task.*` data is persisted in `.ralph/api/tasks-v1.json`
- `loop.*` reads/writes `.ralph/loops.json` and `.ralph/merge-queue.jsonl` via `ralph-core`
- `planning.*` data is persisted under `.ralph/planning-sessions/<session-id>/`
- `collection.*` data is persisted in `.ralph/api/collections-v1.json`
- `worker.*` data is persisted in `.ralph/api/workers-v1.json`
- `config.*` reads/writes `ralph.yml` with YAML validation + atomic replace semantics
- `preset.list` reads builtins from `presets/`, local files from `.ralph/hats/`, and collection-backed presets

Current task board-state semantics:
- Canonical task statuses are `backlog`, `ready`, `in_progress`, `in_review`, `blocked`, `done`, and `cancelled`.
- `task.create` defaults new tasks to `ready` when no status is provided.
- `task.ready` returns unarchived `ready` tasks whose blockers are already `done` (or archived).
- `task.close` transitions a task to `done`.
- `task.cancel` requires `in_progress` and transitions the task to `cancelled`.
- `task.retry` requires `cancelled` and transitions the task back to `ready`.
- `task.delete` is limited to terminal tasks in `done` or `cancelled`.
- Task records may also carry nullable worker ownership metadata: `assigneeWorkerId`, `claimedAt`, and `leaseExpiresAt`.
- `task.create`, `task.get`, and `task.update` surface those ownership/lease fields directly, and the same serde snapshot persists them in `.ralph/api/tasks-v1.json` when present.
- `task.update` accepts either a string or explicit `null` for those fields; malformed JSON types are rejected at the RPC boundary with `INVALID_PARAMS` instead of being silently ignored.

Worker lifecycle semantics:
- Workers are registered via `worker.register` with a unique `workerId`, `workerName`, `loopId`, `backend`, `workspaceRoot`, and `lastHeartbeatAt`.
- `worker.list` returns all registered workers. `worker.get` returns a single worker by ID (404 if unknown).
- `worker.deregister` removes a worker from the registry (404 if unknown).
- `worker.heartbeat` updates a worker's `status`, `currentTaskId`, `currentHat`, and `lastHeartbeatAt`. Returns the updated record (404 if unknown).
- Worker statuses are `idle`, `busy`, and `dead`.
- `worker.claim_next` assigns the highest-priority unblocked `ready` task to an `idle` worker. The task transitions to `in_progress` with `assigneeWorkerId`, `claimedAt`, and `leaseExpiresAt` set. The worker transitions to `busy`. Returns `{ task, worker }` where `task` is `null` if no ready tasks exist.
- `worker.reclaim_expired` accepts an `asOf` ISO 8601 timestamp and reclaims tasks from workers whose lease has expired. Reclaimed tasks return to `ready` with ownership fields cleared. Expired workers transition to `dead`. Returns `{ tasks, workers }` listing affected records.
- All mutating worker methods (`register`, `deregister`, `heartbeat`, `claim_next`, `reclaim_expired`) require `meta.idempotencyKey` in the RPC envelope.

Intentional migration differences vs legacy Node backend:
- `planning.start` returns a full `session` object instead of just `{sessionId}`.

## Run locally

From repository root:

```bash
cargo run -p ralph-api
```

For the MCP server:

```bash
./target/debug/ralph mcp serve --workspace-root /path/to/repo
```

The MCP server is workspace-scoped. One server instance manages one workspace root for
`ralph.yml`, `.ralph/api/*`, loops, planning sessions, and collections.

Environment variables:

- `RALPH_API_HOST` (default: `127.0.0.1`)
- `RALPH_API_PORT` (default: `3000`)
- `RALPH_API_SERVED_BY` (default: `ralph-api`)
- `RALPH_API_AUTH_MODE` (`trusted_local` or `token`, default: `trusted_local`)
  - `trusted_local` is restricted to loopback hosts (`127.0.0.1`, `::1`, `localhost`)
- `RALPH_API_TOKEN` (required for practical token auth use)
- `RALPH_API_IDEMPOTENCY_TTL_SECS` (default: `3600`)
- `RALPH_API_WORKSPACE_ROOT` (default: current working directory)
- `RALPH_API_LOOP_PROCESS_INTERVAL_MS` (default: `30000`)
- `RALPH_API_RALPH_COMMAND` (default: `ralph`; command used for loop-side-effect parity flows like `loop.retry`)

## Smoke call examples

Health:

```bash
curl -s http://127.0.0.1:3000/health | jq .
```

RPC system health:

```bash
curl -s http://127.0.0.1:3000/rpc/v1 \
  -H 'content-type: application/json' \
  -d '{
    "apiVersion": "v1",
    "id": "req-health-1",
    "method": "system.health",
    "params": {}
  }' | jq .
```
