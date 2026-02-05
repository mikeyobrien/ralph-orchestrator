/**
 * TaskStatusBar Component Tests
 *
 * Tests for the TaskStatusBar component that displays:
 * - Status badge with color-coded indicator based on task status
 * - Optional loop badge (clickable, links to loop detail page)
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TaskStatusBar, type TaskStatusBarProps } from "./TaskStatusBar";

// Mock react-router-dom for navigation testing
const mockNavigate = vi.fn();
vi.mock("react-router-dom", () => ({
  useNavigate: () => mockNavigate,
}));

describe("TaskStatusBar", () => {
  const defaultProps: TaskStatusBarProps = {
    status: "open",
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("status badge", () => {
    it("renders a status badge", () => {
      render(<TaskStatusBar {...defaultProps} />);

      // Should render the status label
      expect(screen.getByText("Open")).toBeInTheDocument();
    });

    it("displays 'Open' with secondary styling for open status", () => {
      render(<TaskStatusBar status="open" />);

      const badge = screen.getByText("Open").closest('[class*="badge"]');
      expect(badge).toBeInTheDocument();
      // Open status should have secondary/muted styling
      expect(screen.getByText("Open")).toBeInTheDocument();
    });

    it("displays 'Running' with blue styling for running status", () => {
      render(<TaskStatusBar status="running" />);

      expect(screen.getByText("Running")).toBeInTheDocument();
      // Running tasks should have distinctive blue styling
    });

    it("displays 'Completed' with green styling for completed status", () => {
      render(<TaskStatusBar status="completed" />);

      expect(screen.getByText("Completed")).toBeInTheDocument();
    });

    it("displays 'Failed' with destructive/red styling for failed status", () => {
      render(<TaskStatusBar status="failed" />);

      expect(screen.getByText("Failed")).toBeInTheDocument();
      // Failed status should have destructive variant
      const badge = screen.getByText("Failed").closest('[class*="badge"]');
      expect(badge).toHaveClass("bg-destructive");
    });

    it("displays 'Closed' for closed status", () => {
      render(<TaskStatusBar status="closed" />);

      expect(screen.getByText("Closed")).toBeInTheDocument();
    });

    it("shows a status icon alongside the label", () => {
      render(<TaskStatusBar status="running" />);

      // Running status should have a spinning loader icon
      expect(document.querySelector(".animate-spin")).toBeInTheDocument();
    });

    it("shows check icon for completed status", () => {
      render(<TaskStatusBar status="completed" />);

      // Completed status should have a check icon
      expect(document.querySelector("[class*='lucide']")).toBeInTheDocument();
    });
  });

  describe("loop badge", () => {
    it("does not render loop badge when loopId is not provided", () => {
      render(<TaskStatusBar status="open" />);

      // Should not have any loop-related content
      expect(screen.queryByText(/loop/i)).not.toBeInTheDocument();
    });

    it("renders loop badge when loopId and loopStatus are provided", () => {
      render(
        <TaskStatusBar
          status="running"
          loopId="loop-123"
          loopStatus="running"
        />
      );

      // Should show the loop badge with status
      expect(screen.getByText("running")).toBeInTheDocument();
    });

    it("loop badge is clickable and navigates to loop detail", () => {
      render(
        <TaskStatusBar
          status="running"
          loopId="loop-123"
          loopStatus="running"
        />
      );

      // Find the loop badge and click it
      const loopBadge = screen.getByRole("button");
      fireEvent.click(loopBadge);

      expect(mockNavigate).toHaveBeenCalledWith("/loops/loop-123");
    });

    it("does not render loop badge when loopId is provided but loopStatus is not", () => {
      render(<TaskStatusBar status="open" loopId="loop-123" />);

      // LoopBadge returns null when status is null/undefined
      expect(screen.queryByRole("button")).not.toBeInTheDocument();
    });

    it("shows 'Loop:' prefix in loop badge by default", () => {
      render(
        <TaskStatusBar
          status="running"
          loopId="loop-123"
          loopStatus="queued"
        />
      );

      expect(screen.getByText("Loop:")).toBeInTheDocument();
    });
  });

  describe("layout", () => {
    it("renders status badge and loop badge in a horizontal row", () => {
      render(
        <TaskStatusBar
          status="running"
          loopId="loop-123"
          loopStatus="running"
        />
      );

      // Container should use flexbox for horizontal layout
      const container = screen.getByText("Running").closest("div");
      expect(container?.parentElement).toHaveClass("flex");
    });

    it("has appropriate gap between badges", () => {
      render(
        <TaskStatusBar
          status="running"
          loopId="loop-123"
          loopStatus="running"
        />
      );

      // Should have gap between badges
      const container = screen.getByText("Running").closest("div")?.parentElement;
      expect(container).toHaveClass("gap-2");
    });

    it("aligns badges vertically in the center", () => {
      render(
        <TaskStatusBar
          status="running"
          loopId="loop-123"
          loopStatus="running"
        />
      );

      const container = screen.getByText("Running").closest("div")?.parentElement;
      expect(container).toHaveClass("items-center");
    });
  });

  describe("className prop", () => {
    it("applies additional className to container", () => {
      render(<TaskStatusBar status="open" className="custom-class" />);

      // Find container and check for custom class
      const container = screen.getByText("Open").closest("div")?.parentElement;
      expect(container).toHaveClass("custom-class");
    });
  });
});
