/**
 * TaskDetailHeader Component
 *
 * Header for task detail view with navigation and status-based actions.
 * - Left side: Back navigation button ("← Back to Tasks")
 * - Right side: Delete button (for failed/closed) + Status-based action button (Cancel/Retry/Run)
 */

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { ArrowLeft, Loader2, Trash2, Circle, Check, XCircle, CheckCircle2 } from "lucide-react";

export type TaskStatus = "open" | "running" | "completed" | "closed" | "failed";
export type TaskAction = "run" | "cancel" | "retry";

export interface TaskDetailHeaderProps {
  /** Current task status */
  status: TaskStatus;
  /** Callback when back button is clicked */
  onBack: () => void;
  /** Callback when action button is clicked - if undefined, action button is disabled */
  onAction?: (action: TaskAction) => void;
  /** Whether an action is pending (shows loading state) */
  isActionPending?: boolean;
  /** Whether to show delete button (for terminal states) */
  showDelete?: boolean;
  /** Callback when delete button is clicked */
  onDelete?: () => void;
  /** Whether delete action is pending */
  isDeletePending?: boolean;
}

/**
 * Get the action configuration for a given status
 */
function getActionForStatus(status: TaskStatus): { action: TaskAction; label: string; variant: "default" | "destructive" } | null {
  switch (status) {
    case "running":
      return { action: "cancel", label: "Cancel", variant: "destructive" };
    case "failed":
      return { action: "retry", label: "Retry", variant: "default" };
    case "open":
      return { action: "run", label: "Run", variant: "default" };
    case "completed":
    case "closed":
      return null;
    default:
      return null;
  }
}

/**
 * Status badge configuration
 */
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

export function TaskDetailHeader({
  status,
  onBack,
  onAction,
  isActionPending = false,
  showDelete = false,
  onDelete,
  isDeletePending = false,
}: TaskDetailHeaderProps) {
  const actionConfig = getActionForStatus(status);
  const statusConfig = STATUS_MAP[status];
  const StatusIcon = statusConfig.icon;

  return (
    <div className="flex justify-between items-center">
      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          className="gap-1"
          onClick={onBack}
        >
          <ArrowLeft className="lucide-arrow-left" />
          Back to Tasks
        </Button>

        {/* Status badge */}
        <Badge
          variant={statusConfig.variant}
          className={cn("gap-1", statusConfig.badgeClass)}
        >
          <StatusIcon
            className={cn("h-3 w-3", statusConfig.iconClass)}
            aria-hidden="true"
          />
          <span>{statusConfig.label}</span>
        </Badge>
      </div>

      <div className="flex items-center gap-2">
        {showDelete && (
          <Button
            variant="destructive"
            onClick={onDelete}
            disabled={!onDelete || isDeletePending}
          >
            {isDeletePending ? (
              <Loader2 className="lucide-loader-2 animate-spin mr-2" />
            ) : (
              <Trash2 className="lucide-trash-2 mr-2" />
            )}
            Delete
          </Button>
        )}

        {actionConfig && (
          <Button
            variant={actionConfig.variant}
            onClick={() => onAction?.(actionConfig.action)}
            disabled={!onAction || isActionPending}
          >
            {isActionPending && <Loader2 className="lucide-loader-2 animate-spin" />}
            {actionConfig.label}
          </Button>
        )}
      </div>
    </div>
  );
}
