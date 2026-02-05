/**
 * ThreadList Component Tests - Merge Button State Propagation
 *
 * Tests that ThreadList correctly maps loops to tasks via loopId and
 * passes mergeButtonState through to TaskThread for each worktree loop.
 * Merge buttons are rendered by TaskThread, not by a separate loops section.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";

// Mock tRPC hooks with inline data (vi.mock is hoisted)
vi.mock("@/trpc", () => {
  // Create a mock mutation function - using inline noop functions
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
        list: {
          useQuery: () => ({
            data: [
              {
                id: "task-001",
                title: "Implement feature A",
                status: "running",
                priority: 2,
                blockedBy: null,
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                loopId: "loop-worktree-001",
              },
              {
                id: "task-002",
                title: "Implement feature B",
                status: "running",
                priority: 2,
                blockedBy: null,
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                loopId: "loop-worktree-002",
              },
              {
                id: "task-003",
                title: "Building core module",
                status: "running",
                priority: 2,
                blockedBy: null,
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                loopId: "loop-primary",
              },
            ],
            isLoading: false,
            isError: false,
            error: null,
            isFetching: false,
            refetch: noop,
          }),
        },
        get: {
          useQuery: () => ({
            data: null,
            isLoading: false,
          }),
        },
        run: { useMutation: () => createMockMutation() },
        cancel: { useMutation: () => createMockMutation() },
        retry: { useMutation: () => createMockMutation() },
        close: { useMutation: () => createMockMutation() },
        archive: { useMutation: () => createMockMutation() },
        update: { useMutation: () => createMockMutation() },
        delete: { useMutation: () => createMockMutation() },
        executionStatus: {
          useQuery: () => ({
            data: { isQueued: false },
            isLoading: false,
          }),
        },
      },
      loops: {
        list: {
          useQuery: () => ({
            data: [
              {
                id: "loop-worktree-001",
                status: "queued",
                prompt: "Implement feature A",
                location: ".worktrees/feature-a",
                workspaceRoot: "/path/to/.worktrees/feature-a",
                repoRoot: "/path/to/repo",
                pid: 12345,
                mergeButtonState: { state: "active" },
              },
              {
                id: "loop-worktree-002",
                status: "queued",
                prompt: "Implement feature B",
                location: ".worktrees/feature-b",
                workspaceRoot: "/path/to/.worktrees/feature-b",
                repoRoot: "/path/to/repo",
                pid: 12346,
                mergeButtonState: { state: "blocked", reason: "Primary loop is running: Building core module" },
              },
              {
                id: "loop-primary",
                status: "running",
                prompt: "Building core module",
                location: "(in-place)",
                workspaceRoot: "/path/to/repo",
                repoRoot: "/path/to/repo",
                pid: 12340,
              },
            ],
            isLoading: false,
            isError: false,
            error: null,
          }),
        },
        stop: { useMutation: () => createMockMutation() },
        retry: { useMutation: () => createMockMutation() },
        merge: { useMutation: () => createMockMutation() },
        discard: { useMutation: () => createMockMutation() },
      },
      useUtils: () => ({
        loops: { list: { invalidate: noop } },
        task: { list: { invalidate: noop } },
      }),
    },
  };
});

// Mock the hooks
vi.mock("@/hooks", () => ({
  useNotifications: vi.fn(() => ({
    permission: "default",
    enabled: false,
    requestPermission: vi.fn(),
    setEnabled: vi.fn(),
    checkTaskStatusChanges: vi.fn(),
    isSupported: true,
  })),
  useKeyboardShortcuts: vi.fn(() => ({
    isTaskFocused: vi.fn(() => false),
  })),
}));

import { ThreadList } from "./ThreadList";

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

describe("ThreadList merge button state propagation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("task cards with loop associations", () => {
    it("renders merge button as enabled when mergeButtonState is active", () => {
      // Given: ThreadList with tasks mapped to loops via loopId
      render(<ThreadList />, { wrapper: createTestWrapper() });

      // Then: The task card for feature-a (loopId maps to active merge state) should show enabled merge
      const taskList = screen.getByRole("list", { name: /task list/i });
      const featureACard = within(taskList)
        .getByText(/implement feature a/i)
        .closest("[role='button']") as HTMLElement;

      const mergeButton = within(featureACard).getByRole("button", { name: /merge/i });
      expect(mergeButton).toBeEnabled();
      expect(mergeButton).not.toHaveClass("opacity-50");
    });

    it("renders merge button as blocked when mergeButtonState is blocked", () => {
      // Given: ThreadList with tasks mapped to loops via loopId
      render(<ThreadList />, { wrapper: createTestWrapper() });

      // Then: The task card for feature-b (loopId maps to blocked merge state) should show disabled merge
      const taskList = screen.getByRole("list", { name: /task list/i });
      const featureBCard = within(taskList)
        .getByText(/implement feature b/i)
        .closest("[role='button']") as HTMLElement;

      const mergeButton = within(featureBCard).getByRole("button", { name: /merge/i });
      expect(mergeButton).toBeDisabled();
      expect(mergeButton).toHaveClass("opacity-50");
    });

    it("shows blocked reason in tooltip when mergeButtonState is blocked", () => {
      // Given: ThreadList with tasks mapped to loops via loopId
      render(<ThreadList />, { wrapper: createTestWrapper() });

      // Then: The blocked merge button should show the reason in its tooltip
      const taskList = screen.getByRole("list", { name: /task list/i });
      const featureBCard = within(taskList)
        .getByText(/implement feature b/i)
        .closest("[role='button']") as HTMLElement;

      const mergeButton = within(featureBCard).getByRole("button", { name: /merge/i });
      expect(mergeButton).toHaveAttribute("title", expect.stringContaining("Primary loop is running"));
    });

    it("does not render merge button for primary loop (in-place)", () => {
      // Given: ThreadList with tasks mapped to loops via loopId
      render(<ThreadList />, { wrapper: createTestWrapper() });

      // Then: The task card for the primary loop (in-place) should not have a merge button
      const taskList = screen.getByRole("list", { name: /task list/i });
      const primaryCard = within(taskList)
        .getByText(/building core module/i)
        .closest("[role='button']") as HTMLElement;

      // Primary loops are in-place and should not show Merge button
      expect(within(primaryCard).queryByRole("button", { name: /merge/i })).not.toBeInTheDocument();
    });
  });
});
