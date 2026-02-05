/**
 * TaskMetadataGrid Component Tests
 */

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { TaskMetadataGrid } from "./TaskMetadataGrid";
import type { Task } from "./TaskThread";

const createMockTask = (overrides: Partial<Task> = {}): Task => ({
  id: "task-1",
  title: "Test Task",
  status: "completed",
  priority: 2,
  blockedBy: null,
  createdAt: "2026-01-27T20:00:00.000Z",
  updatedAt: "2026-01-27T20:15:00.000Z",
  startedAt: "2026-01-27T20:01:00.000Z",
  completedAt: "2026-01-27T20:14:00.000Z",
  durationMs: 780000, // 13 minutes
  exitCode: 0,
  errorMessage: null,
  ...overrides,
});

describe("TaskMetadataGrid", () => {
  describe("rendering", () => {
    it("renders the metadata grid container", () => {
      const task = createMockTask();
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByTestId("metadata-grid")).toBeInTheDocument();
    });

    it("displays created timestamp", () => {
      const task = createMockTask();
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByTestId("metadata-created")).toBeInTheDocument();
      expect(screen.getByText("Created")).toBeInTheDocument();
    });

    it("displays updated timestamp", () => {
      const task = createMockTask();
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByTestId("metadata-updated")).toBeInTheDocument();
      expect(screen.getByText("Updated")).toBeInTheDocument();
    });

    it("displays duration from durationMs", () => {
      const task = createMockTask({ durationMs: 780000 }); // 13m
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByTestId("metadata-duration")).toBeInTheDocument();
      expect(screen.getByText("13m 0s")).toBeInTheDocument();
    });

    it("displays exit code", () => {
      const task = createMockTask({ exitCode: 0 });
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByTestId("metadata-exit-code")).toBeInTheDocument();
      expect(screen.getByText("0")).toBeInTheDocument();
    });

    it('displays "-" for missing exit code', () => {
      const task = createMockTask({ exitCode: null });
      render(<TaskMetadataGrid task={task} />);

      const exitCode = screen.getByTestId("metadata-exit-code");
      expect(exitCode).toHaveTextContent("-");
    });
  });

  describe("duration formatting", () => {
    it("formats hours and minutes", () => {
      const task = createMockTask({ durationMs: 3900000 }); // 1h 5m
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByText("1h 5m")).toBeInTheDocument();
    });

    it("formats minutes and seconds", () => {
      const task = createMockTask({ durationMs: 125000 }); // 2m 5s
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByText("2m 5s")).toBeInTheDocument();
    });

    it("formats seconds only", () => {
      const task = createMockTask({ durationMs: 45000 }); // 45s
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByText("45s")).toBeInTheDocument();
    });

    it("calculates duration from timestamps when durationMs not available", () => {
      const task = createMockTask({
        durationMs: undefined,
        startedAt: "2026-01-27T20:00:00.000Z",
        completedAt: "2026-01-27T20:02:00.000Z", // 2 minutes
      });
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByText("2m 0s")).toBeInTheDocument();
    });

    it('displays "-" when no duration data available', () => {
      const task = createMockTask({
        durationMs: undefined,
        startedAt: undefined,
        completedAt: undefined,
      });
      render(<TaskMetadataGrid task={task} />);

      const duration = screen.getByTestId("metadata-duration");
      expect(duration).toHaveTextContent("-");
    });
  });

  describe("metrics", () => {
    it('displays token counts when provided', () => {
      const task = createMockTask();
      render(
        <TaskMetadataGrid
          task={task}
          metrics={{ tokensIn: 45230, tokensOut: 12450 }}
        />
      );

      expect(screen.getByText("45,230 in / 12,450 out")).toBeInTheDocument();
    });

    it('displays "-" for tokens when not provided', () => {
      const task = createMockTask();
      render(<TaskMetadataGrid task={task} />);

      const tokens = screen.getByTestId("metadata-tokens");
      expect(tokens).toHaveTextContent("-");
    });

    it("displays estimated cost when provided", () => {
      const task = createMockTask();
      render(
        <TaskMetadataGrid task={task} metrics={{ estimatedCost: 0.42 }} />
      );

      expect(screen.getByText("~$0.42")).toBeInTheDocument();
    });

    it('displays "-" for cost when not provided', () => {
      const task = createMockTask();
      render(<TaskMetadataGrid task={task} />);

      const cost = screen.getByTestId("metadata-cost");
      expect(cost).toHaveTextContent("-");
    });
  });

  describe("error display", () => {
    it("displays error message when present", () => {
      const task = createMockTask({
        errorMessage: "Process exited with code 137",
      });
      render(<TaskMetadataGrid task={task} />);

      expect(screen.getByTestId("metadata-error")).toBeInTheDocument();
      expect(
        screen.getByText("Process exited with code 137")
      ).toBeInTheDocument();
    });

    it("does not display error section when no error", () => {
      const task = createMockTask({ errorMessage: null });
      render(<TaskMetadataGrid task={task} />);

      expect(screen.queryByTestId("metadata-error")).not.toBeInTheDocument();
    });
  });

  describe("custom className", () => {
    it("applies custom className", () => {
      const task = createMockTask();
      const { container } = render(
        <TaskMetadataGrid task={task} className="custom-class" />
      );

      expect(container.firstChild).toHaveClass("custom-class");
    });
  });
});
