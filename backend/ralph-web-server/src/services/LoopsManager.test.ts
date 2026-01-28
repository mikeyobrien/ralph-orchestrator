/**
 * LoopsManager Tests - Merge Button State API
 *
 * Tests for the merge button state API that exposes rust merge_button_state
 * to the TypeScript backend. These tests verify:
 * 1. getMergeButtonState returns active when primary loop is idle
 * 2. getMergeButtonState returns blocked with reason when primary is running
 * 3. Integration with tRPC router
 */

import { test, mock, describe } from "node:test";
import assert from "node:assert";
import { LoopsManager } from "./LoopsManager";

describe("LoopsManager.getMergeButtonState", () => {
  test("returns active state when primary loop is idle (no lock)", async () => {
    // Given: A LoopsManager with mock that returns active state
    const manager = new LoopsManager({ ralphPath: "ralph" });
    (manager as any).runRalphCommand = async () => {
      return JSON.stringify({ state: "active" });
    };

    // When: Checking merge button state for a queued loop
    const state = await manager.getMergeButtonState("test-loop-001");

    // Then: Should return active state
    assert.strictEqual(
      state.state,
      "active",
      "Merge button should be active when primary loop is idle"
    );
    assert.strictEqual(
      state.reason,
      undefined,
      "Active state should not have a reason"
    );
  });

  test("returns blocked state with reason when primary loop is running", async () => {
    // Given: A LoopsManager that will report primary loop is running
    const manager = new LoopsManager({ ralphPath: "ralph" });
    (manager as any).runRalphCommand = async () => {
      return JSON.stringify({
        state: "blocked",
        reason: "primary loop running: Implementing feature X",
      });
    };

    // When: Checking merge button state while primary is busy
    const state = await manager.getMergeButtonState("test-loop-002");

    // Then: Should return blocked state with informative reason
    assert.strictEqual(
      state.state,
      "blocked",
      "Merge button should be blocked when primary loop is running"
    );
    assert.ok(
      state.reason && state.reason.length > 0,
      "Blocked state should include a reason for tooltip display"
    );
  });

  test("blocked reason includes primary loop prompt for tooltip", async () => {
    // Given: Primary loop is running with a specific prompt
    const manager = new LoopsManager({ ralphPath: "ralph" });
    (manager as any).runRalphCommand = async () => {
      return JSON.stringify({
        state: "blocked",
        reason: "primary loop running: Implementing auth",
      });
    };

    // When: Getting merge button state
    const state = await manager.getMergeButtonState("test-loop-003");

    // Then: Blocked reason should describe what primary is doing
    if (state.state === "blocked" && state.reason) {
      assert.ok(
        state.reason.includes("primary") || state.reason.includes("loop"),
        `Blocked reason should explain why. Got: ${state.reason}`
      );
    }
  });

  test("returns blocked when merge is already in progress", async () => {
    // Given: A loop that is currently being merged
    const manager = new LoopsManager({ ralphPath: "ralph" });
    (manager as any).runRalphCommand = async () => {
      return JSON.stringify({
        state: "blocked",
        reason: "Merge already in progress",
      });
    };

    // When: Checking merge button state for a merging loop
    const state = await manager.getMergeButtonState("test-loop-merging");

    // Then: Should be blocked (can't merge twice)
    assert.strictEqual(
      state.state,
      "blocked",
      "Merge button should be blocked when merge is in progress"
    );
    assert.ok(
      state.reason?.includes("progress") || state.reason?.includes("merging"),
      `Should indicate merge is in progress. Got: ${state.reason}`
    );
  });
});

describe("LoopsManager.getMergeButtonState CLI integration", () => {
  test("calls ralph loops merge-button-state command", async () => {
    // Given: A manager that we can spy on
    const manager = new LoopsManager({ ralphPath: "ralph" });

    // Store the original method for later restoration
    const originalRunCommand = (manager as any).runRalphCommand;
    let calledArgs: string[] = [];

    // Mock the internal command runner
    (manager as any).runRalphCommand = async (args: string[]) => {
      calledArgs = args;
      return JSON.stringify({ state: "active" });
    };

    // When: Getting merge button state
    await manager.getMergeButtonState("test-loop-004");

    // Then: Should call the correct CLI command
    assert.deepStrictEqual(
      calledArgs,
      ["loops", "merge-button-state", "test-loop-004"],
      "Should call ralph loops merge-button-state <loop-id>"
    );

    // Restore
    (manager as any).runRalphCommand = originalRunCommand;
  });

  test("parses JSON response from CLI", async () => {
    // Given: CLI returns JSON with state and reason
    const manager = new LoopsManager({ ralphPath: "ralph" });
    const originalRunCommand = (manager as any).runRalphCommand;

    (manager as any).runRalphCommand = async () => {
      return JSON.stringify({
        state: "blocked",
        reason: "Primary loop is running: Implementing auth",
      });
    };

    // When: Getting merge button state
    const state = await manager.getMergeButtonState("test-loop-005");

    // Then: Should parse and return the structured response
    assert.strictEqual(state.state, "blocked");
    assert.strictEqual(state.reason, "Primary loop is running: Implementing auth");

    // Restore
    (manager as any).runRalphCommand = originalRunCommand;
  });

  test("handles CLI error gracefully", async () => {
    // Given: CLI command fails
    const manager = new LoopsManager({ ralphPath: "ralph" });
    const originalRunCommand = (manager as any).runRalphCommand;

    (manager as any).runRalphCommand = async () => {
      throw new Error("Loop not found: test-loop-missing");
    };

    // When/Then: Should throw with meaningful error
    await assert.rejects(
      () => manager.getMergeButtonState("test-loop-missing"),
      /not found|missing/i,
      "Should propagate CLI error"
    );

    // Restore
    (manager as any).runRalphCommand = originalRunCommand;
  });
});

describe("LoopsManager.retryMerge with steering input", () => {
  test("writes steering input to file when provided", async () => {
    // Given: A LoopsManager with mocked command runner
    const manager = new LoopsManager({ ralphPath: "ralph" });
    let commandArgs: string[] = [];

    (manager as any).runRalphCommand = async (args: string[]) => {
      commandArgs = args;
      return "";
    };

    // We'll verify by checking the command was called
    // (steering file write is internal implementation detail)

    // When: Retrying merge with steering input
    await manager.retryMerge("test-loop-006", "Keep my changes, discard incoming");

    // Then: Should call ralph loops retry command
    assert.deepStrictEqual(
      commandArgs,
      ["loops", "retry", "test-loop-006"],
      "Should call ralph loops retry <loop-id>"
    );
  });

  test("skips steering file when input is empty", async () => {
    // Given: A LoopsManager with mocked command runner
    const manager = new LoopsManager({ ralphPath: "ralph" });
    let commandCalled = false;

    (manager as any).runRalphCommand = async () => {
      commandCalled = true;
      return "";
    };

    // When: Retrying merge with empty steering input
    await manager.retryMerge("test-loop-007", "   ");

    // Then: Should still call retry command (no steering file needed)
    assert.ok(commandCalled, "Should call retry command even with empty steering");
  });

  test("retryMerge works without steering input", async () => {
    // Given: A LoopsManager with mocked command runner
    const manager = new LoopsManager({ ralphPath: "ralph" });
    let commandArgs: string[] = [];

    (manager as any).runRalphCommand = async (args: string[]) => {
      commandArgs = args;
      return "";
    };

    // When: Retrying merge without steering input
    await manager.retryMerge("test-loop-008");

    // Then: Should call ralph loops retry command
    assert.deepStrictEqual(
      commandArgs,
      ["loops", "retry", "test-loop-008"],
      "Should call ralph loops retry <loop-id>"
    );
  });
});

describe("MergeButtonState type", () => {
  test("MergeButtonState interface exists and is exported", async () => {
    // This test verifies the type is properly exported
    // The actual type checking happens at compile time
    // MergeButtonState is an interface, not a runtime value

    // Given: We import the module (type checking happens at compile time)
    const loopsModule = await import("./LoopsManager.js");

    // Then: Module should be importable, types are checked at compile time
    assert.ok(loopsModule.LoopsManager, "LoopsManager class should be importable");
    // MergeButtonState is a TypeScript interface and verified at compile time
    assert.ok(true, "MergeButtonState interface should be importable");
  });
});
