/**
 * WebSocket hook for live loop monitoring.
 *
 * Connects to the WebSocket endpoint and provides real-time updates
 * for iteration count, active hat, events, and streaming output.
 */

import { useState, useEffect, useCallback, useRef } from 'react';

/** Server message types from the WebSocket */
export interface OutputMessage {
  type: 'output';
  lines: string[];
}

export interface IterationStartedMessage {
  type: 'iteration_started';
  iteration: number;
  hat: string;
}

export interface EventMessage {
  type: 'event';
  topic: string;
  payload: string;
}

export interface LoopCompletedMessage {
  type: 'loop_completed';
  reason: string;
}

export interface ConnectedMessage {
  type: 'connected';
  connection_id: number;
}

export interface ErrorMessage {
  type: 'error';
  message: string;
}

export type ServerMessage =
  | OutputMessage
  | IterationStartedMessage
  | EventMessage
  | LoopCompletedMessage
  | ConnectedMessage
  | ErrorMessage;

/** Loop status */
export type LoopState = 'idle' | 'running' | 'completed' | 'error';

/** An event received during the loop */
export interface LoopEvent {
  topic: string;
  payload: string;
  timestamp: Date;
}

/** Return type for useLoopWebSocket hook */
export interface UseLoopWebSocketReturn {
  /** Connection status */
  isConnected: boolean;
  /** Current loop state */
  state: LoopState;
  /** Current iteration number */
  iteration: number;
  /** Active hat name */
  activeHat: string | null;
  /** Streaming output lines */
  output: string[];
  /** Events received */
  events: LoopEvent[];
  /** Elapsed time in seconds since first iteration */
  elapsedSeconds: number;
  /** Error message if any */
  error: string | null;
  /** Clear output and events */
  clear: () => void;
  /** Stop the running loop */
  stop: () => Promise<void>;
  /** Whether a stop operation is in progress */
  isStopping: boolean;
}

/**
 * Hook to connect to the WebSocket and receive live loop updates.
 *
 * @param sessionId - Optional session ID to connect to a specific session
 */
export function useLoopWebSocket(sessionId?: string): UseLoopWebSocketReturn {
  const [isConnected, setIsConnected] = useState(false);
  const [state, setState] = useState<LoopState>('idle');
  const [iteration, setIteration] = useState(0);
  const [activeHat, setActiveHat] = useState<string | null>(null);
  const [output, setOutput] = useState<string[]>([]);
  const [events, setEvents] = useState<LoopEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [startTime, setStartTime] = useState<Date | null>(null);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [isStopping, setIsStopping] = useState(false);

  const wsRef = useRef<WebSocket | null>(null);
  const timerRef = useRef<number | null>(null);
  const stateRef = useRef<LoopState>(state);
  stateRef.current = state;

  const clear = useCallback(() => {
    setOutput([]);
    setEvents([]);
    setIteration(0);
    setActiveHat(null);
    setState('idle');
    setStartTime(null);
    setElapsedSeconds(0);
    setError(null);
  }, []);

  const stop = useCallback(async () => {
    setIsStopping(true);
    try {
      // If we have a sessionId, stop that specific loop; otherwise stop any active loop
      const endpoint = sessionId ? `/api/loops/${sessionId}/stop` : '/api/loops/stop';
      const response = await fetch(endpoint, {
        method: 'POST',
      });

      if (!response.ok && response.status !== 204) {
        const data = await response.json();
        throw new Error(data.error || 'Failed to stop loop');
      }

      // The WebSocket will receive the loop_completed message
      // but we can also update state optimistically
      setState('completed');
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to stop loop');
    } finally {
      setIsStopping(false);
    }
  }, [sessionId]);

  // Update elapsed time every second when running
  useEffect(() => {
    if (state === 'running' && startTime) {
      timerRef.current = window.setInterval(() => {
        const now = new Date();
        const diff = Math.floor((now.getTime() - startTime.getTime()) / 1000);
        setElapsedSeconds(diff);
      }, 1000);
    }

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [state, startTime]);

  useEffect(() => {
    // Track if this effect instance is still active (handles React StrictMode double-mount)
    let isActive = true;

    // Determine WebSocket URL
    // In development (Vite), connect directly to the backend since Vite's WS proxy has issues
    // In production, the frontend is served by the backend so we use the same host
    const isDev = import.meta.env.DEV;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = isDev ? 'localhost:3000' : window.location.host;
    // Always connect to the global /ws endpoint - the backend broadcasts all events
    // Future: could add session-specific filtering on the backend if needed
    const wsUrl = `${protocol}//${host}/ws`;

    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      if (!isActive) {
        ws.close();
        return;
      }
      setIsConnected(true);
      setError(null);
    };

    ws.onclose = () => {
      if (!isActive) return;
      setIsConnected(false);
    };

    ws.onerror = () => {
      if (!isActive) return;
      setError('WebSocket connection error');
      setIsConnected(false);
    };

    ws.onmessage = (event) => {
      if (!isActive) return;
      try {
        const msg = JSON.parse(event.data) as ServerMessage;

        switch (msg.type) {
          case 'connected':
            // Connection established
            break;

          case 'output':
            setOutput((prev) => [...prev, ...msg.lines]);
            break;

          case 'iteration_started':
            setIteration(msg.iteration);
            setActiveHat(msg.hat);
            if (stateRef.current !== 'running') {
              setState('running');
              setStartTime(new Date());
            }
            break;

          case 'event':
            setEvents((prev) => [
              ...prev,
              {
                topic: msg.topic,
                payload: msg.payload,
                timestamp: new Date(),
              },
            ]);
            break;

          case 'loop_completed':
            setState('completed');
            if (timerRef.current) {
              clearInterval(timerRef.current);
              timerRef.current = null;
            }
            break;

          case 'error':
            setError(msg.message);
            setState('error');
            break;
        }
      } catch (e) {
        console.error('Failed to parse WebSocket message:', e);
      }
    };

    return () => {
      isActive = false;
      ws.close();
      wsRef.current = null;
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    };
    // Note: We intentionally don't include sessionId in deps because we use the global /ws endpoint
    // The sessionId is only used for the stop() function which has its own dependency
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    isConnected,
    state,
    iteration,
    activeHat,
    output,
    events,
    elapsedSeconds,
    error,
    clear,
    stop,
    isStopping,
  };
}
