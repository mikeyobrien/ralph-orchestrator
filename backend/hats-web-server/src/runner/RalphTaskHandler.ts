/**
 * HatsTaskHandler
 *
 * Factory function that creates a Dispatcher-compatible task handler
 * wrapping HatsRunner with LogBroadcaster integration.
 *
 * This is the glue between:
 * - Dispatcher: Executes tasks from the queue
 * - HatsRunner: Spawns and manages hats child processes
 * - LogBroadcaster: Streams logs to WebSocket clients
 *
 * Design Notes:
 * - Factory pattern keeps HatsRunner decoupled from WebSocket concerns
 * - Each task execution creates a fresh HatsRunner instance
 * - State changes and output are broadcast to subscribed clients
 */

import { QueuedTask, TaskExecutionContext, TaskHandler } from "../queue";
import { HatsRunner, HatsRunnerOptions, RunnerResult } from "./HatsRunner";
import { getLogBroadcaster } from "../api/LogBroadcaster";
import { RunnerState } from "./RunnerState";
import { HatsEventParser } from "./HatsEventParser";

/**
 * Payload expected by the hats task handler
 */
export interface HatsTaskPayload {
  /** The prompt text to execute */
  prompt: string;
  /** Additional CLI arguments */
  args?: string[];
  /** Working directory override */
  cwd?: string;
  /** Database task ID for broadcasting (allows frontend to subscribe with DB task ID) */
  dbTaskId?: string;
}

/**
 * Options for creating a hats task handler
 */
export interface HatsTaskHandlerOptions extends Omit<HatsRunnerOptions, "onOutput" | "cwd"> {
  /** Default working directory (can be overridden per-task) */
  defaultCwd?: string;
}

/**
 * Creates a task handler that executes hats run commands and broadcasts output.
 *
 * @param options - HatsRunner configuration options
 * @returns TaskHandler compatible with Dispatcher.registerHandler()
 *
 * @example
 * ```typescript
 * const dispatcher = new Dispatcher(queue, eventBus);
 * dispatcher.registerHandler('hats.run', createHatsTaskHandler({
 *   command: 'hats',
 *   defaultCwd: process.cwd(),
 * }));
 * ```
 */
export function createHatsTaskHandler(
  options: HatsTaskHandlerOptions = {}
): TaskHandler<HatsTaskPayload, RunnerResult> {
  const { defaultCwd, ...runnerOptions } = options;

  return async (task: QueuedTask, context: TaskExecutionContext): Promise<RunnerResult> => {
    const payload = task.payload as unknown as HatsTaskPayload;
    const broadcaster = getLogBroadcaster();

    // Use dbTaskId for broadcasting so frontend can subscribe with database task ID
    // Falls back to queue task ID if dbTaskId not provided (for direct queue usage)
    const broadcastId = payload.dbTaskId || task.id;

    // Create a fresh runner for this task
    // Pass dbTaskId as taskId so ProcessSupervisor can find the process for cancellation
    const runner = new HatsRunner({
      ...runnerOptions,
      cwd: payload.cwd ?? defaultCwd,
      taskId: payload.dbTaskId,
    });

    // Create event parser to detect Hats events from stdout
    const eventParser = new HatsEventParser((event) => {
      broadcaster.broadcastEvent(broadcastId, event);
    });

    // Wire output events to LogBroadcaster
    runner.on("output", (entry) => {
      // Broadcast the log entry to clients
      broadcaster.broadcast(broadcastId, entry);

      // Also check if this line is an event and broadcast if so
      eventParser.parseLine(entry.line);
    });

    // Wire state changes to LogBroadcaster
    runner.on("stateChange", (state: RunnerState, _previousState: RunnerState) => {
      broadcaster.broadcastStatus(broadcastId, state);
    });

    // Broadcast task start
    broadcaster.broadcastStatus(broadcastId, "starting");

    try {
      // Execute the hats command
      const result = await runner.run(payload.prompt, payload.args ?? [], context.signal);

      // Broadcast final status based on result
      broadcaster.broadcastStatus(broadcastId, result.state);

      // Clean up
      runner.dispose();

      // If the runner result indicates failure, throw to trigger Dispatcher's failure path
      // This ensures task.failed event is published instead of task.completed
      if (result.state === RunnerState.FAILED) {
        throw new Error(result.error || `Process exited with code ${result.exitCode ?? 1}`);
      }

      return result;
    } catch (error) {
      // Broadcast failure status first, then the error details
      broadcaster.broadcastStatus(broadcastId, "failed");
      const errorMsg = error instanceof Error ? error.message : String(error);
      broadcaster.broadcastError(broadcastId, errorMsg);

      // Clean up
      runner.dispose();

      throw error;
    }
  };
}
