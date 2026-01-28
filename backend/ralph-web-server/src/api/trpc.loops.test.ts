/**
 * tRPC Loops Router Tests - Merge Button State
 *
 * Tests for the loops.mergeButtonState tRPC endpoint that exposes
 * the rust merge_button_state API to the frontend.
 */

import { test, describe, mock } from "node:test";
import assert from "node:assert";
import { loopsRouter, createContext } from "./trpc";
import { initializeDatabase, getDatabase } from "../db/connection";
import { LoopsManager, type MergeButtonState } from "../services/LoopsManager";

// Helper to create a mock LoopsManager for testing
function createMockLoopsManager(
  mergeButtonState: MergeButtonState
): LoopsManager {
  const manager = new LoopsManager();
  // Override the method to return our test data
  manager.getMergeButtonState = async () => mergeButtonState;
  return manager;
}

describe("loops.mergeButtonState tRPC endpoint", () => {
  test("returns active state for mergeable loop", async () => {
    // Given: A loops manager that reports active state
    const mockManager = createMockLoopsManager({
      state: "active",
    });

    // Create a mock context with the manager
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, mockManager);

    // When: Calling the mergeButtonState endpoint
    const caller = loopsRouter.createCaller(ctx);
    const result = await caller.mergeButtonState({ id: "test-loop-001" });

    // Then: Should return the active state
    assert.strictEqual(result.state, "active");
    assert.strictEqual(result.reason, undefined);
  });

  test("returns blocked state with reason when primary is running", async () => {
    // Given: A loops manager that reports blocked state
    const mockManager = createMockLoopsManager({
      state: "blocked",
      reason: "Primary loop is running: Implementing authentication",
    });

    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, mockManager);

    // When: Calling the mergeButtonState endpoint
    const caller = loopsRouter.createCaller(ctx);
    const result = await caller.mergeButtonState({ id: "test-loop-002" });

    // Then: Should return blocked state with the reason
    assert.strictEqual(result.state, "blocked");
    assert.strictEqual(
      result.reason,
      "Primary loop is running: Implementing authentication"
    );
  });

  test("throws error when LoopsManager is not configured", async () => {
    // Given: A context without a LoopsManager
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, undefined);

    // When/Then: Should throw INTERNAL_SERVER_ERROR
    const caller = loopsRouter.createCaller(ctx);
    await assert.rejects(
      () => caller.mergeButtonState({ id: "test-loop-003" }),
      (err: any) => {
        assert.strictEqual(err.code, "INTERNAL_SERVER_ERROR");
        assert.ok(err.message.includes("LoopsManager"));
        return true;
      }
    );
  });

  test("validates loop ID is required", async () => {
    // Given: A configured context
    const mockManager = createMockLoopsManager({ state: "active" });
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, mockManager);

    // When/Then: Calling without id should fail validation
    const caller = loopsRouter.createCaller(ctx);
    await assert.rejects(
      // @ts-expect-error - intentionally passing invalid input
      () => caller.mergeButtonState({}),
      /id/i,
      "Should require loop ID"
    );
  });
});

describe("loops.list includes mergeButtonState field", () => {
  test("loop entries include mergeButtonState for queued loops", async () => {
    // Given: A loops manager that returns loops with merge button states
    const mockManager = new LoopsManager();
    mockManager.listLoops = async () => [
      {
        id: "loop-001",
        status: "queued",
        location: ".worktrees/loop-001",
        pid: 12345,
        prompt: "Add feature X",
      },
    ];
    mockManager.getMergeButtonState = async (id: string) => ({
      state: "active",
    });

    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, mockManager);

    // When: Listing loops
    const caller = loopsRouter.createCaller(ctx);
    const loops = await caller.list();

    // Then: Should include mergeButtonState for worktree loops
    const queuedLoop = loops.find((l: any) => l.id === "loop-001");
    assert.ok(queuedLoop, "Should have the queued loop");
    assert.ok(
      queuedLoop.mergeButtonState !== undefined,
      "Queued loop should include mergeButtonState"
    );
    assert.strictEqual(
      queuedLoop.mergeButtonState.state,
      "active",
      "Should show active merge button state"
    );
  });

  test("loop entries include blocked mergeButtonState when primary running", async () => {
    // Given: A loops manager that returns blocked state
    const mockManager = new LoopsManager();
    mockManager.listLoops = async () => [
      {
        id: "loop-002",
        status: "queued",
        location: ".worktrees/loop-002",
        pid: 12346,
        prompt: "Add feature Y",
      },
    ];
    mockManager.getMergeButtonState = async () => ({
      state: "blocked",
      reason: "Primary loop is busy",
    });

    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, mockManager);

    // When: Listing loops
    const caller = loopsRouter.createCaller(ctx);
    const loops = await caller.list();

    // Then: Should include blocked mergeButtonState
    const queuedLoop = loops.find((l: any) => l.id === "loop-002");
    assert.strictEqual(queuedLoop?.mergeButtonState?.state, "blocked");
    assert.strictEqual(
      queuedLoop?.mergeButtonState?.reason,
      "Primary loop is busy"
    );
  });

  test("primary loop (in-place) does not include mergeButtonState", async () => {
    // Given: A loops manager that returns the primary loop
    const mockManager = new LoopsManager();
    mockManager.listLoops = async () => [
      {
        id: "loop-primary",
        status: "running",
        location: "(in-place)",
        pid: 12347,
        prompt: "Working on main repo",
      },
    ];

    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, mockManager);

    // When: Listing loops
    const caller = loopsRouter.createCaller(ctx);
    const loops = await caller.list();

    // Then: Primary loop should NOT have mergeButtonState (it's the primary, not a worktree)
    const primaryLoop = loops.find((l: any) => l.id === "loop-primary");
    assert.ok(primaryLoop, "Should have the primary loop");
    assert.strictEqual(
      primaryLoop.mergeButtonState,
      undefined,
      "Primary loop should not have mergeButtonState (only worktrees need merge buttons)"
    );
  });
});

describe("loops.triggerMergeTask endpoint", () => {
  // Helper to create a mock TaskBridge
  function createMockTaskBridge(): any {
    return {
      enqueueTask: (task: any) => ({ success: true, queuedTaskId: `queued-${task.id}` }),
    };
  }

  test("creates a merge task for worktree loop", async () => {
    // Given: A loops manager with a worktree loop and a mock task bridge
    const mockManager = new LoopsManager();
    mockManager.listLoops = async () => [
      {
        id: "worktree-loop-123",
        status: "queued",
        location: ".worktrees/worktree-loop-123",
        pid: 12345,
        prompt: "Add user authentication feature",
      },
    ];

    initializeDatabase(getDatabase(":memory:"));
    const mockTaskBridge = createMockTaskBridge();
    const ctx = createContext(getDatabase(), mockTaskBridge, mockManager);

    // When: Triggering a merge task
    const caller = loopsRouter.createCaller(ctx);
    const result = await caller.triggerMergeTask({ loopId: "worktree-loop-123" });

    // Then: Should create a task
    assert.strictEqual(result.success, true);
    assert.ok(result.taskId, "Should return task ID");
    assert.ok(result.taskId.startsWith("merge-worktree-loop-123"), "Task ID should include loop ID");

    // Verify task was created in database
    const task = ctx.taskRepository.findById(result.taskId);
    assert.ok(task, "Task should exist in database");
    assert.ok(task.title.includes("Merge:"), "Task title should indicate merge");
    assert.ok(task.mergeLoopPrompt, "Task should have merge loop prompt");
    assert.ok(task.mergeLoopPrompt?.includes("worktree-loop-123"), "Prompt should include loop ID");
  });

  test("rejects merge task for in-place (primary) loop", async () => {
    // Given: A loops manager with a primary loop
    const mockManager = new LoopsManager();
    mockManager.listLoops = async () => [
      {
        id: "primary-loop",
        status: "running",
        location: "(in-place)",
        pid: 12345,
        prompt: "Working on main",
      },
    ];

    initializeDatabase(getDatabase(":memory:"));
    const mockTaskBridge = createMockTaskBridge();
    const ctx = createContext(getDatabase(), mockTaskBridge, mockManager);

    // When/Then: Should reject merge for primary loop
    const caller = loopsRouter.createCaller(ctx);
    await assert.rejects(
      () => caller.triggerMergeTask({ loopId: "primary-loop" }),
      (err: any) => {
        assert.strictEqual(err.code, "BAD_REQUEST");
        assert.ok(err.message.includes("in-place"));
        return true;
      }
    );
  });

  test("returns NOT_FOUND for non-existent loop", async () => {
    // Given: A loops manager with no loops
    const mockManager = new LoopsManager();
    mockManager.listLoops = async () => [];

    initializeDatabase(getDatabase(":memory:"));
    const mockTaskBridge = createMockTaskBridge();
    const ctx = createContext(getDatabase(), mockTaskBridge, mockManager);

    // When/Then: Should return NOT_FOUND
    const caller = loopsRouter.createCaller(ctx);
    await assert.rejects(
      () => caller.triggerMergeTask({ loopId: "non-existent-loop" }),
      (err: any) => {
        assert.strictEqual(err.code, "NOT_FOUND");
        return true;
      }
    );
  });

  test("throws error when LoopsManager is not configured", async () => {
    // Given: A context without a LoopsManager
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, undefined);

    // When/Then: Should throw INTERNAL_SERVER_ERROR
    const caller = loopsRouter.createCaller(ctx);
    await assert.rejects(
      () => caller.triggerMergeTask({ loopId: "any-loop" }),
      (err: any) => {
        assert.strictEqual(err.code, "INTERNAL_SERVER_ERROR");
        assert.ok(err.message.includes("LoopsManager"));
        return true;
      }
    );
  });

  test("throws error when TaskBridge is not configured", async () => {
    // Given: A context with LoopsManager but no TaskBridge
    const mockManager = new LoopsManager();
    mockManager.listLoops = async () => [
      {
        id: "worktree-loop",
        status: "queued",
        location: ".worktrees/worktree-loop",
        pid: 12345,
        prompt: "Some task",
      },
    ];

    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase(), undefined, mockManager);

    // When/Then: Should throw INTERNAL_SERVER_ERROR about TaskBridge
    const caller = loopsRouter.createCaller(ctx);
    await assert.rejects(
      () => caller.triggerMergeTask({ loopId: "worktree-loop" }),
      (err: any) => {
        assert.strictEqual(err.code, "INTERNAL_SERVER_ERROR");
        assert.ok(err.message.includes("TaskBridge"));
        return true;
      }
    );
  });
});
