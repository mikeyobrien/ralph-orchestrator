/**
 * useKeyboardShortcuts Hook
 *
 * Provides global keyboard navigation for the task list:
 * - j/k: Navigate down/up through tasks
 * - Enter: Toggle expand/collapse on focused task
 * - Escape: Collapse all threads and clear focus
 *
 * The hook manages a focused task index and integrates with the Zustand
 * store for expand/collapse state. It automatically skips keyboard
 * handling when focus is inside interactive elements (inputs, textareas).
 */

import { useEffect, useCallback, useState, useRef } from "react";
import { useUIStore } from "@/store";

/**
 * Configuration for keyboard shortcuts behavior
 */
interface UseKeyboardShortcutsOptions {
  /** Array of task IDs in display order */
  taskIds: string[];
  /** Whether keyboard shortcuts are enabled. Default: true */
  enabled?: boolean;
  /** Callback when focus changes */
  onFocusChange?: (index: number | null) => void;
}

/**
 * Return type for the useKeyboardShortcuts hook
 */
interface UseKeyboardShortcutsReturn {
  /** Currently focused task index (null if no focus) */
  focusedIndex: number | null;
  /** ID of the currently focused task */
  focusedTaskId: string | null;
  /** Set focus to a specific index */
  setFocusedIndex: (index: number | null) => void;
  /** Clear focus */
  clearFocus: () => void;
  /** Check if a task is focused */
  isTaskFocused: (taskId: string) => boolean;
}

/**
 * Check if an element is an interactive input that should block keyboard shortcuts
 */
function isInteractiveElement(element: Element | null): boolean {
  if (!element) return false;

  const tagName = element.tagName.toLowerCase();
  const interactiveTags = ["input", "textarea", "select", "button"];

  if (interactiveTags.includes(tagName)) {
    return true;
  }

  // Check for contenteditable
  if (element.getAttribute("contenteditable") === "true") {
    return true;
  }

  return false;
}

/**
 * Hook for keyboard-based task list navigation.
 *
 * @example
 * ```tsx
 * const { focusedIndex, isTaskFocused } = useKeyboardShortcuts({
 *   taskIds: ['task-1', 'task-2', 'task-3'],
 * });
 * ```
 */
export function useKeyboardShortcuts({
  taskIds,
  enabled = true,
  onFocusChange,
}: UseKeyboardShortcutsOptions): UseKeyboardShortcutsReturn {
  const [focusedIndex, setFocusedIndexState] = useState<number | null>(null);
  const taskIdsRef = useRef(taskIds);

  // Keep taskIds ref in sync and adjust focus if out of bounds
  // This effect legitimately needs to update state when the task list shrinks
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    taskIdsRef.current = taskIds;

    // If focused index is out of bounds after task list changes, adjust it
    if (focusedIndex !== null && focusedIndex >= taskIds.length) {
      const newIndex = taskIds.length > 0 ? taskIds.length - 1 : null;
      setFocusedIndexState(newIndex);
      onFocusChange?.(newIndex);
    }
  }, [taskIds, focusedIndex, onFocusChange]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Get store actions
  const toggleTaskExpanded = useUIStore((state) => state.toggleTaskExpanded);

  // Collapse all expanded tasks
  const collapseAllTasks = useCallback(() => {
    const store = useUIStore.getState();
    // Get all expanded tasks and collapse them
    store.expandedTasks.forEach((taskId) => {
      store.setTaskExpanded(taskId, false);
    });
  }, []);

  // Set focused index with bounds checking and callback
  const setFocusedIndex = useCallback(
    (index: number | null) => {
      const boundedIndex =
        index === null ? null : Math.max(0, Math.min(index, taskIdsRef.current.length - 1));

      // Don't set focus if there are no tasks
      if (boundedIndex !== null && taskIdsRef.current.length === 0) {
        return;
      }

      setFocusedIndexState(boundedIndex);
      onFocusChange?.(boundedIndex);
    },
    [onFocusChange]
  );

  // Clear focus
  const clearFocus = useCallback(() => {
    setFocusedIndexState(null);
    onFocusChange?.(null);
  }, [onFocusChange]);

  // Check if a specific task is focused
  const isTaskFocused = useCallback(
    (taskId: string) => {
      if (focusedIndex === null) return false;
      return taskIdsRef.current[focusedIndex] === taskId;
    },
    [focusedIndex]
  );

  // Get the focused task ID - use taskIds (prop) not taskIdsRef (ref) to avoid accessing refs during render
  const focusedTaskId = focusedIndex !== null ? (taskIds[focusedIndex] ?? null) : null;

  // Keyboard event handler
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      // Skip if shortcuts are disabled
      if (!enabled) return;

      // Skip if focus is in an interactive element
      if (isInteractiveElement(document.activeElement)) return;

      // Skip if no tasks
      if (taskIdsRef.current.length === 0) return;

      const { key } = event;

      switch (key) {
        case "j": // Move focus down
        case "ArrowDown": {
          event.preventDefault();
          const newIndex =
            focusedIndex === null ? 0 : Math.min(focusedIndex + 1, taskIdsRef.current.length - 1);
          setFocusedIndex(newIndex);
          break;
        }

        case "k": // Move focus up
        case "ArrowUp": {
          event.preventDefault();
          const newIndex =
            focusedIndex === null ? taskIdsRef.current.length - 1 : Math.max(focusedIndex - 1, 0);
          setFocusedIndex(newIndex);
          break;
        }

        case "Enter": {
          // Toggle expand/collapse on focused task
          if (focusedIndex !== null) {
            event.preventDefault();
            const taskId = taskIdsRef.current[focusedIndex];
            if (taskId) {
              toggleTaskExpanded(taskId);
            }
          }
          break;
        }

        case "Escape": {
          event.preventDefault();
          // Collapse all threads and clear focus
          collapseAllTasks();
          clearFocus();
          // Also blur any focused element
          if (document.activeElement instanceof HTMLElement) {
            document.activeElement.blur();
          }
          break;
        }

        // Skip processing for other keys
        default:
          return;
      }
    },
    [enabled, focusedIndex, setFocusedIndex, clearFocus, toggleTaskExpanded, collapseAllTasks]
  );

  // Attach global keyboard listener
  useEffect(() => {
    if (!enabled) return;

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [enabled, handleKeyDown]);

  return {
    focusedIndex,
    focusedTaskId,
    setFocusedIndex,
    clearFocus,
    isTaskFocused,
  };
}
