/**
 * TaskLogRepository
 *
 * Data access layer for persistent task log storage.
 */

import { and, asc, eq, gt } from "drizzle-orm";
import { BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import { taskLogs, TaskLog } from "../db/schema";
import * as schema from "../db/schema";
import type { LogEntry } from "../runner/LogStream";

export interface ListTaskLogsOptions {
  /** Only return logs with id greater than this value */
  afterId?: number;
  /** Limit number of log entries returned */
  limit?: number;
}

export class TaskLogRepository {
  private db: BetterSQLite3Database<typeof schema>;

  constructor(db: BetterSQLite3Database<typeof schema>) {
    this.db = db;
  }

  /**
   * Append a single log entry for a task.
   * Returns the inserted log id.
   */
  append(taskId: string, entry: LogEntry): number {
    const timestamp = entry.timestamp instanceof Date ? entry.timestamp : new Date(entry.timestamp);

    const result = this.db
      .insert(taskLogs)
      .values({
        taskId,
        timestamp,
        source: entry.source,
        line: entry.line,
      })
      .run();

    return Number(result.lastInsertRowid);
  }

  /**
   * List logs for a given task, ordered by id ascending.
   */
  listByTaskId(taskId: string, options: ListTaskLogsOptions = {}): TaskLog[] {
    const { afterId, limit } = options;

    const whereClause =
      afterId !== undefined
        ? and(eq(taskLogs.taskId, taskId), gt(taskLogs.id, afterId))
        : eq(taskLogs.taskId, taskId);

    const query = this.db.select().from(taskLogs).where(whereClause).orderBy(asc(taskLogs.id));

    if (limit !== undefined) {
      return query.limit(limit).all();
    }

    return query.all();
  }

  /**
   * Delete all task logs.
   * Returns the number of deleted rows.
   */
  deleteAll(): number {
    const result = this.db.delete(taskLogs).run();
    return result.changes;
  }
}
