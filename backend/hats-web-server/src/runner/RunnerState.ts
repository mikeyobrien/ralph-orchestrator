/**
 * RunnerState
 *
 * Represents the lifecycle states of a HatsRunner child process.
 * Follows the same pattern as TaskState for consistency.
 *
 * State Machine:
 * ```
 *   IDLE → SPAWNING → RUNNING → COMPLETED
 *                        ↓          ↓
 *                     CANCELLED  FAILED
 * ```
 */

/**
 * Possible states for a HatsRunner
 */
export enum RunnerState {
  /** Runner is idle, no process spawned */
  IDLE = "IDLE",
  /** Process is being spawned */
  SPAWNING = "SPAWNING",
  /** Process is running */
  RUNNING = "RUNNING",
  /** Process completed successfully (exit code 0) */
  COMPLETED = "COMPLETED",
  /** Process failed (non-zero exit code or error) */
  FAILED = "FAILED",
  /** Process was cancelled by stop() */
  CANCELLED = "CANCELLED",
}

/**
 * Check if a state is terminal (no further transitions possible)
 */
export function isTerminalRunnerState(state: RunnerState): boolean {
  switch (state) {
    case RunnerState.COMPLETED:
    case RunnerState.FAILED:
    case RunnerState.CANCELLED:
      return true;
    default:
      return false;
  }
}

/**
 * Check if a state transition is valid
 */
export function isValidRunnerTransition(from: RunnerState, to: RunnerState): boolean {
  switch (from) {
    case RunnerState.IDLE:
      return to === RunnerState.SPAWNING;
    case RunnerState.SPAWNING:
      return (
        to === RunnerState.RUNNING || to === RunnerState.FAILED || to === RunnerState.CANCELLED
      );
    case RunnerState.RUNNING:
      return (
        to === RunnerState.COMPLETED || to === RunnerState.FAILED || to === RunnerState.CANCELLED
      );
    default:
      // Terminal states cannot transition
      return false;
  }
}

/**
 * Get the allowed transitions from a state
 */
export function getAllowedRunnerTransitions(state: RunnerState): RunnerState[] {
  switch (state) {
    case RunnerState.IDLE:
      return [RunnerState.SPAWNING];
    case RunnerState.SPAWNING:
      return [RunnerState.RUNNING, RunnerState.FAILED, RunnerState.CANCELLED];
    case RunnerState.RUNNING:
      return [RunnerState.COMPLETED, RunnerState.FAILED, RunnerState.CANCELLED];
    default:
      return [];
  }
}
