/**
 * Database Test Utilities
 *
 * Provides isolated in-memory database for testing.
 * Each test gets a fresh database instance.
 */

import Database from "better-sqlite3";
import { drizzle, BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import * as schema from "./schema";

let testSqlite: Database.Database | null = null;
let testDb: BetterSQLite3Database<typeof schema> | null = null;

/**
 * Initialize an isolated in-memory test database
 */
export function initializeTestDatabase(): void {
  // Close any existing test database
  closeTestDatabase();

  // Create fresh in-memory database
  testSqlite = new Database(":memory:");
  testDb = drizzle(testSqlite, { schema });

  // Create required tables
  testSqlite.exec(`
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
      error_message TEXT,
      execution_summary TEXT,
      exit_code INTEGER,
      duration_ms INTEGER,
      archived_at INTEGER
    )
  `);

  testSqlite.exec(`
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

  testSqlite.exec(`
    CREATE TABLE IF NOT EXISTS task_logs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      task_id TEXT NOT NULL,
      timestamp INTEGER NOT NULL,
      source TEXT NOT NULL,
      line TEXT NOT NULL
    )
  `);

  testSqlite.exec(`
    CREATE INDEX IF NOT EXISTS idx_task_logs_task_id
    ON task_logs (task_id, id)
  `);

  testSqlite.exec(`
    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL,
      updated_at INTEGER NOT NULL
    )
  `);

  testSqlite.exec(`
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
 * Get the test database instance
 */
export function getTestDatabase(): BetterSQLite3Database<typeof schema> {
  if (!testDb) {
    throw new Error("Test database not initialized. Call initializeTestDatabase() first.");
  }
  return testDb;
}

/**
 * Close and cleanup the test database
 */
export function closeTestDatabase(): void {
  if (testSqlite) {
    testSqlite.close();
    testSqlite = null;
    testDb = null;
  }
}
