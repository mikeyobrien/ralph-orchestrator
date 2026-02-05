/**
 * TaskList Component
 *
 * Reusable component for displaying tasks with status, priority, and timestamps.
 * Supports polling/refetch capability for real-time updates.
 */

import { trpc } from "../trpc";

/** Status color mapping for visual distinction */
const STATUS_COLORS: Record<string, string> = {
  open: "#2563eb",
  closed: "#16a34a",
  blocked: "#dc2626",
};

/** Priority labels for display */
const PRIORITY_LABELS: Record<number, string> = {
  1: "Critical",
  2: "High",
  3: "Medium",
  4: "Low",
  5: "Backlog",
};

interface TaskListProps {
  /** Filter tasks by status (optional) */
  statusFilter?: string;
  /** Polling interval in milliseconds. Set to 0 to disable. Default: 5000 */
  pollingInterval?: number;
  /** Show only ready (unblocked) tasks */
  showReadyOnly?: boolean;
  /** Callback when a task is selected */
  onTaskSelect?: (taskId: string) => void;
  /** Currently selected task ID for highlighting */
  selectedTaskId?: string | null;
}

/**
 * Format a timestamp for display
 */
function formatTimestamp(date: Date | number | string): string {
  const d =
    typeof date === "string" ? new Date(date) : typeof date === "number" ? new Date(date) : date;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Format relative time (e.g., "2 minutes ago")
 */
function formatRelativeTime(date: Date | number | string): string {
  const d =
    typeof date === "string" ? new Date(date) : typeof date === "number" ? new Date(date) : date;
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return "just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  return `${diffDays}d ago`;
}

export function TaskList({
  statusFilter,
  pollingInterval = 5000,
  showReadyOnly = false,
  onTaskSelect,
  selectedTaskId,
}: TaskListProps) {
  // Choose query based on showReadyOnly flag
  const tasksQuery = showReadyOnly
    ? trpc.task.ready.useQuery(undefined, {
        refetchInterval: pollingInterval > 0 ? pollingInterval : false,
      })
    : trpc.task.list.useQuery(statusFilter ? { status: statusFilter } : undefined, {
        refetchInterval: pollingInterval > 0 ? pollingInterval : false,
      });

  if (tasksQuery.isLoading) {
    return <div style={{ padding: "1rem", color: "#666" }}>Loading tasks...</div>;
  }

  if (tasksQuery.isError) {
    return (
      <div style={{ padding: "1rem", color: "#dc2626" }}>
        Error loading tasks: {tasksQuery.error.message}
        <button
          onClick={() => tasksQuery.refetch()}
          style={{
            marginLeft: "0.5rem",
            padding: "0.25rem 0.5rem",
            cursor: "pointer",
          }}
        >
          Retry
        </button>
      </div>
    );
  }

  const tasks = tasksQuery.data;

  if (!tasks || tasks.length === 0) {
    return (
      <div style={{ padding: "1rem", color: "#666" }}>
        No tasks found.
        {statusFilter && ` (filtered by status: ${statusFilter})`}
      </div>
    );
  }

  return (
    <div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "0.5rem",
        }}
      >
        <span style={{ color: "#666", fontSize: "0.875rem" }}>
          {tasks.length} task{tasks.length !== 1 ? "s" : ""}
          {pollingInterval > 0 && ` • auto-refresh ${pollingInterval / 1000}s`}
        </span>
        <button
          onClick={() => tasksQuery.refetch()}
          disabled={tasksQuery.isFetching}
          style={{
            padding: "0.25rem 0.5rem",
            fontSize: "0.75rem",
            cursor: tasksQuery.isFetching ? "wait" : "pointer",
            opacity: tasksQuery.isFetching ? 0.6 : 1,
          }}
        >
          {tasksQuery.isFetching ? "Refreshing..." : "Refresh"}
        </button>
      </div>

      <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
        {tasks.map((task) => {
          const isSelected = selectedTaskId === task.id;
          return (
            <li
              key={task.id}
              onClick={() => onTaskSelect?.(task.id)}
              style={{
                padding: "0.75rem",
                marginBottom: "0.5rem",
                border: isSelected ? "2px solid #2563eb" : "1px solid #e5e7eb",
                borderRadius: "0.375rem",
                backgroundColor: isSelected ? "#eff6ff" : "#fff",
                cursor: onTaskSelect ? "pointer" : "default",
                transition: "border-color 0.15s, background-color 0.15s",
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "flex-start",
                }}
              >
                <div>
                  <strong style={{ fontSize: "1rem" }}>{task.title}</strong>
                  <div
                    style={{
                      display: "flex",
                      gap: "0.5rem",
                      marginTop: "0.25rem",
                      fontSize: "0.75rem",
                    }}
                  >
                    <span
                      style={{
                        padding: "0.125rem 0.375rem",
                        borderRadius: "0.25rem",
                        backgroundColor: STATUS_COLORS[task.status] || "#6b7280",
                        color: "#fff",
                        textTransform: "uppercase",
                      }}
                    >
                      {task.status}
                    </span>
                    <span
                      style={{
                        padding: "0.125rem 0.375rem",
                        borderRadius: "0.25rem",
                        backgroundColor: "#f3f4f6",
                        color: "#374151",
                      }}
                    >
                      P{task.priority} {PRIORITY_LABELS[task.priority] || ""}
                    </span>
                    {task.blockedBy && (
                      <span
                        style={{
                          padding: "0.125rem 0.375rem",
                          borderRadius: "0.25rem",
                          backgroundColor: "#fef2f2",
                          color: "#991b1b",
                        }}
                      >
                        blocked by {task.blockedBy}
                      </span>
                    )}
                  </div>
                </div>
                <div
                  style={{
                    textAlign: "right",
                    fontSize: "0.75rem",
                    color: "#6b7280",
                  }}
                >
                  <div title={formatTimestamp(task.createdAt)}>
                    Created {formatRelativeTime(task.createdAt)}
                  </div>
                  <div title={formatTimestamp(task.updatedAt)}>
                    Updated {formatRelativeTime(task.updatedAt)}
                  </div>
                </div>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
