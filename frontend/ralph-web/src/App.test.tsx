/**
 * App Routing Tests
 *
 * Tests that verify the application routing configuration is correct.
 * Specifically validates that all page components are properly exported
 * and routes are correctly wired up.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { App } from "./App";

// Mock all page components to isolate routing tests
vi.mock("./pages", () => ({
  TasksPage: () => <div data-testid="tasks-page">Tasks Page</div>,
  PlanPage: () => <div data-testid="plan-page">Plan Page</div>,
  BuilderPage: () => <div data-testid="builder-page">Builder Page</div>,
  TaskDetailPage: () => <div data-testid="task-detail-page">Task Detail Page</div>,
  SettingsPage: () => <div data-testid="settings-page">Settings Page</div>,
}));

// Mock the layout component - must render Outlet for routes to work
vi.mock("./components/layout", async () => {
  const { Outlet } = await import("react-router-dom");
  return {
    AppShell: () => (
      <div data-testid="app-shell">
        <Outlet />
      </div>
    ),
  };
});

// Helper to render App with a specific route
function renderWithRoute(route: string) {
  return render(
    <MemoryRouter initialEntries={[route]}>
      <App />
    </MemoryRouter>
  );
}

describe("App routing", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("TaskDetailPage route", () => {
    it("renders TaskDetailPage for /tasks/:id route", async () => {
      // Given: A route to a specific task detail
      const taskId = "task-abc-123";

      // When: The app is rendered with the task detail route
      renderWithRoute(`/tasks/${taskId}`);

      // Then: The TaskDetailPage should be rendered
      await waitFor(() => {
        expect(screen.getByTestId("task-detail-page")).toBeInTheDocument();
      });
    });

    it("does not match /tasks/:id when navigating to /tasks", async () => {
      // Given: A route to the task list (not a specific task)

      // When: The app is rendered with the tasks list route
      renderWithRoute("/tasks");

      // Then: The TasksPage should be rendered, not TaskDetailPage
      await waitFor(() => {
        expect(screen.getByTestId("tasks-page")).toBeInTheDocument();
      });
      expect(screen.queryByTestId("task-detail-page")).not.toBeInTheDocument();
    });
  });

  describe("existing routes still work", () => {
    it("renders TasksPage for /tasks route", async () => {
      renderWithRoute("/tasks");

      await waitFor(() => {
        expect(screen.getByTestId("tasks-page")).toBeInTheDocument();
      });
    });

    it("redirects root to /tasks", async () => {
      renderWithRoute("/");

      await waitFor(() => {
        expect(screen.getByTestId("tasks-page")).toBeInTheDocument();
      });
    });
  });
});

describe("pages barrel export", () => {
  it("exports TaskDetailPage from pages/index.ts", async () => {
    // Given: The pages barrel export

    // When: We import from pages (bypass the mock)
    // Use dynamic import with query param to avoid mock
    const pagesModule = await vi.importActual<typeof import("./pages")>("./pages");

    // Then: TaskDetailPage should be exported
    expect(pagesModule.TaskDetailPage).toBeDefined();
    expect(typeof pagesModule.TaskDetailPage).toBe("function");
  });
});
