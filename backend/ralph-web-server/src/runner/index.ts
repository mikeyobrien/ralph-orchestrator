/**
 * Runner module
 *
 * Provides the RalphRunner service for spawning and managing ralph run child processes.
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
export { RalphRunner } from "./RalphRunner";
export type { RalphRunnerOptions, RunnerResult, RalphRunnerEvents } from "./RalphRunner";
export { createTestLogTaskHandler } from "./TestLogTaskHandler";
export type { TestLogTaskPayload } from "./TestLogTaskHandler";

// Task handler factory (integrates with Dispatcher and LogBroadcaster)
export { createRalphTaskHandler } from "./RalphTaskHandler";
export type { RalphTaskPayload, RalphTaskHandlerOptions } from "./RalphTaskHandler";

// Event parsing (detects Ralph orchestrator events from stdout)
