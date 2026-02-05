/**
 * LiveStatus Component
 *
 * Displays real-time status updates for a running task.
 * Shows the latest log line with a pulsing connection indicator.
 *
 * Features:
 * - WebSocket subscription for live updates
 * - Pulsing indicator for active connection
 * - Truncated latest status line preview
 * - Graceful handling of connection states
 */

import { useMemo } from "react";
import { useTaskWebSocket, type ConnectionState, type HatsEvent } from "@/hooks/useTaskWebSocket";
import { cn } from "@/lib/utils";

interface LiveStatusProps {
  /** Task ID to subscribe to */
  taskId: string;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Connection indicator colors
 */
const CONNECTION_COLORS: Record<ConnectionState, string> = {
  connecting: "bg-yellow-500",
  connected: "bg-green-500",
  disconnected: "bg-zinc-500",
  error: "bg-red-500",
};

/**
 * Status labels for different connection states
 */
const CONNECTION_LABELS: Record<ConnectionState, string> = {
  connecting: "Connecting...",
  connected: "Connected",
  disconnected: "Disconnected",
  error: "Connection error",
};

/**
 * Format a Hats event for display
 */
function formatEvent(event: HatsEvent): string {
  // Show topic with optional hat prefix
  const parts: string[] = [];
  if (event.hat) {
    parts.push(`[${event.hat}]`);
  }
  parts.push(event.topic);
  return parts.join(" ");
}

export function LiveStatus({ taskId, className }: LiveStatusProps) {
  // Note: The hook now uses a global store for logs. We only use latestEntry and latestEvent here.
  const { latestEntry, latestEvent, connectionState, error } = useTaskWebSocket(taskId);

  // Format the status line for display (truncate if too long)
  // Prioritize: error > event > log entry > connection state
  const statusLine = useMemo(() => {
    if (error) {
      return error;
    }
    // Show Hats events when available (preferred status)
    if (latestEvent) {
      return formatEvent(latestEvent);
    }
    if (!latestEntry) {
      if (connectionState === "connected") {
        return "Starting...";
      }
      return CONNECTION_LABELS[connectionState];
    }
    // Truncate long lines for preview
    const line = latestEntry.line.trim();
    if (line.length > 80) {
      return line.substring(0, 77) + "...";
    }
    return line;
  }, [latestEntry, latestEvent, connectionState, error]);

  const isConnected = connectionState === "connected";
  const isError = connectionState === "error" || !!error;
  const hasEvent = !!latestEvent;

  return (
    <div className={cn("flex items-center gap-2 text-xs font-mono", className)}>
      {/* Connection indicator with pulse animation when connected */}
      <span
        className={cn(
          "h-2 w-2 rounded-full shrink-0",
          CONNECTION_COLORS[connectionState],
          isConnected && "animate-pulse"
        )}
        aria-label={CONNECTION_LABELS[connectionState]}
        role="status"
      />

      {/* Status line */}
      <span
        className={cn(
          "truncate",
          isError ? "text-red-400" : "text-zinc-400",
          isConnected && latestEntry && !hasEvent && "text-zinc-300",
          // Events get highlighted styling
          hasEvent && "text-amber-400 font-medium"
        )}
      >
        {statusLine}
      </span>
    </div>
  );
}
