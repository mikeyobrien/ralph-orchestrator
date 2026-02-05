/**
 * PersistentTaskQueueService Tests
 *
 * Unit tests for database-backed task queue
 */

import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert";
import { PersistentTaskQueueService } from "./PersistentTaskQueueService";
import { QueuedTaskRepository } from "../repositories/QueuedTaskRepository";
import { TaskState } from "./TaskState";
import { getDatabase, initializeDatabase, closeDatabase } from "../db/connection";

describe("PersistentTaskQueueService", () => {
  let service: PersistentTaskQueueService;
  let repository: QueuedTaskRepository;

  beforeEach(() => {
    // Close any existing connection
    closeDatabase();

    // Initialize in-memory database for testing
    const db = getDatabase(":memory:");
    initializeDatabase(db);
    repository = new QueuedTaskRepository(db);
    service = new PersistentTaskQueueService(repository);
  });

  afterEach(() => {
    closeDatabase();
  });

  it("should persist enqueued tasks to database", () => {
    const task = service.enqueue({
      taskType: "test-task",
      payload: { foo: "bar" },
      priority: 3,
    });

    // Verify task exists in database
    const dbTask = repository.findById(task.id);
    assert.ok(dbTask, "Task should exist in database");
    assert.strictEqual(dbTask.taskType, "test-task");
    assert.strictEqual(dbTask.state, "pending");
    assert.strictEqual(dbTask.priority, 3);
    assert.strictEqual(JSON.parse(dbTask.payload).foo, "bar");
  });

  it("should persist state transitions to database", () => {
    const task = service.enqueue({
      taskType: "test-task",
      payload: {},
    });

    // Transition to RUNNING
    service.transitionState(task.id, TaskState.RUNNING);
    let dbTask = repository.findById(task.id);
    assert.strictEqual(dbTask?.state, "running");
    assert.ok(dbTask?.startedAt);

    // Transition to COMPLETED
    service.transitionState(task.id, TaskState.COMPLETED);
    dbTask = repository.findById(task.id);
    assert.strictEqual(dbTask?.state, "completed");
    assert.ok(dbTask?.completedAt);
  });

  it("should hydrate pending tasks from database", () => {
    // Create tasks directly in database
    repository.create({
      id: "task-1",
      taskType: "test-1",
      payload: "{}",
      state: "pending",
      priority: 5,
      retryCount: 0,
      dbTaskId: null,
    });

    repository.create({
      id: "task-2",
      taskType: "test-2",
      payload: "{}",
      state: "pending",
      priority: 3,
      retryCount: 0,
      dbTaskId: null,
    });

    // Hydrate into memory
    const count = service.hydrate();
    assert.strictEqual(count, 2, "Should restore 2 pending tasks");

    // Verify tasks are in memory
    const pending = service.getPendingTasks();
    assert.strictEqual(pending.length, 2);
    assert.ok(pending.find((t) => t.id === "task-1"));
    assert.ok(pending.find((t) => t.id === "task-2"));
  });

  it("should recover crashed running tasks", () => {
    // Create running task in database (simulating crash)
    repository.create({
      id: "crashed-task",
      taskType: "test-task",
      payload: "{}",
      state: "running",
      priority: 5,
      retryCount: 0,
      dbTaskId: null,
    });

    // Recover crashed tasks
    const count = service.recoverCrashed();
    assert.strictEqual(count, 1, "Should recover 1 crashed task");

    // Verify task is marked as failed
    const task = service.getTask("crashed-task");
    assert.strictEqual(task?.state, TaskState.FAILED);
    assert.strictEqual(task?.error, "Process died during server restart");

    // Verify database is updated
    const dbTask = repository.findById("crashed-task");
    assert.strictEqual(dbTask?.state, "failed");
    assert.strictEqual(dbTask?.error, "Process died during server restart");
  });

  it("should not hydrate non-pending tasks", () => {
    // Create completed task
    repository.create({
      id: "completed-task",
      taskType: "test-task",
      payload: "{}",
      state: "completed",
      priority: 5,
      retryCount: 0,
      dbTaskId: null,
    });

    const count = service.hydrate();
    assert.strictEqual(count, 0, "Should not restore completed tasks");

    const pending = service.getPendingTasks();
    assert.strictEqual(pending.length, 0);
  });
});
