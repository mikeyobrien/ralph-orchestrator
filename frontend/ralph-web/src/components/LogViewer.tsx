/**
 * LogViewer Component
 *
 * Legacy-styled log viewer backed by RPC v1 stream subscriptions.
 */

import { useEffect, useMemo, useRef } from "react";
import {
  useTaskWebSocket,
  type ConnectionState,
  type LogEntry,
} from "@/hooks/useTaskWebSocket";

interface LogViewerProps {
  taskId: string;
  maxEntries?: number;
  autoScroll?: boolean;
  wsUrl?: string;
  height?: string;
  onConnectionChange?: (state: ConnectionState) => void;
  onStatusChange?: (status: string) => void;
}

const SOURCE_COLORS = {
  stdout: "#2563eb",
  stderr: "#dc2626",
};

const CONNECTION_COLORS: Record<ConnectionState, string> = {
  connecting: "#f59e0b",
  connected: "#16a34a",
  disconnected: "#6b7280",
  error: "#dc2626",
};

function formatTime(timestamp: string | Date): string {
  const d = typeof timestamp === "string" ? new Date(timestamp) : timestamp;
  const timeStr = d.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const ms = d.getMilliseconds().toString().padStart(3, "0");
  return `${timeStr}.${ms}`;
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
  const containerRef = useRef<HTMLDivElement>(null);

  const { entries, connectionState, taskStatus, error, clearEntries, connect } = useTaskWebSocket(taskId, {
    wsUrl,
    onConnectionChange,
    onStatusChange,
  });

  const visibleEntries = useMemo<LogEntry[]>(() => {
    if (maxEntries <= 0 || entries.length <= maxEntries) {
      return entries;
    }
    return entries.slice(entries.length - maxEntries);
  }, [entries, maxEntries]);

  useEffect(() => {
    if (!autoScroll || !containerRef.current) {
      return;
    }
    containerRef.current.scrollTop = containerRef.current.scrollHeight;
  }, [visibleEntries, autoScroll]);

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
          <span style={{ color: "#9ca3af" }}>{visibleEntries.length} lines</span>
          <button
            onClick={clearEntries}
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
              onClick={connect}
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
        {visibleEntries.length === 0 ? (
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
          visibleEntries.map((entry, index) => (
            <div
              key={`${entry.id ?? "line"}-${index}`}
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
