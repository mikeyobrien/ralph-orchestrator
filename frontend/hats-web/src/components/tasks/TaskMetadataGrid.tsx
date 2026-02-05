/**
 * TaskMetadataGrid Component
 *
 * Displays task metadata in a two-column grid layout.
 * Left column: Timing information (created, updated, duration)
 * Right column: Execution details (exit code, tokens, cost)
 *
 * Per design spec: .sop/task-ux-improvements/design/detailed-design.md
 */

import { cn } from "@/lib/utils";
import { AlertTriangle } from "lucide-react";
import type { Task } from "./TaskThread";

export interface TaskMetadataGridProps {
  /** The task to display metadata for */
  task: Task;
  /** Optional token/cost metrics */
  metrics?: {
    tokensIn?: number;
    tokensOut?: number;
    estimatedCost?: number;
  };
  /** Additional CSS classes */
  className?: string;
}

/**
 * Format a date to locale string
 */
function formatDate(date: Date | string): string {
  const d = typeof date === "string" ? new Date(date) : date;
  return d.toLocaleString();
}

/**
 * Format duration from milliseconds to human-readable string
 */
function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    const remainingMins = minutes % 60;
    return `${hours}h ${remainingMins}m`;
  }
  if (minutes > 0) {
    const remainingSecs = seconds % 60;
    return `${minutes}m ${remainingSecs}s`;
  }
  return `${seconds}s`;
}

/**
 * Format token count with commas
 */
function formatTokens(tokensIn?: number, tokensOut?: number): string {
  if (tokensIn === undefined && tokensOut === undefined) {
    return "-";
  }
  const formatNum = (n: number) => n.toLocaleString();
  const inStr = tokensIn !== undefined ? formatNum(tokensIn) : "?";
  const outStr = tokensOut !== undefined ? formatNum(tokensOut) : "?";
  return `${inStr} in / ${outStr} out`;
}

/**
 * Format cost estimate
 */
function formatCost(cost?: number): string {
  if (cost === undefined) {
    return "-";
  }
  return `~$${cost.toFixed(2)}`;
}

/**
 * Single metadata item in the grid
 */
function MetadataItem({
  label,
  value,
  testId,
}: {
  label: string;
  value: React.ReactNode;
  testId?: string;
}) {
  return (
    <div className="space-y-1" data-testid={testId}>
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="text-sm font-medium">{value}</dd>
    </div>
  );
}

export function TaskMetadataGrid({
  task,
  metrics,
  className,
}: TaskMetadataGridProps) {
  // Calculate duration if not provided but timestamps are available
  const duration = task.durationMs
    ? formatDuration(task.durationMs)
    : task.startedAt && task.completedAt
      ? formatDuration(
          new Date(task.completedAt).getTime() -
            new Date(task.startedAt).getTime()
        )
      : "-";

  return (
    <div className={cn("space-y-4", className)}>
      {/* Two-column grid */}
      <dl
        className="grid grid-cols-2 gap-4 rounded-lg border bg-card p-4"
        data-testid="metadata-grid"
      >
        {/* Left column: Timing */}
        <MetadataItem
          label="Created"
          value={formatDate(task.createdAt)}
          testId="metadata-created"
        />
        <MetadataItem
          label="Exit Code"
          value={task.exitCode ?? "-"}
          testId="metadata-exit-code"
        />

        <MetadataItem
          label="Updated"
          value={formatDate(task.updatedAt)}
          testId="metadata-updated"
        />
        <MetadataItem
          label="Tokens"
          value={formatTokens(metrics?.tokensIn, metrics?.tokensOut)}
          testId="metadata-tokens"
        />

        <MetadataItem
          label="Duration"
          value={duration}
          testId="metadata-duration"
        />
        <MetadataItem
          label="Est. Cost"
          value={formatCost(metrics?.estimatedCost)}
          testId="metadata-cost"
        />
      </dl>

      {/* Error display below grid */}
      {task.errorMessage && (
        <div
          className="flex items-start gap-2 rounded-lg border border-destructive/50 bg-destructive/10 p-4"
          data-testid="metadata-error"
        >
          <AlertTriangle className="h-5 w-5 text-destructive shrink-0 mt-0.5" />
          <div className="space-y-1">
            <p className="text-sm font-medium text-destructive">Error</p>
            <p className="text-sm text-muted-foreground">{task.errorMessage}</p>
          </div>
        </div>
      )}
    </div>
  );
}
