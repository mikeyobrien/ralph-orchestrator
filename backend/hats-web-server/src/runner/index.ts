/**
 * Runner module
 *
 * Provides the HatsRunner service for spawning and managing hats run child processes.
 * This is Step 4 of the implementation - the bridge between task execution and actual CLI invocation.
 */

// State management
export {
  RunnerState,
  isTerminalRunnerState,
  isValidRunnerTransition,
  getAllowedRunnerTransitions,
} from "./RunnerState";

// Log capture
export { LogStream } from "./LogStream";
export type { LogEntry, LogCallback, LogStreamOptions } from "./LogStream";

// Prompt management
export { PromptWriter } from "./PromptWriter";
export type { PromptContent, PromptWriterOptions } from "./PromptWriter";

// Main runner service
export { HatsRunner } from "./HatsRunner";
export type { HatsRunnerOptions, RunnerResult, HatsRunnerEvents } from "./HatsRunner";
export { createTestLogTaskHandler } from "./TestLogTaskHandler";
export type { TestLogTaskPayload } from "./TestLogTaskHandler";

// Task handler factory (integrates with Dispatcher and LogBroadcaster)
export { createHatsTaskHandler } from "./HatsTaskHandler";
export type { HatsTaskPayload, HatsTaskHandlerOptions } from "./HatsTaskHandler";

// Event parsing (detects Hats orchestrator events from stdout)
export { HatsEventParser } from "./HatsEventParser";
export type { HatsEvent, EventCallback } from "./HatsEventParser";
