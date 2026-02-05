/**
 * LoopActions Component Tests - Merge Button State
 *
 * Tests that the LoopActions component correctly displays
 * merge button state (active/blocked) with appropriate visual
 * states and tooltip showing the blocked reason.
 */

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { LoopActions } from "./LoopActions";

describe("LoopActions merge button state", () => {
  describe("when mergeButtonState is active", () => {
    it("renders merge button with active visual state (green/enabled)", () => {
      // Given: A queued loop with active merge button state
      render(
        <LoopActions
          id="loop-001"
          status="queued"
          isGitWorkspace={true}
          mergeButtonState={{ state: "active" }}
        />
      );

      // Then: Merge button should be enabled
      const mergeButton = screen.getByRole("button", { name: /merge now/i });
      expect(mergeButton).toBeEnabled();
      expect(mergeButton).not.toHaveClass("opacity-50");
    });

    it("does not show blocked tooltip when active", () => {
      // Given: A queued loop with active merge button state
      render(
        <LoopActions
          id="loop-002"
          status="queued"
          isGitWorkspace={true}
          mergeButtonState={{ state: "active" }}
        />
      );

      // Then: Should not show blocked reason in tooltip
      const mergeButton = screen.getByRole("button", { name: /merge now/i });
      expect(mergeButton).not.toHaveAttribute("title", expect.stringContaining("blocked"));
    });
  });

  describe("when mergeButtonState is blocked", () => {
    it("renders merge button with blocked visual state (gray/disabled)", () => {
      // Given: A queued loop with blocked merge button state
      render(
        <LoopActions
          id="loop-003"
          status="queued"
          isGitWorkspace={true}
          mergeButtonState={{
            state: "blocked",
            reason: "Primary loop is running: Implementing authentication",
          }}
        />
      );

      // Then: Merge button should be disabled
      const mergeButton = screen.getByRole("button", { name: /merge now/i });
      expect(mergeButton).toBeDisabled();
    });

    it("shows blocked reason in tooltip", () => {
      // Given: A queued loop with blocked merge button state
      const blockedReason = "Primary loop is running: Implementing authentication";
      render(
        <LoopActions
          id="loop-004"
          status="queued"
          isGitWorkspace={true}
          mergeButtonState={{
            state: "blocked",
            reason: blockedReason,
          }}
        />
      );

      // Then: Merge button should have tooltip with blocked reason
      const mergeButton = screen.getByRole("button", { name: /merge now/i });
      expect(mergeButton).toHaveAttribute("title", expect.stringContaining(blockedReason));
    });

    it("applies blocked styling class to merge button", () => {
      // Given: A queued loop with blocked merge button state
      render(
        <LoopActions
          id="loop-005"
          status="queued"
          isGitWorkspace={true}
          mergeButtonState={{
            state: "blocked",
            reason: "Primary loop is busy",
          }}
        />
      );

      // Then: Merge button should have blocked visual indicator
      const mergeButton = screen.getByRole("button", { name: /merge now/i });
      // The button should have some visual indication of being blocked
      // This could be opacity, color change, or specific class
      expect(mergeButton).toHaveClass("opacity-50");
    });
  });

  describe("backwards compatibility", () => {
    it("renders merge button normally when mergeButtonState is not provided", () => {
      // Given: A queued loop without mergeButtonState (legacy behavior)
      render(
        <LoopActions
          id="loop-006"
          status="queued"
          isGitWorkspace={true}
        />
      );

      // Then: Merge button should render and be enabled (legacy behavior)
      const mergeButton = screen.getByRole("button", { name: /merge now/i });
      expect(mergeButton).toBeEnabled();
    });
  });
});
