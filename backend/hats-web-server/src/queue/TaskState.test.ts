import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { TaskState, isTerminalState, isValidTransition, getAllowedTransitions } from "./TaskState";

describe("TaskState", () => {
  describe("isTerminalState", () => {
    it("should identify COMPLETED as terminal", () => {
      assert.equal(isTerminalState(TaskState.COMPLETED), true);
    });

    it("should identify FAILED as terminal", () => {
      assert.equal(isTerminalState(TaskState.FAILED), true);
    });

    it("should identify CANCELLED as terminal", () => {
      assert.equal(isTerminalState(TaskState.CANCELLED), true);
    });

    it("should identify PENDING as non-terminal", () => {
      assert.equal(isTerminalState(TaskState.PENDING), false);
    });

    it("should identify RUNNING as non-terminal", () => {
      assert.equal(isTerminalState(TaskState.RUNNING), false);
    });
  });

  describe("isValidTransition", () => {
    // PENDING transitions
    it("should allow PENDING -> RUNNING", () => {
      assert.equal(isValidTransition(TaskState.PENDING, TaskState.RUNNING), true);
    });

    it("should allow PENDING -> CANCELLED", () => {
      assert.equal(isValidTransition(TaskState.PENDING, TaskState.CANCELLED), true);
    });

    it("should disallow PENDING -> COMPLETED", () => {
      assert.equal(isValidTransition(TaskState.PENDING, TaskState.COMPLETED), false);
    });

    // RUNNING transitions
    it("should allow RUNNING -> COMPLETED", () => {
      assert.equal(isValidTransition(TaskState.RUNNING, TaskState.COMPLETED), true);
    });

    it("should allow RUNNING -> FAILED", () => {
      assert.equal(isValidTransition(TaskState.RUNNING, TaskState.FAILED), true);
    });

    it("should allow RUNNING -> CANCELLED", () => {
      assert.equal(isValidTransition(TaskState.RUNNING, TaskState.CANCELLED), true);
    });

    it("should disallow RUNNING -> PENDING", () => {
      assert.equal(isValidTransition(TaskState.RUNNING, TaskState.PENDING), false);
    });

    // Terminal state transitions (should all be false)
    it("should disallow transitions from CANCELLED", () => {
      assert.equal(isValidTransition(TaskState.CANCELLED, TaskState.PENDING), false);
      assert.equal(isValidTransition(TaskState.CANCELLED, TaskState.RUNNING), false);
    });
  });

  describe("getAllowedTransitions", () => {
    it("should return correct transitions for PENDING", () => {
      const transitions = getAllowedTransitions(TaskState.PENDING);
      assert.deepEqual(transitions.sort(), [TaskState.RUNNING, TaskState.CANCELLED].sort());
    });

    it("should return correct transitions for RUNNING", () => {
      const transitions = getAllowedTransitions(TaskState.RUNNING);
      assert.deepEqual(
        transitions.sort(),
        [TaskState.COMPLETED, TaskState.FAILED, TaskState.CANCELLED].sort()
      );
    });

    it("should return empty array for CANCELLED", () => {
      const transitions = getAllowedTransitions(TaskState.CANCELLED);
      assert.deepEqual(transitions, []);
    });
  });
});
