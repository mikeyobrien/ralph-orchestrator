import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { initializeDatabase, getDatabase } from "../db/connection";
import { queuedTasks } from "../db/schema";
import { QueuedTaskRepository } from "./QueuedTaskRepository";

describe("QueuedTaskRepository", () => {
  let repo: QueuedTaskRepository;

  beforeEach(() => {
    initializeDatabase(getDatabase(":memory:"));
    const db = getDatabase();
    db.delete(queuedTasks).run();
    repo = new QueuedTaskRepository(db);
  });

  describe("create", () => {
    it("should create a task with auto-generated enqueuedAt", () => {
      const task = repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });

      assert.equal(task.id, "task-1");
      assert.equal(task.state, "pending");
      assert.ok(task.enqueuedAt instanceof Date);
    });
  });

  describe("findById", () => {
    it("should return task when found", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      const task = repo.findById("task-1");
      assert.equal(task?.id, "task-1");
    });

    it("should return undefined when not found", () => {
      const task = repo.findById("nonexistent");
      assert.equal(task, undefined);
    });
  });

  describe("findAll", () => {
    it("should return all tasks", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      repo.create({
        id: "task-2",
        taskType: "hats-run",
        payload: "{}",
        state: "running",
        priority: 3,
        retryCount: 0,
      });
      const tasks = repo.findAll();
      assert.equal(tasks.length, 2);
    });
  });

  describe("findByState", () => {
    it("should return tasks matching state", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      repo.create({
        id: "task-2",
        taskType: "hats-run",
        payload: "{}",
        state: "running",
        priority: 3,
        retryCount: 0,
      });
      const pending = repo.findByState("pending");
      assert.equal(pending.length, 1);
      assert.equal(pending[0].id, "task-1");
    });
  });

  describe("update", () => {
    it("should update existing task", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      const updated = repo.update("task-1", { state: "running" });
      assert.equal(updated?.state, "running");
    });

    it("should return undefined for nonexistent task", () => {
      const updated = repo.update("nonexistent", { state: "running" });
      assert.equal(updated, undefined);
    });
  });

  describe("markRunning", () => {
    it("should transition to running with startedAt", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      const updated = repo.markRunning("task-1");
      assert.equal(updated?.state, "running");
      assert.ok(updated?.startedAt instanceof Date);
    });
  });

  describe("markCompleted", () => {
    it("should transition to completed with completedAt", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "running",
        priority: 5,
        retryCount: 0,
      });
      const updated = repo.markCompleted("task-1");
      assert.equal(updated?.state, "completed");
      assert.ok(updated?.completedAt instanceof Date);
    });
  });

  describe("markFailed", () => {
    it("should transition to failed with error and completedAt", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "running",
        priority: 5,
        retryCount: 0,
      });
      const updated = repo.markFailed("task-1", "Test error");
      assert.equal(updated?.state, "failed");
      assert.equal(updated?.error, "Test error");
      assert.ok(updated?.completedAt instanceof Date);
    });
  });

  describe("incrementRetryCount", () => {
    it("should increment retry count", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      const updated = repo.incrementRetryCount("task-1");
      assert.equal(updated?.retryCount, 1);
    });

    it("should return undefined for nonexistent task", () => {
      const updated = repo.incrementRetryCount("nonexistent");
      assert.equal(updated, undefined);
    });
  });

  describe("delete", () => {
    it("should delete existing task", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      const deleted = repo.delete("task-1");
      assert.equal(deleted, true);
      assert.equal(repo.findById("task-1"), undefined);
    });

    it("should return false for nonexistent task", () => {
      const deleted = repo.delete("nonexistent");
      assert.equal(deleted, false);
    });
  });

  describe("deleteAll", () => {
    it("should delete all tasks and return count", () => {
      repo.create({
        id: "task-1",
        taskType: "hats-run",
        payload: "{}",
        state: "pending",
        priority: 5,
        retryCount: 0,
      });
      repo.create({
        id: "task-2",
        taskType: "hats-run",
        payload: "{}",
        state: "running",
        priority: 3,
        retryCount: 0,
      });
      const count = repo.deleteAll();
      assert.equal(count, 2);
      assert.equal(repo.findAll().length, 0);
    });
  });
});
