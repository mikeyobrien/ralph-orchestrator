/**
 * LogViewer Component
 *
 * Real-time log viewer that subscribes to WebSocket log streams for a given task ID.
 * Displays log entries with source indicators (stdout/stderr), timestamps, and auto-scroll.
 *
 * Architecture Notes:
 * - Connects to /ws/logs WebSocket endpoint
 * - Sends { type: 'subscribe', taskId } to start receiving logs
 * - Receives LogMessage objects with type: 'log' | 'status' | 'error'
 * - Auto-reconnects on connection loss with exponential backoff
 */

import { useEffect, useRef, useState, useCallback } from "react";

/**
 * Log entry from the server (mirrors LogEntry from LogStream.ts)
 */
interface LogEntry {
  id?: number;
  line: string;
  timestamp: string | Date;
  source: "stdout" | "stderr";
}

/**
 * Message received from WebSocket (mirrors LogMessage from LogBroadcaster.ts)
 */
interface LogMessage {
  type: "log" | "status" | "error";
  taskId: string;
  data: LogEntry | { status: string } | { error: string };
  timestamp: string;
}

/**
 * Connection states for the WebSocket
 */
type ConnectionState = "connecting" | "connected" | "disconnected" | "error";

interface LogViewerProps {
  /** Task ID to subscribe to */
  taskId: string;
  /** Maximum number of log entries to keep in memory (default: 1000) */
  maxEntries?: number;
  /** Whether to auto-scroll to bottom on new entries (default: true) */
  autoScroll?: boolean;
  /** Custom WebSocket URL (default: derives from window.location) */
  wsUrl?: string;
  /** Height of the log viewer (default: '400px') */
  height?: string;
  /** Called when connection state changes */
  onConnectionChange?: (state: ConnectionState) => void;
  /** Called when task status changes */
  onStatusChange?: (status: string) => void;
}

/** Colors for different log sources */
const SOURCE_COLORS = {
  stdout: "#2563eb", // Blue
  stderr: "#dc2626", // Red
};

/** Colors for connection states */
const CONNECTION_COLORS: Record<ConnectionState, string> = {
  connecting: "#f59e0b", // Amber
  connected: "#16a34a", // Green
  disconnected: "#6b7280", // Gray
  error: "#dc2626", // Red
};

/**
 * Format a timestamp for display with milliseconds
 */
function formatTime(timestamp: string | Date): string {
  const d = typeof timestamp === "string" ? new Date(timestamp) : timestamp;
  const timeStr = d.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  // Append milliseconds manually since fractionalSecondDigits isn't in all TS libs
  const ms = d.getMilliseconds().toString().padStart(3, "0");
  return `${timeStr}.${ms}`;
}

/**
 * Determine WebSocket URL from current location
 */
function getDefaultWsUrl(): string {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const host = window.location.host;
  return `${protocol}//${host}/ws/logs`;
}

export function LogViewer({
  taskId,
  maxEntries = 1000,
  autoScroll = true,
  wsUrl,
  height = "400px",
  onConnectionChange,
  onStatusChange,
}: LogViewerProps) {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [connectionState, setConnectionState] = useState<ConnectionState>("disconnected");
  const [taskStatus, setTaskStatus] = useState<string>("unknown");
  const [error, setError] = useState<string | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const reconnectAttemptRef = useRef<number>(0);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const connectRef = useRef<() => void>(() => {});

  // Update connection state and notify callback
  const updateConnectionState = useCallback(
    (state: ConnectionState) => {
      setConnectionState(state);
      onConnectionChange?.(state);
    },
    [onConnectionChange]
  );

  // Update task status and notify callback
  const updateTaskStatus = useCallback(
    (status: string) => {
      setTaskStatus(status);
      onStatusChange?.(status);
    },
    [onStatusChange]
  );

  // Connect to WebSocket
  const connect = useCallback(() => {
    // Clean up existing connection
    if (wsRef.current) {
      wsRef.current.close();
    }

    const url = wsUrl ?? getDefaultWsUrl();
    updateConnectionState("connecting");
    setError(null);

    try {
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        updateConnectionState("connected");
        reconnectAttemptRef.current = 0;

        // Subscribe to the task
        ws.send(JSON.stringify({ type: "subscribe", taskId }));
      };

      ws.onmessage = (event) => {
        try {
          const message: LogMessage = JSON.parse(event.data);

          // Only process messages for our task (or empty taskId for system messages)
          if (message.taskId !== taskId && message.taskId !== "") {
            return;
          }

          switch (message.type) {
            case "log": {
              const logEntry = message.data as LogEntry;
              setEntries((prev) => {
                const newEntries = [...prev, logEntry];
                // Trim to maxEntries
                if (newEntries.length > maxEntries) {
                  return newEntries.slice(-maxEntries);
                }
                return newEntries;
              });
              break;
            }

            case "status": {
              const statusData = message.data as { status: string };
              updateTaskStatus(statusData.status);
              break;
            }

            case "error": {
              const errorData = message.data as { error: string };
              setError(errorData.error);
              break;
            }
          }
        } catch {
          // Invalid JSON - ignore
        }
      };

      ws.onclose = () => {
        updateConnectionState("disconnected");

        // Attempt reconnection with exponential backoff
        const attempt = reconnectAttemptRef.current;
        const delay = Math.min(1000 * Math.pow(2, attempt), 30000); // Max 30s

        reconnectTimeoutRef.current = setTimeout(() => {
          reconnectAttemptRef.current++;
          connectRef.current();
        }, delay);
      };

      ws.onerror = () => {
        updateConnectionState("error");
        setError("WebSocket connection failed");
      };
    } catch (err) {
      updateConnectionState("error");
      setError(err instanceof Error ? err.message : "Failed to connect");
    }
  }, [taskId, wsUrl, maxEntries, updateConnectionState, updateTaskStatus]);

  // Keep connectRef in sync with connect function for recursive calls
  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  // Auto-scroll to bottom when new entries arrive
  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [entries, autoScroll]);

  // Connect on mount and when taskId changes
  // connect() internally calls setState to update connection state - this is intentional
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    connect();

    // Cleanup on unmount
    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, [connect]);

  // Clear entries when taskId changes - legitimate state reset on prop change
  useEffect(() => {
    setEntries([]);
    setTaskStatus("unknown");
    setError(null);
  }, [taskId]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const handleClear = () => {
    setEntries([]);
  };

  const handleReconnect = () => {
    reconnectAttemptRef.current = 0;
    connect();
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height,
        border: "1px solid #e5e7eb",
        borderRadius: "0.375rem",
        overflow: "hidden",
        fontFamily: 'ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, monospace',
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          padding: "0.5rem 0.75rem",
          backgroundColor: "#f9fafb",
          borderBottom: "1px solid #e5e7eb",
          fontSize: "0.75rem",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          {/* Connection indicator */}
          <span
            style={{
              width: "8px",
              height: "8px",
              borderRadius: "50%",
              backgroundColor: CONNECTION_COLORS[connectionState],
            }}
          />
          <span style={{ color: "#6b7280" }}>
            Task: <code style={{ color: "#374151" }}>{taskId}</code>
          </span>
          {taskStatus !== "unknown" && (
            <span
              style={{
                padding: "0.125rem 0.375rem",
                borderRadius: "0.25rem",
                backgroundColor: "#e0e7ff",
                color: "#3730a3",
                textTransform: "uppercase",
              }}
            >
              {taskStatus}
            </span>
          )}
        </div>
        <div style={{ display: "flex", gap: "0.5rem" }}>
          <span style={{ color: "#9ca3af" }}>{entries.length} lines</span>
          <button
            onClick={handleClear}
            style={{
              padding: "0.125rem 0.5rem",
              fontSize: "0.75rem",
              cursor: "pointer",
              border: "1px solid #d1d5db",
              borderRadius: "0.25rem",
              backgroundColor: "#fff",
            }}
          >
            Clear
          </button>
          {connectionState !== "connected" && (
            <button
              onClick={handleReconnect}
              style={{
                padding: "0.125rem 0.5rem",
                fontSize: "0.75rem",
                cursor: "pointer",
                border: "1px solid #d1d5db",
                borderRadius: "0.25rem",
                backgroundColor: "#fff",
              }}
            >
              Reconnect
            </button>
          )}
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div
          style={{
            padding: "0.5rem 0.75rem",
            backgroundColor: "#fef2f2",
            color: "#991b1b",
            fontSize: "0.75rem",
            borderBottom: "1px solid #fecaca",
          }}
        >
          {error}
        </div>
      )}

      {/* Log entries */}
      <div
        ref={containerRef}
        style={{
          flex: 1,
          overflow: "auto",
          backgroundColor: "#1f2937",
          color: "#e5e7eb",
          padding: "0.5rem",
        }}
      >
        {entries.length === 0 ? (
          <div
            style={{
              color: "#6b7280",
              fontStyle: "italic",
              padding: "1rem",
              textAlign: "center",
            }}
          >
            {connectionState === "connected"
              ? "Waiting for logs..."
              : connectionState === "connecting"
                ? "Connecting..."
                : "Disconnected"}
          </div>
        ) : (
          entries.map((entry, index) => (
            <div
              key={index}
              style={{
                display: "flex",
                gap: "0.5rem",
                lineHeight: "1.5",
                fontSize: "0.8125rem",
              }}
            >
              <span style={{ color: "#6b7280", flexShrink: 0 }}>{formatTime(entry.timestamp)}</span>
              <span
                style={{
                  color: SOURCE_COLORS[entry.source],
                  flexShrink: 0,
                  width: "48px",
                }}
              >
                [{entry.source}]
              </span>
              <span
                style={{
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                }}
              >
                {entry.line}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
