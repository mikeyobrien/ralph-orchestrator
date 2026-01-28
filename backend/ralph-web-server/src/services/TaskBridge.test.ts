import { test, describe } from "node:test";
import assert from "node:assert";
import stripAnsi from "strip-ansi";
import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import { TaskBridge } from "./TaskBridge";
import { CollectionService } from "./CollectionService";
import { TaskRepository, CollectionRepository } from "../repositories";
import { TaskQueueService, EventBus, QueuedTask } from "../queue";
import { initializeDatabase, getDatabase } from "../db/connection";
import { tasks } from "../db/schema";

test("strip-ansi removes ANSI codes", () => {
  const input = "\u001B[4mHello World\u001B[0m";
  const expected = "Hello World";
  const actual = stripAnsi(input);
  assert.strictEqual(actual, expected);
});

test("strip-ansi handles plain text", () => {
  const input = "Hello World";
  const actual = stripAnsi(input);
  assert.strictEqual(actual, input);
});

// ============================================================================
// cancelTask tests - Ensure task cancellation handles edge cases correctly
// ============================================================================

describe("cancelTask", () => {
  test("handles 'Process already terminated' by updating status and returning success", () => {
    initializeDatabase(getDatabase(":memory:"));
    const db = getDatabase();
    db.delete(tasks).run();

    const taskRepository = new TaskRepository(db);
    const taskQueue = new TaskQueueService();
    const eventBus = new EventBus();

    // Create a mock process supervisor that returns "Process already terminated"
    const mockProcessSupervisor = {
      spawn: () => ({ success: true, pid: 1234 }),
      stop: () => ({ success: false, error: "Process already terminated" }),
      isRunning: () => false,
    };

    const taskBridge = new TaskBridge(taskRepository, taskQueue, eventBus, {
      defaultCwd: process.cwd(),
      processSupervisor: mockProcessSupervisor as never,
    });

    // Create a running task
    const task = taskRepository.create({
      id: "task-terminated",
      title: "Task with terminated process",
      status: "running",
      priority: 1,
    });

    // Cancel the task - should succeed even though process is already terminated
    const result = taskBridge.cancelTask(task.id);

    assert.strictEqual(result.success, true, "Should return success: true");
    assert.strictEqual(result.error, undefined, "Should not have error");

    // Verify task status was updated to failed
    const updatedTask = taskRepository.findById(task.id);
    assert.strictEqual(updatedTask?.status, "failed", "Task should be failed");
    assert.strictEqual(
      updatedTask?.errorMessage,
      "Process terminated unexpectedly",
      "Should have correct error message"
    );
    assert.strictEqual(updatedTask?.exitCode, -1, "Should have exit code -1");
    assert.ok(updatedTask?.completedAt, "Should have completedAt set");
  });

  test("returns fallback error message when stop returns no error property", () => {
    initializeDatabase(getDatabase(":memory:"));
    const db = getDatabase();
    db.delete(tasks).run();

    const taskRepository = new TaskRepository(db);
    const taskQueue = new TaskQueueService();
    const eventBus = new EventBus();

    // Create a mock process supervisor that returns success: false with no error property
    const mockProcessSupervisor = {
      spawn: () => ({ success: true, pid: 1234 }),
      stop: () => ({ success: false }), // No error property
      isRunning: () => true,
    };

    const taskBridge = new TaskBridge(taskRepository, taskQueue, eventBus, {
      defaultCwd: process.cwd(),
      processSupervisor: mockProcessSupervisor as never,
    });

    // Create a running task
    const task = taskRepository.create({
      id: "task-no-error-prop",
      title: "Task with no error property",
      status: "running",
      priority: 1,
    });

    // Cancel the task - should fail with fallback message
    const result = taskBridge.cancelTask(task.id);

    assert.strictEqual(result.success, false, "Should return success: false");
    assert.strictEqual(result.error, "Failed to stop process", "Should use fallback error message");

    // Verify task status was NOT updated
    const updatedTask = taskRepository.findById(task.id);
    assert.strictEqual(updatedTask?.status, "running", "Task should still be running");
  });

  test("returns error for other stop failures", () => {
    initializeDatabase(getDatabase(":memory:"));
    const db = getDatabase();
    db.delete(tasks).run();

    const taskRepository = new TaskRepository(db);
    const taskQueue = new TaskQueueService();
    const eventBus = new EventBus();

    // Create a mock process supervisor that returns a different error
    const mockProcessSupervisor = {
      spawn: () => ({ success: true, pid: 1234 }),
      stop: () => ({ success: false, error: "Permission denied" }),
      isRunning: () => true,
    };

    const taskBridge = new TaskBridge(taskRepository, taskQueue, eventBus, {
      defaultCwd: process.cwd(),
      processSupervisor: mockProcessSupervisor as never,
    });

    // Create a running task
    const task = taskRepository.create({
      id: "task-permission-denied",
      title: "Task with permission error",
      status: "running",
      priority: 1,
    });

    // Cancel the task - should fail with the error
    const result = taskBridge.cancelTask(task.id);

    assert.strictEqual(result.success, false, "Should return success: false");
    assert.strictEqual(result.error, "Permission denied", "Should propagate error");

    // Verify task status was NOT updated
    const updatedTask = taskRepository.findById(task.id);
    assert.strictEqual(updatedTask?.status, "running", "Task should still be running");
  });
});

test("recoverStuckTasks marks running tasks as failed", async () => {
  // Setup in-memory DB
  initializeDatabase(getDatabase(":memory:"));
  const db = getDatabase();

  // Clean up
  db.delete(tasks).run();

  const taskRepository = new TaskRepository(db);
  const taskQueue = new TaskQueueService();
  const eventBus = new EventBus();

  const taskBridge = new TaskBridge(taskRepository, taskQueue, eventBus, {
    defaultCwd: process.cwd(),
  });

  // Create a stuck task
  const now = new Date();
  const stuckTask = {
    id: "task-stuck",
    title: "Stuck Task",
    status: "running",
    priority: 1,
    createdAt: now,
    updatedAt: now,
    startedAt: now,
  };

  // Manually insert stuck task
  db.insert(tasks).values(stuckTask).run();

  // Create a normal task
  taskRepository.create({
    id: "task-normal",
    title: "Normal Task",
    status: "open",
    priority: 1,
  });

  // Run recovery
  const recoveredCount = taskBridge.recoverStuckTasks();

  // Assertions
  assert.strictEqual(recoveredCount, 1, "Should recover 1 task");

  const updatedStuckTask = taskRepository.findById("task-stuck");
  assert.strictEqual(updatedStuckTask?.status, "failed", "Stuck task should be failed");
  assert.ok(
    updatedStuckTask?.errorMessage?.includes("Server restarted"),
    "Should have error message"
  );

  const normalTask = taskRepository.findById("task-normal");
  assert.strictEqual(normalTask?.status, "open", "Normal task should be untouched");
});

// ============================================================================
// Preset handling tests - Regression tests for hat collection dropdown bug
// ============================================================================

describe("enqueueTask preset handling", () => {
  function setupTest(defaultConfigPath?: string) {
    initializeDatabase(getDatabase(":memory:"));
    const db = getDatabase();
    db.delete(tasks).run();

    const taskRepository = new TaskRepository(db);
    const taskQueue = new TaskQueueService();
    const eventBus = new EventBus();
    const defaultCwd = "/test/cwd";

    const taskBridge = new TaskBridge(taskRepository, taskQueue, eventBus, {
      defaultCwd,
      defaultConfigPath,
    });

    return { taskRepository, taskQueue, eventBus, taskBridge, defaultCwd };
  }

  function setupTestWithCollection(defaultConfigPath?: string) {
    initializeDatabase(getDatabase(":memory:"));
    const db = getDatabase();
    db.delete(tasks).run();

    const taskRepository = new TaskRepository(db);
    const collectionRepository = new CollectionRepository(db);
    const collectionService = new CollectionService(collectionRepository);
    const taskQueue = new TaskQueueService();
    const eventBus = new EventBus();
    // Use a temp directory for defaultCwd so we can write temp files
    const defaultCwd = fs.mkdtempSync(path.join(os.tmpdir(), "taskbridge-test-"));

    const taskBridge = new TaskBridge(taskRepository, taskQueue, eventBus, {
      defaultCwd,
      defaultConfigPath,
      collectionService,
    });

    return { taskRepository, collectionRepository, collectionService, taskQueue, eventBus, taskBridge, defaultCwd, db };
  }

  test("builtin preset passes full builtin:name format to args", async () => {
    const { taskRepository, taskQueue, taskBridge } = setupTest();

    // Create a task
    const task = taskRepository.create({
      id: "task-builtin-preset",
      title: "Test task with builtin preset",
      status: "open",
      priority: 2,
    });

    // Enqueue with builtin preset
    const result = taskBridge.enqueueTask(task, "builtin:feature");

    // Verify enqueue succeeded
    assert.strictEqual(result.success, true, "Enqueue should succeed");
    assert.ok(result.queuedTaskId, "Should have queued task ID");

    // Get the queued task and verify args
    const queuedTask = taskQueue.getTask(result.queuedTaskId!) as QueuedTask;
    assert.ok(queuedTask, "Queued task should exist");

    const payload = queuedTask.payload as { args?: string[] };
    assert.ok(payload.args, "Payload should have args");
    assert.deepStrictEqual(
      payload.args,
      ["-c", "builtin:feature"],
      "Args should contain full builtin:name format"
    );
  });

  test("directory preset resolves to .ralph/hats/ file path", async () => {
    const { taskRepository, taskQueue, taskBridge, defaultCwd } = setupTest();

    // Create a task
    const task = taskRepository.create({
      id: "task-directory-preset",
      title: "Test task with directory preset",
      status: "open",
      priority: 2,
    });

    // Enqueue with directory preset
    const result = taskBridge.enqueueTask(task, "directory:my-preset");

    // Verify enqueue succeeded
    assert.strictEqual(result.success, true, "Enqueue should succeed");
    assert.ok(result.queuedTaskId, "Should have queued task ID");

    // Get the queued task and verify args
    const queuedTask = taskQueue.getTask(result.queuedTaskId!) as QueuedTask;
    assert.ok(queuedTask, "Queued task should exist");

    const payload = queuedTask.payload as { args?: string[] };
    assert.ok(payload.args, "Payload should have args");

    const expectedPath = path.join(defaultCwd, ".ralph", "hats", "my-preset.yml");
    assert.deepStrictEqual(
      payload.args,
      ["-c", expectedPath],
      "Args should contain resolved file path"
    );
  });

  test("collection preset (UUID) exports to temp file and uses that path", async () => {
    const { taskRepository, collectionRepository, taskQueue, taskBridge, defaultCwd } =
      setupTestWithCollection("/default/config.yml");

    // Create a collection with some hats
    const collection = collectionRepository.create({
      name: "Test Collection",
      description: "A test hat collection",
      graph: {
        nodes: [
          {
            id: "planner",
            type: "hatNode",
            position: { x: 100, y: 100 },
            data: {
              key: "planner",
              name: "Planner",
              description: "Plans the work",
              triggersOn: ["task.start"],
              publishes: ["plan.done"],
            },
          },
          {
            id: "builder",
            type: "hatNode",
            position: { x: 100, y: 200 },
            data: {
              key: "builder",
              name: "Builder",
              description: "Builds the code",
              triggersOn: ["plan.done"],
              publishes: ["build.done"],
            },
          },
        ],
        edges: [
          {
            id: "edge-1",
            source: "planner",
            target: "builder",
            label: "plan.done",
          },
        ],
        viewport: { x: 0, y: 0, zoom: 1 },
      },
    });

    // Create a task
    const task = taskRepository.create({
      id: "task-collection-preset",
      title: "Test task with collection preset",
      status: "open",
      priority: 2,
    });

    // Enqueue with collection UUID preset
    const result = taskBridge.enqueueTask(task, collection.id);

    // Verify enqueue succeeded
    assert.strictEqual(result.success, true, "Enqueue should succeed");
    assert.ok(result.queuedTaskId, "Should have queued task ID");

    // Get the queued task and verify it uses the exported temp file
    const queuedTask = taskQueue.getTask(result.queuedTaskId!) as QueuedTask;
    assert.ok(queuedTask, "Queued task should exist");

    const payload = queuedTask.payload as { args?: string[] };
    assert.ok(payload.args, "Payload should have args");
    assert.strictEqual(payload.args.length, 2, "Should have -c and path");
    assert.strictEqual(payload.args[0], "-c", "First arg should be -c");

    // Verify the path is a temp file in .ralph/temp/
    const configPath = payload.args[1];
    assert.ok(
      configPath.includes(".ralph/temp/collection-"),
      `Config path should be in .ralph/temp/: ${configPath}`
    );
    assert.ok(configPath.endsWith(".yml"), "Config path should end with .yml");

    // Verify the temp file was actually created
    assert.ok(fs.existsSync(configPath), `Temp config file should exist: ${configPath}`);

    // Verify the content includes the hat names from our collection
    const content = fs.readFileSync(configPath, "utf-8");
    assert.ok(content.includes("Planner"), "Config should include Planner hat");
    assert.ok(content.includes("Builder"), "Config should include Builder hat");

    // Cleanup temp directory
    fs.rmSync(defaultCwd, { recursive: true, force: true });
  });

  test("collection preset with nonexistent UUID falls back to default config", async () => {
    const { taskRepository, taskQueue, taskBridge, defaultCwd } =
      setupTestWithCollection("/default/config.yml");

    // Create a task
    const task = taskRepository.create({
      id: "task-nonexistent-collection",
      title: "Test task with nonexistent collection",
      status: "open",
      priority: 2,
    });

    // Enqueue with a UUID that doesn't exist in the database
    const result = taskBridge.enqueueTask(task, "550e8400-e29b-41d4-a716-446655440000");

    // Verify enqueue succeeded
    assert.strictEqual(result.success, true, "Enqueue should succeed");
    assert.ok(result.queuedTaskId, "Should have queued task ID");

    // Get the queued task and verify it falls back to default config
    const queuedTask = taskQueue.getTask(result.queuedTaskId!) as QueuedTask;
    assert.ok(queuedTask, "Queued task should exist");

    const payload = queuedTask.payload as { args?: string[] };
    assert.deepStrictEqual(
      payload.args,
      ["-c", "/default/config.yml"],
      "Nonexistent collection should fall back to default config"
    );

    // Cleanup temp directory
    fs.rmSync(defaultCwd, { recursive: true, force: true });
  });

  test("no preset with default config uses default config", async () => {
    const { taskRepository, taskQueue, taskBridge } = setupTest("/default/config.yml");

    // Create a task
    const task = taskRepository.create({
      id: "task-no-preset-with-default",
      title: "Test task without preset but with default config",
      status: "open",
      priority: 2,
    });

    // Enqueue without preset
    const result = taskBridge.enqueueTask(task);

    // Verify enqueue succeeded
    assert.strictEqual(result.success, true, "Enqueue should succeed");

    // Get the queued task and verify it uses default config
    const queuedTask = taskQueue.getTask(result.queuedTaskId!) as QueuedTask;
    const payload = queuedTask.payload as { args?: string[] };
    assert.deepStrictEqual(
      payload.args,
      ["-c", "/default/config.yml"],
      "Should use default config when no preset specified"
    );
  });

  test("no preset without default config results in no args", async () => {
    const { taskRepository, taskQueue, taskBridge } = setupTest(); // No default config

    // Create a task
    const task = taskRepository.create({
      id: "task-no-preset-no-default",
      title: "Test task without preset or default config",
      status: "open",
      priority: 2,
    });

    // Enqueue without preset
    const result = taskBridge.enqueueTask(task);

    // Verify enqueue succeeded
    assert.strictEqual(result.success, true, "Enqueue should succeed");

    // Get the queued task and verify no args
    const queuedTask = taskQueue.getTask(result.queuedTaskId!) as QueuedTask;
    const payload = queuedTask.payload as { args?: string[] };
    assert.strictEqual(payload.args, undefined, "Should have no args when no preset and no default config");
  });

  test("builtin preset overrides default config", async () => {
    const { taskRepository, taskQueue, taskBridge } = setupTest("/default/config.yml");

    // Create a task
    const task = taskRepository.create({
      id: "task-builtin-override",
      title: "Test builtin preset overrides default",
      status: "open",
      priority: 2,
    });

    // Enqueue with builtin preset
    const result = taskBridge.enqueueTask(task, "builtin:feature");

    // Verify enqueue succeeded
    assert.strictEqual(result.success, true, "Enqueue should succeed");

    // Get the queued task and verify it uses builtin preset, NOT default config
    const queuedTask = taskQueue.getTask(result.queuedTaskId!) as QueuedTask;
    const payload = queuedTask.payload as { args?: string[] };
    assert.deepStrictEqual(
      payload.args,
      ["-c", "builtin:feature"],
      "Builtin preset should override default config"
    );
  });
});
