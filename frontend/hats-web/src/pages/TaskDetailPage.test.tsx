/**
 * TaskDetailPage Component Tests
 *
 * Tests for the dedicated task detail page that replaces the inline
 * expansion pattern. The page displays:
 * - Full prompt display (not truncated)
 * - Rich status metrics (duration, timestamps, exit code)
 * - Log viewer
 * - Action buttons (run, retry, cancel)
 * - Navigation back to task list
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { TaskDetailPage } from "./TaskDetailPage";

// Mock tRPC hooks
const mockTask = {
  id: "task-001",
  title: "Implement user authentication",
  status: "running",
  priority: 2,
  blockedBy: null,
  createdAt: "2024-01-15T10:00:00Z",
  updatedAt: "2024-01-15T12:30:00Z",
  startedAt: "2024-01-15T10:05:00Z",
  completedAt: null,
  errorMessage: null,
  executionSummary: null,
  exitCode: null,
  durationMs: null,
  archivedAt: null,
  loopId: "loop-001",
};

const mockCompletedTask = {
  ...mockTask,
  id: "task-002",
  status: "completed",
  completedAt: "2024-01-15T11:30:00Z",
  durationMs: 5400000, // 1.5 hours
  executionSummary: `## What Was Done
Implemented JWT-based authentication with refresh tokens.

## Key Changes
- Added auth middleware
- Created login/logout endpoints
- Integrated with user service

## Notes
Used bcrypt for password hashing.`,
  exitCode: 0,
};

const mockFailedTask = {
  ...mockTask,
  id: "task-003",
  status: "failed",
  completedAt: "2024-01-15T10:45:00Z",
  durationMs: 2400000, // 40 minutes
  errorMessage: "Build failed: TypeScript compilation error",
  exitCode: 1,
};

const mockOpenTask = {
  ...mockTask,
  id: "task-004",
  status: "open",
  startedAt: null,
  loopId: null,
};

// Task with "closed" status - this is what the database actually uses
// for successfully completed tasks (not "completed")
const mockClosedTask = {
  ...mockTask,
  id: "task-005",
  status: "closed",
  completedAt: "2024-01-15T11:30:00Z",
  durationMs: 3600000, // 1 hour
  executionSummary: "Task completed successfully.",
  exitCode: 0,
};

// Mock EnhancedLogViewer component
vi.mock("@/components/tasks/EnhancedLogViewer", () => ({
  EnhancedLogViewer: vi.fn(({ taskId }: { taskId: string }) => (
    <div data-testid="enhanced-log-viewer" data-task-id={taskId}>
      Mocked EnhancedLogViewer
    </div>
  )),
}));

// Mock TaskCardSkeleton component to track rendering
vi.mock("@/components/tasks/TaskCardSkeleton", () => ({
  TaskCardSkeleton: vi.fn(() => (
    <div data-testid="task-card-skeleton">Mocked TaskCardSkeleton</div>
  )),
}));

// Mock EmptyState component to track rendering
vi.mock("@/components/tasks/EmptyState", () => ({
  EmptyState: vi.fn(({ icon: Icon, title, description }: { icon: React.ComponentType<{ className?: string }>, title: string, description: string }) => (
    <div data-testid="empty-state">
      <Icon data-testid="empty-state-icon" className="test-icon" />
      <span data-testid="empty-state-title">{title}</span>
      <span data-testid="empty-state-description">{description}</span>
    </div>
  )),
}));

// Mock trpc
vi.mock("@/trpc", () => ({
  trpc: {
    task: {
      get: {
        useQuery: vi.fn(),
      },
      run: {
        useMutation: vi.fn(() => ({
          mutate: vi.fn(),
          isPending: false,
        })),
      },
      retry: {
        useMutation: vi.fn(() => ({
          mutate: vi.fn(),
          isPending: false,
        })),
      },
      cancel: {
        useMutation: vi.fn(() => ({
          mutate: vi.fn(),
          isPending: false,
        })),
      },
      delete: {
        useMutation: vi.fn(() => ({
          mutate: vi.fn(),
          isPending: false,
        })),
      },
    },
    loops: {
      list: {
        useQuery: vi.fn(() => ({
          data: [],
          isLoading: false,
          isError: false,
        })),
      },
      retry: {
        useMutation: vi.fn(() => ({
          mutate: vi.fn(),
          isPending: false,
        })),
      },
    },
    useUtils: vi.fn(() => ({
      task: { list: { invalidate: vi.fn() } },
      loops: { list: { invalidate: vi.fn() } },
    })),
  },
}));

// Mock react-router-dom useParams
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useParams: vi.fn(() => ({ id: "task-001" })),
    useNavigate: vi.fn(() => vi.fn()),
  };
});

// Helper to render with router
function renderWithRouter(taskId: string = "task-001") {
  return render(
    <MemoryRouter initialEntries={[`/tasks/${taskId}`]}>
      <Routes>
        <Route path="/tasks/:id" element={<TaskDetailPage />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("TaskDetailPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("page layout and navigation", () => {
    it("renders page title with task title", async () => {
      // Given: A task is loaded
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Task title should be displayed (rendered via ReactMarkdown as paragraph)
      expect(screen.getByText(/implement user authentication/i)).toBeInTheDocument();
    });

    it("renders back navigation button to task list", async () => {
      // Given: A task is loaded
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Back navigation should be present (now a button via TaskDetailHeader)
      expect(screen.getByRole("button", { name: /back to tasks/i })).toBeInTheDocument();
    });

    it("shows loading state while fetching task", async () => {
      // Given: Task is loading
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: undefined,
        isLoading: true,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Loading skeletons should be shown (multiple skeletons rendered)
      expect(screen.getAllByTestId("task-card-skeleton").length).toBeGreaterThan(0);
    });

    it("uses TaskCardSkeleton component for loading state", async () => {
      // Given: Task is loading
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: undefined,
        isLoading: true,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: TaskCardSkeleton should be rendered (multiple for better loading UX)
      expect(screen.getAllByTestId("task-card-skeleton").length).toBe(2);
    });

    it("shows error state when task fetch fails", async () => {
      // Given: Task fetch failed
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: undefined,
        isLoading: false,
        isError: true,
        error: { message: "Task not found" },
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Error message should be displayed
      expect(screen.getByText(/error/i)).toBeInTheDocument();
      expect(screen.getByText(/task not found/i)).toBeInTheDocument();
    });

    it("uses EmptyState component with AlertCircle icon for error state", async () => {
      // Given: Task fetch failed
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: undefined,
        isLoading: false,
        isError: true,
        error: { message: "Failed to load task" },
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: EmptyState should be rendered with AlertCircle icon
      expect(screen.getByTestId("empty-state")).toBeInTheDocument();
      const icon = screen.getByTestId("empty-state-icon");
      expect(icon).toBeInTheDocument();
      // Verify icon is AlertCircle (lucide class)
      expect(icon).toHaveClass("lucide-circle-alert");
    });

    it("shows not found state when task does not exist", async () => {
      // Given: Task query returns null
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: null,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Not found message should be displayed
      expect(screen.getByText(/task not found/i)).toBeInTheDocument();
    });

    it("uses EmptyState component with FileQuestion icon for not-found state", async () => {
      // Given: Task query returns null (task doesn't exist)
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: null,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: EmptyState should be rendered
      expect(screen.getByTestId("empty-state")).toBeInTheDocument();
      // Title should indicate task not found
      expect(screen.getByTestId("empty-state-title")).toHaveTextContent(/task not found/i);
    });
  });

  describe("full prompt display", () => {
    it("displays the full task title without truncation", async () => {
      // Given: A task with a long title
      const longTitleTask = {
        ...mockTask,
        title: "This is a very long task title that would normally be truncated in the list view but should be fully visible on the detail page",
      };
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: longTitleTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: The full title should be visible
      expect(screen.getByText(longTitleTask.title)).toBeInTheDocument();
    });
  });

  describe("status metrics", () => {
    it("displays current status badge for running task", async () => {
      // Given: A running task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Status badge should show "Running"
      expect(screen.getByText(/running/i)).toBeInTheDocument();
    });

    it("displays created and updated timestamps", async () => {
      // Given: A task with timestamps
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Timestamps should be displayed
      expect(screen.getByText(/created/i)).toBeInTheDocument();
      expect(screen.getByText(/updated/i)).toBeInTheDocument();
    });

    it("displays duration for completed tasks", async () => {
      // Given: A completed task with duration
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockCompletedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-002");

      // Then: Duration should be displayed (1.5 hours = "1h 30m")
      expect(screen.getByText(/1h 30m/i)).toBeInTheDocument();
    });

    it("displays exit code for completed tasks", async () => {
      // Given: A completed task with exit code
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockCompletedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-002");

      // Then: Exit code should be displayed (via TaskMetadataGrid)
      expect(screen.getByText(/exit code/i)).toBeInTheDocument();
      expect(screen.getByTestId("metadata-exit-code")).toHaveTextContent("0");
    });

    it("displays error message for failed tasks", async () => {
      // Given: A failed task with error message
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockFailedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-003");

      // Then: Error message should be displayed (via TaskMetadataGrid)
      expect(screen.getByTestId("metadata-error")).toBeInTheDocument();
      expect(screen.getByText(/typescript compilation error/i)).toBeInTheDocument();
    });

    it("displays metadata grid for completed tasks", async () => {
      // Given: A completed task with duration and exit code
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockCompletedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-002");

      // Then: Metadata grid should display timing and exit code info
      expect(screen.getByText(/created/i)).toBeInTheDocument();
      expect(screen.getByText(/exit code/i)).toBeInTheDocument();
    });
  });

  describe("action buttons", () => {
    it("shows Run button for open tasks", async () => {
      // Given: An open task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockOpenTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-004");

      // Then: Run button should be present
      expect(screen.getByRole("button", { name: /run/i })).toBeInTheDocument();
    });

    it("shows Cancel button for running tasks", async () => {
      // Given: A running task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Cancel button should be present
      expect(screen.getByRole("button", { name: /cancel/i })).toBeInTheDocument();
    });

    it("shows Retry button for failed tasks", async () => {
      // Given: A failed task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockFailedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-003");

      // Then: Retry button should be present
      expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();
    });

    it("does not show Run button for non-open tasks", async () => {
      // Given: A running task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Run button should not be present
      expect(screen.queryByRole("button", { name: /^run$/i })).not.toBeInTheDocument();
    });

    it("invokes run mutation when Run button is clicked", async () => {
      // Given: An open task
      const { trpc } = await import("@/trpc");
      const mockMutate = vi.fn();
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockOpenTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.task.run.useMutation).mockReturnValue({
        mutate: mockMutate,
        isPending: false,
      } as unknown as ReturnType<typeof trpc.task.run.useMutation>);

      // When: The Run button is clicked
      renderWithRouter("task-004");
      const user = userEvent.setup();
      await user.click(screen.getByRole("button", { name: /run/i }));

      // Then: The mutation should be invoked
      expect(mockMutate).toHaveBeenCalledWith({ id: "task-004" });
    });
  });

  describe("log viewer", () => {
    it("renders EnhancedLogViewer with task.id prop for running tasks", async () => {
      // Given: A running task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: EnhancedLogViewer should be rendered with task.id prop
      const { EnhancedLogViewer } = await import("@/components/tasks/EnhancedLogViewer");
      expect(vi.mocked(EnhancedLogViewer)).toHaveBeenCalledWith(
        expect.objectContaining({ taskId: "task-001" }),
        undefined
      );
    });

    it("renders log viewer for running tasks", async () => {
      // Given: A running task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Log viewer should be present
      expect(screen.getByTestId("log-viewer")).toBeInTheDocument();
    });

    it("renders log viewer for completed tasks", async () => {
      // Given: A completed task
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockCompletedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-002");

      // Then: Log viewer should be present
      expect(screen.getByTestId("log-viewer")).toBeInTheDocument();
    });

    it("does not render log viewer for open tasks", async () => {
      // Given: An open task (not yet run)
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockOpenTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-004");

      // Then: Log viewer should not be present
      expect(screen.queryByTestId("log-viewer")).not.toBeInTheDocument();
    });

    it("renders log viewer for closed tasks (database status for completed)", async () => {
      // Given: A closed task (this is the actual database status for completed tasks)
      // The database uses "closed", not "completed", for successfully finished tasks
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockClosedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-005");

      // Then: Log viewer should be present (users need to see logs for finished tasks)
      expect(screen.getByTestId("log-viewer")).toBeInTheDocument();
    });
  });

  describe("keyboard navigation", () => {
    it("navigates back to task list on Escape key press", async () => {
      // Given: A task is displayed
      const { trpc } = await import("@/trpc");
      const { useNavigate } = await import("react-router-dom");
      const mockNavigate = vi.fn();
      vi.mocked(useNavigate).mockReturnValue(mockNavigate);
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);

      // When: Escape key is pressed
      renderWithRouter("task-001");
      const user = userEvent.setup();
      await user.keyboard("{Escape}");

      // Then: Should navigate back to tasks list
      expect(mockNavigate).toHaveBeenCalledWith("/tasks");
    });
  });

  describe("user steering UI", () => {
    it("shows user steering callout when associated loop is in needs-review status", async () => {
      // Given: A task with a loop in needs-review status
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask, // mockTask.loopId = "loop-001"
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [
          {
            id: "loop-001", // Matches mockTask.loopId
            status: "needs-review",
            location: "/some/worktree",
            prompt: "Test prompt",
            failureReason: "Merge conflict in file.ts",
          },
        ],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: User steering callout should be shown
      expect(screen.getByTestId("user-steering-callout")).toBeInTheDocument();
      expect(screen.getByText(/merge needs your input/i)).toBeInTheDocument();
      expect(screen.getByText(/merge conflict in file.ts/i)).toBeInTheDocument();
    });

    it("does not show user steering callout when loop is not in needs-review status", async () => {
      // Given: A task with a loop in running status
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask, // mockTask.loopId = "loop-001"
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [
          {
            id: "loop-001", // Matches mockTask.loopId
            status: "running",
            location: "/some/worktree",
            prompt: "Test prompt",
          },
        ],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: User steering callout should not be shown
      expect(screen.queryByTestId("user-steering-callout")).not.toBeInTheDocument();
    });

    it("does not show user steering callout when no associated loop exists", async () => {
      // Given: A task without a matching loop
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask, // mockTask.loopId = "loop-001"
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [
          {
            id: "loop-002", // Different loop ID, no match with mockTask.loopId
            status: "needs-review",
            location: "/some/worktree",
            prompt: "Test prompt",
            failureReason: "Some failure",
          },
        ],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: User steering callout should not be shown
      expect(screen.queryByTestId("user-steering-callout")).not.toBeInTheDocument();
    });

    it("shows loop badge when task has associated loop", async () => {
      // Given: A task with an associated loop
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask, // mockTask.loopId = "loop-001"
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [
          {
            id: "loop-001", // Matches mockTask.loopId
            status: "running",
            location: "/some/worktree",
            prompt: "Test prompt",
          },
        ],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-001");

      // Then: Loop badge should be shown
      // The LoopBadge component renders with "Loop:" prefix
      expect(screen.getByText("Loop:")).toBeInTheDocument();
    });

    it("passes steering input when retrying merge", async () => {
      // Given: A task with a loop in needs-review status
      const user = userEvent.setup();
      const mockMutate = vi.fn();
      const { trpc } = await import("@/trpc");

      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockTask, // mockTask.loopId = "loop-001"
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [
          {
            id: "loop-001", // Matches mockTask.loopId
            status: "needs-review",
            location: "/some/worktree",
            prompt: "Test prompt",
            failureReason: "Merge conflict in file.ts",
          },
        ],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);
      vi.mocked(trpc.loops.retry.useMutation).mockReturnValue({
        mutate: mockMutate,
        isPending: false,
      } as unknown as ReturnType<typeof trpc.loops.retry.useMutation>);

      // When: The page is rendered and user enters steering input
      renderWithRouter("task-001");
      const textarea = screen.getByPlaceholderText(/keep my changes/i);
      await user.type(textarea, "Keep the worktree changes");

      // And clicks retry merge
      const retryButton = screen.getByRole("button", { name: /retry merge/i });
      await user.click(retryButton);

      // Then: The mutation should be called with steering input
      expect(mockMutate).toHaveBeenCalledWith({
        id: "loop-001",
        steeringInput: "Keep the worktree changes",
      });
    });
  });

  describe("completed task rendering", () => {
    it("renders log viewer and metadata for completed tasks without associated loop", async () => {
      // Given: A completed task with no associated loop
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: mockCompletedTask,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-002");

      // Then: Log viewer and metadata should be rendered
      expect(screen.getByTestId("log-viewer")).toBeInTheDocument();
      expect(screen.getByText(/created/i)).toBeInTheDocument();
    });

    it("renders loop badge for tasks with merged loop", async () => {
      // Given: A completed task with a merged loop
      const taskWithMerge = {
        ...mockCompletedTask,
        loopId: "loop-001",
      };
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: taskWithMerge,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [
          {
            id: "loop-001",
            status: "merged",
            location: "/some/worktree",
            prompt: "Test prompt",
            mergeCommit: "abc123def456789",
          },
        ],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-002");

      // Then: Loop badge should be rendered showing merged status
      expect(screen.getByText("Loop:")).toBeInTheDocument();
    });

    it("does not render loop badge when loop status is running but task is terminal", async () => {
      // Given: A completed task whose loop slot was reused (running != terminal task)
      const taskWithMerge = {
        ...mockCompletedTask,
        status: "closed",
        loopId: "loop-001",
      };
      const { trpc } = await import("@/trpc");
      vi.mocked(trpc.task.get.useQuery).mockReturnValue({
        data: taskWithMerge,
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.task.get.useQuery>);
      vi.mocked(trpc.loops.list.useQuery).mockReturnValue({
        data: [
          {
            id: "loop-001",
            status: "running",
            location: "/some/worktree",
            prompt: "Different prompt",
          },
        ],
        isLoading: false,
        isError: false,
      } as ReturnType<typeof trpc.loops.list.useQuery>);

      // When: The page is rendered
      renderWithRouter("task-002");

      // Then: Loop badge should NOT be shown (stale association guard)
      expect(screen.queryByText("Loop:")).not.toBeInTheDocument();
    });
  });
});
