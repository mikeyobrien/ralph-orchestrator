/**
 * Task Queue Module
 *
 * Implements the task dispatcher and execution queue system.
 * Core component of the "Employee" execution model.
 */

export { TaskState, isTerminalState, isValidTransition, getAllowedTransitions } from "./TaskState";
export { TaskQueueService } from "./TaskQueueService";
export type { QueuedTask, EnqueueOptions, DequeueResult } from "./TaskQueueService";
export { EventBus } from "./EventBus";
export type {
  Event,
  EventHandler,
  Subscription,
  SubscriptionOptions,
  PublishOptions,
  PublishResult,
} from "./EventBus";
export { Dispatcher } from "./Dispatcher";
export type {
  TaskHandler,
  TaskExecutionContext,
  TaskExecutionResult,
  DispatcherOptions,
  DispatcherEventType,
  DispatcherStats,
} from "./Dispatcher";
