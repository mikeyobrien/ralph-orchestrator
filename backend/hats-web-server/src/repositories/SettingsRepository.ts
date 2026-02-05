/**
 * SettingsRepository
 *
 * Data access layer for key-value settings storage using Drizzle ORM.
 * Implements get/set/delete operations with JSON serialization for complex values.
 */

import { eq } from "drizzle-orm";
import { BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import { settings, Setting, NewSetting } from "../db/schema";
import * as schema from "../db/schema";

export class SettingsRepository {
  private db: BetterSQLite3Database<typeof schema>;

  constructor(db: BetterSQLite3Database<typeof schema>) {
    this.db = db;
  }

  /**
   * Get a setting value by key
   * Returns undefined if the key doesn't exist
   * Values are JSON-parsed automatically
   */
  get<T = unknown>(key: string): T | undefined {
    const results = this.db.select().from(settings).where(eq(settings.key, key)).all();

    if (results.length === 0) {
      return undefined;
    }

    try {
      return JSON.parse(results[0].value) as T;
    } catch {
      // If JSON parsing fails, return the raw string value
      return results[0].value as T;
    }
  }

  /**
   * Get a setting with its metadata (key, value, updatedAt)
   * Returns the full Setting record or undefined
   */
  getWithMetadata(key: string): Setting | undefined {
    const results = this.db.select().from(settings).where(eq(settings.key, key)).all();
    return results[0];
  }

  /**
   * Set a setting value (create or update)
   * Values are JSON-serialized automatically
   * Returns the setting record
   */
  set<T>(key: string, value: T): Setting {
    const now = new Date();
    const serializedValue = JSON.stringify(value);

    // Check if key exists
    const existing = this.getWithMetadata(key);

    if (existing) {
      // Update existing
      this.db
        .update(settings)
        .set({
          value: serializedValue,
          updatedAt: now,
        })
        .where(eq(settings.key, key))
        .run();
    } else {
      // Insert new
      const newSetting: NewSetting = {
        key,
        value: serializedValue,
        updatedAt: now,
      };
      this.db.insert(settings).values(newSetting).run();
    }

    return this.getWithMetadata(key)!;
  }

  /**
   * Delete a setting by key
   * Returns true if a setting was deleted, false if not found
   */
  delete(key: string): boolean {
    const result = this.db.delete(settings).where(eq(settings.key, key)).run();
    return result.changes > 0;
  }

  /**
   * Get all settings
   * Returns array of Setting records (raw, not parsed)
   */
  getAll(): Setting[] {
    return this.db.select().from(settings).all();
  }

  /**
   * Get all settings as a key-value object
   * Values are JSON-parsed automatically
   */
  getAllAsObject(): Record<string, unknown> {
    const allSettings = this.getAll();
    const result: Record<string, unknown> = {};

    for (const setting of allSettings) {
      try {
        result[setting.key] = JSON.parse(setting.value);
      } catch {
        result[setting.key] = setting.value;
      }
    }

    return result;
  }

  /**
   * Check if a setting exists
   */
  has(key: string): boolean {
    return this.getWithMetadata(key) !== undefined;
  }

  /**
   * Delete all settings (useful for testing)
   */
  deleteAll(): number {
    const result = this.db.delete(settings).run();
    return result.changes;
  }
}
