/**
 * TaskStatusBar Component
 *
 * Displays task status with an optional loop badge.
 * - Status badge with color-coded indicator based on task status
 * - Optional loop badge (clickable, links to loop detail page)
 */

import { useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { LoopBadge, type LoopStatus } from "./LoopBadge";
import { cn } from "@/lib/utils";
import { Loader2, Check, XCircle, Circle, CheckCircle2 } from "lucide-react";

export type TaskStatus = "open" | "running" | "completed" | "failed" | "closed";

export interface TaskStatusBarProps {
  /** The task status to display */
  status: TaskStatus;
  /** Optional loop ID for the loop badge */
  loopId?: string;
  /** Optional loop status for the loop badge */
  loopStatus?: LoopStatus | string;
  /** Additional CSS classes */
  className?: string;
}

interface StatusConfig {
  label: string;
  icon: typeof Loader2;
  variant: "default" | "secondary" | "destructive" | "outline";
  iconClass?: string;
  badgeClass?: string;
}

const STATUS_MAP: Record<TaskStatus, StatusConfig> = {
  open: {
    label: "Open",
    icon: Circle,
    variant: "secondary",
  },
  running: {
    label: "Running",
    icon: Loader2,
    variant: "outline",
    iconClass: "animate-spin",
    badgeClass: "bg-blue-500/10 border-blue-500/20 text-blue-400",
  },
  completed: {
    label: "Completed",
    icon: CheckCircle2,
    variant: "outline",
    badgeClass: "bg-green-500/10 border-green-500/20 text-green-400",
  },
  failed: {
    label: "Failed",
    icon: XCircle,
    variant: "destructive",
  },
  closed: {
    label: "Closed",
    icon: Check,
    variant: "secondary",
  },
};

export function TaskStatusBar({
  status,
  loopId,
  loopStatus,
  className,
}: TaskStatusBarProps) {
  const navigate = useNavigate();
  const config = STATUS_MAP[status];
  const StatusIcon = config.icon;

  const handleLoopClick = () => {
    if (loopId) {
      navigate(`/loops/${loopId}`);
    }
  };

  return (
    <div className={cn("flex items-center gap-2", className)}>
      <Badge
        variant={config.variant}
        className={cn("badge gap-1", config.badgeClass)}
      >
        <StatusIcon
          className={cn("h-3 w-3", config.iconClass)}
          aria-hidden="true"
        />
        <span>{config.label}</span>
      </Badge>
      {loopId && loopStatus && (
        <LoopBadge
          status={loopStatus}
          onClick={handleLoopClick}
          showPrefix={true}
        />
      )}
    </div>
  );
}
