/**
 * PersistentTaskQueueService
 *
 * Extends TaskQueueService with database persistence for restart survival.
 * Synchronizes in-memory queue state with the queued_tasks table.
 */

import { TaskQueueService, QueuedTask, EnqueueOptions } from "./TaskQueueService";
import { TaskState } from "./TaskState";
import { QueuedTaskRepository } from "../repositories/QueuedTaskRepository";
import { QueuedTask as DbQueuedTask } from "../db/schema";

export class PersistentTaskQueueService extends TaskQueueService {
  constructor(private repository: QueuedTaskRepository) {
    super();
  }

  /**
   * Convert database QueuedTask to in-memory QueuedTask format
   */
  private dbToMemory(dbTask: DbQueuedTask): QueuedTask {
    return {
      id: dbTask.id,
      taskType: dbTask.taskType,
      payload: JSON.parse(dbTask.payload),
      state: dbTask.state.toUpperCase() as TaskState,
      priority: dbTask.priority,
      enqueuedAt: dbTask.enqueuedAt,
      startedAt: dbTask.startedAt ?? undefined,
      completedAt: dbTask.completedAt ?? undefined,
      error: dbTask.error ?? undefined,
      retryCount: dbTask.retryCount,
    };
  }

  /**
   * Convert TaskState enum to database format (lowercase)
   */
  private stateToDb(state: TaskState): "pending" | "running" | "completed" | "failed" {
    return state.toLowerCase() as "pending" | "running" | "completed" | "failed";
  }

  /**
   * Enqueue a new task and persist to database
   */
  override enqueue(options: EnqueueOptions): QueuedTask {
    const task = super.enqueue(options);

    // Persist to database
    this.repository.create({
      id: task.id,
      taskType: task.taskType,
      payload: JSON.stringify(task.payload),
      state: this.stateToDb(task.state),
      priority: task.priority,
      retryCount: task.retryCount,
      dbTaskId: null,
    });

    return task;
  }

  /**
   * Transition task state and persist to database
   */
  override transitionState(
    id: string,
    newState: TaskState,
    error?: string
  ): QueuedTask | undefined {
    const task = super.transitionState(id, newState, error);
    if (!task) {
      return undefined;
    }

    // Persist state change to database
    this.repository.update(id, {
      state: this.stateToDb(newState),
      startedAt: task.startedAt,
      completedAt: task.completedAt,
      error: task.error,
    });

    return task;
  }

  /**
   * Load pending tasks from database into memory queue
   * @returns Number of tasks restored
   */
  hydrate(): number {
    const pendingTasks = this.repository.findPending();

    for (const dbTask of pendingTasks) {
      const memoryTask = this.dbToMemory(dbTask);
      // Directly add to internal queue without re-persisting
      this["queue"].set(memoryTask.id, memoryTask);
    }

    return pendingTasks.length;
  }

  /**
   * Mark running tasks as failed (for crash recovery)
   * @returns Number of tasks recovered
   */
  recoverCrashed(): number {
    const runningTasks = this.repository.findRunning();

    for (const dbTask of runningTasks) {
      // Load into memory
      const memoryTask = this.dbToMemory(dbTask);
      this["queue"].set(memoryTask.id, memoryTask);

      // Mark as failed
      this.transitionState(dbTask.id, TaskState.FAILED, "Process died during server restart");
    }

    return runningTasks.length;
  }
}
