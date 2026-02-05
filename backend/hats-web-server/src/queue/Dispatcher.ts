/**
 * Dispatcher
 *
 * The core execution engine of the "Employee" model. The Dispatcher:
 * 1. Polls TaskQueueService for pending tasks
 * 2. Executes tasks by invoking registered handlers
 * 3. Manages state transitions (PENDING → RUNNING → COMPLETED/FAILED)
 * 4. Publishes events via EventBus for coordination with other components
 *
 * Integration:
 * - TaskQueueService: Task storage and state management
 * - EventBus: Pub/sub for workflow coordination (e.g., 'task.started', 'task.completed')
 * - TaskHandlers: Registered functions that execute specific task types
 *
 * Lifecycle:
 *   dispatcher.start() → polling loop begins
 *   dispatcher.stop()  → graceful shutdown, waits for running tasks
 */

import { TaskQueueService, QueuedTask } from "./TaskQueueService";
import { EventBus, Event } from "./EventBus";
import { TaskState } from "./TaskState";

/**
 * Handler function for executing a specific task type.
 * Receives the task and returns a result or throws an error.
 */
export type TaskHandler<TPayload = Record<string, unknown>, TResult = unknown> = (
  task: QueuedTask,
  context: TaskExecutionContext
) => Promise<TResult> | TResult;

/**
 * Context provided to task handlers during execution
 */
export interface TaskExecutionContext {
  /** EventBus for publishing events during execution */
  eventBus: EventBus;
  /** Correlation ID for tracing */
  correlationId: string;
  /** Signal that can be checked for cancellation */
  signal: AbortSignal;
}

/**
 * Result of a task execution
 */
export interface TaskExecutionResult {
  /** The executed task */
  task: QueuedTask;
  /** Whether execution succeeded */
  success: boolean;
  /** Result returned by the handler (if successful) */
  result?: unknown;
  /** Error message (if failed) */
  error?: string;
  /** Execution duration in milliseconds */
  durationMs: number;
}

/**
 * Dispatcher configuration options
 */
export interface DispatcherOptions {
  /** Polling interval in milliseconds (default: 100ms) */
  pollIntervalMs?: number;
  /** Maximum concurrent tasks (default: 1 for sequential execution) */
  maxConcurrent?: number;
  /** Task timeout in milliseconds (default: 30000ms = 30s) */
  taskTimeoutMs?: number;
  /** Whether to auto-start on construction (default: false) */
  autoStart?: boolean;
}

/**
 * Event types published by the Dispatcher
 */
export type DispatcherEventType =
  | "dispatcher.started"
  | "dispatcher.stopped"
  | "dispatcher.idle"
  | "task.started"
  | "task.completed"
  | "task.failed"
  | "task.cancelled"
  | "task.timeout";

/**
 * Dispatcher statistics
 */
export interface DispatcherStats {
  /** Whether the dispatcher is running */
  isRunning: boolean;
  /** Total tasks processed */
  totalProcessed: number;
  /** Tasks that completed successfully */
  successCount: number;
  /** Tasks that failed */
  failureCount: number;
  /** Tasks that were cancelled */
  cancelledCount: number;
  /** Currently running tasks */
  runningCount: number;
  /** Tasks that timed out */
  timeoutCount: number;
  /** Average execution time in ms */
  avgDurationMs: number;
  /** Uptime in milliseconds */
  uptimeMs: number;
}

/**
 * Dispatcher
 *
 * Manages the task execution lifecycle. Uses a polling loop to dequeue
 * pending tasks and execute them via registered handlers.
 */
export class Dispatcher {
  /** Task queue service for enqueueing/dequeueing */
  private readonly queue: TaskQueueService;
  /** Event bus for publishing lifecycle events */
  private readonly eventBus: EventBus;
  /** Registered task handlers by type */
  private readonly handlers: Map<string, TaskHandler> = new Map();
  /** Default handler for unregistered task types */
  private defaultHandler?: TaskHandler;

  /** Configuration */
  private readonly pollIntervalMs: number;
  private readonly maxConcurrent: number;
  private readonly taskTimeoutMs: number;

  /** Runtime state */
  private running: boolean = false;
  private pollTimeoutId?: ReturnType<typeof setTimeout>;
  private readonly runningTasks: Map<string, AbortController> = new Map();
  private startedAt?: Date;

  /** Statistics */
  private stats = {
    totalProcessed: 0,
    successCount: 0,
    failureCount: 0,
    cancelledCount: 0,
    timeoutCount: 0,
    totalDurationMs: 0,
  };

  /**
   * Create a new Dispatcher
   *
   * @param queue - TaskQueueService instance
   * @param eventBus - EventBus instance for pub/sub
   * @param options - Configuration options
   */
  constructor(queue: TaskQueueService, eventBus: EventBus, options: DispatcherOptions = {}) {
    this.queue = queue;
    this.eventBus = eventBus;
    this.pollIntervalMs = options.pollIntervalMs ?? 100;
    this.maxConcurrent = options.maxConcurrent ?? 1;
    this.taskTimeoutMs = options.taskTimeoutMs ?? 7200000;

    if (options.autoStart) {
      this.start();
    }
  }

  /**
   * Register a handler for a specific task type.
   *
   * @param taskType - The task type string (e.g., 'build.compile')
   * @param handler - Function to execute for this task type
   * @returns this for chaining
   */
  registerHandler<TPayload = Record<string, unknown>, TResult = unknown>(
    taskType: string,
    handler: TaskHandler<TPayload, TResult>
  ): this {
    this.handlers.set(taskType, handler as TaskHandler);
    return this;
  }

  /**
   * Register a default handler for unregistered task types.
   *
   * @param handler - Function to execute for unknown task types
   * @returns this for chaining
   */
  registerDefaultHandler<TResult = unknown>(
    handler: TaskHandler<Record<string, unknown>, TResult>
  ): this {
    this.defaultHandler = handler as TaskHandler;
    return this;
  }

  /**
   * Unregister a handler for a task type.
   *
   * @param taskType - The task type to unregister
   * @returns true if handler was found and removed
   */
  unregisterHandler(taskType: string): boolean {
    return this.handlers.delete(taskType);
  }

  /**
   * Get the handler for a task type.
   * Returns the registered handler or the default handler if set.
   */
  private getHandler(taskType: string): TaskHandler | undefined {
    return this.handlers.get(taskType) ?? this.defaultHandler;
  }

  /**
   * Cancel a task by its ID.
   *
   * - If task is PENDING, transitions to CANCELLED.
   * - If task is RUNNING, aborts execution and transitions to CANCELLED.
   *
   * @param taskId - ID of the task to cancel
   * @returns true if task was cancelled, false if not found or already terminal
   */
  async cancelTask(taskId: string): Promise<boolean> {
    // Check if running
    const controller = this.runningTasks.get(taskId);
    if (controller) {
      controller.abort("cancelled");
      return true;
    }

    // Check if pending in queue
    const task = this.queue.getTask(taskId);
    if (task && task.state === TaskState.PENDING) {
      this.queue.cancel(taskId);
      this.stats.cancelledCount++;

      await this.eventBus.publish("task.cancelled", {
        taskId,
        taskType: task.taskType,
        reason: "cancelled by user",
        durationMs: 0,
      });

      return true;
    }

    return false;
  }

  /**
   * Start the dispatcher polling loop.
   * Does nothing if already running.
   */
  start(): void {
    if (this.running) {
      return;
    }

    this.running = true;
    this.startedAt = new Date();

    // Publish start event
    this.eventBus.publishSync("dispatcher.started", {
      timestamp: this.startedAt,
      config: {
        pollIntervalMs: this.pollIntervalMs,
        maxConcurrent: this.maxConcurrent,
        taskTimeoutMs: this.taskTimeoutMs,
      },
    });

    // Start the polling loop
    this.poll();
  }

  /**
   * Stop the dispatcher gracefully.
   * Waits for currently running tasks to complete.
   *
   * @param forceTimeoutMs - Force stop after this many ms (default: wait indefinitely)
   * @returns Promise that resolves when stopped
   */
  async stop(forceTimeoutMs?: number): Promise<void> {
    if (!this.running) {
      return;
    }

    this.running = false;

    // Clear the poll timeout
    if (this.pollTimeoutId) {
      clearTimeout(this.pollTimeoutId);
      this.pollTimeoutId = undefined;
    }

    // Wait for running tasks to complete
    const waitForRunning = async () => {
      while (this.runningTasks.size > 0) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
    };

    if (forceTimeoutMs !== undefined) {
      // Race between waiting and timeout
      await Promise.race([
        waitForRunning(),
        new Promise<void>((resolve) => {
          setTimeout(() => {
            // Force cancel all running tasks
            for (const [, controller] of this.runningTasks) {
              controller.abort();
            }
            resolve();
          }, forceTimeoutMs);
        }),
      ]);
    } else {
      await waitForRunning();
    }

    // Publish stop event
    this.eventBus.publishSync("dispatcher.stopped", {
      timestamp: new Date(),
      stats: this.getStats(),
    });
  }

  /**
   * Main polling loop.
   * Dequeues pending tasks and executes them.
   * Fills all available slots in a single poll cycle for parallel execution.
   */
  private poll(): void {
    if (!this.running) {
      return;
    }

    const availableSlots = this.maxConcurrent - this.runningTasks.size;

    if (availableSlots > 0) {
      let tasksStarted = 0;

      // Dequeue up to availableSlots tasks
      for (let i = 0; i < availableSlots; i++) {
        const { task } = this.queue.dequeue();

        if (task) {
          // Execute the task asynchronously
          this.executeTask(task).catch((error) => {
            // This shouldn't happen as executeTask handles its own errors
            console.error("Unexpected error in executeTask:", error);
          });
          tasksStarted++;
        } else {
          break; // No more tasks in queue
        }
      }

      // Emit idle only if queue empty AND nothing running
      if (tasksStarted === 0 && this.runningTasks.size === 0) {
        this.eventBus.publishSync("dispatcher.idle", {
          timestamp: new Date(),
          stats: this.getStats(),
        });
      }
    }

    // Schedule next poll
    this.pollTimeoutId = setTimeout(() => this.poll(), this.pollIntervalMs);
  }

  /**
   * Execute a single task.
   * Handles timeouts, errors, and state transitions.
   */
  private async executeTask(task: QueuedTask): Promise<TaskExecutionResult> {
    const startTime = Date.now();
    const correlationId = `exec-${task.id}-${startTime}`;

    // Create abort controller for this task
    const abortController = new AbortController();
    this.runningTasks.set(task.id, abortController);

    // Create execution context
    const context: TaskExecutionContext = {
      eventBus: this.eventBus,
      correlationId,
      signal: abortController.signal,
    };

    // Publish task started event
    await this.eventBus.publish(
      "task.started",
      {
        taskId: task.id,
        taskType: task.taskType,
        payload: task.payload,
        priority: task.priority,
      },
      { correlationId }
    );

    // Get the handler
    const handler = this.getHandler(task.taskType);

    let result: TaskExecutionResult;

    if (!handler) {
      // No handler registered for this task type
      const errorMsg = `No handler registered for task type: ${task.taskType}`;
      this.queue.fail(task.id, errorMsg);

      result = {
        task: this.queue.getTask(task.id) ?? task,
        success: false,
        error: errorMsg,
        durationMs: Date.now() - startTime,
      };

      // Publish failure event
      await this.eventBus.publish(
        "task.failed",
        {
          taskId: task.id,
          taskType: task.taskType,
          error: errorMsg,
          durationMs: result.durationMs,
        },
        { correlationId }
      );
    } else {
      // Set up timeout
      let timeoutId: ReturnType<typeof setTimeout> | undefined;
      const timeoutError = new Error(`Task timeout after ${this.taskTimeoutMs}ms`);
      const timeoutPromise = new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          // Pass the timeout error as the abort reason so cancellation promise
          // won't race ahead with a generic "aborted" message
          abortController.abort(timeoutError);
          reject(timeoutError);
        }, this.taskTimeoutMs);
      });

      // Set up cancellation monitoring
      const cancellationPromise = new Promise<never>((_, reject) => {
        if (abortController.signal.aborted) {
          reject(abortController.signal.reason || new Error("Task cancelled"));
        } else {
          abortController.signal.addEventListener("abort", () => {
            reject(abortController.signal.reason || new Error("Task cancelled"));
          });
        }
      });

      try {
        // Execute handler with timeout and cancellation
        const handlerResult = await Promise.race([
          Promise.resolve(handler(task, context)),
          timeoutPromise,
          cancellationPromise,
        ]);

        // Clear timeout
        if (timeoutId) {
          clearTimeout(timeoutId);
        }

        // Mark as completed
        this.queue.complete(task.id);

        result = {
          task: this.queue.getTask(task.id) ?? task,
          success: true,
          result: handlerResult,
          durationMs: Date.now() - startTime,
        };

        // Publish success event
        await this.eventBus.publish(
          "task.completed",
          {
            taskId: task.id,
            taskType: task.taskType,
            result: handlerResult,
            durationMs: result.durationMs,
          },
          { correlationId }
        );

        this.stats.successCount++;
      } catch (error) {
        // Clear timeout
        if (timeoutId) {
          clearTimeout(timeoutId);
        }

        const errorMsg = error instanceof Error ? error.message : String(error);
        const isTimeout = errorMsg.includes("Task timeout");
        // Check for cancellation (passed as string "cancelled" or AbortError)
        const isCancelled =
          error === "cancelled" ||
          (error instanceof Error && error.name === "AbortError") ||
          abortController.signal.aborted;

        result = {
          task: this.queue.getTask(task.id) ?? task,
          success: false,
          error: errorMsg,
          durationMs: Date.now() - startTime,
        };

        if (isTimeout) {
          // Check timeout FIRST - the timeout handler aborts the controller,
          // so we'd otherwise incorrectly detect this as a cancellation
          // Mark as failed (timeout)
          this.queue.fail(task.id, errorMsg);

          await this.eventBus.publish(
            "task.timeout",
            {
              taskId: task.id,
              taskType: task.taskType,
              timeoutMs: this.taskTimeoutMs,
              durationMs: result.durationMs,
            },
            { correlationId }
          );
          this.stats.timeoutCount++;
          this.stats.failureCount++;
        } else if (isCancelled) {
          // Mark as cancelled (user-initiated cancellation, not timeout)
          this.queue.cancel(task.id);

          await this.eventBus.publish(
            "task.cancelled",
            {
              taskId: task.id,
              taskType: task.taskType,
              reason: error === "cancelled" ? "cancelled by user" : errorMsg,
              durationMs: result.durationMs,
            },
            { correlationId }
          );
          this.stats.cancelledCount++;
        } else {
          // Mark as failed (generic error)
          this.queue.fail(task.id, errorMsg);

          await this.eventBus.publish(
            "task.failed",
            {
              taskId: task.id,
              taskType: task.taskType,
              error: errorMsg,
              durationMs: result.durationMs,
            },
            { correlationId }
          );
          this.stats.failureCount++;
        }
      }
    }

    // Update stats
    this.stats.totalProcessed++;
    this.stats.totalDurationMs += result.durationMs;

    // Remove from running tasks
    this.runningTasks.delete(task.id);

    return result;
  }

  /**
   * Check if the dispatcher is currently running.
   */
  isRunning(): boolean {
    return this.running;
  }

  /**
   * Get dispatcher statistics.
   */
  getStats(): DispatcherStats {
    return {
      isRunning: this.running,
      totalProcessed: this.stats.totalProcessed,
      successCount: this.stats.successCount,
      failureCount: this.stats.failureCount,
      cancelledCount: this.stats.cancelledCount,
      runningCount: this.runningTasks.size,
      timeoutCount: this.stats.timeoutCount,
      avgDurationMs:
        this.stats.totalProcessed > 0 ? this.stats.totalDurationMs / this.stats.totalProcessed : 0,
      uptimeMs: this.startedAt ? Date.now() - this.startedAt.getTime() : 0,
    };
  }

  /**
   * Get list of registered task types.
   */
  getRegisteredTaskTypes(): string[] {
    return Array.from(this.handlers.keys());
  }

  /**
   * Check if a handler is registered for a task type.
   */
  hasHandler(taskType: string): boolean {
    return this.handlers.has(taskType) || this.defaultHandler !== undefined;
  }

  /**
   * Execute a single task immediately without polling.
   * Useful for testing or one-off executions.
   *
   * @param task - Task to execute
   * @returns Execution result
   */
  async executeOnce(task: QueuedTask): Promise<TaskExecutionResult> {
    return this.executeTask(task);
  }
}
