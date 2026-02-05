/**
 * TaskRepository
 *
 * Data access layer for task operations using Drizzle ORM.
 * Implements CRUD operations with proper typing and error handling.
 */

import { eq, and, isNull, isNotNull } from "drizzle-orm";
import { BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import { tasks, Task, NewTask } from "../db/schema";
import * as schema from "../db/schema";

export class TaskRepository {
  private db: BetterSQLite3Database<typeof schema>;

  constructor(db: BetterSQLite3Database<typeof schema>) {
    this.db = db;
  }

  /**
   * Create a new task
   * Automatically sets createdAt and updatedAt timestamps
   */
  create(task: Omit<NewTask, "createdAt" | "updatedAt">): Task {
    const now = new Date();
    const taskWithTimestamps: NewTask = {
      ...task,
      createdAt: now,
      updatedAt: now,
    };

    this.db.insert(tasks).values(taskWithTimestamps).run();
    return this.findById(task.id)!;
  }

  /**
   * Find a task by its ID
   */
  findById(id: string): Task | undefined {
    const results = this.db.select().from(tasks).where(eq(tasks.id, id)).all();
    return results[0];
  }

  /**
   * Find all tasks, optionally filtered by status and archival state
   */
  findAll(status?: string, includeArchived: boolean = false): Task[] {
    const conditions = [];

    if (status) {
      conditions.push(eq(tasks.status, status));
    }

    if (!includeArchived) {
      conditions.push(isNull(tasks.archivedAt));
    }

    let query = this.db.select().from(tasks);

    if (conditions.length > 0) {
      // @ts-expect-error - drizzle spread operator typing issue with dynamic conditions
      query = query.where(and(...conditions));
    }

    return query.all();
  }

  /**
   * Find tasks that are ready (not blocked)
   * Returns tasks that have no blockedBy value or whose blocker is closed OR archived
   */
  findReady(): Task[] {
    const allTasks = this.findAll("open");

    // Tasks that can unblock others: closed tasks (active) OR any archived task
    const closedTasks = this.findAll("closed");
    const archivedTasks = this.db.select().from(tasks).where(isNotNull(tasks.archivedAt)).all();

    const unblockingIds = new Set([
      ...closedTasks.map((t) => t.id),
      ...archivedTasks.map((t) => t.id),
    ]);

    return allTasks.filter((task) => {
      if (!task.blockedBy) return true;
      return unblockingIds.has(task.blockedBy);
    });
  }

  /**
   * Update a task by ID
   * Automatically updates the updatedAt timestamp
   */
  update(
    id: string,
    updates: Partial<Omit<Task, "id" | "createdAt" | "updatedAt">>
  ): Task | undefined {
    const existing = this.findById(id);
    if (!existing) {
      return undefined;
    }

    this.db
      .update(tasks)
      .set({
        ...updates,
        updatedAt: new Date(),
      })
      .where(eq(tasks.id, id))
      .run();

    return this.findById(id);
  }

  /**
   * Close a task (set status to 'closed')
   */
  close(id: string): Task | undefined {
    return this.update(id, { status: "closed" });
  }

  /**
   * Archive a task
   */
  archive(id: string): Task | undefined {
    return this.update(id, { archivedAt: new Date() });
  }

  /**
   * Unarchive a task
   */
  unarchive(id: string): Task | undefined {
    return this.update(id, { archivedAt: null });
  }

  /**
   * Delete a task by ID
   * Returns true if a task was deleted, false if not found
   */
  delete(id: string): boolean {
    const result = this.db.delete(tasks).where(eq(tasks.id, id)).run();
    return result.changes > 0;
  }

  /**
   * Delete all tasks (useful for testing)
   */
  deleteAll(): number {
    const result = this.db.delete(tasks).run();
    return result.changes;
  }
}
