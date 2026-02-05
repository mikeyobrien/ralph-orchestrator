/**
 * LogBroadcaster
 *
 * Manages WebSocket connections and broadcasts log entries from running processes.
 * Supports multiple concurrent viewers and handles subscription/unsubscription.
 *
 * Design Notes:
 * - Single broadcaster instance shared across all WebSocket connections
 * - Clients subscribe to specific task IDs
 * - Broadcasts LogEntry objects serialized as JSON
 * - Handles client disconnection gracefully
 */

import { WebSocket } from "ws";
import { LogEntry } from "../runner/LogStream";
import { TaskLogRepository } from "../repositories/TaskLogRepository";
import { HatsEvent } from "../runner/HatsEventParser";

/**
 * Message sent to WebSocket clients
 */
export interface LogMessage {
  type: "log" | "status" | "error" | "event";
  taskId: string;
  data: LogEntry | { status: string } | { error: string } | HatsEvent;
  timestamp: string;
}

export interface LogBroadcasterOptions {
  /** Optional log repository for persistence and backlog replay */
  logRepository?: TaskLogRepository;
}

interface SubscribeOptions {
  /** Send backlog after this log id (exclusive) */
  sinceId?: number;
}

/**
 * Client subscription info
 */
interface ClientSubscription {
  socket: WebSocket;
  taskIds: Set<string>;
}

/**
 * LogBroadcaster
 *
 * Singleton service that manages log streaming to WebSocket clients.
 */
export class LogBroadcaster {
  /** Map of client ID to subscription info */
  private clients: Map<string, ClientSubscription> = new Map();
  /** Map of task ID to set of subscribed client IDs */
  private taskSubscribers: Map<string, Set<string>> = new Map();
  /** Counter for generating unique client IDs */
  private clientIdCounter: number = 0;
  /** Optional log repository for persistence */
  private logRepository?: TaskLogRepository;

  constructor(options: LogBroadcasterOptions = {}) {
    this.logRepository = options.logRepository;
  }

  /**
   * Update the log repository used for persistence.
   */
  setLogRepository(logRepository?: TaskLogRepository): void {
    this.logRepository = logRepository;
  }

  /**
   * Register a new WebSocket client
   *
   * @returns The client ID for this connection
   */
  addClient(socket: WebSocket): string {
    const clientId = `client-${++this.clientIdCounter}-${Date.now()}`;

    this.clients.set(clientId, {
      socket,
      taskIds: new Set(),
    });

    // Handle client disconnection
    socket.on("close", () => {
      this.removeClient(clientId);
    });

    socket.on("error", () => {
      this.removeClient(clientId);
    });

    return clientId;
  }

  /**
   * Remove a client and all its subscriptions
   */
  removeClient(clientId: string): void {
    const client = this.clients.get(clientId);
    if (!client) return;

    // Remove from all task subscriptions
    for (const taskId of client.taskIds) {
      const subscribers = this.taskSubscribers.get(taskId);
      if (subscribers) {
        subscribers.delete(clientId);
        if (subscribers.size === 0) {
          this.taskSubscribers.delete(taskId);
        }
      }
    }

    this.clients.delete(clientId);
  }

  /**
   * Subscribe a client to a task's log stream
   */
  subscribe(clientId: string, taskId: string, options: SubscribeOptions = {}): boolean {
    const client = this.clients.get(clientId);
    if (!client) return false;

    client.taskIds.add(taskId);

    if (!this.taskSubscribers.has(taskId)) {
      this.taskSubscribers.set(taskId, new Set());
    }
    this.taskSubscribers.get(taskId)!.add(clientId);

    // Send subscription confirmation
    this.sendToClient(clientId, {
      type: "status",
      taskId,
      data: { status: "subscribed" },
      timestamp: new Date().toISOString(),
    });

    // Send backlog logs to the new subscriber
    if (this.logRepository) {
      const backlog = this.logRepository.listByTaskId(taskId, {
        afterId: options.sinceId,
      });

      for (const log of backlog) {
        this.sendToClient(clientId, {
          type: "log",
          taskId,
          data: {
            id: log.id,
            line: log.line,
            timestamp: log.timestamp,
            source: log.source,
          },
          timestamp: new Date().toISOString(),
        });
      }
    }

    return true;
  }

  /**
   * Unsubscribe a client from a task's log stream
   */
  unsubscribe(clientId: string, taskId: string): boolean {
    const client = this.clients.get(clientId);
    if (!client) return false;

    client.taskIds.delete(taskId);

    const subscribers = this.taskSubscribers.get(taskId);
    if (subscribers) {
      subscribers.delete(clientId);
      if (subscribers.size === 0) {
        this.taskSubscribers.delete(taskId);
      }
    }

    // Send unsubscription confirmation
    this.sendToClient(clientId, {
      type: "status",
      taskId,
      data: { status: "unsubscribed" },
      timestamp: new Date().toISOString(),
    });

    return true;
  }

  /**
   * Broadcast a log entry to all clients subscribed to a task
   */
  broadcast(taskId: string, entry: LogEntry): void {
    const persistedEntry = this.persistLogEntry(taskId, entry);
    const subscribers = this.taskSubscribers.get(taskId);
    if (!subscribers || subscribers.size === 0) return;

    const message: LogMessage = {
      type: "log",
      taskId,
      data: persistedEntry,
      timestamp: new Date().toISOString(),
    };

    const json = JSON.stringify(message);

    for (const clientId of subscribers) {
      const client = this.clients.get(clientId);
      if (client && client.socket.readyState === WebSocket.OPEN) {
        client.socket.send(json);
      }
    }
  }

  /**
   * Send a status update to all clients subscribed to a task
   */
  broadcastStatus(taskId: string, status: string): void {
    const subscribers = this.taskSubscribers.get(taskId);
    if (!subscribers || subscribers.size === 0) return;

    const message: LogMessage = {
      type: "status",
      taskId,
      data: { status },
      timestamp: new Date().toISOString(),
    };

    const json = JSON.stringify(message);

    for (const clientId of subscribers) {
      const client = this.clients.get(clientId);
      if (client && client.socket.readyState === WebSocket.OPEN) {
        client.socket.send(json);
      }
    }
  }

  /**
   * Send an error to all clients subscribed to a task
   */
  broadcastError(taskId: string, error: string): void {
    const subscribers = this.taskSubscribers.get(taskId);
    if (!subscribers || subscribers.size === 0) return;

    const message: LogMessage = {
      type: "error",
      taskId,
      data: { error },
      timestamp: new Date().toISOString(),
    };

    const json = JSON.stringify(message);

    for (const clientId of subscribers) {
      const client = this.clients.get(clientId);
      if (client && client.socket.readyState === WebSocket.OPEN) {
        client.socket.send(json);
      }
    }
  }

  /**
   * Broadcast a Hats orchestrator event to all clients subscribed to a task.
   * Events are parsed from stdout lines that match the JSONL event format.
   */
  broadcastEvent(taskId: string, event: HatsEvent): void {
    const subscribers = this.taskSubscribers.get(taskId);
    if (!subscribers || subscribers.size === 0) return;

    const message: LogMessage = {
      type: "event",
      taskId,
      data: event,
      timestamp: new Date().toISOString(),
    };

    const json = JSON.stringify(message);

    for (const clientId of subscribers) {
      const client = this.clients.get(clientId);
      if (client && client.socket.readyState === WebSocket.OPEN) {
        client.socket.send(json);
      }
    }
  }

  /**
   * Send a message to a specific client
   */
  private sendToClient(clientId: string, message: LogMessage): void {
    const client = this.clients.get(clientId);
    if (client && client.socket.readyState === WebSocket.OPEN) {
      client.socket.send(JSON.stringify(message));
    }
  }

  /**
   * Persist a log entry if a repository is configured.
   * Returns the entry with id attached when persisted.
   */
  private persistLogEntry(taskId: string, entry: LogEntry): LogEntry {
    if (!this.logRepository) {
      return entry;
    }

    const id = this.logRepository.append(taskId, entry);
    return {
      ...entry,
      id,
    };
  }

  /**
   * Get the number of subscribers for a task
   */
  getSubscriberCount(taskId: string): number {
    return this.taskSubscribers.get(taskId)?.size ?? 0;
  }

  /**
   * Get the total number of connected clients
   */
  getClientCount(): number {
    return this.clients.size;
  }

  /**
   * Check if a task has any subscribers
   */
  hasSubscribers(taskId: string): boolean {
    const subscribers = this.taskSubscribers.get(taskId);
    return subscribers !== undefined && subscribers.size > 0;
  }

  /**
   * Get all task IDs that have active subscribers
   */
  getActiveTaskIds(): string[] {
    return Array.from(this.taskSubscribers.keys());
  }

  /**
   * Close all connections and clean up
   */
  dispose(): void {
    for (const client of this.clients.values()) {
      client.socket.close();
    }
    this.clients.clear();
    this.taskSubscribers.clear();
  }
}

// Singleton instance
let broadcasterInstance: LogBroadcaster | null = null;

/**
 * Get the singleton LogBroadcaster instance
 */
export function getLogBroadcaster(): LogBroadcaster {
  if (!broadcasterInstance) {
    broadcasterInstance = new LogBroadcaster();
  }
  return broadcasterInstance;
}

/**
 * Configure the singleton broadcaster (e.g., attach a log repository).
 */
export function configureLogBroadcaster(options: LogBroadcasterOptions): LogBroadcaster {
  if (!broadcasterInstance) {
    broadcasterInstance = new LogBroadcaster(options);
  } else if (options.logRepository) {
    broadcasterInstance.setLogRepository(options.logRepository);
  }
  return broadcasterInstance;
}

/**
 * Reset the singleton (for testing)
 */
export function resetLogBroadcaster(): void {
  if (broadcasterInstance) {
    broadcasterInstance.dispose();
    broadcasterInstance = null;
  }
}
