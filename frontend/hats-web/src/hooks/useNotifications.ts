/**
 * useNotifications Hook
 *
 * Manages browser notifications for task status changes.
 * Handles permission requests and provides a simple API for showing notifications.
 *
 * Features:
 * - Permission request flow with user-friendly state tracking
 * - Task status change detection with previous state comparison
 * - Configurable notification options (icon, sound, auto-close)
 * - Respect for user preferences (localStorage persistence)
 */

import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Notification permission states
 */
export type NotificationPermission = "default" | "granted" | "denied" | "unsupported";

/**
 * Task status types that trigger notifications
 */
export type NotifiableStatus = "completed" | "failed" | "blocked" | "closed";

/**
 * Options for showing a notification
 */
interface ShowNotificationOptions {
  /** Notification body text */
  body?: string;
  /** Icon URL (defaults to app icon) */
  icon?: string;
  /** Tag to prevent duplicate notifications */
  tag?: string;
  /** Auto-close after this many milliseconds (0 = no auto-close) */
  autoClose?: number;
  /** Click handler for the notification */
  onClick?: () => void;
}

/**
 * Task data structure for status change detection
 */
interface TaskStatusInfo {
  id: string;
  status: string;
  title?: string;
}

interface UseNotificationsOptions {
  /** Whether notifications are enabled by user preference (default: true) */
  enabled?: boolean;
  /** Storage key for user preference (default: 'hats-notifications') */
  storageKey?: string;
}

interface UseNotificationsReturn {
  /** Current permission state */
  permission: NotificationPermission;
  /** Whether notifications are enabled by user preference */
  enabled: boolean;
  /** Request permission from the user */
  requestPermission: () => Promise<boolean>;
  /** Show a notification */
  showNotification: (title: string, options?: ShowNotificationOptions) => void;
  /** Toggle notifications enabled state */
  setEnabled: (enabled: boolean) => void;
  /** Check task status changes and show notifications for terminal states */
  checkTaskStatusChanges: (tasks: TaskStatusInfo[]) => void;
  /** Whether the browser supports notifications */
  isSupported: boolean;
}

/**
 * Get the current notification permission
 */
function getNotificationPermission(): NotificationPermission {
  if (typeof window === "undefined" || !("Notification" in window)) {
    return "unsupported";
  }
  return Notification.permission as NotificationPermission;
}

/**
 * Hook for managing browser notifications.
 *
 * @param options - Configuration options
 */
export function useNotifications(options: UseNotificationsOptions = {}): UseNotificationsReturn {
  const { storageKey = "hats-notifications" } = options;

  // Track notification permission
  const [permission, setPermission] = useState<NotificationPermission>(() =>
    getNotificationPermission()
  );

  // Track user preference (enabled/disabled)
  const [enabled, setEnabledState] = useState<boolean>(() => {
    if (typeof window === "undefined") return true;
    const stored = localStorage.getItem(storageKey);
    return stored === null ? true : stored === "true";
  });

  // Track previous task states for change detection
  const previousTasksRef = useRef<Map<string, string>>(new Map());

  // Check if browser supports notifications
  const isSupported = typeof window !== "undefined" && "Notification" in window;

  // Persist enabled preference
  const setEnabled = useCallback(
    (value: boolean) => {
      setEnabledState(value);
      if (typeof window !== "undefined") {
        localStorage.setItem(storageKey, String(value));
      }
    },
    [storageKey]
  );

  // Request notification permission
  const requestPermission = useCallback(async (): Promise<boolean> => {
    if (!isSupported) {
      return false;
    }

    if (permission === "granted") {
      return true;
    }

    if (permission === "denied") {
      return false;
    }

    try {
      const result = await Notification.requestPermission();
      setPermission(result as NotificationPermission);
      return result === "granted";
    } catch {
      // Safari may throw on requestPermission
      return false;
    }
  }, [isSupported, permission]);

  // Show a notification (only when tab is not focused)
  const showNotification = useCallback(
    (title: string, notificationOptions: ShowNotificationOptions = {}) => {
      if (!isSupported || !enabled || permission !== "granted") {
        return;
      }

      // Only show notifications when the tab is not focused (Page Visibility API)
      // document.hidden is true when the page is not visible (background tab, minimized, etc.)
      if (typeof document !== "undefined" && !document.hidden) {
        return;
      }

      const { body, icon = "/favicon.svg", tag, autoClose = 5000, onClick } = notificationOptions;

      try {
        const notification = new Notification(title, {
          body,
          icon,
          tag,
          silent: false,
        });

        if (onClick) {
          notification.onclick = () => {
            window.focus();
            onClick();
            notification.close();
          };
        }

        if (autoClose > 0) {
          setTimeout(() => notification.close(), autoClose);
        }
      } catch {
        // Notification may fail in some contexts (e.g., insecure origins)
      }
    },
    [isSupported, enabled, permission]
  );

  // Check for task status changes and show notifications
  const checkTaskStatusChanges = useCallback(
    (tasks: TaskStatusInfo[]) => {
      if (!enabled || permission !== "granted") {
        return;
      }

      const previousTasks = previousTasksRef.current;
      const currentTasks = new Map<string, string>();

      for (const task of tasks) {
        currentTasks.set(task.id, task.status);

        const previousStatus = previousTasks.get(task.id);

        // Only notify if:
        // 1. Task existed before (not initial load)
        // 2. Status has changed
        // 3. New status is a terminal/notifiable state
        if (
          previousStatus !== undefined &&
          previousStatus !== task.status &&
          isNotifiableStatus(task.status)
        ) {
          const statusLabel = getStatusLabel(task.status);
          const icon = getStatusIcon(task.status);

          showNotification(`Task ${statusLabel}`, {
            body: task.title || `Task ${task.id.slice(0, 8)}`,
            icon,
            tag: `task-${task.id}`,
            onClick: () => {
              // Focus the window and potentially navigate to task
              window.focus();
            },
          });
        }
      }

      // Update reference for next check
      previousTasksRef.current = currentTasks;
    },
    [enabled, permission, showNotification]
  );

  // Sync permission state with browser (for changes made in browser settings)
  useEffect(() => {
    if (!isSupported) return;

    // Check permission periodically (every 5s) in case user changes it in browser settings
    const interval = setInterval(() => {
      const currentPermission = getNotificationPermission();
      if (currentPermission !== permission) {
        setPermission(currentPermission);
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [isSupported, permission]);

  return {
    permission,
    enabled,
    requestPermission,
    showNotification,
    setEnabled,
    checkTaskStatusChanges,
    isSupported,
  };
}

/**
 * Check if a status should trigger a notification
 */
function isNotifiableStatus(status: string): boolean {
  return ["completed", "failed", "blocked", "closed"].includes(status);
}

/**
 * Get human-readable label for status
 */
function getStatusLabel(status: string): string {
  switch (status) {
    case "completed":
    case "closed":
      return "Completed";
    case "failed":
      return "Failed";
    case "blocked":
      return "Blocked";
    default:
      return status.charAt(0).toUpperCase() + status.slice(1);
  }
}

/**
 * Get icon path for status (uses emoji as fallback for simple implementation)
 */
function getStatusIcon(_status: string): string {
  // In a real app, these would be actual icon URLs based on status
  // For now, use the default favicon for all statuses
  return "/favicon.svg";
}
