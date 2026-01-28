/**
 * RalphTaskHandler
 *
 * Factory function that creates a Dispatcher-compatible task handler
 * wrapping RalphRunner with LogBroadcaster integration.
 *
 * This is the glue between:
 * - Dispatcher: Executes tasks from the queue
 * - RalphRunner: Spawns and manages ralph child processes
 * - LogBroadcaster: Streams logs to WebSocket clients
 *
 * Design Notes:
 * - Factory pattern keeps RalphRunner decoupled from WebSocket concerns
 * - Each task execution creates a fresh RalphRunner instance
 * - State changes and output are broadcast to subscribed clients
 */

import { QueuedTask, TaskExecutionContext, TaskHandler } from "../queue";
import { RalphRunner, RalphRunnerOptions, RunnerResult } from "./RalphRunner";
import { getLogBroadcaster } from "../api/LogBroadcaster";
import { RunnerState } from "./RunnerState";
import { RalphEventParser } from "./RalphEventParser";

/**
 * Payload expected by the ralph task handler
 */
export interface RalphTaskPayload {
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
 * Options for creating a ralph task handler
 */
export interface RalphTaskHandlerOptions extends Omit<RalphRunnerOptions, "onOutput" | "cwd"> {
  /** Default working directory (can be overridden per-task) */
  defaultCwd?: string;
}

/**
 * Creates a task handler that executes ralph run commands and broadcasts output.
 *
 * @param options - RalphRunner configuration options
 * @returns TaskHandler compatible with Dispatcher.registerHandler()
 *
 * @example
 * ```typescript
 * const dispatcher = new Dispatcher(queue, eventBus);
 * dispatcher.registerHandler('ralph.run', createRalphTaskHandler({
 *   command: 'ralph',
 *   defaultCwd: process.cwd(),
 * }));
 * ```
 */
export function createRalphTaskHandler(
  options: RalphTaskHandlerOptions = {}
): TaskHandler<RalphTaskPayload, RunnerResult> {
  const { defaultCwd, ...runnerOptions } = options;

  return async (task: QueuedTask, context: TaskExecutionContext): Promise<RunnerResult> => {
    const payload = task.payload as unknown as RalphTaskPayload;
    const broadcaster = getLogBroadcaster();

    // Use dbTaskId for broadcasting so frontend can subscribe with database task ID
    // Falls back to queue task ID if dbTaskId not provided (for direct queue usage)
    const broadcastId = payload.dbTaskId || task.id;

    // Create a fresh runner for this task
    // Pass dbTaskId as taskId so ProcessSupervisor can find the process for cancellation
    const runner = new RalphRunner({
      ...runnerOptions,
      cwd: payload.cwd ?? defaultCwd,
      taskId: payload.dbTaskId,
    });

    // Create event parser to detect Ralph events from stdout
    const eventParser = new RalphEventParser((event) => {
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
      // Execute the ralph command
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
