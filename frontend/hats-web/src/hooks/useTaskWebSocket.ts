/**
 * useTaskWebSocket Hook
 *
 * Manages WebSocket connection for real-time task log streaming.
 * Extracts reusable WebSocket logic from LogViewer for use across components.
 *
 * Features:
 * - Automatic reconnection with exponential backoff
 * - Subscription/unsubscription lifecycle management
 * - Connection state tracking
 * - Log persistence via Zustand store (survives component unmount)
 */

import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { useLogStore } from "@/stores/logStore";

/** Stable empty array to avoid creating new references in selectors */
const EMPTY_ENTRIES: LogEntry[] = [];

/**
 * Log entry from the server (mirrors LogEntry from LogStream.ts)
 */
export interface LogEntry {
  /** Optional persisted log id */
  id?: number;
  line: string;
  timestamp: string | Date;
  source: "stdout" | "stderr";
}

/**
 * Hats orchestrator event (mirrors HatsEvent from HatsEventParser.ts)
 */
export interface HatsEvent {
  /** ISO timestamp of the event */
  ts: string;
  /** Iteration number (optional) */
  iteration?: number;
  /** Hat that emitted the event (optional) */
  hat?: string;
  /** Event topic (e.g., "build.done", "confession.clean") */
  topic: string;
  /** Event that triggered this one (optional) */
  triggered?: string;
  /** Event payload - can be string, object, or null */
  payload: string | Record<string, unknown> | null;
}

/**
 * Message received from WebSocket (mirrors LogMessage from LogBroadcaster.ts)
 */
interface LogMessage {
  type: "log" | "status" | "error" | "event";
  taskId: string;
  data: LogEntry | { status: string } | { error: string } | HatsEvent;
  timestamp: string;
}

/**
 * Connection states for the WebSocket
 */
export type ConnectionState = "connecting" | "connected" | "disconnected" | "error";

interface UseTaskWebSocketOptions {
  /** Custom WebSocket URL (default: derives from window.location) */
  wsUrl?: string;
  /** Whether to automatically connect (default: true) */
  autoConnect?: boolean;
  /** Called when connection state changes */
  onConnectionChange?: (state: ConnectionState) => void;
  /** Called when task status changes */
  onStatusChange?: (status: string) => void;
  /** Called when a new log entry is received */
  onLogEntry?: (entry: LogEntry) => void;
  /** Called when a Hats orchestrator event is received */
  onEvent?: (event: HatsEvent) => void;
}

interface UseTaskWebSocketReturn {
  /** All log entries received */
  entries: LogEntry[];
  /** Latest log entry (most recent) */
  latestEntry: LogEntry | null;
  /** All Hats orchestrator events received */
  events: HatsEvent[];
  /** Latest Hats event (most recent) */
  latestEvent: HatsEvent | null;
  /** Current connection state */
  connectionState: ConnectionState;
  /** Task status from server */
  taskStatus: string;
  /** Current error message, if any */
  error: string | null;
  /** Manually connect to WebSocket */
  connect: () => void;
  /** Manually disconnect from WebSocket */
  disconnect: () => void;
  /** Clear all log entries */
  clearEntries: () => void;
}

/**
 * Determine WebSocket URL from current location
 */
function getDefaultWsUrl(): string {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const host = window.location.host;
  return `${protocol}//${host}/ws/logs`;
}

/**
 * Hook for managing WebSocket connection to a task's log stream.
 *
 * @param taskId - The task ID to subscribe to
 * @param options - Configuration options
 */
export function useTaskWebSocket(
  taskId: string | null,
  options: UseTaskWebSocketOptions = {}
): UseTaskWebSocketReturn {
  const {
    wsUrl,
    autoConnect = true,
    onConnectionChange,
    onStatusChange,
    onLogEntry,
    onEvent,
  } = options;

  // Use Zustand store for log persistence across mount/unmount cycles
  const appendLogs = useLogStore((state) => state.appendLogs);
  const clearLogs = useLogStore((state) => state.clearLogs);
  // Use direct property access with stable empty array to avoid infinite re-renders
  // IMPORTANT: Never create new arrays in selectors - use stable references
  const entries = useLogStore((state) =>
    taskId ? (state.taskLogs[taskId] ?? EMPTY_ENTRIES) : EMPTY_ENTRIES
  );

  const [connectionState, setConnectionState] = useState<ConnectionState>("disconnected");
  const [taskStatus, setTaskStatus] = useState<string>("unknown");
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<HatsEvent[]>([]);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttemptRef = useRef<number>(0);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const subscribedTaskIdRef = useRef<string | null>(null);
  const logBufferRef = useRef<LogEntry[]>([]);
  const flushTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const connectRef = useRef<() => void>(() => {});
  // Flag to prevent processing messages after disconnect starts
  const isDisconnectingRef = useRef<boolean>(false);

  // Memoize callbacks to prevent unnecessary reconnections
  const onConnectionChangeRef = useRef(onConnectionChange);
  const onStatusChangeRef = useRef(onStatusChange);
  const onLogEntryRef = useRef(onLogEntry);
  const onEventRef = useRef(onEvent);

  useEffect(() => {
    onConnectionChangeRef.current = onConnectionChange;
    onStatusChangeRef.current = onStatusChange;
    onLogEntryRef.current = onLogEntry;
    onEventRef.current = onEvent;
  }, [onConnectionChange, onStatusChange, onLogEntry, onEvent]);

  // Update connection state and notify callback
  const updateConnectionState = useCallback((state: ConnectionState) => {
    setConnectionState(state);
    onConnectionChangeRef.current?.(state);
  }, []);

  // Update task status and notify callback
  const updateTaskStatus = useCallback((status: string) => {
    setTaskStatus(status);
    onStatusChangeRef.current?.(status);
  }, []);

  const flushLogBuffer = useCallback(() => {
    if (!taskId || logBufferRef.current.length === 0) {
      logBufferRef.current = [];
      return;
    }

    const batch = logBufferRef.current;
    logBufferRef.current = [];
    appendLogs(taskId, batch);
  }, [appendLogs, taskId]);

  const scheduleFlush = useCallback(() => {
    if (flushTimeoutRef.current) return;
    flushTimeoutRef.current = setTimeout(() => {
      flushTimeoutRef.current = null;
      flushLogBuffer();
    }, 50);
  }, [flushLogBuffer]);

  // Disconnect from WebSocket
  const disconnect = useCallback(() => {
    // Set flag FIRST to prevent any late-arriving messages from being processed
    isDisconnectingRef.current = true;

    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (flushTimeoutRef.current) {
      clearTimeout(flushTimeoutRef.current);
      flushTimeoutRef.current = null;
    }
    flushLogBuffer();
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    subscribedTaskIdRef.current = null;
    updateConnectionState("disconnected");
  }, [flushLogBuffer, updateConnectionState]);

  // Connect to WebSocket
  const connect = useCallback(() => {
    // Don't connect if no taskId
    if (!taskId) {
      disconnect();
      return;
    }

    // Reset disconnecting flag for new connection
    isDisconnectingRef.current = false;

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
        setError(null); // Clear any previous error on successful connection

        // Subscribe to the task
        const lastLogId = useLogStore.getState().getLastLogId(taskId);
        ws.send(
          JSON.stringify({
            type: "subscribe",
            taskId,
            sinceId: lastLogId ?? undefined,
          })
        );
        subscribedTaskIdRef.current = taskId;
      };

      ws.onmessage = (event) => {
        // Skip processing if we're disconnecting (prevents race condition duplicates)
        if (isDisconnectingRef.current) {
          return;
        }

        try {
          const message: LogMessage = JSON.parse(event.data);

          // Only process messages for our task (or empty taskId for system messages)
          if (message.taskId !== taskId && message.taskId !== "") {
            return;
          }

          switch (message.type) {
            case "log": {
              const logEntry = message.data as LogEntry;
              // Buffer log entries to reduce render churn
              logBufferRef.current.push(logEntry);
              scheduleFlush();
              onLogEntryRef.current?.(logEntry);
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

            case "event": {
              const eventData = message.data as HatsEvent;
              setEvents((prev) => [...prev, eventData]);
              onEventRef.current?.(eventData);
              break;
            }
          }
        } catch {
          // Invalid JSON - ignore
        }
      };

      ws.onclose = () => {
        updateConnectionState("disconnected");
        flushLogBuffer();

        // Only attempt reconnection if we still have a taskId
        if (taskId && subscribedTaskIdRef.current === taskId) {
          const attempt = reconnectAttemptRef.current;
          const delay = Math.min(1000 * Math.pow(2, attempt), 30000); // Max 30s

          reconnectTimeoutRef.current = setTimeout(() => {
            reconnectAttemptRef.current++;
            connectRef.current();
          }, delay);
        }
      };

      ws.onerror = () => {
        updateConnectionState("error");
        setError("WebSocket connection failed");
      };
    } catch (err) {
      updateConnectionState("error");
      setError(err instanceof Error ? err.message : "Failed to connect");
    }
  }, [
    taskId,
    wsUrl,
    scheduleFlush,
    flushLogBuffer,
    updateConnectionState,
    updateTaskStatus,
    disconnect,
  ]);

  // Keep connectRef in sync with connect function for recursive calls
  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  // Clear entries from store
  const clearEntries = useCallback(() => {
    if (taskId) {
      clearLogs(taskId);
    }
    setError(null);
  }, [taskId, clearLogs]);

  // Connect on mount when autoConnect is true, or when taskId changes
  // State resets are legitimate when taskId changes - this is a controlled reset
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    if (autoConnect && taskId) {
      // Note: We intentionally do NOT clear logs when taskId changes.
      // Logs are preserved in the Zustand store to survive component unmounts.
      // Users can explicitly clear via the clearEntries function if needed.
      // Events are cleared since they're not persisted.
      setTaskStatus("unknown");
      setError(null);
      setEvents([]);
      connect();
    } else if (!taskId) {
      disconnect();
    }

    return () => {
      disconnect();
    };
  }, [taskId, autoConnect, connect, disconnect]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Compute latest entry
  const latestEntry = useMemo(() => {
    return entries.length > 0 ? entries[entries.length - 1] : null;
  }, [entries]);

  // Compute latest event
  const latestEvent = useMemo(() => {
    return events.length > 0 ? events[events.length - 1] : null;
  }, [events]);

  return {
    entries,
    latestEntry,
    events,
    latestEvent,
    connectionState,
    taskStatus,
    error,
    connect,
    disconnect,
    clearEntries,
  };
}
