/**
 * Background fetch service for Ralph Mobile app.
 *
 * Polls the API for session status updates when app is backgrounded.
 * Integrates with NotificationService to alert users of important changes.
 */

import * as TaskManager from "expo-task-manager";
import * as BackgroundFetch from "expo-background-fetch";

export const BACKGROUND_FETCH_TASK = "ralph-session-status-fetch";

export type BackgroundFetchStatus = "available" | "restricted" | "denied";

export interface BackgroundFetchOptions {
  /** Minimum interval in minutes (default: 15) */
  intervalMinutes?: number;
  /** Stop task when app is terminated (default: false) */
  stopOnTerminate?: boolean;
  /** Start task on device boot (default: true) */
  startOnBoot?: boolean;
}

export interface TaskResult {
  hasNewData: boolean;
  data?: any;
}

export type TaskHandler = () => Promise<TaskResult>;

export class BackgroundFetchService {
  /**
   * Check if background fetch is available on this device.
   */
  async isAvailable(): Promise<boolean> {
    const status = await BackgroundFetch.getStatusAsync();
    return status === BackgroundFetch.BackgroundFetchStatus.Available;
  }

  /**
   * Get the current background fetch status.
   */
  async getStatus(): Promise<BackgroundFetchStatus> {
    const status = await BackgroundFetch.getStatusAsync();

    switch (status) {
      case BackgroundFetch.BackgroundFetchStatus.Available:
        return "available";
      case BackgroundFetch.BackgroundFetchStatus.Restricted:
        return "restricted";
      case BackgroundFetch.BackgroundFetchStatus.Denied:
        return "denied";
      default:
        return "denied";
    }
  }

  /**
   * Check if the background fetch task is registered.
   */
  async isRegistered(): Promise<boolean> {
    return await TaskManager.isTaskRegisteredAsync(BACKGROUND_FETCH_TASK);
  }

  /**
   * Register the background fetch task.
   */
  async register(options: BackgroundFetchOptions = {}): Promise<void> {
    const isRegistered = await this.isRegistered();
    if (isRegistered) {
      return;
    }

    const {
      intervalMinutes = 15,
      stopOnTerminate = false,
      startOnBoot = true,
    } = options;

    await BackgroundFetch.registerTaskAsync(BACKGROUND_FETCH_TASK, {
      minimumInterval: intervalMinutes * 60,
      stopOnTerminate,
      startOnBoot,
    });
  }

  /**
   * Unregister the background fetch task.
   */
  async unregister(): Promise<void> {
    const isRegistered = await this.isRegistered();
    if (!isRegistered) {
      return;
    }

    await BackgroundFetch.unregisterTaskAsync(BACKGROUND_FETCH_TASK);
  }

  /**
   * Define the task handler for background fetch.
   * This must be called outside of any React component.
   */
  defineTask(handler: TaskHandler): void {
    TaskManager.defineTask(
      BACKGROUND_FETCH_TASK,
      async ({ data, error }: TaskManager.TaskManagerTaskBody<any>) => {
        if (error) {
          console.error("Background fetch error:", error);
          return BackgroundFetch.BackgroundFetchResult.Failed;
        }

        try {
          const result = await handler();

          if (result.hasNewData) {
            return BackgroundFetch.BackgroundFetchResult.NewData;
          }

          return BackgroundFetch.BackgroundFetchResult.NoData;
        } catch (err) {
          console.error("Background fetch handler error:", err);
          return BackgroundFetch.BackgroundFetchResult.Failed;
        }
      }
    );
  }

  /**
   * Set the minimum interval for background fetch (iOS only).
   * @param minutes Minimum interval in minutes
   */
  async setMinimumInterval(minutes: number): Promise<void> {
    await BackgroundFetch.setMinimumIntervalAsync(minutes * 60);
  }
}
