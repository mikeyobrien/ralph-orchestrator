/**
 * Drizzle ORM Schema Definitions
 * Database schema for ralphbot task and settings storage
 */

import { sqliteTable, text, integer } from "drizzle-orm/sqlite-core";

/**
 * Tasks table - stores task entries with their metadata
 *
 * Design decisions:
 * - Using text for id to support UUID-style identifiers
 * - status is text to allow flexible status values (open, closed, blocked)
 * - priority is integer (1-5 scale, 1 being highest priority)
 * - blockedBy is nullable text for task dependency relationships
 * - timestamps stored as integer (Unix epoch) for SQLite compatibility
 */
export const tasks = sqliteTable("tasks", {
  id: text("id").primaryKey(),
  title: text("title").notNull(),
  status: text("status").notNull().default("open"),
  priority: integer("priority").notNull().default(2),
  blockedBy: text("blocked_by"),
  createdAt: integer("created_at", { mode: "timestamp" }).notNull(),
  updatedAt: integer("updated_at", { mode: "timestamp" }).notNull(),
  // Execution tracking fields (added for TaskBridge)
  queuedTaskId: text("queued_task_id"), // Links to QueuedTask.id for correlation
  startedAt: integer("started_at", { mode: "timestamp" }), // When execution began
  completedAt: integer("completed_at", { mode: "timestamp" }), // When execution finished
  errorMessage: text("error_message"), // Failure reason if task failed
  // Execution summary fields (for visibility into what was accomplished)
  executionSummary: text("execution_summary"), // Markdown summary from .agent/summary.md
  exitCode: integer("exit_code"), // Process exit code (0 = success)
  durationMs: integer("duration_ms"), // Total execution time in milliseconds
  // Archival fields
  archivedAt: integer("archived_at", { mode: "timestamp" }), // When the task was archived
  // Merge loop tracking - stores the prompt used when this task triggered a merge loop
  mergeLoopPrompt: text("merge_loop_prompt"),
});

/**
 * Queued tasks table - persists task queue state for restart survival
 *
 * Design decisions:
 * - Separate from tasks table to isolate execution state from user-facing task metadata
 * - payload is JSON-serialized for flexible task parameters
 * - state tracks execution lifecycle: pending -> running -> completed/failed
 * - dbTaskId links to tasks table for correlation
 * - Enables recovery of pending tasks and detection of crashed running tasks
 */
export const queuedTasks = sqliteTable("queued_tasks", {
  id: text("id").primaryKey(),
  taskType: text("task_type").notNull(),
  payload: text("payload").notNull(), // JSON-serialized
  state: text("state", { enum: ["pending", "running", "completed", "failed"] })
    .notNull()
    .default("pending"),
  priority: integer("priority").notNull().default(5),
  enqueuedAt: integer("enqueued_at", { mode: "timestamp" }).notNull(),
  startedAt: integer("started_at", { mode: "timestamp" }),
  completedAt: integer("completed_at", { mode: "timestamp" }),
  error: text("error"),
  retryCount: integer("retry_count").notNull().default(0),
  dbTaskId: text("db_task_id"), // Foreign key to tasks table
});

/**
 * Task logs table - stores all stdout/stderr lines for each task
 *
 * Design decisions:
 * - Auto-increment id provides stable ordering across log lines
 * - taskId links logs to the task record (no FK for now)
 * - timestamp stored as integer (Unix epoch) for SQLite compatibility
 */
export const taskLogs = sqliteTable("task_logs", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  taskId: text("task_id").notNull(),
  timestamp: integer("timestamp", { mode: "timestamp" }).notNull(),
  source: text("source", { enum: ["stdout", "stderr"] }).notNull(),
  line: text("line").notNull(),
});

/**
 * Settings table - key-value store for configuration
 *
 * Design decisions:
 * - Simple key-value structure for flexibility
 * - value is text to store JSON-serialized complex values
 * - timestamps for audit trail
 */
export const settings = sqliteTable("settings", {
  key: text("key").primaryKey(),
  value: text("value").notNull(),
  updatedAt: integer("updated_at", { mode: "timestamp" }).notNull(),
});

/**
 * Hat collections table - named groups of interconnected hats
 *
 * Design decisions:
 * - Collections represent a visual workflow of hats (like n8n workflows)
 * - name is the display name shown in the UI
 * - description provides context about the collection's purpose
 * - graphData stores the React Flow state (nodes, edges, viewport) as JSON
 * - Enables visual building and exporting to YAML presets
 */
export const collections = sqliteTable("collections", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  description: text("description"),
  // JSON-serialized React Flow state: { nodes: Node[], edges: Edge[], viewport: Viewport }
  graphData: text("graph_data").notNull(),
  createdAt: integer("created_at", { mode: "timestamp" }).notNull(),
  updatedAt: integer("updated_at", { mode: "timestamp" }).notNull(),
});

// Type exports for use in repositories
export type Task = typeof tasks.$inferSelect;
export type NewTask = typeof tasks.$inferInsert;
export type QueuedTask = typeof queuedTasks.$inferSelect;
export type NewQueuedTask = typeof queuedTasks.$inferInsert;
export type TaskLog = typeof taskLogs.$inferSelect;
export type NewTaskLog = typeof taskLogs.$inferInsert;
export type Setting = typeof settings.$inferSelect;
export type NewSetting = typeof settings.$inferInsert;
export type Collection = typeof collections.$inferSelect;
export type NewCollection = typeof collections.$inferInsert;
