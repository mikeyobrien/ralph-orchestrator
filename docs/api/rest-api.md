# REST API Reference

Ralph Web Server exposes a REST API at `/api/v1/*` for external consumers that cannot use the tRPC client. The server runs on port 3000 by default.

## Base URL

```
http://localhost:3000/api/v1
```

## Endpoints

### Health

#### GET /api/v1/health

Returns server health status.

**Response** `200 OK`
```json
{
  "status": "ok",
  "version": "1.0.0",
  "timestamp": "2026-01-29T12:00:00.000Z"
}
```

---

### Tasks

#### GET /api/v1/tasks

List all tasks.

**Query Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `status` | string | Filter by status (`open`, `running`, `closed`, `failed`, `pending`) |
| `includeArchived` | string | Set to `"true"` to include archived tasks |

**Response** `200 OK`
```json
[
  {
    "id": "task-abc123",
    "title": "Implement feature",
    "status": "open",
    "priority": 2,
    "blockedBy": null,
    "preset": null,
    "currentIteration": null,
    "maxIterations": null,
    "loopId": null,
    "createdAt": "2026-01-29T10:00:00.000Z"
  }
]
```

#### POST /api/v1/tasks

Create a new task.

**Request Body**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique task identifier |
| `title` | string | Yes | Task title |
| `status` | string | No | Initial status (default: `"open"`) |
| `priority` | number | No | Priority 1-5, 1=highest (default: `2`) |
| `blockedBy` | string\|null | No | ID of blocking task |
| `autoExecute` | boolean | No | Auto-enqueue for execution if no blockers |
| `preset` | string | No | Associated preset name |

**Response** `201 Created`
```json
{
  "id": "task-abc123",
  "title": "Implement feature",
  "status": "open",
  "priority": 2
}
```

**Errors**
- `400` — Missing `id` or `title`, or `priority` out of range (1-5)

#### GET /api/v1/tasks/:id

Get a single task by ID.

**Response** `200 OK` — Task object (same shape as list items)

**Errors**
- `404` — Task not found

#### PATCH /api/v1/tasks/:id

Update an existing task.

**Request Body** (all fields optional)

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | New title (must not be empty) |
| `status` | string | New status |
| `priority` | number | New priority (1-5) |
| `blockedBy` | string\|null | Set or clear blocker |

**Response** `200 OK` — Updated task object

**Errors**
- `400` — Invalid priority or empty title
- `404` — Task not found

#### DELETE /api/v1/tasks/:id

Delete a task. Only tasks in `failed` or `closed` state can be deleted.

**Response** `204 No Content`

**Errors**
- `404` — Task not found
- `409` — Task is in a non-deletable state (e.g. `running`, `open`, `pending`)

#### POST /api/v1/tasks/:id/run

Enqueue a task for execution. Requires the TaskBridge to be configured on the server.

**Response** `200 OK`
```json
{
  "success": true,
  "queuedTaskId": "queued-xyz",
  "task": { ... }
}
```

**Errors**
- `400` — Failed to enqueue
- `404` — Task not found
- `503` — Task execution not configured (no TaskBridge)

---

### Hats

#### GET /api/v1/hats

List all hat definitions with their active status.

**Response** `200 OK`
```json
[
  {
    "key": "execution-lead",
    "name": "Execution Lead",
    "description": "Implements tasks and verifies results",
    "isActive": true
  }
]
```

#### GET /api/v1/hats/:key

Get a specific hat by its key.

**Response** `200 OK` — Hat object with `isActive` flag

**Errors**
- `404` — Hat not found

---

### Presets

#### GET /api/v1/presets

List all available presets from all sources.

Presets are returned in priority order:
1. **builtin** — Shipped with Ralph (from `presets/` directory)
2. **directory** — User-created (from `.ralph/hats/`)
3. **collection** — Database collections (created via Builder)

**Response** `200 OK`
```json
[
  {
    "id": "tdd-red-green",
    "name": "tdd-red-green",
    "source": "builtin",
    "description": "TDD workflow with red-green-refactor cycle"
  },
  {
    "id": "my-custom",
    "name": "my-custom",
    "source": "directory",
    "path": ".ralph/hats/my-custom.yml"
  },
  {
    "id": "uuid-abc-123",
    "name": "My Collection",
    "source": "collection",
    "description": "Custom hat collection"
  }
]
```

---

## Error Format

All error responses follow this structure:

```json
{
  "error": "Not Found",
  "message": "Task with id 'task-xyz' not found"
}
```

## Running the Server

```bash
ralph web                    # Launch backend (port 3000) and frontend (port 5173)
npm run dev:server           # Backend only in dev mode
```

## Authentication

The REST API does not currently require authentication. It is designed for local development use.
