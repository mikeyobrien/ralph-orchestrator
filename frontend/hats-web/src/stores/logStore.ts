/**
 * Log Store
 *
 * Zustand store for persisting task logs across component mount/unmount cycles.
 * This solves the problem where logs are lost when collapsing/expanding task cards,
 * because the log viewer unmounts and loses its local state.
 *
 * The store maintains a Map of taskId → LogEntry[] that persists in memory
 * regardless of which components are mounted.
 */

import { create } from "zustand";
import type { LogEntry } from "@/hooks/useTaskWebSocket";

/** Stable empty array to avoid creating new references in selectors */
const EMPTY_LOGS: LogEntry[] = [];

interface LogStore {
  /**
   * Map of taskId → LogEntry[]
   * Using a plain object for Zustand compatibility (Map doesn't trigger re-renders)
   */
  taskLogs: Record<string, LogEntry[]>;

  /**
   * Metadata per task (e.g., last log id for dedupe)
   */
  taskLogMeta: Record<string, { lastId?: number }>;

  /**
   * Append a single log entry to a task's log buffer.
   * Automatically trims to MAX_ENTRIES_PER_TASK.
   */
  appendLog: (taskId: string, entry: LogEntry) => void;

  /**
   * Append multiple log entries at once (for batch operations).
   * More efficient than calling appendLog repeatedly.
   */
  appendLogs: (taskId: string, entries: LogEntry[]) => void;

  /**
   * Clear all logs for a specific task.
   * Call this when a task is deleted or explicitly cleared by the user.
   */
  clearLogs: (taskId: string) => void;

  /**
   * Get all logs for a task. Returns empty array if no logs exist.
   */
  getLogs: (taskId: string) => LogEntry[];

  /**
   * Check if we have any logs for a task.
   */
  hasLogs: (taskId: string) => boolean;

  /**
   * Get the count of logs for a task.
   */
  getLogCount: (taskId: string) => number;

  /**
   * Get the last persisted log id for a task.
   */
  getLastLogId: (taskId: string) => number | null;
}

/**
 * Task log store - persists logs across component mount/unmount cycles.
 *
 * Usage:
 *   const logs = useLogStore(state => state.getLogs(taskId));
 *   const appendLog = useLogStore(state => state.appendLog);
 */
export const useLogStore = create<LogStore>()((set, get) => ({
  taskLogs: {},
  taskLogMeta: {},

  appendLog: (taskId, entry) => {
    set((state) => {
      const existing = state.taskLogs[taskId] || [];
      const meta = state.taskLogMeta[taskId] || {};
      const lastId = meta.lastId;

      if (entry.id !== undefined && lastId !== undefined && entry.id <= lastId) {
        return state;
      }

      const newLogs = [...existing, entry];
      const nextLastId = entry.id !== undefined ? entry.id : lastId;

      return {
        taskLogs: {
          ...state.taskLogs,
          [taskId]: newLogs,
        },
        taskLogMeta: {
          ...state.taskLogMeta,
          [taskId]: { lastId: nextLastId },
        },
      };
    });
  },

  appendLogs: (taskId, entries) => {
    if (entries.length === 0) return;

    set((state) => {
      const existing = state.taskLogs[taskId] || [];
      const meta = state.taskLogMeta[taskId] || {};
      let lastId = meta.lastId;
      const toAppend: LogEntry[] = [];

      for (const entry of entries) {
        if (entry.id !== undefined && lastId !== undefined && entry.id <= lastId) {
          continue;
        }
        toAppend.push(entry);
        if (entry.id !== undefined) {
          lastId = entry.id;
        }
      }

      if (toAppend.length === 0) {
        return state;
      }

      const newLogs = [...existing, ...toAppend];

      return {
        taskLogs: {
          ...state.taskLogs,
          [taskId]: newLogs,
        },
        taskLogMeta: {
          ...state.taskLogMeta,
          [taskId]: { lastId },
        },
      };
    });
  },

  clearLogs: (taskId) => {
    set((state) => {
      const { [taskId]: _, ...restLogs } = state.taskLogs;
      const { [taskId]: __, ...restMeta } = state.taskLogMeta;
      return { taskLogs: restLogs, taskLogMeta: restMeta };
    });
  },

  getLogs: (taskId) => {
    return get().taskLogs[taskId] ?? EMPTY_LOGS;
  },

  hasLogs: (taskId) => {
    const logs = get().taskLogs[taskId];
    return logs !== undefined && logs.length > 0;
  },

  getLogCount: (taskId) => {
    return get().taskLogs[taskId]?.length || 0;
  },

  getLastLogId: (taskId) => {
    return get().taskLogMeta[taskId]?.lastId ?? null;
  },
}));
