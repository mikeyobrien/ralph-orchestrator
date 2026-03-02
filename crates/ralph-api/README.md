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
  - Full `task.*` family (`list/get/ready/create/update/close/archive/unarchive/delete/clear/run/run_all/retry/cancel/status`)
  - Full `loop.*` family (`list/status/process/prune/retry/discard/stop/merge/merge_button_state/trigger_merge_task`)
  - Full `planning.*` family (`list/get/start/respond/resume/delete/get_artifact`)
  - Full `config.*` family (`get/update`)
  - Full `preset.*` family (`list`)
  - Full `collection.*` family (`list/get/create/update/delete/import/export`)

Persistence notes:
- `task.*` data is persisted in `.ralph/api/tasks-v1.json`
- `loop.*` reads/writes `.ralph/loops.json` and `.ralph/merge-queue.jsonl` via `ralph-core`
- `planning.*` data is persisted under `.ralph/planning-sessions/<session-id>/`
- `collection.*` data is persisted in `.ralph/api/collections-v1.json`
- `config.*` reads/writes `ralph.yml` with YAML validation + atomic replace semantics
- `preset.list` reads builtins from `presets/`, local files from `.ralph/hats/`, and collection-backed presets

Intentional migration differences vs legacy Node backend:
- `task.cancel` currently allows cancelling `pending` tasks (legacy allowed only `running`).
- `planning.start` returns a full `session` object instead of just `{sessionId}`.

## Run locally

From repository root:

```bash
cargo run -p ralph-api
```

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
