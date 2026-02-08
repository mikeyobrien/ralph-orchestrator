/**
 * WebSocket client for real-time session updates from Ralph Orchestrator.
 *
 * Events:
 * - connected: WebSocket connection opened
 * - disconnected: WebSocket connection closed
 * - reconnecting: Attempting to reconnect
 * - error: WebSocket error occurred
 * - session_update: Session status changed
 * - iteration_complete: Iteration finished
 * - log: Log message received
 */

export type WebSocketEvent =
  | "connected"
  | "disconnected"
  | "reconnecting"
  | "error"
  | "session_update"
  | "iteration_complete"
  | "log";

export interface SessionUpdateMessage {
  type: "session_update";
  sessionId: string;
  status: string;
  iteration?: number;
}

export interface IterationCompleteMessage {
  type: "iteration_complete";
  sessionId: string;
  iteration: number;
  tokensUsed: number;
  cost: number;
}

export interface LogMessage {
  type: "log";
  sessionId: string;
  level: "debug" | "info" | "warn" | "error";
  message: string;
  timestamp: string;
}

export type WebSocketMessage =
  | SessionUpdateMessage
  | IterationCompleteMessage
  | LogMessage;

export interface WebSocketClientOptions {
  reconnect?: boolean;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

type EventCallback = (data?: any) => void;

export class WebSocketClient {
  private baseUrl: string;
  private ws: WebSocket | null = null;
  private options: Required<WebSocketClientOptions>;
  private listeners: Map<WebSocketEvent, Set<EventCallback>> = new Map();
  private reconnectAttempts = 0;
  private reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
  private intentionalClose = false;
  private subscribedSessions: Set<string> = new Set();

  constructor(baseUrl: string, options: WebSocketClientOptions = {}) {
    this.baseUrl = baseUrl;
    this.options = {
      reconnect: options.reconnect ?? true,
      reconnectInterval: options.reconnectInterval ?? 5000,
      maxReconnectAttempts: options.maxReconnectAttempts ?? 10,
    };
  }

  /** Get the full WebSocket URL */
  getUrl(): string {
    return `${this.baseUrl}/ws`;
  }

  /** Connect to the WebSocket server */
  connect(): void {
    if (this.ws && this.ws.readyState !== WebSocket.CLOSED) {
      return;
    }

    this.intentionalClose = false;
    this.ws = new WebSocket(this.getUrl());

    this.ws.onopen = () => {
      this.reconnectAttempts = 0;
      this.emit("connected");

      // Resubscribe to any sessions we were tracking
      for (const sessionId of this.subscribedSessions) {
        this.sendMessage({ type: "subscribe", sessionId });
      }
    };

    this.ws.onclose = (event) => {
      this.emit("disconnected");

      if (!this.intentionalClose && this.options.reconnect) {
        this.scheduleReconnect();
      }
    };

    this.ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data) as WebSocketMessage;
        this.handleMessage(data);
      } catch (err) {
        console.error("Failed to parse WebSocket message:", err);
      }
    };

    this.ws.onerror = (event) => {
      const error =
        (event as any).error || new Error("WebSocket connection error");
      this.emit("error", error);
    };
  }

  /** Disconnect from the WebSocket server */
  disconnect(): void {
    this.intentionalClose = true;

    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  /** Check if currently connecting */
  isConnecting(): boolean {
    return this.ws?.readyState === WebSocket.CONNECTING;
  }

  /** Check if connected */
  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /** Subscribe to session updates */
  subscribeToSession(sessionId: string): void {
    this.subscribedSessions.add(sessionId);
    if (this.isConnected()) {
      this.sendMessage({ type: "subscribe", sessionId });
    }
  }

  /** Unsubscribe from session updates */
  unsubscribeFromSession(sessionId: string): void {
    this.subscribedSessions.delete(sessionId);
    if (this.isConnected()) {
      this.sendMessage({ type: "unsubscribe", sessionId });
    }
  }

  /** Add event listener */
  on(event: WebSocketEvent, callback: EventCallback): void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(callback);
  }

  /** Remove event listener */
  off(event: WebSocketEvent, callback: EventCallback): void {
    this.listeners.get(event)?.delete(callback);
  }

  /** Emit event to all listeners */
  private emit(event: WebSocketEvent, data?: any): void {
    this.listeners.get(event)?.forEach((callback) => callback(data));
  }

  /** Handle incoming WebSocket message */
  private handleMessage(data: WebSocketMessage): void {
    switch (data.type) {
      case "session_update":
        this.emit("session_update", data);
        break;
      case "iteration_complete":
        this.emit("iteration_complete", data);
        break;
      case "log":
        this.emit("log", data);
        break;
    }
  }

  /** Send message to WebSocket server */
  private sendMessage(message: object): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }

  /** Schedule a reconnection attempt */
  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.options.maxReconnectAttempts) {
      console.log("Max reconnection attempts reached");
      return;
    }

    this.reconnectAttempts++;
    this.emit("reconnecting");

    this.reconnectTimeout = setTimeout(() => {
      this.connect();
    }, this.options.reconnectInterval);
  }
}
