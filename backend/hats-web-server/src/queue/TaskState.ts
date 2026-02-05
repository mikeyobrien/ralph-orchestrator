/**
 * TaskState Enum
 *
 * Represents the execution state of a task in the dispatcher queue.
 * Implements the "Employee" model state machine:
 *
 *   PENDING → RUNNING → COMPLETED
 *                ↓
 *             FAILED
 *
 * State transitions:
 * - PENDING: Task is queued and waiting for execution
 * - RUNNING: Task is currently being processed by a worker
 * - COMPLETED: Task finished successfully
 * - FAILED: Task encountered an error during execution
 */
export enum TaskState {
  /**
   * Task is queued and waiting to be picked up by a worker.
   * Initial state for all new tasks.
   */
  PENDING = "PENDING",

  /**
   * Task is currently being executed by a worker.
   * Only one worker should have a task in RUNNING state at a time.
   */
  RUNNING = "RUNNING",

  /**
   * Task completed successfully.
   * Terminal state - task will not be re-executed.
   */
  COMPLETED = "COMPLETED",

  /**
   * Task failed during execution.
   * May be retried depending on retry policy.
   */
  FAILED = "FAILED",

  /**
   * Task was cancelled by user.
   * Terminal state - task will not be executed.
   */
  CANCELLED = "CANCELLED",
}

/**
 * Check if a state is terminal (no further transitions possible)
 */
export function isTerminalState(state: TaskState): boolean {
  return (
    state === TaskState.COMPLETED || state === TaskState.FAILED || state === TaskState.CANCELLED
  );
}

/**
 * Check if a state transition is valid
 */
export function isValidTransition(from: TaskState, to: TaskState): boolean {
  switch (from) {
    case TaskState.PENDING:
      return to === TaskState.RUNNING || to === TaskState.CANCELLED;
    case TaskState.RUNNING:
      return to === TaskState.COMPLETED || to === TaskState.FAILED || to === TaskState.CANCELLED;
    case TaskState.COMPLETED:
    case TaskState.FAILED:
    case TaskState.CANCELLED:
      return false; // Terminal states
    default:
      return false;
  }
}

/**
 * Get allowed next states from current state
 */
export function getAllowedTransitions(state: TaskState): TaskState[] {
  switch (state) {
    case TaskState.PENDING:
      return [TaskState.RUNNING, TaskState.CANCELLED];
    case TaskState.RUNNING:
      return [TaskState.COMPLETED, TaskState.FAILED, TaskState.CANCELLED];
    case TaskState.COMPLETED:
    case TaskState.FAILED:
    case TaskState.CANCELLED:
      return [];
    default:
      return [];
  }
}
