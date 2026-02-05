/**
 * LoopBadge Component
 *
 * A compact badge displaying the loop/merge state for task headers.
 * Shows loop status with appropriate color coding aligned with CLI conventions.
 *
 * States and their meanings (from docs/advanced/parallel-loops.md):
 * - running: Loop is actively executing
 * - queued: Completed, waiting for merge
 * - merging: Merge operation in progress
 * - merged: Successfully merged to main
 * - needs-review: Merge failed, requires manual resolution
 * - crashed: Process died unexpectedly
 * - orphan: Worktree exists but not tracked
 * - discarded: Explicitly abandoned by user
 */

import { memo, useMemo } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { RefreshCw, GitMerge, AlertTriangle, Clock, CheckCircle2, XCircle, Trash2 } from "lucide-react";

export type LoopStatus =
  | "running"
  | "queued"
  | "merging"
  | "merged"
  | "needs-review"
  | "crashed"
  | "orphan"
  | "discarded";

interface LoopBadgeProps {
  /** The loop status to display */
  status: LoopStatus | string | null | undefined;
  /** Optional click handler */
  onClick?: () => void;
  /** Additional CSS classes */
  className?: string;
  /** Whether to show the "Loop:" prefix */
  showPrefix?: boolean;
}

/**
 * Status configuration for visual styling
 */
interface StatusConfig {
  icon: typeof RefreshCw;
  color: string;
  bgColor: string;
  label: string;
}

/**
 * Status configurations aligned with CLI and spec conventions
 */
const STATUS_MAP: Record<LoopStatus, StatusConfig> = {
  running: {
    icon: RefreshCw,
    color: "text-blue-400",
    bgColor: "bg-blue-500/10 border-blue-500/20",
    label: "running",
  },
  queued: {
    icon: Clock,
    color: "text-yellow-400",
    bgColor: "bg-yellow-500/10 border-yellow-500/20",
    label: "queued",
  },
  merging: {
    icon: GitMerge,
    color: "text-blue-400",
    bgColor: "bg-blue-500/10 border-blue-500/20",
    label: "merging",
  },
  merged: {
    icon: CheckCircle2,
    color: "text-green-400",
    bgColor: "bg-green-500/10 border-green-500/20",
    label: "merged",
  },
  "needs-review": {
    icon: AlertTriangle,
    color: "text-red-400",
    bgColor: "bg-red-500/10 border-red-500/20",
    label: "needs-review",
  },
  crashed: {
    icon: XCircle,
    color: "text-red-400",
    bgColor: "bg-red-500/10 border-red-500/20",
    label: "crashed",
  },
  orphan: {
    icon: AlertTriangle,
    color: "text-orange-400",
    bgColor: "bg-orange-500/10 border-orange-500/20",
    label: "orphan",
  },
  discarded: {
    icon: Trash2,
    color: "text-zinc-400",
    bgColor: "bg-zinc-500/10 border-zinc-500/20",
    label: "discarded",
  },
};

const DEFAULT_STATUS: StatusConfig = {
  icon: Clock,
  color: "text-zinc-400",
  bgColor: "bg-zinc-500/10 border-zinc-500/20",
  label: "unknown",
};

function LoopBadgeComponent({
  status,
  onClick,
  className,
  showPrefix = true,
}: LoopBadgeProps) {
  // Don't render if no status
  if (!status) return null;

  const config = useMemo(() => {
    return STATUS_MAP[status as LoopStatus] ?? DEFAULT_STATUS;
  }, [status]);

  const StatusIcon = config.icon;
  const isAnimated = status === "running" || status === "merging";
  const isClickable = !!onClick;

  return (
    <Badge
      variant="outline"
      className={cn(
        "gap-1 px-2 py-0.5 text-xs font-medium border",
        config.bgColor,
        config.color,
        isClickable && "cursor-pointer hover:opacity-80 transition-opacity",
        className
      )}
      onClick={onClick}
      role={isClickable ? "button" : undefined}
    >
      <StatusIcon
        className={cn(
          "h-3 w-3",
          isAnimated && "animate-spin"
        )}
        aria-hidden="true"
      />
      {showPrefix && <span className="opacity-70">Loop:</span>}
      <span>{config.label}</span>
    </Badge>
  );
}

LoopBadgeComponent.displayName = "LoopBadge";

export const LoopBadge = memo(LoopBadgeComponent);
