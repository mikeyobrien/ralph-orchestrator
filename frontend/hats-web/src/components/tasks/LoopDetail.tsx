/**
 * LoopDetail Component
 *
 * An expandable section showing loop details including prompt, worktree path,
 * merge PID/commit, and failure reason. Hides git-specific labels when
 * repoRoot is null (non-git workspace).
 *
 * Per spec lines 115-117:
 * - Expandable section showing prompt, worktree path, merge PID/commit, failure reason
 * - If non-git workspace, show "Workspace" instead of "Worktree" and hide git labels
 */

import { memo, useState, useCallback, useMemo } from "react";
import { ChevronDown, ChevronRight, Folder, GitBranch, Terminal, AlertTriangle, Hash, Clock } from "lucide-react";
import { cn } from "@/lib/utils";
import { LoopBadge, type LoopStatus } from "./LoopBadge";
import type { MergeButtonState } from "./LoopActions";

/**
 * Loop data shape matching spec's LoopRow interface (lines 47-62)
 */
export interface LoopDetailData {
  id: string;
  status: LoopStatus | string;
  location: string;
  workspaceRoot?: string;
  repoRoot?: string | null;
  cwd?: string;
  prompt: string;
  startedAt?: string;
  queuedAt?: string;
  pid?: number;
  mergePid?: number;
  mergeCommit?: string;
  failureReason?: string;
  isPrimary?: boolean;
  mergeButtonState?: MergeButtonState;
  // Iteration tracking for two-row TaskCard layout
  currentIteration?: number;
  maxIterations?: number;
}

interface LoopDetailProps {
  /** The loop data to display */
  loop: LoopDetailData;
  /** Whether the detail is initially expanded */
  defaultExpanded?: boolean;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Format a relative time string (e.g., "2h ago", "3d ago")
 */
function formatRelativeAge(isoDate: string): string {
  const date = new Date(isoDate);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return `${diffSecs}s ago`;
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

/**
 * Truncate text with ellipsis and optional tooltip
 */
function truncatePath(path: string, maxLength: number = 50): string {
  if (path.length <= maxLength) return path;
  return "..." + path.slice(-maxLength + 3);
}

/**
 * Shorten loop ID for display (first 12 chars)
 */
function shortId(id: string): string {
  return id.slice(0, 12);
}

function LoopDetailComponent({
  loop,
  defaultExpanded = false,
  className,
}: LoopDetailProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  const handleToggle = useCallback(() => {
    setIsExpanded((prev) => !prev);
  }, []);

  // Determine if this is a git workspace
  const isGitWorkspace = loop.repoRoot !== null && loop.repoRoot !== undefined;

  // Get the appropriate path label and value
  const pathLabel = isGitWorkspace ? "Worktree" : "Workspace";
  const pathValue = loop.workspaceRoot || loop.location || loop.cwd || "(unknown)";

  // Compute age from startedAt or queuedAt
  const age = useMemo(() => {
    if (loop.startedAt) return formatRelativeAge(loop.startedAt);
    if (loop.queuedAt) return formatRelativeAge(loop.queuedAt);
    return null;
  }, [loop.startedAt, loop.queuedAt]);

  // States that show merge-related info
  const showMergePid = loop.status === "merging" && loop.mergePid;
  const showMergeCommit = loop.status === "merged" && loop.mergeCommit;
  const showFailureReason = loop.status === "needs-review" && loop.failureReason;

  return (
    <div className={cn("border border-border rounded-md", className)}>
      {/* Expandable header */}
      <button
        type="button"
        className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent/50 transition-colors"
        onClick={handleToggle}
        aria-expanded={isExpanded}
      >
        {isExpanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground shrink-0" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground shrink-0" />
        )}
        <span className="font-medium text-muted-foreground">Loop Details</span>
        <LoopBadge status={loop.status} showPrefix={false} className="ml-auto" />
      </button>

      {/* Expanded content */}
      {isExpanded && (
        <div className="px-3 py-2 border-t border-border space-y-3 text-sm">
          {/* Loop ID */}
          <div className="flex items-start gap-2">
            <Hash className="h-4 w-4 text-muted-foreground shrink-0 mt-0.5" />
            <div className="flex-1 min-w-0">
              <span className="text-muted-foreground text-xs block">Loop ID</span>
              <span
                className="font-mono text-xs text-foreground"
                title={loop.id}
              >
                {shortId(loop.id)}
                {loop.isPrimary && (
                  <span className="ml-1 text-blue-400">(primary)</span>
                )}
              </span>
            </div>
            {age && (
              <span className="text-xs text-muted-foreground flex items-center gap-1">
                <Clock className="h-3 w-3" />
                {age}
              </span>
            )}
          </div>

          {/* Worktree/Workspace Path */}
          <div className="flex items-start gap-2">
            {isGitWorkspace ? (
              <GitBranch className="h-4 w-4 text-muted-foreground shrink-0 mt-0.5" />
            ) : (
              <Folder className="h-4 w-4 text-muted-foreground shrink-0 mt-0.5" />
            )}
            <div className="flex-1 min-w-0">
              <span className="text-muted-foreground text-xs block">
                {pathLabel}
              </span>
              <span
                className="font-mono text-xs text-foreground break-all"
                title={pathValue}
              >
                {truncatePath(pathValue, 60)}
              </span>
            </div>
          </div>

          {/* Prompt */}
          <div className="flex items-start gap-2">
            <Terminal className="h-4 w-4 text-muted-foreground shrink-0 mt-0.5" />
            <div className="flex-1 min-w-0">
              <span className="text-muted-foreground text-xs block">Prompt</span>
              <p className="text-xs text-foreground whitespace-pre-wrap break-words">
                {loop.prompt || "(no prompt)"}
              </p>
            </div>
          </div>

          {/* Merge PID (only when merging) */}
          {showMergePid && (
            <div className="flex items-start gap-2">
              <Terminal className="h-4 w-4 text-blue-400 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <span className="text-muted-foreground text-xs block">Merge PID</span>
                <span className="font-mono text-xs text-blue-400">
                  {loop.mergePid}
                </span>
              </div>
            </div>
          )}

          {/* Merge Commit (only when merged - for --all view) */}
          {showMergeCommit && (
            <div className="flex items-start gap-2">
              <GitBranch className="h-4 w-4 text-green-400 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <span className="text-muted-foreground text-xs block">Merge Commit</span>
                <span className="font-mono text-xs text-green-400">
                  {loop.mergeCommit?.slice(0, 8) || loop.mergeCommit}
                </span>
              </div>
            </div>
          )}

          {/* Failure Reason (only for needs-review) */}
          {showFailureReason && (
            <div className="flex items-start gap-2">
              <AlertTriangle className="h-4 w-4 text-red-400 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <span className="text-muted-foreground text-xs block">Failure Reason</span>
                <p className="text-xs text-red-400 whitespace-pre-wrap break-words">
                  {loop.failureReason}
                </p>
              </div>
            </div>
          )}

          {/* PID (for running loops) */}
          {loop.status === "running" && loop.pid && (
            <div className="flex items-start gap-2">
              <Terminal className="h-4 w-4 text-blue-400 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <span className="text-muted-foreground text-xs block">Process ID</span>
                <span className="font-mono text-xs text-blue-400">
                  {loop.pid}
                </span>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

LoopDetailComponent.displayName = "LoopDetail";

export const LoopDetail = memo(LoopDetailComponent);
