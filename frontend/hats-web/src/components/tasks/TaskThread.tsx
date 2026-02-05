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
  Trash2,
} from "lucide-react";
import { Card, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { LiveStatus } from "./LiveStatus";
import { trpc } from "@/trpc";
import { LoopBadge } from "./LoopBadge";
import { WorktreeBadge } from "./WorktreeBadge";
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
  // Loop ID for direct task↔loop mapping (preferred over PID)
  loopId?: string | null;
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
    icon: Play,
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
  const discardMutation = trpc.loops.discard.useMutation({
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
      if (loop && window.confirm("Merge this worktree branch into main?")) {
        mergeMutation.mutate({ id: loop.id });
      }
    },
    [loop, mergeMutation]
  );

  const handleDiscard = useCallback(
    (e: MouseEvent) => {
      e.stopPropagation();
      if (loop && window.confirm("Discard this worktree? This cannot be undone.")) {
        discardMutation.mutate({ id: loop.id });
      }
    },
    [loop, discardMutation]
  );

  const handleNavigate = useCallback(() => {
    navigate(`/tasks/${task.id}`);
  }, [task.id, navigate]);

  const relativeTime = useMemo(
    () => formatRelativeTime(new Date(task.updatedAt)),
    [task.updatedAt]
  );

  const isExecuting = runMutation.isPending || retryMutation.isPending || mergeMutation.isPending || discardMutation.isPending;

  // Determine if merge/discard buttons should be shown
  // Per spec: Show for worktree loops in "queued" or "needs-review" status
  // Hide for running and merging states
  const isWorktreeLoop = loop && loop.location !== "(in-place)";
  const loopStatus = loop?.status;
  const showMergeDiscardButtons = isWorktreeLoop && (loopStatus === "queued" || loopStatus === "needs-review");
  const isMergeBlocked = showMergeDiscardButtons && loop?.mergeButtonState?.state === "blocked";
  const mergeTooltip = isMergeBlocked && loop?.mergeButtonState?.reason
    ? loop.mergeButtonState.reason
    : "Merge this branch into main";

  // Visual distinction for merge-related loop tasks
  // Shows when loop is in merging, needs-review, or merged state
  const isMergeLoopTask = loop && ["merging", "needs-review", "merged"].includes(loop.status);

  // Check if iteration data is available from the loop
  const hasIteration = loop?.currentIteration != null && loop?.maxIterations != null;

  return (
    <Card
      ref={ref}
      className={cn(
        "cursor-pointer hover:bg-accent/50 transition-colors duration-150",
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
        <div className="flex flex-col gap-1.5">
          {/* Row 1: Status icon + Title */}
          <div className="flex items-center gap-3">
            <StatusIcon
              className={cn("h-5 w-5 shrink-0", statusConfig.color)}
              aria-hidden="true"
            />
            <span className="font-medium text-foreground flex-1 truncate">
              {task.title}
            </span>
          </div>

          {/* Row 2: StatusBadge + IterationBadge? + dot + RelativeTime + ActionButton */}
          <div className="flex items-center gap-2 ml-8 text-xs text-muted-foreground">
            {/* Status badge */}
            <Badge variant={statusConfig.badgeVariant} className="shrink-0">
              {statusConfig.label}
            </Badge>

            {/* Iteration badge - only shown when iteration data exists */}
            {hasIteration && (
              <span className="shrink-0 bg-blue-500/20 text-blue-400 px-2 py-0.5 rounded text-xs tabular-nums">
                {loop.currentIteration}/{loop.maxIterations}
              </span>
            )}

            {/* Worktree badge - only shown for non-primary (worktree) loops */}
            {isWorktreeLoop && loop && <WorktreeBadge loopId={loop.id} className="shrink-0" />}

            {/* Loop badge - only shown when a loop match exists */}
            {loop && <LoopBadge status={loop.status} className="shrink-0" />}

            {/* Dot separator */}
            <span className="text-muted-foreground/50" aria-hidden="true">•</span>

            {/* Relative time */}
            <span className="shrink-0 tabular-nums">
              {relativeTime}
            </span>

            {/* Spacer to push action buttons right */}
            <span className="flex-1" />

            {/* Merge button for worktree tasks - per explicit-merge-loop-ux spec */}
            {showMergeDiscardButtons && (
              <Button
                size="sm"
                variant={isMergeBlocked ? "ghost" : "default"}
                className={cn(
                  "shrink-0 h-6 px-2 text-xs",
                  !isMergeBlocked && "bg-green-600 hover:bg-green-700 text-white",
                  isMergeBlocked && "opacity-50"
                )}
                onClick={handleMerge}
                disabled={isExecuting || isMergeBlocked}
                title={mergeTooltip}
              >
                {mergeMutation.isPending ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <GitMerge className="h-3 w-3" />
                )}
                <span className="ml-1">Merge</span>
              </Button>
            )}

            {/* Discard button for worktree tasks */}
            {showMergeDiscardButtons && (
              <Button
                size="sm"
                variant="ghost"
                className="shrink-0 h-6 px-2 text-xs text-red-400 hover:text-red-300 hover:bg-red-500/10"
                onClick={handleDiscard}
                disabled={isExecuting}
                title="Discard this worktree"
              >
                {discardMutation.isPending ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <Trash2 className="h-3 w-3" />
                )}
                <span className="ml-1">Discard</span>
              </Button>
            )}

            {/* Run button */}
            {canRun && (
              <Button
                size="sm"
                variant="ghost"
                className="shrink-0 h-6 px-2 text-xs"
                onClick={handleRun}
                disabled={isExecuting}
              >
                {isExecuting ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <Play className="h-3 w-3" />
                )}
                <span className="ml-1">Run</span>
              </Button>
            )}

            {/* Retry button */}
            {canRetry && (
              <Button
                size="sm"
                variant="ghost"
                className="shrink-0 h-6 px-2 text-xs"
                onClick={handleRetry}
                disabled={isExecuting}
              >
                {isExecuting ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <RotateCcw className="h-3 w-3" />
                )}
                <span className="ml-1">Retry</span>
              </Button>
            )}
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
