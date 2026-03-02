/** Streams task logs/events via RPC v1 subscription + stream websocket. */

import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import {
  RpcClientError,
  buildStreamWebSocketUrl,
  rpcAck,
  rpcSubscribe,
  rpcUnsubscribe,
  type StreamEventEnvelope,
} from "@/rpc/client";
import { useLogStore } from "@/stores/logStore";

/** Stable empty array to avoid creating new references in selectors */
const EMPTY_ENTRIES: LogEntry[] = [];
const STREAM_TOPICS = ["task.log.line", "task.status.changed", "error.raised", "stream.keepalive"];
const MAX_EVENTS = 200;
const ACK_DEBOUNCE_MS = 250;

type TimeoutHandle = ReturnType<typeof setTimeout>;

/**
 * Log entry from the stream.
 */
export interface LogEntry {
  /** Monotonic stream sequence id (used for dedupe) */
  id?: number;
  /** Stream cursor for replay resume */
  cursor?: string;
  line: string;
  timestamp: string | Date;
  source: "stdout" | "stderr";
}

/**
 * Ralph orchestrator event for lightweight UI status previews.
 */
export interface RalphEvent {
  ts: string;
  iteration?: number;
  hat?: string;
  topic: string;
  triggered?: string;
  payload: string | Record<string, unknown> | null;
}

/**
 * Connection states for the WebSocket
 */
export type ConnectionState = "connecting" | "connected" | "disconnected" | "error";

interface UseTaskWebSocketOptions {
  /** Optional explicit stream WebSocket URL */
  wsUrl?: string;
  /** Whether to automatically connect (default: true) */
  autoConnect?: boolean;
  /** Called when connection state changes */
  onConnectionChange?: (state: ConnectionState) => void;
  /** Called when task status changes */
  onStatusChange?: (status: string) => void;
  /** Called when a new log entry is received */
  onLogEntry?: (entry: LogEntry) => void;
  /** Called when a new stream event is received */
  onEvent?: (event: RalphEvent) => void;
}

interface UseTaskWebSocketReturn {
  entries: LogEntry[];
  latestEntry: LogEntry | null;
  events: RalphEvent[];
  latestEvent: RalphEvent | null;
  connectionState: ConnectionState;
  taskStatus: string;
  error: string | null;
  connect: () => void;
  disconnect: () => void;
  clearEntries: () => void;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function normalizePayload(payload: unknown): string | Record<string, unknown> | null {
  if (payload === null || payload === undefined) {
    return null;
  }
  if (typeof payload === "string") {
    return payload;
  }
  if (typeof payload === "object" && !Array.isArray(payload)) {
    return payload as Record<string, unknown>;
  }
  return String(payload);
}

function toRalphEvent(event: StreamEventEnvelope): RalphEvent {
  const payload = asRecord(event.payload);
  return {
    ts: event.ts,
    iteration: typeof payload?.iteration === "number" ? payload.iteration : undefined,
    hat: typeof payload?.hat === "string" ? payload.hat : undefined,
    topic: event.topic,
    triggered: typeof payload?.triggered === "string" ? payload.triggered : undefined,
    payload: normalizePayload(event.payload),
  };
}

function toLogEntry(event: StreamEventEnvelope): LogEntry {
  const payload = asRecord(event.payload);
  const source = payload?.source === "stderr" ? "stderr" : "stdout";
  const lineCandidate = payload?.line ?? payload?.message ?? payload?.text;
  const line =
    typeof lineCandidate === "string"
      ? lineCandidate
      : payload
        ? JSON.stringify(payload)
        : "";

  return {
    id: Number.isFinite(event.sequence) ? event.sequence : undefined,
    cursor: event.cursor,
    line,
    timestamp: typeof payload?.timestamp === "string" ? payload.timestamp : event.ts,
    source,
  };
}

function rpcErrorMessage(error: unknown): string {
  if (error instanceof RpcClientError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Stream connection failed";
}

/**
 * Hook for streaming a task's logs/events from RPC v1.
 */
export function useTaskWebSocket(
  taskId: string | null,
  options: UseTaskWebSocketOptions = {}
): UseTaskWebSocketReturn {
  const { wsUrl, autoConnect = true, onConnectionChange, onStatusChange, onLogEntry, onEvent } = options;

  const appendLogs = useLogStore((state) => state.appendLogs);
  const clearLogs = useLogStore((state) => state.clearLogs);
  const entries = useLogStore((state) =>
    taskId ? (state.taskLogs[taskId] ?? EMPTY_ENTRIES) : EMPTY_ENTRIES
  );

  const [connectionState, setConnectionState] = useState<ConnectionState>("disconnected");
  const [taskStatus, setTaskStatus] = useState<string>("unknown");
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<RalphEvent[]>([]);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttemptRef = useRef<number>(0);
  const reconnectTimeoutRef = useRef<TimeoutHandle | null>(null);
  const flushTimeoutRef = useRef<TimeoutHandle | null>(null);
  const ackTimeoutRef = useRef<TimeoutHandle | null>(null);
  const connectRef = useRef<() => void>(() => {});
  const logBufferRef = useRef<LogEntry[]>([]);
  const subscriptionIdRef = useRef<string | null>(null);
  const pendingAckCursorRef = useRef<string | null>(null);
  const lastCursorRef = useRef<string | null>(null);
  const isDisconnectingRef = useRef<boolean>(false);

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

  const updateConnectionState = useCallback((state: ConnectionState) => {
    setConnectionState(state);
    onConnectionChangeRef.current?.(state);
  }, []);

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

  const scheduleAck = useCallback((cursor: string) => {
    pendingAckCursorRef.current = cursor;
    if (ackTimeoutRef.current) return;

    ackTimeoutRef.current = setTimeout(() => {
      ackTimeoutRef.current = null;
      const subscriptionId = subscriptionIdRef.current;
      const ackCursor = pendingAckCursorRef.current;
      pendingAckCursorRef.current = null;

      if (!subscriptionId || !ackCursor || isDisconnectingRef.current) {
        return;
      }

      void rpcAck(subscriptionId, ackCursor).catch(() => {
        // Best-effort checkpointing; reconnect flow uses last seen cursor anyway.
      });
    }, ACK_DEBOUNCE_MS);
  }, []);

  const scheduleReconnect = useCallback(() => {
    if (!taskId || isDisconnectingRef.current) {
      return;
    }

    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
    }

    const attempt = reconnectAttemptRef.current;
    const delay = Math.min(1000 * Math.pow(2, attempt), 30000);
    reconnectTimeoutRef.current = setTimeout(() => {
      reconnectAttemptRef.current += 1;
      connectRef.current();
    }, delay);
  }, [taskId]);

  const disconnect = useCallback(() => {
    isDisconnectingRef.current = true;

    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    if (flushTimeoutRef.current) {
      clearTimeout(flushTimeoutRef.current);
      flushTimeoutRef.current = null;
    }

    if (ackTimeoutRef.current) {
      clearTimeout(ackTimeoutRef.current);
      ackTimeoutRef.current = null;
    }

    flushLogBuffer();

    if (wsRef.current) {
      wsRef.current.onclose = null;
      wsRef.current.close();
      wsRef.current = null;
    }

    const subscriptionId = subscriptionIdRef.current;
    subscriptionIdRef.current = null;
    pendingAckCursorRef.current = null;

    if (subscriptionId) {
      void rpcUnsubscribe(subscriptionId).catch(() => {
        // If unsubscribe fails, the server-side retention window will eventually reclaim state.
      });
    }

    updateConnectionState("disconnected");
  }, [flushLogBuffer, updateConnectionState]);

  const connect = useCallback(() => {
    if (!taskId) {
      disconnect();
      return;
    }

    isDisconnectingRef.current = false;

    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    if (ackTimeoutRef.current) {
      clearTimeout(ackTimeoutRef.current);
      ackTimeoutRef.current = null;
    }

    if (wsRef.current) {
      wsRef.current.onclose = null;
      wsRef.current.close();
      wsRef.current = null;
    }

    const priorSubscription = subscriptionIdRef.current;
    subscriptionIdRef.current = null;
    if (priorSubscription) {
      void rpcUnsubscribe(priorSubscription).catch(() => {
        // Best effort cleanup.
      });
    }

    updateConnectionState("connecting");
    setError(null);

    const resumeCursor =
      lastCursorRef.current ?? useLogStore.getState().getLastCursor(taskId) ?? undefined;

    void (async () => {
      try {
        const subscription = await rpcSubscribe({
          topics: STREAM_TOPICS,
          cursor: resumeCursor,
          replayLimit: 400,
          filters: { taskId },
        });

        if (isDisconnectingRef.current) {
          void rpcUnsubscribe(subscription.subscriptionId).catch(() => {});
          return;
        }

        subscriptionIdRef.current = subscription.subscriptionId;
        lastCursorRef.current = subscription.cursor;

        const ws = new WebSocket(buildStreamWebSocketUrl(subscription.subscriptionId, wsUrl));
        wsRef.current = ws;

        ws.onopen = () => {
          updateConnectionState("connected");
          reconnectAttemptRef.current = 0;
          setError(null);
        };

        ws.onmessage = (message) => {
          if (isDisconnectingRef.current) {
            return;
          }

          let event: StreamEventEnvelope;
          try {
            event = JSON.parse(String(message.data)) as StreamEventEnvelope;
          } catch {
            return;
          }

          if (!event || typeof event.topic !== "string" || typeof event.cursor !== "string") {
            return;
          }

          lastCursorRef.current = event.cursor;
          scheduleAck(event.cursor);

          if (event.topic === "task.log.line") {
            if (event.resource?.id !== taskId) {
              return;
            }
            const logEntry = toLogEntry(event);
            logBufferRef.current.push(logEntry);
            scheduleFlush();
            onLogEntryRef.current?.(logEntry);
            return;
          }

          if (event.topic === "task.status.changed" && event.resource?.id === taskId) {
            const payload = asRecord(event.payload);
            const status =
              (typeof payload?.to === "string" && payload.to) ||
              (typeof payload?.status === "string" && payload.status);
            if (status) {
              updateTaskStatus(status);
            }
          }

          if (event.topic === "error.raised") {
            const payload = asRecord(event.payload);
            const messageText =
              typeof payload?.message === "string" ? payload.message : "Stream error";
            const code = typeof payload?.code === "string" ? payload.code : "INTERNAL";
            if (code === "BACKPRESSURE_DROPPED") {
              setError(messageText);
            }
          }

          if (event.topic !== "stream.keepalive") {
            const normalized = toRalphEvent(event);
            setEvents((prev) => {
              const next = [...prev, normalized];
              return next.length > MAX_EVENTS ? next.slice(next.length - MAX_EVENTS) : next;
            });
            onEventRef.current?.(normalized);
          }
        };

        ws.onclose = () => {
          updateConnectionState("disconnected");
          flushLogBuffer();

          const closedSubscription = subscriptionIdRef.current;
          subscriptionIdRef.current = null;
          if (closedSubscription) {
            void rpcUnsubscribe(closedSubscription).catch(() => {});
          }

          if (!isDisconnectingRef.current) {
            scheduleReconnect();
          }
        };

        ws.onerror = () => {
          updateConnectionState("error");
          setError("WebSocket stream connection failed");
        };
      } catch (err) {
        updateConnectionState("error");
        setError(rpcErrorMessage(err));
        scheduleReconnect();
      }
    })();
  }, [taskId, wsUrl, disconnect, flushLogBuffer, scheduleAck, scheduleFlush, scheduleReconnect, updateConnectionState, updateTaskStatus]);

  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  const clearEntries = useCallback(() => {
    if (taskId) {
      clearLogs(taskId);
    }
    setError(null);
  }, [taskId, clearLogs]);

  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    if (autoConnect && taskId) {
      const resumeCursor = useLogStore.getState().getLastCursor(taskId);
      lastCursorRef.current = resumeCursor;
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

  const latestEntry = useMemo(() => {
    return entries.length > 0 ? entries[entries.length - 1] : null;
  }, [entries]);

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
