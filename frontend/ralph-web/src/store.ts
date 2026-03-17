/**
 * Zustand Store
 *
 * Global UI state management with localStorage persistence.
 * Uses Zustand's persist middleware for cross-session state.
 */

import { create } from "zustand";
import { persist } from "zustand/middleware";

interface UIState {
  // Sidebar state
  sidebarOpen: boolean;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;

  // Active folder (multi-folder domain support)
  activeFolderSlug: string | null;
  setActiveFolder: (slug: string | null) => void;

  // Expanded tasks (for TaskThread component in future steps)
  expandedTasks: Set<string>;
  toggleTaskExpanded: (taskId: string) => void;
  setTaskExpanded: (taskId: string, expanded: boolean) => void;
}

/**
 * Main UI store with sidebar and task expansion state.
 * Persisted to localStorage under 'ralph-ui' key.
 */
export const useUIStore = create<UIState>()(
  persist(
    (set) => ({
      // Sidebar defaults to open
      sidebarOpen: true,
      toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),
      setSidebarOpen: (open) => set({ sidebarOpen: open }),

      // Active folder (null = default/first registered)
      activeFolderSlug: null,
      setActiveFolder: (slug) => set({ activeFolderSlug: slug }),

      // Task expansion state (Set serialized as array)
      expandedTasks: new Set<string>(),
      toggleTaskExpanded: (taskId) =>
        set((state) => {
          const next = new Set(state.expandedTasks);
          if (next.has(taskId)) {
            next.delete(taskId);
          } else {
            next.add(taskId);
          }
          return { expandedTasks: next };
        }),
      setTaskExpanded: (taskId, expanded) =>
        set((state) => {
          const next = new Set(state.expandedTasks);
          if (expanded) {
            next.add(taskId);
          } else {
            next.delete(taskId);
          }
          return { expandedTasks: next };
        }),
    }),
    {
      name: "ralph-ui",
      // Custom storage serialization to handle Set<string>
      storage: {
        getItem: (name) => {
          const str = localStorage.getItem(name);
          if (!str) return null;
          const parsed = JSON.parse(str);
          // Convert expandedTasks array back to Set
          if (parsed.state?.expandedTasks) {
            parsed.state.expandedTasks = new Set(parsed.state.expandedTasks);
          }
          return parsed;
        },
        setItem: (name, value) => {
          // Convert Set to array for JSON serialization
          const toStore = {
            ...value,
            state: {
              ...value.state,
              expandedTasks: Array.from(value.state.expandedTasks || []),
            },
          };
          localStorage.setItem(name, JSON.stringify(toStore));
        },
        removeItem: (name) => localStorage.removeItem(name),
      },
    }
  )
);
