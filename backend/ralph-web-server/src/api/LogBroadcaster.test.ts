/**
 * LogBroadcaster Tests
 *
 * Unit tests for WebSocket log broadcasting functionality.
 * Focuses on backlog delivery when subscribing to completed tasks.
 */

import { test, describe, beforeEach, afterEach, mock, Mock } from "node:test";
import assert from "node:assert";
import { LogBroadcaster, resetLogBroadcaster } from "./LogBroadcaster.js";
import { TaskLogRepository } from "../repositories/TaskLogRepository.js";
import { WebSocket, OPEN } from "ws";

// Helper type for mock function extraction
type MockFn = Mock<(...args: unknown[]) => unknown>;

/**
 * Create a mock WebSocket for testing.
 */
function createMockWebSocket(): WebSocket {
  const mockSocket = {
    readyState: OPEN,
    send: mock.fn(),
    on: mock.fn(),
    close: mock.fn(),
  } as unknown as WebSocket;
  return mockSocket;
}

/**
 * Create a mock TaskLogRepository for testing.
 */
function createMockLogRepository(logs: Array<{
  id: number;
  taskId: string;
  line: string;
  timestamp: Date;
  source: "stdout" | "stderr";
}>): TaskLogRepository {
  const mockRepo = {
    listByTaskId: mock.fn((taskId: string, options?: { afterId?: number }) => {
      return logs.filter((log) => {
        if (log.taskId !== taskId) return false;
        if (options?.afterId !== undefined && log.id <= options.afterId) return false;
        return true;
      });
    }),
    append: mock.fn(),
  } as unknown as TaskLogRepository;
  return mockRepo;
}

describe("LogBroadcaster", () => {
  let broadcaster: LogBroadcaster;

  afterEach(() => {
    resetLogBroadcaster();
  });

  describe("subscribe with backlog", () => {
    test("sends all backlog logs when subscribing to completed task with no sinceId", () => {
      // Given: A task with persisted logs (simulating completed task)
      const taskId = "completed-task-123";
      const persistedLogs = [
        { id: 1, taskId, line: "Starting task...", timestamp: new Date("2024-01-15T10:00:00Z"), source: "stdout" as const },
        { id: 2, taskId, line: "Processing...", timestamp: new Date("2024-01-15T10:00:01Z"), source: "stdout" as const },
        { id: 3, taskId, line: "Warning: something", timestamp: new Date("2024-01-15T10:00:02Z"), source: "stderr" as const },
        { id: 4, taskId, line: "Task complete", timestamp: new Date("2024-01-15T10:00:03Z"), source: "stdout" as const },
      ];
      const mockRepo = createMockLogRepository(persistedLogs);
      broadcaster = new LogBroadcaster({ logRepository: mockRepo });

      const mockSocket = createMockWebSocket();
      const clientId = broadcaster.addClient(mockSocket);

      // When: Subscribing with no sinceId (first time viewing completed task)
      broadcaster.subscribe(clientId, taskId, {}); // No sinceId

      // Then: All 4 backlog logs should be sent to client
      const sendMock = mockSocket.send as unknown as MockFn;
      const sentMessages = sendMock.mock.calls.map((call) => JSON.parse(call.arguments[0] as string));

      // First message is status: subscribed
      assert.strictEqual(sentMessages[0].type, "status");
      assert.strictEqual(sentMessages[0].data.status, "subscribed");

      // Then 4 log messages should follow
      const logMessages = sentMessages.filter((msg) => msg.type === "log");
      assert.strictEqual(logMessages.length, 4, "Expected 4 backlog log entries to be sent");

      // Verify log content and order
      assert.strictEqual(logMessages[0].data.line, "Starting task...");
      assert.strictEqual(logMessages[0].data.id, 1);
      assert.strictEqual(logMessages[1].data.line, "Processing...");
      assert.strictEqual(logMessages[1].data.id, 2);
      assert.strictEqual(logMessages[2].data.line, "Warning: something");
      assert.strictEqual(logMessages[2].data.id, 3);
      assert.strictEqual(logMessages[2].data.source, "stderr");
      assert.strictEqual(logMessages[3].data.line, "Task complete");
      assert.strictEqual(logMessages[3].data.id, 4);
    });

    test("sends partial backlog when subscribing with sinceId", () => {
      // Given: A task with persisted logs
      const taskId = "task-456";
      const persistedLogs = [
        { id: 1, taskId, line: "Line 1", timestamp: new Date(), source: "stdout" as const },
        { id: 2, taskId, line: "Line 2", timestamp: new Date(), source: "stdout" as const },
        { id: 3, taskId, line: "Line 3", timestamp: new Date(), source: "stdout" as const },
      ];
      const mockRepo = createMockLogRepository(persistedLogs);
      broadcaster = new LogBroadcaster({ logRepository: mockRepo });

      const mockSocket = createMockWebSocket();
      const clientId = broadcaster.addClient(mockSocket);

      // When: Subscribing with sinceId=1 (client already has log 1)
      broadcaster.subscribe(clientId, taskId, { sinceId: 1 });

      // Then: Only logs after id=1 should be sent (logs 2 and 3)
      const sendMock = mockSocket.send as unknown as MockFn;
      const sentMessages = sendMock.mock.calls.map((call) => JSON.parse(call.arguments[0] as string));

      const logMessages = sentMessages.filter((msg) => msg.type === "log");
      assert.strictEqual(logMessages.length, 2, "Expected 2 backlog log entries (after sinceId=1)");
      assert.strictEqual(logMessages[0].data.id, 2);
      assert.strictEqual(logMessages[1].data.id, 3);
    });

    test("sends no backlog when task has no persisted logs", () => {
      // Given: A task with no persisted logs (e.g., open task)
      const taskId = "empty-task-789";
      const mockRepo = createMockLogRepository([]);
      broadcaster = new LogBroadcaster({ logRepository: mockRepo });

      const mockSocket = createMockWebSocket();
      const clientId = broadcaster.addClient(mockSocket);

      // When: Subscribing with no sinceId
      broadcaster.subscribe(clientId, taskId, {});

      // Then: Only the status message should be sent (no logs)
      const sendMock = mockSocket.send as unknown as MockFn;
      const sentMessages = sendMock.mock.calls.map((call) => JSON.parse(call.arguments[0] as string));

      assert.strictEqual(sentMessages.length, 1, "Expected only status message");
      assert.strictEqual(sentMessages[0].type, "status");
      assert.strictEqual(sentMessages[0].data.status, "subscribed");
    });

    test("sends no backlog when logRepository is not configured", () => {
      // Given: Broadcaster without a log repository
      broadcaster = new LogBroadcaster(); // No logRepository

      const mockSocket = createMockWebSocket();
      const clientId = broadcaster.addClient(mockSocket);

      // When: Subscribing
      broadcaster.subscribe(clientId, "task-no-repo", {});

      // Then: Only the status message should be sent
      const sendMock = mockSocket.send as unknown as MockFn;
      const sentMessages = sendMock.mock.calls.map((call) => JSON.parse(call.arguments[0] as string));

      assert.strictEqual(sentMessages.length, 1);
      assert.strictEqual(sentMessages[0].type, "status");
    });
  });

  describe("server integration - backlog for completed tasks", () => {
    test("singleton broadcaster sends backlog when logRepository is configured before server start", async () => {
      // Given: A log repository with persisted logs for a completed task
      const taskId = "completed-task-integration-001";
      const persistedLogs = [
        { id: 1, taskId, line: "Task started", timestamp: new Date("2024-01-15T10:00:00Z"), source: "stdout" as const },
        { id: 2, taskId, line: "Work in progress...", timestamp: new Date("2024-01-15T10:00:01Z"), source: "stdout" as const },
        { id: 3, taskId, line: "Task complete", timestamp: new Date("2024-01-15T10:00:02Z"), source: "stdout" as const },
      ];
      const mockRepo = createMockLogRepository(persistedLogs);

      // Configure the singleton BEFORE any server usage (simulates serve.ts line 36)
      const { configureLogBroadcaster, getLogBroadcaster } = await import("./LogBroadcaster.js");
      configureLogBroadcaster({ logRepository: mockRepo });

      // When: A client subscribes via the singleton (as server.ts would do)
      const singleton = getLogBroadcaster();
      const mockSocket = createMockWebSocket();
      const clientId = singleton.addClient(mockSocket);
      singleton.subscribe(clientId, taskId, {}); // No sinceId = new client

      // Then: Backlog should be sent from the repository
      const sendMock = mockSocket.send as unknown as MockFn;
      const sentMessages = sendMock.mock.calls.map((call) => JSON.parse(call.arguments[0] as string));

      // Status + 3 log messages
      const logMessages = sentMessages.filter((msg) => msg.type === "log");
      assert.strictEqual(logMessages.length, 3, "Expected 3 backlog entries from DB for completed task");
      assert.strictEqual(logMessages[0].data.line, "Task started");
      assert.strictEqual(logMessages[1].data.line, "Work in progress...");
      assert.strictEqual(logMessages[2].data.line, "Task complete");
    });

    test("singleton broadcaster sends NO backlog when logRepository is NOT configured", async () => {
      // Given: Singleton without a configured log repository
      // (This is what happens if configureLogBroadcaster is not called)
      const { getLogBroadcaster } = await import("./LogBroadcaster.js");
      const singleton = getLogBroadcaster();

      // When: A client subscribes to a task that should have backlog
      const mockSocket = createMockWebSocket();
      const clientId = singleton.addClient(mockSocket);
      singleton.subscribe(clientId, "some-completed-task", {});

      // Then: Only the status message is sent (no backlog!)
      const sendMock = mockSocket.send as unknown as MockFn;
      const sentMessages = sendMock.mock.calls.map((call) => JSON.parse(call.arguments[0] as string));

      // BUG: This would pass even if there are logs in DB, because repo is undefined
      assert.strictEqual(sentMessages.length, 1);
      assert.strictEqual(sentMessages[0].type, "status");
    });
  });

  describe("backlog delivery for completed tasks via real database", () => {
    /**
     * This test verifies the FULL integration path:
     * 1. Logs are persisted to real SQLite database
     * 2. LogBroadcaster fetches backlog from TaskLogRepository on subscribe
     * 3. New subscriber receives all logs for a completed task
     *
     * This is the failing test that captures the bug: when a user navigates
     * directly to a completed task page, they should see all historical logs.
     */
    test("new subscriber receives full log history from database for completed task", async () => {
      // Skip if running in CI without full deps
      const { initializeTestDatabase, getTestDatabase, closeTestDatabase } = await import("../db/testUtils.js");

      // Given: A real database with persisted logs
      initializeTestDatabase();
      const db = getTestDatabase();

      // Import real repository
      const { TaskLogRepository } = await import("../repositories/TaskLogRepository.js");
      const logRepo = new TaskLogRepository(db);

      // Simulate logs written during task execution (completed task)
      const taskId = "completed-task-e2e-test-001";
      logRepo.append(taskId, { line: "Starting task execution...", timestamp: new Date(), source: "stdout" });
      logRepo.append(taskId, { line: "Processing step 1...", timestamp: new Date(), source: "stdout" });
      logRepo.append(taskId, { line: "Processing step 2...", timestamp: new Date(), source: "stdout" });
      logRepo.append(taskId, { line: "Error: minor warning", timestamp: new Date(), source: "stderr" });
      logRepo.append(taskId, { line: "Task completed successfully!", timestamp: new Date(), source: "stdout" });

      // Configure broadcaster with real repo
      broadcaster = new LogBroadcaster({ logRepository: logRepo });

      // When: A new client subscribes (simulating user opening completed task page)
      const mockSocket = createMockWebSocket();
      const clientId = broadcaster.addClient(mockSocket);
      broadcaster.subscribe(clientId, taskId, {}); // No sinceId = first time viewing

      // Then: Client should receive ALL 5 log entries as backlog
      const sendMock = mockSocket.send as unknown as MockFn;
      const sentMessages = sendMock.mock.calls.map((call) => JSON.parse(call.arguments[0] as string));

      const logMessages = sentMessages.filter((msg) => msg.type === "log");

      // THIS IS THE ASSERTION THAT SHOULD FAIL IF BACKLOG ISN'T WORKING
      assert.strictEqual(
        logMessages.length,
        5,
        `Expected 5 log entries for completed task, got ${logMessages.length}. ` +
          "This indicates the LogBroadcaster is not fetching backlog from the database."
      );

      // Verify log order and content
      assert.strictEqual(logMessages[0].data.line, "Starting task execution...");
      assert.strictEqual(logMessages[4].data.line, "Task completed successfully!");

      // Verify stderr log is included
      const stderrLog = logMessages.find((msg) => msg.data.source === "stderr");
      assert.ok(stderrLog, "Expected stderr log entry to be included in backlog");

      // Cleanup
      closeTestDatabase();
    });
  });
});
