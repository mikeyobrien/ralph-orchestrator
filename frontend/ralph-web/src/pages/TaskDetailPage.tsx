/**
 * TaskDetailPage Component
 *
 * Dedicated page for viewing task details.
 * Replaces the inline expansion pattern with a full-page view.
 *
 * Features:
 * - Full prompt display (not truncated)
 * - Rich status metrics (duration, timestamps, exit code)
 * - Log viewer
 * - Action buttons (run, retry, cancel)
 * - Navigation back to task list
 */

import { useEffect, useState, useCallback, useMemo } from "react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { trpc } from "@/trpc";
import { formatDuration, formatDate } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import { EnhancedLogViewer } from "@/components/tasks/EnhancedLogViewer";
import { AlertTriangle, Send, Loader2, GitMerge, CheckCircle2, FileText, GitCommit } from "lucide-react";
import { type LoopDetailData } from "@/components/tasks/LoopDetail";
import { LoopBadge } from "@/components/tasks/LoopBadge";

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
      className={`mt-4 rounded-lg border ${
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
            isMerged ? "text-green-700 dark:text-green-400" : "text-muted-foreground"
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

/**
 * Reusable component for displaying a label-value metric pair
 */
function MetricRow({
  label,
  value,
  valueClassName,
  testId,
}: {
  label: string;
  value: React.ReactNode;
  valueClassName?: string;
  testId?: string;
}) {
  return (
    <div>
      <span className="text-muted-foreground">{label}: </span>
      <span className={valueClassName} data-testid={testId}>
        {value}
      </span>
    </div>
  );
}

/**
 * Map task status to badge variant
 */
function getStatusVariant(
  status: string
): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "running":
      return "default";
    case "completed":
      return "secondary";
    case "failed":
      return "destructive";
    default:
      return "outline";
  }
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

  // Fetch loops for PID-based mapping to associate task with loop
  const loopsQuery = trpc.loops.list.useQuery(
    { includeTerminal: true },
    { refetchInterval: 5000 }
  );

  // Create PID to loop map for task-loop association
  const associatedLoop = useMemo(() => {
    if (!loopsQuery.data || !task?.pid) return undefined;
    const loops = loopsQuery.data as LoopDetailData[];
    return loops.find((loop) => loop.pid === task.pid);
  }, [loopsQuery.data, task?.pid]);

  // User steering state for needs-review loops
  const [steeringInput, setSteeringInput] = useState("");

  // Mutations
  const utils = trpc.useUtils();
  const runMutation = trpc.task.run.useMutation();
  const retryMutation = trpc.task.retry.useMutation();
  const cancelMutation = trpc.task.cancel.useMutation();
  const retryMergeMutation = trpc.loops.retry.useMutation({
    onSuccess: () => {
      utils.loops.list.invalidate();
      setSteeringInput("");
    },
  });

  // Handle retry merge with user steering input
  const handleRetryMerge = useCallback(() => {
    if (!associatedLoop) return;
    retryMergeMutation.mutate({
      id: associatedLoop.id,
      steeringInput: steeringInput.trim() || undefined,
    });
  }, [associatedLoop, retryMergeMutation, steeringInput]);

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

  // Loading state
  if (isLoading) {
    return (
      <div className="p-6">
        <p>Loading...</p>
      </div>
    );
  }

  // Error state
  if (isError) {
    return (
      <div className="p-6">
        <p>Error</p>
        <p>{error?.message || "Task not found"}</p>
      </div>
    );
  }

  // Not found state
  if (!task) {
    return (
      <div className="p-6">
        <p>Task not found</p>
      </div>
    );
  }

  // Determine which action buttons to show
  const showRunButton = task.status === "open";
  const showCancelButton = task.status === "running";
  const showRetryButton = task.status === "failed";

  // Determine if log viewer should be shown (for running or completed tasks)
  const showLogViewer =
    task.status === "running" ||
    task.status === "completed" ||
    task.status === "failed";

  return (
    <div className="p-6 space-y-6">
      {/* Back navigation */}
      <Link to="/tasks" className="text-blue-500 hover:underline">
        Back to Tasks
      </Link>

      {/* Page title */}
      <h1 className="text-2xl font-bold">{task.title}</h1>

      {/* Status metrics section */}
      <div className="space-y-4">
        {/* Status badge with loop badge */}
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground">Status: </span>
          <Badge variant={getStatusVariant(task.status)} className="capitalize">
            {task.status}
          </Badge>
          {associatedLoop && (
            <LoopBadge status={associatedLoop.status} />
          )}
        </div>

        {/* Timestamps */}
        <MetricRow label="Created" value={formatDate(task.createdAt)} />
        <MetricRow label="Updated" value={formatDate(task.updatedAt)} />

        {/* Duration (for completed/failed tasks) */}
        {task.durationMs && (
          <MetricRow label="Duration" value={formatDuration(task.durationMs)} />
        )}

        {/* Exit code (for completed/failed tasks) */}
        {task.exitCode !== null && task.exitCode !== undefined && (
          <MetricRow
            label="Exit Code"
            value={task.exitCode}
            testId="exit-code-value"
          />
        )}

        {/* Error message (for failed tasks) */}
        {task.errorMessage && (
          <MetricRow
            label="Error"
            value={task.errorMessage}
            valueClassName="text-red-500"
          />
        )}

        {/* Execution summary (for completed tasks) */}
        {task.executionSummary && (
          <ExecutionSummary summary={task.executionSummary} loop={associatedLoop} />
        )}
      </div>

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

      {/* Action buttons */}
      <div className="flex gap-2">
        {showRunButton && (
          <Button
            onClick={() => runMutation.mutate({ id: task.id })}
            disabled={runMutation.isPending}
          >
            Run
          </Button>
        )}
        {showCancelButton && (
          <Button
            variant="destructive"
            onClick={() => cancelMutation.mutate({ id: task.id })}
            disabled={cancelMutation.isPending}
          >
            Cancel
          </Button>
        )}
        {showRetryButton && (
          <Button
            variant="secondary"
            onClick={() => retryMutation.mutate({ id: task.id })}
            disabled={retryMutation.isPending}
          >
            Retry
          </Button>
        )}
      </div>

      {/* Log viewer (for running/completed/failed tasks) */}
      {showLogViewer && (
        <div data-testid="log-viewer">
          <EnhancedLogViewer taskId={task.id} />
        </div>
      )}
    </div>
  );
}
