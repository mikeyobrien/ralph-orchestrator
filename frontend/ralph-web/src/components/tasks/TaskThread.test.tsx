/**
 * TaskThread Component Tests - Navigation Behavior
 *
 * Tests that TaskThread navigates to /tasks/:id instead of
 * expanding inline content. This follows the list-to-detail
 * pattern used by GitHub Issues, Linear, Jira, etc.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";

// Mock react-router-dom useNavigate
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

// Mock tRPC hooks
vi.mock("@/trpc", () => {
  const noop = () => {};
  const createMockMutation = () => ({
    mutate: noop,
    mutateAsync: noop,
    isPending: false,
    isError: false,
    error: null,
  });

  return {
    trpc: {
      task: {
        run: { useMutation: () => createMockMutation() },
        retry: { useMutation: () => createMockMutation() },
        cancel: { useMutation: () => createMockMutation() },
      },
      loops: {
        retry: { useMutation: () => createMockMutation() },
        merge: { useMutation: () => createMockMutation() },
        discard: { useMutation: () => createMockMutation() },
        stop: { useMutation: () => createMockMutation() },
      },
      useUtils: () => ({
        task: { list: { invalidate: noop } },
        loops: { list: { invalidate: noop } },
      }),
    },
  };
});

// Mock the UI store - currently used for expand state, but after refactoring
// should NOT be needed for navigation behavior
const mockToggleTaskExpanded = vi.fn();
vi.mock("@/store", () => ({
  useUIStore: vi.fn((selector: (state: unknown) => unknown) => {
    // Provide a mock state that the selector can use
    const mockState = {
      expandedTasks: new Set<string>(),
      toggleTaskExpanded: mockToggleTaskExpanded,
    };
    return selector(mockState);
  }),
}));

import { TaskThread, type Task } from "./TaskThread";

function createTestWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{children}</MemoryRouter>
    </QueryClientProvider>
  );
}

const mockTask: Task = {
  id: "task-123",
  title: "Test task for navigation",
  status: "open",
  priority: 2,
  blockedBy: null,
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
};

describe("TaskThread navigation behavior", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Helper to find the task card element.
   * After refactoring, this should be role="link", but currently it's role="button".
   * We find by the task title text and traverse up to the card container.
   */
  function getTaskCard(): HTMLElement {
    const titleElement = screen.getByText(mockTask.title);
    // Find the outermost Card element (has rounded-xl class)
    let current: HTMLElement | null = titleElement;
    while (current && !current.classList?.contains("rounded-xl")) {
      current = current.parentElement;
    }
    if (!current) throw new Error("Could not find task card");
    return current;
  }

  describe("click-to-navigate", () => {
    it("navigates to /tasks/:id when clicked", () => {
      // Given: A TaskThread component rendered with a task
      render(<TaskThread task={mockTask} />, { wrapper: createTestWrapper() });

      // When: User clicks on the task card
      const taskCard = getTaskCard();
      fireEvent.click(taskCard);

      // Then: Should navigate to the task detail page (NOT toggle expand state)
      expect(mockNavigate).toHaveBeenCalledWith(`/tasks/${mockTask.id}`);
    });

    it("navigates to /tasks/:id when Enter key is pressed", () => {
      // Given: A TaskThread component rendered with a task
      render(<TaskThread task={mockTask} />, { wrapper: createTestWrapper() });

      // When: User presses Enter on the task card
      const taskCard = getTaskCard();
      fireEvent.keyDown(taskCard, { key: "Enter" });

      // Then: Should navigate to the task detail page
      expect(mockNavigate).toHaveBeenCalledWith(`/tasks/${mockTask.id}`);
    });

    it("navigates to /tasks/:id when Space key is pressed", () => {
      // Given: A TaskThread component rendered with a task
      render(<TaskThread task={mockTask} />, { wrapper: createTestWrapper() });

      // When: User presses Space on the task card
      const taskCard = getTaskCard();
      fireEvent.keyDown(taskCard, { key: " " });

      // Then: Should navigate to the task detail page
      expect(mockNavigate).toHaveBeenCalledWith(`/tasks/${mockTask.id}`);
    });
  });

  describe("no expand/collapse UI", () => {
    it("does not render chevron icons", () => {
      // Given: A TaskThread component rendered with a task
      render(<TaskThread task={mockTask} />, { wrapper: createTestWrapper() });

      // Then: No chevron icons should be present
      // Chevrons were used for expand/collapse indication
      // lucide-react adds class like "lucide-chevron-right" to SVGs
      const chevronRight = document.querySelector(".lucide-chevron-right");
      const chevronDown = document.querySelector(".lucide-chevron-down");

      expect(chevronRight).not.toBeInTheDocument();
      expect(chevronDown).not.toBeInTheDocument();
    });

    it("does not have aria-expanded attribute", () => {
      // Given: A TaskThread component rendered with a task
      render(<TaskThread task={mockTask} />, { wrapper: createTestWrapper() });

      // Then: The card should not have aria-expanded since it's not expandable
      const taskCard = getTaskCard();
      expect(taskCard).not.toHaveAttribute("aria-expanded");
    });

    it("does not render expanded content section", () => {
      // Given: A TaskThread component rendered with a task
      render(<TaskThread task={mockTask} />, { wrapper: createTestWrapper() });

      // Then: There should be no expanded content area
      // The old implementation had a CardContent with expanded details
      expect(screen.queryByText(/Created:/)).not.toBeInTheDocument();
    });
  });

  describe("action buttons do not navigate", () => {
    it("Run button does not trigger navigation", () => {
      // Given: An open task with a Run button
      render(<TaskThread task={mockTask} />, { wrapper: createTestWrapper() });

      // When: User clicks the actual Run button (not the card which also has role="button")
      // The Run button has "Run" text visible
      const runButton = screen.getByText("Run").closest("button");
      expect(runButton).not.toBeNull();
      fireEvent.click(runButton!);

      // Then: Should NOT navigate (action buttons should stop propagation)
      expect(mockNavigate).not.toHaveBeenCalled();
    });
  });

  describe("merge loop visual distinction", () => {
    it("shows green left border for merge loop tasks (merging status)", () => {
      // Given: A task with a loop in "merging" status
      const loop = {
        id: "loop-123",
        status: "merging" as const,
        location: ".worktrees/ralph-test",
        prompt: "Test merge loop",
      };
      render(<TaskThread task={mockTask} loop={loop} />, {
        wrapper: createTestWrapper(),
      });

      // Then: The card should have the green left border class
      const taskCard = getTaskCard();
      expect(taskCard).toHaveClass("border-l-4");
      expect(taskCard).toHaveClass("border-l-green-500/60");
    });

    it("shows green left border for tasks with needs-review status", () => {
      // Given: A task with a loop in "needs-review" status
      const loop = {
        id: "loop-123",
        status: "needs-review" as const,
        location: ".worktrees/ralph-test",
        prompt: "Test merge loop",
      };
      render(<TaskThread task={mockTask} loop={loop} />, {
        wrapper: createTestWrapper(),
      });

      // Then: The card should have the green left border class
      const taskCard = getTaskCard();
      expect(taskCard).toHaveClass("border-l-4");
    });

    it("does not show green left border for regular running loops", () => {
      // Given: A task with a loop in "running" status (not merge-related)
      const loop = {
        id: "loop-123",
        status: "running" as const,
        location: ".worktrees/ralph-test",
        prompt: "Regular dev loop",
      };
      render(<TaskThread task={mockTask} loop={loop} />, {
        wrapper: createTestWrapper(),
      });

      // Then: The card should NOT have the green left border class
      const taskCard = getTaskCard();
      expect(taskCard).not.toHaveClass("border-l-4");
    });
  });
});
