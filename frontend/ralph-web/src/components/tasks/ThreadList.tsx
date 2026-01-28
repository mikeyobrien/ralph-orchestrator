/**
 * ThreadList Component
 *
 * Composes TaskThread components into a list connected to real task data
 * via tRPC. Handles sorting (running first, then pending, then completed),
 * polling for updates, empty state display, and browser notifications.
 *
 * Features:
 * - Real-time task list with configurable polling
 * - Browser notifications for task status changes (completed/failed)
 * - Notification permission request UI
 * - Sorted display: running > pending > completed
 */

import { useMemo, useEffect, useRef, useState, useCallback } from "react";
import { Inbox, RefreshCw, Bell, BellOff, Archive } from "lucide-react";
import { trpc } from "@/trpc";
import { TaskThread, type Task } from "./TaskThread";
import { type LoopDetailData } from "./LoopDetail";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useNotifications, useKeyboardShortcuts } from "@/hooks";

interface ThreadListProps {
  /** Polling interval in milliseconds. Set to 0 to disable. Default: 5000 */
  pollingInterval?: number;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Status priority for sorting: running tasks first, then pending, then completed.
 * Lower number = higher priority (appears first in list).
 */
const STATUS_PRIORITY: Record<string, number> = {
  running: 0,
  pending: 1,
  open: 2,
  blocked: 3,
  completed: 4,
  closed: 5,
  failed: 6,
  archived: 7,
};

/**
 * Sort tasks by status priority, then by most recent first within each group.
 */
function sortTasks(tasks: Task[]): Task[] {
  return [...tasks].sort((a, b) => {
    const priorityA = STATUS_PRIORITY[a.status] ?? 99;
    const priorityB = STATUS_PRIORITY[b.status] ?? 99;

    // First sort by status priority
    if (priorityA !== priorityB) {
      return priorityA - priorityB;
    }

    // Within same status, sort by most recent (updatedAt desc)
    const dateA = new Date(a.updatedAt).getTime();
    const dateB = new Date(b.updatedAt).getTime();
    return dateB - dateA;
  });
}

export function ThreadList({ pollingInterval = 5000, className }: ThreadListProps) {
  const [showArchived, setShowArchived] = useState(false);
  const [showClosed, setShowClosed] = useState(false);
  const [archiveAfterDaysInput, setArchiveAfterDaysInput] = useState("7");
  const [isArchiving, setIsArchiving] = useState(false);

  // Notification hook for browser notifications on status changes
  const {
    permission,
    enabled: notificationsEnabled,
    requestPermission,
    setEnabled: setNotificationsEnabled,
    checkTaskStatusChanges,
    isSupported: notificationsSupported,
  } = useNotifications();

  // Always fetch archived tasks to maintain count even when hidden
  const tasksQuery = trpc.task.list.useQuery(
    { includeArchived: true },
    {
      refetchInterval: pollingInterval > 0 ? pollingInterval : false,
    }
  );

  // Fetch loops for PID-based mapping to tasks (per spec lines 65-68)
  const loopsQuery = trpc.loops.list.useQuery(
    { includeTerminal: false },
    {
      refetchInterval: pollingInterval > 0 ? pollingInterval : false,
    }
  );

  // Create loop ID mapping for direct task↔loop association
  // Maps loop IDs to their loop data for O(1) lookup when rendering tasks
  const loopIdToLoopMap = useMemo(() => {
    if (!loopsQuery.data) return new Map<string, LoopDetailData>();
    const map = new Map<string, LoopDetailData>();
    for (const loop of loopsQuery.data as LoopDetailData[]) {
      map.set(loop.id, loop);
    }
    return map;
  }, [loopsQuery.data]);

  // Create PID-based mapping from task PID to loop (fallback for legacy tasks)
  // Maps loop PIDs to their loop data for O(1) lookup when rendering tasks
  const pidToLoopMap = useMemo(() => {
    if (!loopsQuery.data) return new Map<number, LoopDetailData>();
    const map = new Map<number, LoopDetailData>();
    for (const loop of loopsQuery.data as LoopDetailData[]) {
      if (loop.pid !== undefined && loop.pid !== null) {
        map.set(loop.pid, loop);
      }
    }
    return map;
  }, [loopsQuery.data]);

  // Helper to get loop for a task - checks loopId first, falls back to PID
  // Guard: if task is terminal (failed/closed) but the loop slot shows "running",
  // the loop was reused for a different run — don't show a stale association.
  const getLoopForTask = useCallback(
    (task: Task): LoopDetailData | undefined => {
      let loop: LoopDetailData | undefined;
      // Prefer loopId (direct association)
      if (task.loopId) {
        loop = loopIdToLoopMap.get(task.loopId);
      }
      // Fallback to PID-based mapping
      if (!loop && task.pid) {
        loop = pidToLoopMap.get(task.pid);
      }
      if (!loop) return undefined;
      const isTaskTerminal = task.status === "failed" || task.status === "closed";
      if (isTaskTerminal && loop.status === "running") return undefined;
      return loop;
    },
    [loopIdToLoopMap, pidToLoopMap]
  );

  const archiveAfterDays = Math.max(1, Number(archiveAfterDaysInput) || 1);

  const archivedCount = useMemo(() => {
    if (!tasksQuery.data) return 0;
    return (tasksQuery.data as Task[]).filter((task) => !!task.archivedAt).length;
  }, [tasksQuery.data]);

  const closedCount = useMemo(() => {
    if (!tasksQuery.data) return 0;
    return (tasksQuery.data as Task[]).filter(
      (task) => task.status === "closed" && !task.archivedAt
    ).length;
  }, [tasksQuery.data]);

  const sortedTasks = useMemo(() => {
    if (!tasksQuery.data) return [];
    const tasks = tasksQuery.data as Task[];
    // Filter out archived tasks (unless showArchived), then filter out closed tasks (unless showClosed)
    let visibleTasks = showArchived ? tasks : tasks.filter((task) => !task.archivedAt);
    if (!showClosed) {
      visibleTasks = visibleTasks.filter((task) => task.status !== "closed");
    }
    return sortTasks(visibleTasks);
  }, [tasksQuery.data, showArchived, showClosed]);

  const archiveCandidates = useMemo(() => {
    if (!tasksQuery.data) return [];
    const tasks = tasksQuery.data as Task[];
    const now = Date.now();
    const cutoffMs = archiveAfterDays * 24 * 60 * 60 * 1000;
    const archivableStatuses = new Set(["closed", "completed", "failed"]);

    return tasks.filter((task) => {
      if (task.archivedAt) return false;
      if (!archivableStatuses.has(task.status)) return false;
      const reference = getArchiveReferenceDate(task);
      if (!reference) return false;
      return now - reference.getTime() >= cutoffMs;
    });
  }, [tasksQuery.data, archiveAfterDays]);

  const utils = trpc.useUtils();
  const archiveMutation = trpc.task.archive.useMutation();

  const handleArchiveOld = useCallback(async () => {
    if (archiveCandidates.length === 0 || isArchiving) return;
    setIsArchiving(true);
    try {
      await Promise.all(
        archiveCandidates.map((task) => archiveMutation.mutateAsync({ id: task.id }))
      );
      utils.task.list.invalidate();
    } finally {
      setIsArchiving(false);
    }
  }, [archiveCandidates, archiveMutation, isArchiving, utils]);

  // Extract task IDs for keyboard navigation
  const taskIds = useMemo(() => sortedTasks.map((task) => task.id), [sortedTasks]);

  // Task refs for scrolling focused task into view
  const taskRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  // Keyboard navigation
  const { isTaskFocused } = useKeyboardShortcuts({
    taskIds,
    enabled: sortedTasks.length > 0,
    onFocusChange: (index) => {
      // Scroll focused task into view
      if (index !== null && taskIds[index]) {
        const ref = taskRefs.current.get(taskIds[index]);
        ref?.scrollIntoView({ behavior: "smooth", block: "nearest" });
      }
    },
  });

  // Check for task status changes and trigger notifications
  useEffect(() => {
    if (tasksQuery.data && notificationsEnabled && permission === "granted") {
      const tasks = tasksQuery.data.map((task: Task) => ({
        id: task.id,
        status: task.status,
        title: task.title?.slice(0, 50) || `Task ${task.id.slice(0, 8)}`,
      }));
      checkTaskStatusChanges(tasks);
    }
  }, [tasksQuery.data, notificationsEnabled, permission, checkTaskStatusChanges]);

  // Loading state
  if (tasksQuery.isLoading) {
    return (
      <div className={cn("space-y-3", className)}>
        {/* Skeleton loading placeholders */}
        {[1, 2, 3].map((i) => (
          <div key={i} className="h-16 rounded-lg bg-muted/50 animate-pulse" aria-hidden="true" />
        ))}
        <span className="sr-only">Loading tasks...</span>
      </div>
    );
  }

  // Error state
  if (tasksQuery.isError) {
    return (
      <div className={cn("rounded-lg border border-destructive/50 p-4", className)}>
        <p className="text-destructive text-sm mb-3">
          Error loading tasks: {tasksQuery.error.message}
        </p>
        <Button
          variant="outline"
          size="sm"
          onClick={() => tasksQuery.refetch()}
          disabled={tasksQuery.isFetching}
        >
          <RefreshCw className={cn("h-4 w-4 mr-2", tasksQuery.isFetching && "animate-spin")} />
          {tasksQuery.isFetching ? "Retrying..." : "Retry"}
        </Button>
      </div>
    );
  }

  // Empty state
  if (sortedTasks.length === 0) {
    const hasClosedHidden = closedCount > 0 && !showClosed;
    const hasArchivedHidden = archivedCount > 0 && !showArchived;
    const hasHiddenTasks = hasClosedHidden || hasArchivedHidden;
    return (
      <div className={cn("flex flex-col items-center justify-center py-12 text-center", className)}>
        <Inbox className="h-12 w-12 text-muted-foreground mb-4" />
        <p className="text-muted-foreground text-lg font-medium">
          {hasHiddenTasks ? "No active tasks" : "No tasks yet"}
        </p>
        <p className="text-muted-foreground text-sm mt-1">
          {hasHiddenTasks
            ? `${hasClosedHidden ? `${closedCount} closed` : ""}${hasClosedHidden && hasArchivedHidden ? ", " : ""}${hasArchivedHidden ? `${archivedCount} archived` : ""} hidden.`
            : "Give Ralph something to do!"}
        </p>
        {hasHiddenTasks && (
          <div className="flex gap-2 mt-3">
            {hasClosedHidden && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => setShowClosed(true)}
              >
                Show closed
              </Button>
            )}
            {hasArchivedHidden && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => setShowArchived(true)}
              >
                Show archived
              </Button>
            )}
          </div>
        )}
      </div>
    );
  }

  // Handle notification button click
  const handleNotificationToggle = async () => {
    if (permission === "default") {
      // Request permission first
      await requestPermission();
    } else if (permission === "granted") {
      // Toggle enabled state
      setNotificationsEnabled(!notificationsEnabled);
    }
    // If denied, clicking shows a tooltip (handled by title attribute)
  };

  // Get notification button state and tooltip
  const getNotificationButtonState = () => {
    if (!notificationsSupported) {
      return {
        icon: BellOff,
        title: "Browser notifications not supported",
        disabled: true,
        active: false,
      };
    }
    if (permission === "denied") {
      return {
        icon: BellOff,
        title: "Notifications blocked. Enable in browser settings.",
        disabled: true,
        active: false,
      };
    }
    if (permission === "default") {
      return {
        icon: Bell,
        title: "Click to enable browser notifications",
        disabled: false,
        active: false,
      };
    }
    // permission === 'granted'
    return {
      icon: notificationsEnabled ? Bell : BellOff,
      title: notificationsEnabled
        ? "Notifications enabled. Click to disable."
        : "Notifications disabled. Click to enable.",
      disabled: false,
      active: notificationsEnabled,
    };
  };

  const notificationState = getNotificationButtonState();
  const NotificationIcon = notificationState.icon;

  // Task list
  return (
    <div className={cn("space-y-3", className)}>
      {/* Header with count, notifications toggle, and refresh */}
      <div className="flex items-center justify-between text-sm text-muted-foreground">
        <span>
          {sortedTasks.length} task{sortedTasks.length !== 1 ? "s" : ""}
          {pollingInterval > 0 && (
            <span className="ml-2 text-xs">• auto-refresh {pollingInterval / 1000}s</span>
          )}
          {closedCount > 0 && !showClosed && (
            <span className="ml-2 text-xs">• {closedCount} closed hidden</span>
          )}
          {archivedCount > 0 && !showArchived && (
            <span className="ml-2 text-xs">• {archivedCount} archived hidden</span>
          )}
        </span>
        <div className="flex items-center gap-1">
          {/* Notification toggle button */}
          <Button
            variant="ghost"
            size="sm"
            onClick={handleNotificationToggle}
            disabled={notificationState.disabled}
            title={notificationState.title}
            className={cn("h-7 px-2", notificationState.active && "text-primary")}
          >
            <NotificationIcon className="h-3.5 w-3.5" />
            <span className="sr-only">{notificationState.title}</span>
          </Button>
          {/* Refresh button */}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => tasksQuery.refetch()}
            disabled={tasksQuery.isFetching}
            className="h-7 px-2"
            title="Refresh task list"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", tasksQuery.isFetching && "animate-spin")} />
            <span className="sr-only">Refresh tasks</span>
          </Button>
        </div>
      </div>

      {/* Archive controls */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <Archive className="h-3.5 w-3.5" />
          <span>Archive completed older than</span>
          <Input
            type="number"
            min={1}
            value={archiveAfterDaysInput}
            onChange={(event) => setArchiveAfterDaysInput(event.target.value)}
            className="h-7 w-20 text-xs"
            aria-label="Archive completed tasks older than days"
          />
          <span>day{archiveAfterDays === 1 ? "" : "s"}</span>
          <span className="text-xs text-muted-foreground">
            ({archiveCandidates.length} eligible)
          </span>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={handleArchiveOld}
            disabled={archiveCandidates.length === 0 || isArchiving}
            className="h-7 px-2 text-xs"
            title="Archive completed tasks older than the selected age"
          >
            {isArchiving ? "Archiving..." : "Archive old"}
          </Button>
          <Button
            variant={showClosed ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setShowClosed((prev) => !prev)}
            className="h-7 px-2 text-xs"
            title={showClosed ? "Hide closed tasks" : "Show closed tasks"}
          >
            {showClosed ? "Hide closed" : `Show closed${closedCount > 0 ? ` (${closedCount})` : ""}`}
          </Button>
          <Button
            variant={showArchived ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setShowArchived((prev) => !prev)}
            className="h-7 px-2 text-xs"
            title={showArchived ? "Hide archived tasks" : "Show archived tasks"}
          >
            {showArchived ? "Hide archived" : "Show archived"}
          </Button>
        </div>
      </div>

      {/* Task threads */}
      <div className="space-y-2" role="list" aria-label="Task list">
        {sortedTasks.map((task) => (
          <TaskThread
            key={task.id}
            task={task}
            loop={getLoopForTask(task)}
            isFocused={isTaskFocused(task.id)}
            ref={(el) => {
              if (el) {
                taskRefs.current.set(task.id, el);
              } else {
                taskRefs.current.delete(task.id);
              }
            }}
          />
        ))}
      </div>
    </div>
  );
}

function getArchiveReferenceDate(task: Task): Date | null {
  const candidates = [task.completedAt, task.updatedAt, task.createdAt];
  for (const value of candidates) {
    if (!value) continue;
    const date = new Date(value);
    if (!Number.isNaN(date.getTime())) {
      return date;
    }
  }
  return null;
}
