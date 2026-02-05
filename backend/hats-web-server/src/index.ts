/**
 * @hats-web/server
 * Hats web dashboard server
 */

// Database exports
export {
  getDatabase,
  initializeDatabase,
  closeDatabase,
  getSqliteConnection,
  schema,
} from "./db/connection";
export type { Task, NewTask, TaskLog, NewTaskLog, Setting, NewSetting } from "./db/schema";

// Repository exports
export { TaskRepository, SettingsRepository, TaskLogRepository } from "./repositories";

// Queue exports
export {
  TaskState,
  isTerminalState,
  isValidTransition,
  getAllowedTransitions,
  TaskQueueService,
  EventBus,
  Dispatcher,
} from "./queue";
export type {
  QueuedTask,
  EnqueueOptions,
  DequeueResult,
  Event,
  EventHandler,
  Subscription,
  SubscriptionOptions,
  PublishOptions,
  PublishResult,
  TaskHandler,
  TaskExecutionContext,
  TaskExecutionResult,
  DispatcherOptions,
  DispatcherEventType,
  DispatcherStats,
} from "./queue";

// Runner exports
export {
  RunnerState,
  isTerminalRunnerState,
  isValidRunnerTransition,
  getAllowedRunnerTransitions,
  LogStream,
  PromptWriter,
  HatsRunner,
} from "./runner";
export type {
  LogEntry,
  LogCallback,
  LogStreamOptions,
  PromptContent,
  PromptWriterOptions,
  HatsRunnerOptions,
  RunnerResult,
  HatsRunnerEvents,
} from "./runner";

// API exports
export { createServer, startServer, appRouter, taskRouter, createContext } from "./api";
export type { ServerOptions, AppRouter, Context } from "./api";

console.log("Hats Web Server initialized");
