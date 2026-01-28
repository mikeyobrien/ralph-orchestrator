/**
 * Server Entry Point
 *
 * Starts the Fastify server with TRPC integration and the task dispatcher.
 * Can be run directly with: tsx src/serve.ts
 */

import path from "path";
import { startServer, configureLogBroadcaster } from "./api";
import { initializeDatabase, getDatabase } from "./db/connection";
import { TaskQueueService, EventBus, Dispatcher } from "./queue";
import { PersistentTaskQueueService } from "./queue/PersistentTaskQueueService";
import { createRalphTaskHandler } from "./runner/RalphTaskHandler";
import { createTestLogTaskHandler } from "./runner/TestLogTaskHandler";
import { TaskBridge, LoopsManager, PlanningService, CollectionService, ConfigMerger } from "./services";
import { TaskRepository, TaskLogRepository, QueuedTaskRepository, CollectionRepository } from "./repositories";
import { ProcessSupervisor } from "./runner/ProcessSupervisor";
import { FileOutputStreamer } from "./runner/FileOutputStreamer";

const PORT = parseInt(process.env.PORT || "3000", 10);
const HOST = process.env.HOST || "0.0.0.0";

// Resolve workspace root:
// 1. RALPH_WORKSPACE_ROOT env var (explicit override)
// 2. process.cwd() (where user launched the server)
// 3. Fallback to repo root (computed from script location)
const REPO_ROOT = path.resolve(__dirname, "../../..");
const CWD = process.env.RALPH_WORKSPACE_ROOT || process.cwd() || REPO_ROOT;

// Initialize database tables before starting server
initializeDatabase();
const db = getDatabase();

// Configure log persistence for WebSocket streaming
const taskLogRepository = new TaskLogRepository(db);
configureLogBroadcaster({ logRepository: taskLogRepository });

// Initialize the task execution system
const queuedTaskRepository = new QueuedTaskRepository(db);
const taskQueue = new PersistentTaskQueueService(queuedTaskRepository);
const eventBus = new EventBus({ maxHistorySize: 100 });

// Read configuration from environment variables with validation
// RALPH_MAX_CONCURRENT: Max parallel tasks (default: 3, range: 1-10)
const MAX_CONCURRENT = parseInt(process.env.RALPH_MAX_CONCURRENT || "3", 10);
const maxConcurrent = Math.max(1, Math.min(MAX_CONCURRENT, 10)); // Cap at 10

// RALPH_POLL_INTERVAL_MS: Dispatcher poll interval (default: 100ms)
const pollIntervalMs = parseInt(process.env.RALPH_POLL_INTERVAL_MS || "100", 10);

// RALPH_TASK_TIMEOUT_MS: Task timeout (default: 14400000ms = 4 hours)
const taskTimeoutMs = parseInt(process.env.RALPH_TASK_TIMEOUT_MS || "14400000", 10);

// RALPH_LOOPS_PROCESS_INTERVAL_MS: Merge queue processing interval (default: 30000ms = 30s)
const loopsProcessIntervalMs = parseInt(process.env.RALPH_LOOPS_PROCESS_INTERVAL_MS || "30000", 10);

// Create and configure the dispatcher
const dispatcher = new Dispatcher(taskQueue, eventBus, {
  pollIntervalMs,
  maxConcurrent,
  taskTimeoutMs,
});

console.log(
  `Dispatcher configured: maxConcurrent=${maxConcurrent}, pollIntervalMs=${pollIntervalMs}, taskTimeoutMs=${taskTimeoutMs}ms`
);

const isTestMode = process.env.RALPH_TEST_MODE === "1";

// Default config path for ralph runs (can be overridden by user preset)
const defaultConfigPath = process.env.RALPH_CONFIG_PATH ?? path.resolve(REPO_ROOT, "ralph.yml");

if (isTestMode) {
  dispatcher.registerHandler("test.log", createTestLogTaskHandler());
} else {
  // Register the ralph task handler
  // Note: Config (-c) is NOT included in baseArgs - it's handled by TaskBridge
  // to allow user-selected presets to override the default config
  console.log(`Default ralph config: ${defaultConfigPath}`);
  dispatcher.registerHandler(
    "ralph.run",
    createRalphTaskHandler({
      defaultCwd: CWD,
      baseArgs: ["run", "--no-tui"], // Disable TUI for streaming output to WebSocket
    })
  );
}

// Create the TaskBridge to connect DB tasks with the execution queue
const taskRepository = new TaskRepository(db);
const collectionRepository = new CollectionRepository(db);
const collectionService = new CollectionService(collectionRepository);
const configMerger = isTestMode ? undefined : new ConfigMerger({
  presetsDir: path.resolve(REPO_ROOT, "presets"),
  directoryPresetsRoot: CWD,
  collectionService,
  tempDir: path.join(CWD, ".ralph", "temp"),
});
const processSupervisor = new ProcessSupervisor();
const outputStreamer = new FileOutputStreamer();
const taskBridge = new TaskBridge(taskRepository, taskQueue, eventBus, {
  defaultCwd: CWD,
  taskType: isTestMode ? "test.log" : "ralph.run",
  processSupervisor,
  outputStreamer,
  defaultConfigPath: isTestMode ? undefined : defaultConfigPath,
  collectionService,
  configMerger,
});

// Make queue available globally for backward compatibility
// TODO: Remove when all code uses TaskBridge
(globalThis as Record<string, unknown>).__taskQueue = taskQueue;
(globalThis as Record<string, unknown>).__dispatcher = dispatcher;

// Create LoopsManager for periodic merge queue processing
// This handles git merge conflicts when multiple worktree loops complete in parallel
const loopsManager = new LoopsManager({
  processIntervalMs: loopsProcessIntervalMs,
  workspaceRoot: CWD,
});

// Make LoopsManager available globally for potential API access
(globalThis as Record<string, unknown>).__loopsManager = loopsManager;

// Create PlanningService for planning session management
// Use REPO_ROOT for PlanningService because the planning preset (planning.yml)
// is located at presets/ relative to the monorepo root,
// not relative to the web server directory.
const planningService = new PlanningService({
  workspaceRoot: REPO_ROOT,
  ralphPath: "ralph",
  defaultTimeoutSeconds: 300,
});

// Make PlanningService available globally
(globalThis as Record<string, unknown>).__planningService = planningService;

// Graceful shutdown handler
let isShuttingDown = false;

async function gracefulShutdown(signal: string, timeoutMs: number): Promise<void> {
  if (isShuttingDown) return;
  isShuttingDown = true;

  console.log(`\n${signal} received, initiating graceful shutdown...`);

  try {
    // Stop accepting new tasks
    await dispatcher.stop(timeoutMs);

    // Stop loops manager
    loopsManager.stop();

    // Cleanup TaskBridge subscriptions
    taskBridge.destroy();

    console.log("Shutdown complete");
    process.exit(0);
  } catch (err) {
    console.error("Error during shutdown:", err);
    process.exit(1);
  }
}

// Register signal handlers
process.on("SIGTERM", () => gracefulShutdown("SIGTERM", 30000));
process.on("SIGINT", () => gracefulShutdown("SIGINT", 10000));

startServer({ port: PORT, host: HOST, db, taskBridge, loopsManager, planningService })
  .then(() => {
    // Restore pending tasks from database
    const restoredCount = taskQueue.hydrate();
    if (restoredCount > 0) {
      console.log(`Restored ${restoredCount} pending tasks from database`);
    }

    // Reconnect to running ralph processes (Phase 5)
    const { reconnected, failed } = taskBridge.reconnectRunningTasks();
    if (reconnected > 0 || failed > 0) {
      console.log(`Recovery complete: ${reconnected} reconnected, ${failed} failed`);
    }

    // Start the dispatcher after server is ready
    dispatcher.start();

    // Start LoopsManager for periodic merge queue processing
    loopsManager.start();
    loopsManager.on(LoopsManager.Events.PROCESSED, () => {
      console.log("Merge queue processed successfully");
    });
    loopsManager.on(LoopsManager.Events.ERROR, (err) => {
      console.error("LoopsManager error:", err);
    });

    console.log(`Server started on http://${HOST}:${PORT}`);
    console.log(`Health check: http://${HOST}:${PORT}/health`);
    console.log(`TRPC endpoint: http://${HOST}:${PORT}/trpc`);
    console.log(`REST API: http://${HOST}:${PORT}/api/v1`);
    console.log(`Dispatcher started (polling for tasks, maxConcurrent=${maxConcurrent})`);
    console.log(`LoopsManager active (processing every ${loopsProcessIntervalMs}ms)`);
    console.log(`TaskBridge active (DB tasks → execution queue)`);
  })
  .catch((err) => {
    console.error("Failed to start server:", err);
    process.exit(1);
  });
