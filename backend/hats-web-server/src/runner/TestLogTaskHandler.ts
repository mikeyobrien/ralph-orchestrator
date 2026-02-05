/**
 * TestLogTaskHandler
 *
 * Task handler used in test mode to emit deterministic log lines.
 */

import { TaskExecutionContext, TaskHandler, QueuedTask } from "../queue";
import { getLogBroadcaster } from "../api/LogBroadcaster";
import { LogEntry } from "./LogStream";

export interface TestLogTaskPayload {
  /** Prompt text used to build default log lines */
  prompt?: string;
  /** Database task ID for broadcasting (allows frontend to subscribe with DB task ID) */
  dbTaskId?: string;
  /** Custom log lines to emit */
  lines?: string[];
  /** Delay between lines in milliseconds */
  intervalMs?: number;
  /** Delay before first log line in milliseconds */
  initialDelayMs?: number;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export function createTestLogTaskHandler(): TaskHandler<TestLogTaskPayload, { lines: string[] }> {
  return async (task: QueuedTask, context: TaskExecutionContext) => {
    const payload = task.payload as unknown as TestLogTaskPayload;
    const prompt = typeof payload.prompt === "string" ? payload.prompt : "task";
    const lines = payload.lines ?? [`running: ${prompt}`, `completed: ${prompt}`];

    const intervalMs = payload.intervalMs ?? Number(process.env.HATS_TEST_LOG_INTERVAL_MS ?? 500);
    const initialDelayMs =
      payload.initialDelayMs ?? Number(process.env.HATS_TEST_LOG_INITIAL_DELAY_MS ?? 300);

    const broadcaster = getLogBroadcaster();
    const broadcastId = payload.dbTaskId || task.id;

    broadcaster.broadcastStatus(broadcastId, "running");

    if (initialDelayMs > 0) {
      await sleep(initialDelayMs);
    }

    for (let i = 0; i < lines.length; i += 1) {
      if (context.signal.aborted) {
        throw new Error("Task aborted");
      }

      const entry: LogEntry = {
        line: lines[i],
        timestamp: new Date(),
        source: i === 0 ? "stdout" : "stderr",
      };

      broadcaster.broadcast(broadcastId, entry);

      if (intervalMs > 0 && i < lines.length - 1) {
        await sleep(intervalMs);
      }
    }

    broadcaster.broadcastStatus(broadcastId, "completed");

    return { lines };
  };
}
