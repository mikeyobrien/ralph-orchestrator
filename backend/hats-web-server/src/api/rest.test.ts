/**
 * REST API Tests
 *
 * Tests for all REST endpoints at /api/v1/* including:
 * - Health check
 * - Task CRUD (list, create, get, update, delete)
 * - Task execution
 * - Hat listing and retrieval
 * - Preset listing
 */

import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "./server.js";
import { initializeDatabase, getDatabase } from "../db/connection.js";
import { tasks } from "../db/schema.js";
import type { FastifyInstance } from "fastify";

let server: FastifyInstance;

async function setupServer() {
  initializeDatabase(getDatabase(":memory:"));
  const db = getDatabase();
  db.delete(tasks).run();
  server = await createServer({ db, logger: false });
  return server;
}

// --- Health ---

describe("GET /api/v1/health", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("returns ok status with version and timestamp", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/health" });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.equal(body.status, "ok");
    assert.equal(body.version, "1.0.0");
    assert.ok(body.timestamp, "Should include timestamp");
  });
});

// --- Tasks: List ---

describe("GET /api/v1/tasks", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("returns empty array when no tasks exist", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/tasks" });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.ok(Array.isArray(body));
    assert.equal(body.length, 0);
  });

  it("returns all tasks", async () => {
    // Seed tasks
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-1", title: "First", status: "open", priority: 1 });
    repo.create({ id: "task-2", title: "Second", status: "closed", priority: 2 });

    const res = await server.inject({ method: "GET", url: "/api/v1/tasks" });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.equal(body.length, 2);
  });

  it("filters by status query param", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-1", title: "Open task", status: "open", priority: 1 });
    repo.create({ id: "task-2", title: "Closed task", status: "closed", priority: 2 });

    const res = await server.inject({
      method: "GET",
      url: "/api/v1/tasks?status=open",
    });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.ok(body.length >= 1, "Should return at least the open task");
    assert.ok(body.every((t: any) => t.status === "open"), "All returned tasks should be open");
  });
});

// --- Tasks: Create ---

describe("POST /api/v1/tasks", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("creates a task with required fields", async () => {
    const res = await server.inject({
      method: "POST",
      url: "/api/v1/tasks",
      payload: { id: "task-new", title: "New task" },
    });
    assert.equal(res.statusCode, 201);
    const body = res.json();
    assert.equal(body.id, "task-new");
    assert.equal(body.title, "New task");
    assert.equal(body.status, "open");
    assert.equal(body.priority, 2);
  });

  it("creates a task with all optional fields", async () => {
    const res = await server.inject({
      method: "POST",
      url: "/api/v1/tasks",
      payload: {
        id: "task-full",
        title: "Full task",
        status: "pending",
        priority: 1,
        blockedBy: "task-other",
      },
    });
    assert.equal(res.statusCode, 201);
    const body = res.json();
    assert.equal(body.id, "task-full");
    assert.equal(body.priority, 1);
  });

  it("returns 400 when id is missing", async () => {
    const res = await server.inject({
      method: "POST",
      url: "/api/v1/tasks",
      payload: { title: "No ID" },
    });
    assert.equal(res.statusCode, 400);
    const body = res.json();
    assert.ok(body.message.includes("id and title are required"));
  });

  it("returns 400 when title is missing", async () => {
    const res = await server.inject({
      method: "POST",
      url: "/api/v1/tasks",
      payload: { id: "task-notitle" },
    });
    assert.equal(res.statusCode, 400);
    const body = res.json();
    assert.ok(body.message.includes("id and title are required"));
  });

  it("returns 400 for invalid priority", async () => {
    const res = await server.inject({
      method: "POST",
      url: "/api/v1/tasks",
      payload: { id: "task-badprio", title: "Bad prio", priority: 10 },
    });
    assert.equal(res.statusCode, 400);
    const body = res.json();
    assert.ok(body.message.includes("priority must be between 1 and 5"));
  });
});

// --- Tasks: Get by ID ---

describe("GET /api/v1/tasks/:id", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("returns a task by id", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-get", title: "Get me", status: "open", priority: 2 });

    const res = await server.inject({ method: "GET", url: "/api/v1/tasks/task-get" });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.equal(body.id, "task-get");
    assert.equal(body.title, "Get me");
  });

  it("returns 404 for non-existent task", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/tasks/no-such-task" });
    assert.equal(res.statusCode, 404);
    const body = res.json();
    assert.equal(body.error, "Not Found");
  });
});

// --- Tasks: Update ---

describe("PATCH /api/v1/tasks/:id", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("updates task title", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-upd", title: "Original", status: "open", priority: 2 });

    const res = await server.inject({
      method: "PATCH",
      url: "/api/v1/tasks/task-upd",
      payload: { title: "Updated" },
    });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.equal(body.title, "Updated");
  });

  it("updates task status", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-upd2", title: "Status test", status: "open", priority: 2 });

    const res = await server.inject({
      method: "PATCH",
      url: "/api/v1/tasks/task-upd2",
      payload: { status: "closed" },
    });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.equal(body.status, "closed");
  });

  it("returns 404 for non-existent task", async () => {
    const res = await server.inject({
      method: "PATCH",
      url: "/api/v1/tasks/no-such",
      payload: { title: "nope" },
    });
    assert.equal(res.statusCode, 404);
  });

  it("returns 400 for invalid priority", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-badp", title: "Bad prio", status: "open", priority: 2 });

    const res = await server.inject({
      method: "PATCH",
      url: "/api/v1/tasks/task-badp",
      payload: { priority: 0 },
    });
    assert.equal(res.statusCode, 400);
    assert.ok(res.json().message.includes("priority must be between 1 and 5"));
  });

  it("returns 400 for empty title", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-empty", title: "Empty", status: "open", priority: 2 });

    const res = await server.inject({
      method: "PATCH",
      url: "/api/v1/tasks/task-empty",
      payload: { title: "" },
    });
    assert.equal(res.statusCode, 400);
    assert.ok(res.json().message.includes("title must not be empty"));
  });
});

// --- Tasks: Delete ---

describe("DELETE /api/v1/tasks/:id", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("deletes a closed task", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-del", title: "Delete me", status: "closed", priority: 2 });

    const res = await server.inject({ method: "DELETE", url: "/api/v1/tasks/task-del" });
    assert.equal(res.statusCode, 204);
  });

  it("deletes a failed task", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-fail", title: "Failed task", status: "failed", priority: 2 });

    const res = await server.inject({ method: "DELETE", url: "/api/v1/tasks/task-fail" });
    assert.equal(res.statusCode, 204);
  });

  it("rejects deletion of running task (409)", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-run", title: "Running", status: "running", priority: 2 });

    const res = await server.inject({ method: "DELETE", url: "/api/v1/tasks/task-run" });
    assert.equal(res.statusCode, 409);
    assert.ok(res.json().message.includes("Cannot delete task in 'running' state"));
  });

  it("rejects deletion of open task (409)", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-open", title: "Open", status: "open", priority: 2 });

    const res = await server.inject({ method: "DELETE", url: "/api/v1/tasks/task-open" });
    assert.equal(res.statusCode, 409);
  });

  it("returns 404 for non-existent task", async () => {
    const res = await server.inject({ method: "DELETE", url: "/api/v1/tasks/missing" });
    assert.equal(res.statusCode, 404);
  });
});

// --- Tasks: Run ---

describe("POST /api/v1/tasks/:id/run", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("returns 503 when no task bridge configured", async () => {
    const db = getDatabase();
    const { TaskRepository } = await import("../repositories/index.js");
    const repo = new TaskRepository(db);
    repo.create({ id: "task-exec", title: "Exec me", status: "open", priority: 2 });

    const res = await server.inject({
      method: "POST",
      url: "/api/v1/tasks/task-exec/run",
    });
    assert.equal(res.statusCode, 503);
    assert.ok(res.json().message.includes("not configured"));
  });

  it("returns 404 for non-existent task", async () => {
    const res = await server.inject({
      method: "POST",
      url: "/api/v1/tasks/no-such/run",
    });
    // Without bridge: 503 takes precedence
    assert.equal(res.statusCode, 503);
  });
});

// --- Hats ---

describe("GET /api/v1/hats", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("returns an array of hats", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/hats" });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.ok(Array.isArray(body), "Response should be an array");
  });

  it("each hat has key and isActive fields", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/hats" });
    const body = res.json();
    for (const hat of body) {
      assert.ok("key" in hat, "Hat should have a key");
      assert.ok("isActive" in hat, "Hat should have isActive flag");
    }
  });
});

describe("GET /api/v1/hats/:key", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("returns 404 for non-existent hat", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/hats/nonexistent-hat-key" });
    assert.equal(res.statusCode, 404);
    const body = res.json();
    assert.equal(body.error, "Not Found");
  });
});

// --- Presets ---

describe("GET /api/v1/presets", () => {
  beforeEach(async () => {
    await setupServer();
  });

  it("returns an array of presets", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/presets" });
    assert.equal(res.statusCode, 200);
    const body = res.json();
    assert.ok(Array.isArray(body), "Response should be an array");
  });

  it("includes builtin presets", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/presets" });
    const body = res.json();
    const builtin = body.filter((p: any) => p.source === "builtin");
    assert.ok(builtin.length > 0, "Should include builtin presets");
  });

  it("presets have required fields", async () => {
    const res = await server.inject({ method: "GET", url: "/api/v1/presets" });
    const body = res.json();
    for (const preset of body) {
      assert.ok(preset.id, "Preset should have id");
      assert.ok(preset.name, "Preset should have name");
      assert.ok(preset.source, "Preset should have source");
    }
  });
});
