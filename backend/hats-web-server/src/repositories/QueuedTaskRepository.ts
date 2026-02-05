/**
 * QueuedTaskRepository
 *
 * Data access layer for queued task operations using Drizzle ORM.
 * Implements CRUD operations for the task queue persistence.
 */

import { eq } from "drizzle-orm";
import { BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import { queuedTasks, QueuedTask, NewQueuedTask } from "../db/schema";
import * as schema from "../db/schema";

export class QueuedTaskRepository {
  private db: BetterSQLite3Database<typeof schema>;

  constructor(db: BetterSQLite3Database<typeof schema>) {
    this.db = db;
  }

  /**
   * Create a new queued task
   */
  create(task: Omit<NewQueuedTask, "enqueuedAt">): QueuedTask {
    const taskWithTimestamp: NewQueuedTask = {
      ...task,
      enqueuedAt: new Date(),
    };

    this.db.insert(queuedTasks).values(taskWithTimestamp).run();
    return this.findById(task.id)!;
  }

  /**
   * Find a queued task by its ID
   */
  findById(id: string): QueuedTask | undefined {
    const results = this.db.select().from(queuedTasks).where(eq(queuedTasks.id, id)).all();
    return results[0];
  }

  /**
   * Find all queued tasks
   */
  findAll(): QueuedTask[] {
    return this.db.select().from(queuedTasks).all();
  }

  /**
   * Find tasks by state
   */
  findByState(state: "pending" | "running" | "completed" | "failed"): QueuedTask[] {
    return this.db.select().from(queuedTasks).where(eq(queuedTasks.state, state)).all();
  }

  /**
   * Find pending tasks
   */
  findPending(): QueuedTask[] {
    return this.findByState("pending");
  }

  /**
   * Find running tasks
   */
  findRunning(): QueuedTask[] {
    return this.findByState("running");
  }

  /**
   * Update a queued task by ID
   */
  update(
    id: string,
    updates: Partial<Omit<QueuedTask, "id" | "enqueuedAt">>
  ): QueuedTask | undefined {
    const existing = this.findById(id);
    if (!existing) {
      return undefined;
    }

    this.db.update(queuedTasks).set(updates).where(eq(queuedTasks.id, id)).run();

    return this.findById(id);
  }

  /**
   * Mark task as running
   */
  markRunning(id: string): QueuedTask | undefined {
    return this.update(id, {
      state: "running",
      startedAt: new Date(),
    });
  }

  /**
   * Mark task as completed
   */
  markCompleted(id: string): QueuedTask | undefined {
    return this.update(id, {
      state: "completed",
      completedAt: new Date(),
    });
  }

  /**
   * Mark task as failed
   */
  markFailed(id: string, error: string): QueuedTask | undefined {
    return this.update(id, {
      state: "failed",
      completedAt: new Date(),
      error,
    });
  }

  /**
   * Increment retry count
   */
  incrementRetryCount(id: string): QueuedTask | undefined {
    const task = this.findById(id);
    if (!task) {
      return undefined;
    }

    return this.update(id, {
      retryCount: task.retryCount + 1,
    });
  }

  /**
   * Delete a queued task by ID
   */
  delete(id: string): boolean {
    const result = this.db.delete(queuedTasks).where(eq(queuedTasks.id, id)).run();
    return result.changes > 0;
  }

  /**
   * Delete all queued tasks
   */
  deleteAll(): number {
    const result = this.db.delete(queuedTasks).run();
    return result.changes;
  }
}
