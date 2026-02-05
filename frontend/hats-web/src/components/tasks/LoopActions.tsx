/**
 * LoopActions Component
 *
 * Action buttons for merge queue loops. Shows contextual actions based on
 * the current loop state.
 *
 * Per spec lines 106-110, 154-157:
 * - retry merge (needs-review)
 * - merge now (queued)
 * - discard (queued/needs-review/orphan)
 * - stop (running)
 * - Destructive actions require confirmation
 * - Actions show success/failure feedback
 */

import { memo, useState, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  RefreshCw,
  GitMerge,
  Trash2,
  Square,
  Loader2,
  AlertTriangle,
} from "lucide-react";
import type { LoopStatus } from "./LoopBadge";

/**
 * Merge button state from the server
 */
export interface MergeButtonState {
  state: "active" | "blocked";
  reason?: string;
}

/**
 * Actions available for loops
 */
export type LoopAction = "retry" | "merge" | "discard" | "stop";

/**
 * Callback for loop actions
 */
export interface LoopActionCallbacks {
  onRetry?: (id: string) => Promise<void>;
  onMerge?: (id: string, force?: boolean) => Promise<void>;
  onDiscard?: (id: string) => Promise<void>;
  onStop?: (id: string, force?: boolean) => Promise<void>;
}

interface LoopActionsProps {
  /** The loop ID */
  id: string;
  /** The loop status */
  status: LoopStatus | string;
  /** Optional: whether this is a git workspace (affects available actions) */
  isGitWorkspace?: boolean;
  /** Action callbacks */
  callbacks?: LoopActionCallbacks;
  /** Additional CSS classes */
  className?: string;
  /** Layout direction */
  direction?: "row" | "column";
  /** Merge button state (active or blocked with reason) */
  mergeButtonState?: MergeButtonState;
}

/**
 * Determine which actions are valid for a given loop state
 */
function getAvailableActions(
  status: LoopStatus | string,
  isGitWorkspace: boolean
): LoopAction[] {
  const actions: LoopAction[] = [];

  switch (status) {
    case "running":
      actions.push("stop");
      break;
    case "queued":
      if (isGitWorkspace) {
        actions.push("merge");
      }
      actions.push("discard");
      break;
    case "merging":
      // No actions while merge is in progress
      break;
    case "needs-review":
      if (isGitWorkspace) {
        actions.push("retry");
      }
      actions.push("discard");
      break;
    case "crashed":
    case "orphan":
      actions.push("discard");
      break;
    // merged, discarded - no actions needed
  }

  return actions;
}

/**
 * Action button configuration
 */
interface ActionConfig {
  label: string;
  icon: typeof RefreshCw;
  variant: "default" | "destructive" | "outline" | "secondary" | "ghost";
  description: string;
  requiresConfirmation: boolean;
  confirmMessage: string;
}

const ACTION_CONFIG: Record<LoopAction, ActionConfig> = {
  retry: {
    label: "Retry Merge",
    icon: RefreshCw,
    variant: "default",
    description: "Retry the failed merge operation",
    requiresConfirmation: false,
    confirmMessage: "",
  },
  merge: {
    label: "Merge Now",
    icon: GitMerge,
    variant: "default",
    description: "Immediately merge this loop's changes",
    requiresConfirmation: false,
    confirmMessage: "",
  },
  discard: {
    label: "Discard",
    icon: Trash2,
    variant: "destructive",
    description: "Permanently discard this loop",
    requiresConfirmation: true,
    confirmMessage: "Are you sure you want to discard this loop? This cannot be undone.",
  },
  stop: {
    label: "Stop",
    icon: Square,
    variant: "destructive",
    description: "Stop the running loop",
    requiresConfirmation: true,
    confirmMessage: "Are you sure you want to stop this loop?",
  },
};

function LoopActionsComponent({
  id,
  status,
  isGitWorkspace = true,
  callbacks = {},
  className,
  direction = "row",
  mergeButtonState,
}: LoopActionsProps) {
  const [loadingAction, setLoadingAction] = useState<LoopAction | null>(null);
  const [confirmingAction, setConfirmingAction] = useState<LoopAction | null>(null);
  const [error, setError] = useState<string | null>(null);

  const availableActions = getAvailableActions(status, isGitWorkspace);

  const executeAction = useCallback(
    async (action: LoopAction) => {
      setError(null);
      setLoadingAction(action);

      try {
        switch (action) {
          case "retry":
            await callbacks.onRetry?.(id);
            break;
          case "merge":
            await callbacks.onMerge?.(id);
            break;
          case "discard":
            await callbacks.onDiscard?.(id);
            break;
          case "stop":
            await callbacks.onStop?.(id);
            break;
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Action failed";
        setError(message);
      } finally {
        setLoadingAction(null);
        setConfirmingAction(null);
      }
    },
    [id, callbacks]
  );

  const handleActionClick = useCallback(
    (action: LoopAction) => {
      const config = ACTION_CONFIG[action];
      if (config.requiresConfirmation) {
        setConfirmingAction(action);
      } else {
        executeAction(action);
      }
    },
    [executeAction]
  );

  const handleConfirm = useCallback(() => {
    if (confirmingAction) {
      executeAction(confirmingAction);
    }
  }, [confirmingAction, executeAction]);

  const handleCancel = useCallback(() => {
    setConfirmingAction(null);
  }, []);

  // No actions available
  if (availableActions.length === 0) {
    return null;
  }

  // If confirming a destructive action
  if (confirmingAction) {
    const config = ACTION_CONFIG[confirmingAction];
    return (
      <div
        className={cn(
          "flex gap-2 p-2 bg-destructive/10 border border-destructive/20 rounded-md",
          direction === "column" ? "flex-col" : "flex-row items-center",
          className
        )}
      >
        <div className="flex items-center gap-2 text-sm text-destructive flex-1">
          <AlertTriangle className="h-4 w-4 shrink-0" />
          <span>{config.confirmMessage}</span>
        </div>
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="ghost"
            onClick={handleCancel}
            disabled={loadingAction !== null}
          >
            Cancel
          </Button>
          <Button
            size="sm"
            variant="destructive"
            onClick={handleConfirm}
            disabled={loadingAction !== null}
          >
            {loadingAction === confirmingAction && (
              <Loader2 className="h-3 w-3 animate-spin mr-1" />
            )}
            Confirm
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className={cn("space-y-2", className)}>
      {/* Action buttons */}
      <div
        className={cn(
          "flex gap-2",
          direction === "column" ? "flex-col" : "flex-row flex-wrap"
        )}
      >
        {availableActions.map((action) => {
          const config = ACTION_CONFIG[action];
          const ActionIcon = config.icon;
          const isLoading = loadingAction === action;
          const isMergeBlocked = action === "merge" && mergeButtonState?.state === "blocked";
          const isDisabled = loadingAction !== null || isMergeBlocked;
          const tooltip = isMergeBlocked && mergeButtonState?.reason
            ? mergeButtonState.reason
            : config.description;

          return (
            <Button
              key={action}
              size="sm"
              variant={config.variant}
              onClick={() => handleActionClick(action)}
              disabled={isDisabled}
              title={tooltip}
              className={cn(isMergeBlocked && "opacity-50")}
            >
              {isLoading ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <ActionIcon className="h-3 w-3" />
              )}
              <span>{config.label}</span>
            </Button>
          );
        })}
      </div>

      {/* Error message */}
      {error && (
        <div className="flex items-center gap-2 text-sm text-destructive">
          <AlertTriangle className="h-4 w-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}
    </div>
  );
}

LoopActionsComponent.displayName = "LoopActions";

export const LoopActions = memo(LoopActionsComponent);
