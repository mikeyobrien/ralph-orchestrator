/**
 * Notification service for Ralph Mobile app.
 *
 * Handles push notifications for session events:
 * - Session started/completed/failed
 * - Iteration progress updates
 * - Error alerts
 */

import * as Notifications from "expo-notifications";
import { Subscription } from "expo-notifications";

export interface NotificationConfig {
  shouldShowAlert: boolean;
  shouldPlaySound: boolean;
  shouldSetBadge: boolean;
}

export interface SessionNotification {
  sessionId: string;
  title: string;
  body: string;
  data?: Record<string, any>;
}

export type PermissionStatus = "granted" | "denied" | "undetermined";

export class NotificationService {
  private defaultConfig: NotificationConfig = {
    shouldShowAlert: true,
    shouldPlaySound: true,
    shouldSetBadge: true,
  };

  /**
   * Initialize notifications by requesting permissions.
   * @returns true if permissions granted, false otherwise
   */
  async initialize(): Promise<boolean> {
    const { status } = await Notifications.requestPermissionsAsync();
    return status === "granted";
  }

  /**
   * Get current permission status.
   */
  async getPermissionStatus(): Promise<PermissionStatus> {
    const { status } = await Notifications.getPermissionsAsync();
    return status as PermissionStatus;
  }

  /**
   * Schedule a notification to be displayed.
   * @param notification The notification content
   * @param trigger Optional trigger (null = immediate)
   * @returns The notification identifier
   */
  async scheduleNotification(
    notification: SessionNotification,
    trigger?: { seconds: number } | null
  ): Promise<string> {
    const id = await Notifications.scheduleNotificationAsync({
      content: {
        title: notification.title,
        body: notification.body,
        data: notification.data,
      },
      trigger: trigger ?? null,
    });
    return id;
  }

  /**
   * Cancel a specific scheduled notification.
   */
  async cancelNotification(notificationId: string): Promise<void> {
    await Notifications.cancelScheduledNotificationAsync(notificationId);
  }

  /**
   * Cancel all scheduled notifications.
   */
  async cancelAllNotifications(): Promise<void> {
    await Notifications.cancelAllScheduledNotificationsAsync();
  }

  /**
   * Configure how notifications appear when app is in foreground.
   */
  configureForegroundHandler(config?: Partial<NotificationConfig>): void {
    const finalConfig = { ...this.defaultConfig, ...config };

    Notifications.setNotificationHandler({
      handleNotification: async () => ({
        shouldShowAlert: finalConfig.shouldShowAlert,
        shouldPlaySound: finalConfig.shouldPlaySound,
        shouldSetBadge: finalConfig.shouldSetBadge,
      }),
    });
  }

  /**
   * Add listener for when a notification is received while app is in foreground.
   */
  onNotificationReceived(
    callback: (notification: Notifications.Notification) => void
  ): Subscription {
    return Notifications.addNotificationReceivedListener(callback);
  }

  /**
   * Add listener for when user taps on a notification.
   */
  onNotificationTapped(
    callback: (response: Notifications.NotificationResponse) => void
  ): Subscription {
    return Notifications.addNotificationResponseReceivedListener(callback);
  }

  /**
   * Set the app badge count (iOS).
   */
  async setBadgeCount(count: number): Promise<boolean> {
    return await Notifications.setBadgeCountAsync(count);
  }

  /**
   * Get the current badge count (iOS).
   */
  async getBadgeCount(): Promise<number> {
    return await Notifications.getBadgeCountAsync();
  }

  /**
   * Clear the app badge (set to 0).
   */
  async clearBadge(): Promise<boolean> {
    return await Notifications.setBadgeCountAsync(0);
  }
}
