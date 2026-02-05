/**
 * WorktreeBadge Component
 *
 * A compact purple badge displaying the worktree name for non-primary loops.
 * Shows [GitBranch icon] worktree: <memorable-name> extracted from loop ID.
 *
 * Only shown for worktree (non-primary) loops. The memorable name is
 * extracted from the loop ID format "loop-<adjective>-<noun>-<hash>".
 */

import { memo, useMemo } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { GitBranch } from "lucide-react";

interface WorktreeBadgeProps {
  /** The loop ID to extract the memorable name from */
  loopId: string;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Extract the memorable name from a loop ID.
 * Format: "loop-<adjective>-<noun>-<hash>" -> "<adjective>-<noun>"
 * Falls back to first 12 chars of ID if pattern doesn't match.
 */
function extractMemorableName(loopId: string): string {
  // Match pattern: loop-<word>-<word>-<hash>
  const match = loopId.match(/^loop-([a-z]+)-([a-z]+)-/i);
  if (match) {
    return `${match[1]}-${match[2]}`;
  }
  // Fallback: return shortened ID
  return loopId.slice(0, 12);
}

function WorktreeBadgeComponent({ loopId, className }: WorktreeBadgeProps) {
  const memorableName = useMemo(() => extractMemorableName(loopId), [loopId]);

  return (
    <Badge
      variant="outline"
      className={cn(
        "gap-1 px-2 py-0.5 text-xs font-medium border",
        "bg-purple-500/10 border-purple-500/20 text-purple-400",
        className
      )}
    >
      <GitBranch className="h-3 w-3" aria-hidden="true" />
      <span className="opacity-70">worktree:</span>
      <span>{memorableName}</span>
    </Badge>
  );
}

WorktreeBadgeComponent.displayName = "WorktreeBadge";

export const WorktreeBadge = memo(WorktreeBadgeComponent);
