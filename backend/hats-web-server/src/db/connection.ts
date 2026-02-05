/**
 * Database Connection Module
 *
 * Provides SQLite database connection using better-sqlite3 and Drizzle ORM.
 *
 * Design decisions:
 * - Uses better-sqlite3 for synchronous, fast SQLite access
 * - Lazy initialization pattern for on-demand connection
 * - WAL mode enabled for better concurrent read performance
 * - Connection cleanup function for graceful shutdown
 */

import fs from "fs";
import path from "path";
import Database from "better-sqlite3";
import { drizzle, BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import * as schema from "./schema";

// Module-level connection state
let sqlite: Database.Database | null = null;
let db: BetterSQLite3Database<typeof schema> | null = null;

/**
 * Get or create the database connection
 *
 * @param dbPath - Path to the SQLite database file (defaults to ~/.hats/web/hats.db)
 * @returns Drizzle database instance with typed schema
 */
export function getDatabase(dbPath?: string): BetterSQLite3Database<typeof schema> {
  if (db) {
    return db;
  }

  const resolvedPath = dbPath ?? process.env.HATS_DB_PATH ?? getDefaultDbPath();

  if (resolvedPath !== ":memory:") {
    const dir = path.dirname(resolvedPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
  }

  // Create the SQLite connection
  sqlite = new Database(resolvedPath);

  // Enable WAL mode for better concurrent read performance
  sqlite.pragma("journal_mode = WAL");

  // Enable foreign keys (not currently used but good practice)
  sqlite.pragma("foreign_keys = ON");

  // Create Drizzle ORM instance with typed schema
  db = drizzle(sqlite, { schema });

  return db;
}

/**
 * Get the default database path
 * Uses ~/.hats/web/hats.db for consistency with CLI config location
 */
function getDefaultDbPath(): string {
  const homeDir = process.env.HOME || process.env.USERPROFILE || ".";
  return `${homeDir}/.hats/web/hats.db`;
}

/**
 * Initialize database tables
 * Creates tables if they don't exist using raw SQL
 *
 * Note: For production, use drizzle-kit migrations instead.
 * This is a convenience function for development/testing.
 */
export function initializeDatabase(database?: BetterSQLite3Database<typeof schema>): void {
  const targetDb = database ?? getDatabase();

  // Get raw SQLite connection for table creation
  if (!sqlite) {
    throw new Error("Database not initialized. Call getDatabase() first.");
  }

  // Create tasks table
  sqlite.exec(`
    CREATE TABLE IF NOT EXISTS tasks (
      id TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'open',
      priority INTEGER NOT NULL DEFAULT 2,
      blocked_by TEXT,
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL,
      queued_task_id TEXT,
      started_at INTEGER,
      completed_at INTEGER,
      error_message TEXT
    )
  `);

  // Add new execution tracking columns to existing databases
  // SQLite doesn't have IF NOT EXISTS for ALTER TABLE, so we handle errors
  const addColumnIfNotExists = (table: string, column: string, type: string) => {
    try {
      sqlite!.exec(`ALTER TABLE ${table} ADD COLUMN ${column} ${type}`);
    } catch (e) {
      // Column already exists - ignore the error
    }
  };

  addColumnIfNotExists("tasks", "queued_task_id", "TEXT");
  addColumnIfNotExists("tasks", "started_at", "INTEGER");
  addColumnIfNotExists("tasks", "completed_at", "INTEGER");
  addColumnIfNotExists("tasks", "error_message", "TEXT");
  // Execution summary columns for visibility into what was accomplished
  addColumnIfNotExists("tasks", "execution_summary", "TEXT");
  addColumnIfNotExists("tasks", "exit_code", "INTEGER");
  addColumnIfNotExists("tasks", "duration_ms", "INTEGER");
  addColumnIfNotExists("tasks", "archived_at", "INTEGER");
  // Merge loop tracking - stores the prompt used when this task triggered a merge loop
  addColumnIfNotExists("tasks", "merge_loop_prompt", "TEXT");
  // UX improvement fields (Step 9) - for rich task detail display
  addColumnIfNotExists("tasks", "preset", "TEXT");
  addColumnIfNotExists("tasks", "current_iteration", "INTEGER");
  addColumnIfNotExists("tasks", "max_iterations", "INTEGER");
  addColumnIfNotExists("tasks", "loop_id", "TEXT");

  // Create queued_tasks table for task queue persistence
  sqlite.exec(`
    CREATE TABLE IF NOT EXISTS queued_tasks (
      id TEXT PRIMARY KEY,
      task_type TEXT NOT NULL,
      payload TEXT NOT NULL,
      state TEXT NOT NULL DEFAULT 'pending',
      priority INTEGER NOT NULL DEFAULT 5,
      enqueued_at INTEGER NOT NULL,
      started_at INTEGER,
      completed_at INTEGER,
      error TEXT,
      retry_count INTEGER NOT NULL DEFAULT 0,
      db_task_id TEXT
    )
  `);

  // Create task logs table
  sqlite.exec(`
    CREATE TABLE IF NOT EXISTS task_logs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      task_id TEXT NOT NULL,
      timestamp INTEGER NOT NULL,
      source TEXT NOT NULL,
      line TEXT NOT NULL
    )
  `);

  // Index for fast task log lookups
  sqlite.exec(`
    CREATE INDEX IF NOT EXISTS idx_task_logs_task_id
    ON task_logs (task_id, id)
  `);

  // Create settings table
  sqlite.exec(`
    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL,
      updated_at INTEGER NOT NULL
    )
  `);

  // Create collections table for hat collection builder
  sqlite.exec(`
    CREATE TABLE IF NOT EXISTS collections (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      description TEXT,
      graph_data TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL
    )
  `);
}

/**
 * Close the database connection
 * Should be called during graceful shutdown
 */
export function closeDatabase(): void {
  if (sqlite) {
    sqlite.close();
    sqlite = null;
    db = null;
  }
}

/**
 * Get the raw SQLite connection
 * Useful for advanced operations or testing
 */
export function getSqliteConnection(): Database.Database | null {
  return sqlite;
}

// Re-export schema for convenience
export { schema };
export type { Task, NewTask, Setting, NewSetting } from "./schema";
