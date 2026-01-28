/**
 * TaskThread Component
 *
 * A compact task card that displays essential task information and navigates
 * to a dedicated detail page on click. Shows task title, status badge,
 * timestamp, and action buttons.
 *
 * For running tasks, displays a LiveStatus component with real-time
 * WebSocket updates showing the latest status line.
 */

import { useMemo, useCallback, forwardRef, type MouseEvent, memo } from "react";
import { useNavigate } from "react-router-dom";
import {
  CheckCircle2,
  Circle,
  Clock,
  Loader2,
  XCircle,
  Play,
  RotateCcw,
  Archive,
  GitMerge,
} from "lucide-react";
import { Card, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { LiveStatus } from "./LiveStatus";
import { trpc } from "@/trpc";
import { LoopBadge } from "./LoopBadge";
import { type LoopDetailData } from "./LoopDetail";

/**
 * Task shape from the tRPC API.
 * Note: Dates come as ISO strings over JSON, so we accept both Date and string.
 */
export interface Task {
  id: string;
  title: string;
  status: string;
  priority: number;
  blockedBy: string | null;
  createdAt: Date | string;
  updatedAt: Date | string;
  // Execution tracking fields
  queuedTaskId?: string | null;
  startedAt?: Date | string | null;
  completedAt?: Date | string | null;
  errorMessage?: string | null;
  // Execution summary fields
  executionSummary?: string | null;
  exitCode?: number | null;
  durationMs?: number | null;
  archivedAt?: Date | string | null;
  // PID field for task↔loop mapping per spec lines 65-68
  // Backend must populate this from ProcessSupervisor for running tasks
  pid?: number | null;
}

interface TaskThreadProps {
  /** The task to display */
  task: Task;
  /** Optional loop data for loop visibility per spec lines 100-117 */
  loop?: LoopDetailData;
  /** Whether this task is focused via keyboard navigation */
  isFocused?: boolean;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Status configuration for visual styling
 */
interface StatusConfig {
  icon: typeof Circle;
  color: string;
  badgeVariant: "default" | "secondary" | "destructive" | "outline";
  label: string;
}

const STATUS_MAP: Record<string, StatusConfig> = {
  open: {
    icon: Circle,
    color: "text-zinc-400",
    badgeVariant: "secondary",
    label: "Open",
  },
  pending: {
    icon: Clock,
    color: "text-yellow-500",
    badgeVariant: "outline",
    label: "Pending",
  },
  running: {
    icon: Loader2,
    color: "text-blue-500",
    badgeVariant: "default",
    label: "Running",
  },
  completed: {
    icon: CheckCircle2,
    color: "text-green-500",
    badgeVariant: "secondary",
    label: "Completed",
  },
  closed: {
    icon: CheckCircle2,
    color: "text-green-500",
    badgeVariant: "secondary",
    label: "Closed",
  },
  failed: {
    icon: XCircle,
    color: "text-red-500",
    badgeVariant: "destructive",
    label: "Failed",
  },
  cancelled: {
    icon: XCircle,
    color: "text-orange-500",
    badgeVariant: "outline",
    label: "Cancelled",
  },
  archived: {
    icon: Archive,
    color: "text-zinc-500",
    badgeVariant: "outline",
    label: "Archived",
  },
  blocked: {
    icon: Clock,
    color: "text-orange-500",
    badgeVariant: "outline",
    label: "Blocked",
  },
};

const DEFAULT_STATUS: StatusConfig = {
  icon: Circle,
  color: "text-zinc-400",
  badgeVariant: "outline",
  label: "Unknown",
};

/**
 * Format a relative time string (e.g., "2 hours ago", "just now")
 */
function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return "just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;

  return date.toLocaleDateString();
}

const TaskThreadComponent = forwardRef<HTMLDivElement, TaskThreadProps>(function TaskThread(
  { task, loop, isFocused = false, className },
  ref
) {
  const navigate = useNavigate();

  const statusConfig = useMemo(() => {
    if (task.archivedAt) return STATUS_MAP.archived;
    return STATUS_MAP[task.status] || DEFAULT_STATUS;
  }, [task.status, task.archivedAt]);

  const StatusIcon = statusConfig.icon;
  const isArchived = !!task.archivedAt;
  const isArchivedFailed = isArchived && (!!task.errorMessage || (task.exitCode ?? 0) !== 0);
  const isRunning = task.status === "running";
  const isFailed = task.status === "failed" || isArchivedFailed;
  const isOpen = task.status === "open";

  // Can run: open or pending (not yet running)
  const canRun = isOpen && !task.blockedBy;
  // Can retry: only failed tasks
  const canRetry = isFailed;

  // tRPC mutations
  const utils = trpc.useUtils();
  const runMutation = trpc.task.run.useMutation({
    onSuccess: () => {
      utils.task.list.invalidate();
    },
  });
  const retryMutation = trpc.task.retry.useMutation({
    onSuccess: () => {
      utils.task.list.invalidate();
    },
  });
  const mergeMutation = trpc.loops.merge.useMutation({
    onSuccess: () => {
      utils.loops.list.invalidate();
    },
  });

  const handleRun = useCallback(
    (e: MouseEvent) => {
      e.stopPropagation();
      runMutation.mutate({ id: task.id });
    },
    [task.id, runMutation]
  );

  const handleRetry = useCallback(
    (e: MouseEvent) => {
      e.stopPropagation();
      retryMutation.mutate({ id: task.id });
    },
    [task.id, retryMutation]
  );

  const handleMerge = useCallback(
    (e: MouseEvent) => {
      e.stopPropagation();
      if (loop) {
        mergeMutation.mutate({ id: loop.id });
      }
    },
    [loop, mergeMutation]
  );

  const handleNavigate = useCallback(() => {
    navigate(`/tasks/${task.id}`);
  }, [task.id, navigate]);

  const relativeTime = useMemo(
    () => formatRelativeTime(new Date(task.updatedAt)),
    [task.updatedAt]
  );

  const isExecuting = runMutation.isPending || retryMutation.isPending || mergeMutation.isPending;

  // Determine if merge button should be shown
  // Per spec: Show merge button for tasks with worktree loops that are in "queued" status
  const isWorktreeLoop = loop && loop.location !== "(in-place)";
  const canMerge = isWorktreeLoop && loop?.status === "queued";
  const isMergeBlocked = canMerge && loop?.mergeButtonState?.state === "blocked";
  const mergeTooltip = isMergeBlocked && loop?.mergeButtonState?.reason
    ? loop.mergeButtonState.reason
    : "Merge this branch into main";

  // Visual distinction for merge-related loop tasks
  // Shows when loop is in merging, needs-review, or merged state
  const isMergeLoopTask = loop && ["merging", "needs-review", "merged"].includes(loop.status);

  return (
    <Card
      ref={ref}
      className={cn(
        "transition-all duration-200 cursor-pointer hover:bg-accent/50",
        isFocused && "ring-2 ring-primary bg-accent/30",
        // Visual distinction for merge loop tasks: green left border
        isMergeLoopTask && "border-l-4 border-l-green-500/60",
        className
      )}
      onClick={handleNavigate}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          handleNavigate();
        }
      }}
    >
      <CardHeader className="p-4">
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-3">
            {/* Status icon */}
            <StatusIcon
              className={cn("h-5 w-5 shrink-0", statusConfig.color, isRunning && "animate-spin")}
              aria-hidden="true"
            />

            {/* Task title */}
            <span className="font-medium flex-1 truncate">
              {task.title}
            </span>

            {/* Status badge */}
            <Badge variant={statusConfig.badgeVariant} className="shrink-0">
              {statusConfig.label}
            </Badge>

            {/* Loop badge - only shown when a loop match exists (spec line 150) */}
            {loop && <LoopBadge status={loop.status} className="shrink-0" />}

            {/* Merge button for worktree tasks - per explicit-merge-loop-ux spec */}
            {canMerge && (
              <Button
                size="sm"
                variant={isMergeBlocked ? "ghost" : "default"}
                className={cn(
                  "shrink-0 h-7 px-2",
                  !isMergeBlocked && "bg-green-600 hover:bg-green-700 text-white",
                  isMergeBlocked && "opacity-50"
                )}
                onClick={handleMerge}
                disabled={isExecuting || isMergeBlocked}
                title={mergeTooltip}
              >
                {mergeMutation.isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <GitMerge className="h-4 w-4" />
                )}
                <span className="ml-1">Merge</span>
              </Button>
            )}

            {/* Run/Retry/Cancel buttons */}
            {canRun && (
              <Button
                size="sm"
                variant="ghost"
                className="shrink-0 h-7 px-2"
                onClick={handleRun}
                disabled={isExecuting}
              >
                {isExecuting ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Play className="h-4 w-4" />
                )}
                <span className="ml-1">Run</span>
              </Button>
            )}
            {canRetry && (
              <Button
                size="sm"
                variant="ghost"
                className="shrink-0 h-7 px-2"
                onClick={handleRetry}
                disabled={isExecuting}
              >
                {isExecuting ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <RotateCcw className="h-4 w-4" />
                )}
                <span className="ml-1">Retry</span>
              </Button>
            )}

            {/* Relative time */}
            <span className="text-xs text-muted-foreground shrink-0 tabular-nums">
              {relativeTime}
            </span>
          </div>

          {/* Live status for running tasks */}
          {isRunning && <LiveStatus taskId={task.id} className="ml-8" />}
        </div>
      </CardHeader>
    </Card>
  );
});

TaskThreadComponent.displayName = "TaskThread";

function getUpdatedAtValue(value: Date | string): string {
  return typeof value === "string" ? value : value.toISOString();
}

const areTasksEqual = (prev: TaskThreadProps, next: TaskThreadProps): boolean => {
  if (prev.isFocused !== next.isFocused) return false;
  if (prev.className !== next.className) return false;
  if (prev.task.id !== next.task.id) return false;
  if (prev.task.status !== next.task.status) return false;
  if (prev.task.title !== next.task.title) return false;
  if (prev.task.blockedBy !== next.task.blockedBy) return false;
  if (getUpdatedAtValue(prev.task.updatedAt) !== getUpdatedAtValue(next.task.updatedAt)) {
    return false;
  }
  const prevArchived = prev.task.archivedAt ? getUpdatedAtValue(prev.task.archivedAt) : null;
  const nextArchived = next.task.archivedAt ? getUpdatedAtValue(next.task.archivedAt) : null;
  if (prevArchived !== nextArchived) return false;

  // Compare loop props for re-render when loop state changes
  if (prev.loop?.id !== next.loop?.id) return false;
  if (prev.loop?.status !== next.loop?.status) return false;
  // Compare mergeButtonState for merge button reactivity
  if (prev.loop?.mergeButtonState?.state !== next.loop?.mergeButtonState?.state) return false;
  if (prev.loop?.mergeButtonState?.reason !== next.loop?.mergeButtonState?.reason) return false;

  return true;
};

export const TaskThread = memo(TaskThreadComponent, areTasksEqual);
