/**
 * TaskDetailPage Component
 *
 * Dedicated page for viewing task details with improved UX.
 * Per spec: .sop/task-ux-improvements/design/detailed-design.md
 *
 * Layout:
 * - TaskDetailHeader: Back navigation + action buttons
 * - Title: Full prompt display
 * - TaskStatusBar: Status, iteration, loop, preset badges
 * - TaskMetadataGrid: Two-column timing and execution details
 * - ExecutionSummary: Collapsible execution results
 * - User steering UI (for needs-review loops)
 * - EnhancedLogViewer: Real-time log streaming
 */

import { useEffect, useState, useCallback, useMemo } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { trpc } from "@/trpc";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  EnhancedLogViewer,
  TaskCardSkeleton,
  EmptyState,
  TaskDetailHeader,
  TaskStatusBar,
  TaskMetadataGrid,
  type LoopDetailData,
} from "@/components/tasks";
import {
  AlertTriangle,
  Loader2,
  GitMerge,
  CheckCircle2,
  FileText,
  GitCommit,
  AlertCircle,
  FileQuestion,
  Trash2,
} from "lucide-react";
import type { TaskAction } from "@/components/tasks/TaskDetailHeader";

/**
 * ExecutionSummary Component
 *
 * Displays task execution results with special handling for merge loops.
 * Shows merge-specific information (commit SHA) when the associated loop
 * has been successfully merged.
 */
function ExecutionSummary({
  summary,
  loop,
}: {
  summary: string;
  loop?: LoopDetailData;
}) {
  const isMerged = loop?.status === "merged";
  const mergeCommit = loop?.mergeCommit;

  return (
    <div
      className={`rounded-lg border ${
        isMerged
          ? "border-green-500/30 bg-green-500/5"
          : "border-border bg-muted/50"
      }`}
      data-testid="execution-summary"
    >
      {/* Header */}
      <div
        className={`flex items-center gap-2 px-4 py-3 border-b ${
          isMerged ? "border-green-500/20" : "border-border"
        }`}
      >
        {isMerged ? (
          <CheckCircle2 className="h-5 w-5 text-green-500" />
        ) : (
          <FileText className="h-5 w-5 text-muted-foreground" />
        )}
        <h3
          className={`font-semibold ${
            isMerged
              ? "text-green-700 dark:text-green-400"
              : "text-muted-foreground"
          }`}
        >
          {isMerged ? "Merge Complete" : "Execution Summary"}
        </h3>
      </div>

      {/* Merge commit info (for merged loops) */}
      {isMerged && mergeCommit && (
        <div
          className="flex items-center gap-2 px-4 py-2 border-b border-green-500/20 bg-green-500/10"
          data-testid="merge-commit-info"
        >
          <GitCommit className="h-4 w-4 text-green-600 dark:text-green-400" />
          <span className="text-sm text-muted-foreground">Merge commit:</span>
          <code className="text-sm font-mono text-green-700 dark:text-green-400">
            {mergeCommit.slice(0, 8)}
          </code>
        </div>
      )}

      {/* Summary content */}
      <div className="p-4">
        <div className="whitespace-pre-wrap text-sm">{summary}</div>
      </div>
    </div>
  );
}

export function TaskDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  // Fetch task data
  const {
    data: task,
    isLoading,
    isError,
    error,
  } = trpc.task.get.useQuery({ id: id! }, { enabled: !!id });

  // Fetch loops for loopId-based mapping to associate task with loop
  const loopsQuery = trpc.loops.list.useQuery(
    { includeTerminal: true },
    { refetchInterval: 5000 }
  );

  // Find the associated loop by loopId
  const associatedLoop = useMemo(() => {
    if (!loopsQuery.data || !task?.loopId) return undefined;
    const loops = loopsQuery.data as LoopDetailData[];
    return loops.find((loop) => loop.id === task.loopId);
  }, [loopsQuery.data, task?.loopId]);

  // User steering state for needs-review loops
  const [steeringInput, setSteeringInput] = useState("");

  // Mutations
  const utils = trpc.useUtils();
  const runMutation = trpc.task.run.useMutation();
  const retryMutation = trpc.task.retry.useMutation();
  const cancelMutation = trpc.task.cancel.useMutation();
  const deleteMutation = trpc.task.delete.useMutation({
    onSuccess: () => {
      navigate("/tasks");
    },
  });
  const retryMergeMutation = trpc.loops.retry.useMutation({
    onSuccess: () => {
      utils.loops.list.invalidate();
      setSteeringInput("");
    },
  });

  // Handle actions from TaskDetailHeader
  const handleAction = useCallback(
    (action: TaskAction) => {
      if (!task) return;
      switch (action) {
        case "run":
          runMutation.mutate({ id: task.id });
          break;
        case "retry":
          retryMutation.mutate({ id: task.id });
          break;
        case "cancel":
          cancelMutation.mutate({ id: task.id });
          break;
      }
    },
    [task, runMutation, retryMutation, cancelMutation]
  );

  // Handle retry merge with user steering input
  const handleRetryMerge = useCallback(() => {
    if (!associatedLoop) return;
    retryMergeMutation.mutate({
      id: associatedLoop.id,
      steeringInput: steeringInput.trim() || undefined,
    });
  }, [associatedLoop, retryMergeMutation, steeringInput]);

  // Handle task deletion with confirmation
  const handleDelete = useCallback(() => {
    if (!task) return;
    const confirmed = window.confirm(
      `Are you sure you want to delete this task?\n\n"${task.title}"\n\nThis action cannot be undone.`
    );
    if (confirmed) {
      deleteMutation.mutate({ id: task.id });
    }
  }, [task, deleteMutation]);

  // Keyboard navigation - Escape to go back
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        navigate("/tasks");
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [navigate]);

  // Loading state with skeletons
  if (isLoading) {
    return (
      <div className="p-6 space-y-4">
        <TaskCardSkeleton />
        <TaskCardSkeleton />
      </div>
    );
  }

  // Error state
  if (isError) {
    return (
      <div className="p-6">
        <EmptyState
          icon={AlertCircle}
          title="Error"
          description={error?.message || "Task not found"}
        />
      </div>
    );
  }

  // Not found state
  if (!task) {
    return (
      <div className="p-6">
        <EmptyState
          icon={FileQuestion}
          title="Task not found"
          description="The requested task could not be found."
        />
      </div>
    );
  }

  // Allow deletion only for terminal states (failed or closed)
  const showDeleteButton = task.status === "failed" || task.status === "closed";

  // Determine if log viewer should be shown (for running or completed tasks)
  const showLogViewer =
    task.status === "running" ||
    task.status === "completed" ||
    task.status === "closed" ||
    task.status === "failed";

  // Check if any action is pending
  const isActionPending =
    runMutation.isPending ||
    retryMutation.isPending ||
    cancelMutation.isPending;

  // Map task status for components
  const taskStatus = task.status as
    | "open"
    | "running"
    | "completed"
    | "closed"
    | "failed";

  return (
    <div className="p-6 space-y-6">
      {/* Header with back navigation and action buttons */}
      <TaskDetailHeader
        status={taskStatus}
        onBack={() => navigate("/tasks")}
        onAction={handleAction}
        isActionPending={isActionPending}
      />

      {/* Page title - full prompt display */}
      <h1 className="text-xl font-semibold">{task.title}</h1>

      {/* Status bar with badges */}
      <TaskStatusBar
        status={taskStatus}
        loopId={associatedLoop?.id}
        loopStatus={associatedLoop?.status}
      />

      {/* Metadata grid - two column layout */}
      <TaskMetadataGrid
        task={task}
        // Future: Pass metrics when backend supports token/cost tracking
        // metrics={{ tokensIn: task.tokensIn, tokensOut: task.tokensOut, estimatedCost: task.estimatedCost }}
      />

      {/* Execution summary (for completed tasks) */}
      {task.executionSummary && (
        <ExecutionSummary summary={task.executionSummary} loop={associatedLoop} />
      )}

      {/* User steering UI for needs-review loops */}
      {associatedLoop?.status === "needs-review" && (
        <div
          className="border border-amber-500/30 bg-amber-500/10 rounded-lg p-4 space-y-4"
          data-testid="user-steering-callout"
        >
          <div className="flex items-start gap-3">
            <AlertTriangle className="h-5 w-5 text-amber-500 shrink-0 mt-0.5" />
            <div className="flex-1">
              <h3 className="font-semibold text-amber-700 dark:text-amber-400">
                Merge Needs Your Input
              </h3>
              {associatedLoop.failureReason && (
                <p className="text-sm text-muted-foreground mt-1">
                  {associatedLoop.failureReason}
                </p>
              )}
            </div>
          </div>

          <div className="space-y-3">
            <label className="text-sm font-medium" htmlFor="steering-input">
              Provide clarification or guidance for the merge
            </label>
            <Textarea
              id="steering-input"
              value={steeringInput}
              onChange={(e) => setSteeringInput(e.target.value)}
              placeholder="e.g., 'Keep my changes, discard incoming' or 'Prefer the newer API version'"
              className="min-h-[80px] resize-none"
              disabled={retryMergeMutation.isPending}
            />
            <div className="flex items-center justify-between">
              <span className="text-xs text-muted-foreground">
                Your input will guide the next merge attempt
              </span>
              <Button
                onClick={handleRetryMerge}
                disabled={retryMergeMutation.isPending}
                className="bg-amber-600 hover:bg-amber-700 text-white"
              >
                {retryMergeMutation.isPending ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    Retrying...
                  </>
                ) : (
                  <>
                    <GitMerge className="h-4 w-4 mr-2" />
                    Retry Merge
                  </>
                )}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Delete button for terminal states */}
      {showDeleteButton && (
        <div className="flex gap-2">
          <Button
            variant="destructive"
            onClick={handleDelete}
            disabled={deleteMutation.isPending}
          >
            {deleteMutation.isPending ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                Deleting...
              </>
            ) : (
              <>
                <Trash2 className="h-4 w-4 mr-2" />
                Delete
              </>
            )}
          </Button>
        </div>
      )}

      {/* Log viewer (for running/completed/failed tasks) */}
      {showLogViewer && (
        <div data-testid="log-viewer">
          <EnhancedLogViewer taskId={task.id} />
        </div>
      )}
    </div>
  );
}
