/**
 * EventBus
 *
 * Implements a type-safe publish/subscribe event system for the dispatcher workflow.
 * This is the communication backbone of the "Hat" delegation model where different
 * personas (Builder, Validator, Confessor, etc.) coordinate via events.
 *
 * Design:
 * - Type-safe event definitions via generic constraints
 * - Multiple subscribers per event type
 * - Async handler support with Promise.allSettled for fault tolerance
 * - Optional event history for debugging/replay
 * - Wildcard subscription support ('*')
 *
 * Example usage:
 * ```typescript
 * const bus = new EventBus<MyEvents>();
 * bus.subscribe('build.done', (event) => console.log(event.payload));
 * bus.publish('build.done', { tests: 'pass' });
 * ```
 */

/**
 * Base event interface that all events must implement
 */
export interface Event<T = unknown> {
  /** Event type/name (e.g., 'build.done', 'validation.done') */
  type: string;
  /** Event payload data */
  payload: T;
  /** When the event was created */
  timestamp: Date;
  /** Optional correlation ID for tracing related events */
  correlationId?: string;
}

/**
 * Event handler function type
 */
export type EventHandler<T = unknown> = (event: Event<T>) => void | Promise<void>;

/**
 * Options for subscribing to events
 */
export interface SubscriptionOptions {
  /** If true, handler is removed after first invocation */
  once?: boolean;
  /** Optional filter function to conditionally process events */
  filter?: (event: Event) => boolean;
}

/**
 * Subscription handle returned from subscribe()
 */
export interface Subscription {
  /** Unique subscription ID */
  id: string;
  /** Event type this subscription listens to */
  eventType: string;
  /** Unsubscribe function */
  unsubscribe: () => void;
}

/**
 * Options for publishing events
 */
export interface PublishOptions {
  /** Optional correlation ID for event tracing */
  correlationId?: string;
  /** If true, wait for all handlers to complete (default: true) */
  waitForHandlers?: boolean;
}

/**
 * Result of a publish operation
 */
export interface PublishResult {
  /** Event that was published */
  event: Event;
  /** Number of handlers that were invoked */
  handlerCount: number;
  /** Number of handlers that completed successfully */
  successCount: number;
  /** Errors from handlers that failed */
  errors: Error[];
}

/**
 * Internal subscription record
 */
interface SubscriptionRecord {
  id: string;
  handler: EventHandler;
  options: SubscriptionOptions;
}

/**
 * EventBus
 *
 * A type-safe publish/subscribe event bus for inter-component communication.
 * Supports async handlers, once subscriptions, wildcard subscriptions, and event history.
 */
export class EventBus {
  /** Map of event type to list of subscriptions */
  private subscriptions: Map<string, SubscriptionRecord[]> = new Map();

  /** Counter for generating unique subscription IDs */
  private subscriptionCounter: number = 0;

  /** Optional event history for debugging */
  private history: Event[] = [];

  /** Maximum history size (0 = disabled) */
  private maxHistorySize: number;

  /**
   * Create a new EventBus
   *
   * @param options - Configuration options
   * @param options.maxHistorySize - Maximum number of events to keep in history (0 to disable)
   */
  constructor(options: { maxHistorySize?: number } = {}) {
    this.maxHistorySize = options.maxHistorySize ?? 0;
  }

  /**
   * Generate a unique subscription ID
   */
  private generateSubscriptionId(): string {
    return `sub-${Date.now()}-${(++this.subscriptionCounter).toString(16)}`;
  }

  /**
   * Subscribe to an event type.
   *
   * @param eventType - Event type to subscribe to, or '*' for all events
   * @param handler - Function to call when event is published
   * @param options - Subscription options
   * @returns Subscription handle with unsubscribe function
   */
  subscribe<T = unknown>(
    eventType: string,
    handler: EventHandler<T>,
    options: SubscriptionOptions = {}
  ): Subscription {
    const id = this.generateSubscriptionId();

    const record: SubscriptionRecord = {
      id,
      handler: handler as EventHandler,
      options,
    };

    // Get or create the subscription list for this event type
    const subs = this.subscriptions.get(eventType) ?? [];
    subs.push(record);
    this.subscriptions.set(eventType, subs);

    return {
      id,
      eventType,
      unsubscribe: () => this.unsubscribe(eventType, id),
    };
  }

  /**
   * Subscribe to an event type, automatically unsubscribing after first invocation.
   *
   * @param eventType - Event type to subscribe to
   * @param handler - Function to call when event is published
   * @returns Subscription handle
   */
  once<T = unknown>(eventType: string, handler: EventHandler<T>): Subscription {
    return this.subscribe(eventType, handler, { once: true });
  }

  /**
   * Unsubscribe from an event type.
   *
   * @param eventType - Event type
   * @param subscriptionId - Subscription ID to remove
   * @returns true if subscription was found and removed
   */
  unsubscribe(eventType: string, subscriptionId: string): boolean {
    const subs = this.subscriptions.get(eventType);
    if (!subs) {
      return false;
    }

    const index = subs.findIndex((s) => s.id === subscriptionId);
    if (index === -1) {
      return false;
    }

    subs.splice(index, 1);

    // Clean up empty subscription lists
    if (subs.length === 0) {
      this.subscriptions.delete(eventType);
    }

    return true;
  }

  /**
   * Publish an event to all subscribers.
   *
   * @param eventType - Event type to publish
   * @param payload - Event payload data
   * @param options - Publish options
   * @returns PublishResult with handler execution details
   */
  async publish<T = unknown>(
    eventType: string,
    payload: T,
    options: PublishOptions = {}
  ): Promise<PublishResult> {
    const { correlationId, waitForHandlers = true } = options;

    // Create the event
    const event: Event<T> = {
      type: eventType,
      payload,
      timestamp: new Date(),
      correlationId,
    };

    // Add to history if enabled
    if (this.maxHistorySize > 0) {
      this.history.push(event);
      // Trim history if over limit
      if (this.history.length > this.maxHistorySize) {
        this.history = this.history.slice(-this.maxHistorySize);
      }
    }

    // Collect all matching handlers
    const handlers: SubscriptionRecord[] = [];
    const toRemove: { eventType: string; id: string }[] = [];

    // Get specific event type subscribers
    const specificSubs = this.subscriptions.get(eventType) ?? [];
    for (const sub of specificSubs) {
      // Check filter if present
      if (sub.options.filter && !sub.options.filter(event)) {
        continue;
      }
      handlers.push(sub);
      if (sub.options.once) {
        toRemove.push({ eventType, id: sub.id });
      }
    }

    // Get wildcard subscribers
    const wildcardSubs = this.subscriptions.get("*") ?? [];
    for (const sub of wildcardSubs) {
      // Check filter if present
      if (sub.options.filter && !sub.options.filter(event)) {
        continue;
      }
      handlers.push(sub);
      if (sub.options.once) {
        toRemove.push({ eventType: "*", id: sub.id });
      }
    }

    // Execute handlers
    const errors: Error[] = [];
    let successCount = 0;

    if (waitForHandlers) {
      // Wait for all handlers, using Promise.allSettled for fault tolerance
      // Wrap in async IIFE to catch sync throws before Promise.resolve
      const results = await Promise.allSettled(
        handlers.map((sub) =>
          (async () => {
            return sub.handler(event);
          })()
        )
      );

      for (const result of results) {
        if (result.status === "fulfilled") {
          successCount++;
        } else {
          errors.push(
            result.reason instanceof Error ? result.reason : new Error(String(result.reason))
          );
        }
      }
    } else {
      // Fire and forget
      for (const sub of handlers) {
        try {
          const result = sub.handler(event);
          // If it's a promise, we don't await but catch errors
          if (result instanceof Promise) {
            result.catch(() => {
              // Intentionally swallowed in fire-and-forget mode
            });
          }
          successCount++;
        } catch (e) {
          errors.push(e instanceof Error ? e : new Error(String(e)));
        }
      }
    }

    // Remove 'once' subscriptions after execution
    for (const { eventType: et, id } of toRemove) {
      this.unsubscribe(et, id);
    }

    return {
      event,
      handlerCount: handlers.length,
      successCount,
      errors,
    };
  }

  /**
   * Synchronous publish for simple use cases.
   * Handlers are still called, but we don't wait for async handlers to complete.
   *
   * @param eventType - Event type to publish
   * @param payload - Event payload data
   * @param options - Publish options (waitForHandlers is ignored)
   * @returns PublishResult (successCount may be inaccurate for async handlers)
   */
  publishSync<T = unknown>(
    eventType: string,
    payload: T,
    options: PublishOptions = {}
  ): PublishResult {
    // Call the async version but don't await (fire-and-forget)
    // This is a trade-off: we can't accurately report async handler results
    void this.publish(eventType, payload, {
      ...options,
      waitForHandlers: false,
    });

    // Return a placeholder result - the actual execution happens asynchronously
    // For sync use cases, caller doesn't need accurate counts
    return {
      event: {
        type: eventType,
        payload,
        timestamp: new Date(),
        correlationId: options.correlationId,
      },
      handlerCount: this.getSubscriberCount(eventType),
      successCount: 0, // Unknown until async handlers complete
      errors: [],
    };
  }

  /**
   * Get the number of subscribers for an event type.
   *
   * @param eventType - Event type (use '*' to get wildcard subscriber count)
   * @returns Number of subscribers
   */
  getSubscriberCount(eventType: string): number {
    const specific = this.subscriptions.get(eventType)?.length ?? 0;
    const wildcard = eventType !== "*" ? (this.subscriptions.get("*")?.length ?? 0) : 0;
    return specific + wildcard;
  }

  /**
   * Get all subscribed event types.
   *
   * @returns Array of event type strings
   */
  getEventTypes(): string[] {
    return Array.from(this.subscriptions.keys());
  }

  /**
   * Check if there are any subscribers for an event type.
   *
   * @param eventType - Event type to check
   * @returns true if there are subscribers
   */
  hasSubscribers(eventType: string): boolean {
    return this.getSubscriberCount(eventType) > 0;
  }

  /**
   * Get event history (if enabled).
   *
   * @param limit - Maximum number of events to return (default: all)
   * @returns Array of historical events, newest last
   */
  getHistory(limit?: number): Event[] {
    if (limit === undefined) {
      return [...this.history];
    }
    return this.history.slice(-limit);
  }

  /**
   * Get events from history by type.
   *
   * @param eventType - Event type to filter by
   * @param limit - Maximum number of events to return
   * @returns Filtered events
   */
  getHistoryByType(eventType: string, limit?: number): Event[] {
    const filtered = this.history.filter((e) => e.type === eventType);
    if (limit === undefined) {
      return filtered;
    }
    return filtered.slice(-limit);
  }

  /**
   * Clear event history.
   */
  clearHistory(): void {
    this.history = [];
  }

  /**
   * Remove all subscriptions.
   * Useful for cleanup/testing.
   */
  clear(): void {
    this.subscriptions.clear();
    this.subscriptionCounter = 0;
  }

  /**
   * Wait for an event to be published.
   * Returns a Promise that resolves when the specified event type is published.
   *
   * @param eventType - Event type to wait for
   * @param timeout - Optional timeout in milliseconds
   * @returns Promise that resolves with the event
   */
  waitFor<T = unknown>(eventType: string, timeout?: number): Promise<Event<T>> {
    return new Promise((resolve, reject) => {
      let timeoutId: ReturnType<typeof setTimeout> | undefined;

      const subscription = this.once<T>(eventType, (event) => {
        if (timeoutId) {
          clearTimeout(timeoutId);
        }
        resolve(event);
      });

      if (timeout !== undefined) {
        timeoutId = setTimeout(() => {
          subscription.unsubscribe();
          reject(new Error(`Timeout waiting for event: ${eventType}`));
        }, timeout);
      }
    });
  }
}
