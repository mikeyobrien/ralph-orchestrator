/**
 * TaskQueueService
 *
 * Implements the task queue with enqueue/dequeue operations for the dispatcher.
 * This is the core of the "Employee" execution model - tasks are enqueued as PENDING
 * and dequeued to transition to RUNNING state.
 *
 * Architecture:
 * - In-memory queue backed by database for persistence
 * - FIFO ordering with priority support
 * - State machine enforcement via TaskState helpers
 */

import { TaskState, isValidTransition, isTerminalState } from "./TaskState";

/**
 * Represents a task in the execution queue.
 * Separate from the CLI Task entity - this is for dispatcher execution tracking.
 */
export interface QueuedTask {
  /** Unique identifier for this queued task */
  id: string;
  /** Type/name of the task to execute */
  taskType: string;
  /** Arbitrary payload data for the task */
  payload: Record<string, unknown>;
  /** Current execution state */
  state: TaskState;
  /** Priority (lower = higher priority, default 5) */
  priority: number;
  /** When the task was enqueued */
  enqueuedAt: Date;
  /** When the task started running (if applicable) */
  startedAt?: Date;
  /** When the task completed/failed (if applicable) */
  completedAt?: Date;
  /** Error message if task failed */
  error?: string;
  /** Number of retry attempts */
  retryCount: number;
}

/**
 * Options for creating a new queued task
 */
export interface EnqueueOptions {
  /** Type/name of the task */
  taskType: string;
  /** Arbitrary payload data */
  payload?: Record<string, unknown>;
  /** Priority (1-10, lower = higher priority, default 5) */
  priority?: number;
}

/**
 * Result of a dequeue operation
 */
export interface DequeueResult {
  /** The dequeued task, or undefined if queue is empty */
  task: QueuedTask | undefined;
  /** Number of remaining pending tasks */
  remaining: number;
}

/**
 * TaskQueueService
 *
 * Manages the task execution queue with proper state transitions.
 * Implements FIFO ordering with priority support.
 */
export class TaskQueueService {
  /** In-memory queue storage */
  private queue: Map<string, QueuedTask> = new Map();

  /** Counter for generating unique IDs */
  private idCounter: number = 0;

  /**
   * Generate a unique task ID
   */
  private generateId(): string {
    const timestamp = Date.now();
    const counter = ++this.idCounter;
    return `qtask-${timestamp}-${counter.toString(16)}`;
  }

  /**
   * Enqueue a new task for execution.
   * Task starts in PENDING state.
   *
   * @param options - Task configuration
   * @returns The created QueuedTask
   */
  enqueue(options: EnqueueOptions): QueuedTask {
    const task: QueuedTask = {
      id: this.generateId(),
      taskType: options.taskType,
      payload: options.payload ?? {},
      state: TaskState.PENDING,
      priority: options.priority ?? 5,
      enqueuedAt: new Date(),
      retryCount: 0,
    };

    this.queue.set(task.id, task);
    return task;
  }

  /**
   * Dequeue the next pending task for execution.
   * Transitions the task from PENDING to RUNNING.
   *
   * Returns tasks in priority order (lower priority number = higher priority),
   * with FIFO ordering within the same priority level.
   *
   * @returns DequeueResult with the task and remaining count
   */
  dequeue(): DequeueResult {
    // Get all pending tasks sorted by priority, then by enqueue time
    const pendingTasks = this.getPendingTasks();

    if (pendingTasks.length === 0) {
      return { task: undefined, remaining: 0 };
    }

    // Sort by priority (ascending), then by enqueuedAt (ascending)
    pendingTasks.sort((a, b) => {
      if (a.priority !== b.priority) {
        return a.priority - b.priority;
      }
      return a.enqueuedAt.getTime() - b.enqueuedAt.getTime();
    });

    // Take the highest priority (first) task
    const task = pendingTasks[0];

    // Transition to RUNNING
    this.transitionState(task.id, TaskState.RUNNING);

    return {
      task: this.queue.get(task.id),
      remaining: pendingTasks.length - 1,
    };
  }

  /**
   * Get a task by its ID
   */
  getTask(id: string): QueuedTask | undefined {
    return this.queue.get(id);
  }

  /**
   * Get all pending tasks
   */
  getPendingTasks(): QueuedTask[] {
    return Array.from(this.queue.values()).filter((task) => task.state === TaskState.PENDING);
  }

  /**
   * Get all running tasks
   */
  getRunningTasks(): QueuedTask[] {
    return Array.from(this.queue.values()).filter((task) => task.state === TaskState.RUNNING);
  }

  /**
   * Get all completed tasks (including failed)
   */
  getCompletedTasks(): QueuedTask[] {
    return Array.from(this.queue.values()).filter((task) => isTerminalState(task.state));
  }

  /**
   * Get all tasks in the queue
   */
  getAllTasks(): QueuedTask[] {
    return Array.from(this.queue.values());
  }

  /**
   * Get queue statistics
   */
  getStats(): {
    pending: number;
    running: number;
    completed: number;
    failed: number;
    total: number;
  } {
    const tasks = Array.from(this.queue.values());
    return {
      pending: tasks.filter((t) => t.state === TaskState.PENDING).length,
      running: tasks.filter((t) => t.state === TaskState.RUNNING).length,
      completed: tasks.filter((t) => t.state === TaskState.COMPLETED).length,
      failed: tasks.filter((t) => t.state === TaskState.FAILED).length,
      total: tasks.length,
    };
  }

  /**
   * Transition a task to a new state.
   * Enforces valid state transitions via the state machine.
   *
   * @param id - Task ID
   * @param newState - Target state
   * @param error - Optional error message (for FAILED state)
   * @returns Updated task or undefined if not found/invalid transition
   */
  transitionState(id: string, newState: TaskState, error?: string): QueuedTask | undefined {
    const task = this.queue.get(id);
    if (!task) {
      return undefined;
    }

    // Validate the transition
    if (!isValidTransition(task.state, newState)) {
      throw new Error(`Invalid state transition: ${task.state} -> ${newState} for task ${id}`);
    }

    // Update the task
    task.state = newState;

    // Set timestamps based on the new state
    if (newState === TaskState.RUNNING) {
      task.startedAt = new Date();
    } else if (isTerminalState(newState)) {
      task.completedAt = new Date();
      if (newState === TaskState.FAILED && error) {
        task.error = error;
      }
    }

    return task;
  }

  /**
   * Mark a task as completed successfully.
   * Convenience method for transitionState(id, COMPLETED).
   */
  complete(id: string): QueuedTask | undefined {
    return this.transitionState(id, TaskState.COMPLETED);
  }

  /**
   * Mark a task as failed with an error message.
   * Convenience method for transitionState(id, FAILED).
   */
  fail(id: string, error: string): QueuedTask | undefined {
    return this.transitionState(id, TaskState.FAILED, error);
  }

  /**
   * Mark a task as cancelled.
   * Convenience method for transitionState(id, CANCELLED).
   */
  cancel(id: string): QueuedTask | undefined {
    return this.transitionState(id, TaskState.CANCELLED);
  }

  /**
   * Remove a task from the queue.
   * Only terminal-state tasks can be removed.
   *
   * @returns true if removed, false if not found or not in terminal state
   */
  remove(id: string): boolean {
    const task = this.queue.get(id);
    if (!task) {
      return false;
    }

    // Only allow removing completed/failed tasks
    if (!isTerminalState(task.state)) {
      throw new Error(
        `Cannot remove task ${id} in state ${task.state} - must be in terminal state`
      );
    }

    return this.queue.delete(id);
  }

  /**
   * Clear all tasks from the queue.
   * Useful for testing.
   *
   * @param includeRunning - If true, also clears running tasks (dangerous!)
   * @returns Number of tasks cleared
   */
  clear(includeRunning: boolean = false): number {
    let cleared = 0;

    for (const [id, task] of this.queue) {
      // Skip running tasks unless explicitly requested
      if (task.state === TaskState.RUNNING && !includeRunning) {
        continue;
      }
      this.queue.delete(id);
      cleared++;
    }

    return cleared;
  }

  /**
   * Get the number of tasks in a specific state
   */
  countByState(state: TaskState): number {
    return Array.from(this.queue.values()).filter((t) => t.state === state).length;
  }

  /**
   * Check if the queue has any pending tasks
   */
  hasPending(): boolean {
    return this.countByState(TaskState.PENDING) > 0;
  }

  /**
   * Check if any tasks are currently running
   */
  hasRunning(): boolean {
    return this.countByState(TaskState.RUNNING) > 0;
  }

  /**
   * Check if the queue is idle (no pending or running tasks)
   */
  isIdle(): boolean {
    return !this.hasPending() && !this.hasRunning();
  }
}
