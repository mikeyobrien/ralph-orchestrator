/**
 * Push Notification API Client
 *
 * API functions for push token registration and notification preferences.
 */

import { createNotificationPreferences, type NotificationPreferences } from './pushNotificationHelpers';

// ============================================================================
// Types
// ============================================================================

export interface RegisterTokenResult {
  success: boolean;
  error?: string;
}

export interface UnregisterTokenResult {
  success: boolean;
  error?: string;
}

// ============================================================================
// Configuration
// ============================================================================

const API_BASE_URL = process.env.EXPO_PUBLIC_API_URL || 'http://localhost:8080';

// ============================================================================
// Token Registration
// ============================================================================

/**
 * Registers a push notification token with the server.
 */
export async function registerPushToken(
  token: string,
  authToken: string
): Promise<RegisterTokenResult> {
  try {
    const response = await fetch(`${API_BASE_URL}/api/push/register`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${authToken}`,
      },
      body: JSON.stringify({ token }),
    });

    const data = await response.json();

    if (!response.ok) {
      return {
        success: false,
        error: data.error || `Registration failed with status ${response.status}`,
      };
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

/**
 * Unregisters a push notification token from the server.
 */
export async function unregisterPushToken(
  token: string,
  authToken: string
): Promise<UnregisterTokenResult> {
  try {
    const response = await fetch(`${API_BASE_URL}/api/push/unregister`, {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${authToken}`,
      },
      body: JSON.stringify({ token }),
    });

    const data = await response.json();

    if (!response.ok) {
      return {
        success: false,
        error: data.error || `Unregistration failed with status ${response.status}`,
      };
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

// ============================================================================
// Notification Preferences
// ============================================================================

/**
 * Fetches notification preferences from the server.
 */
export async function getNotificationPreferences(
  authToken: string
): Promise<NotificationPreferences> {
  try {
    const response = await fetch(`${API_BASE_URL}/api/push/preferences`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${authToken}`,
      },
    });

    if (!response.ok) {
      return createNotificationPreferences();
    }

    const data = await response.json();
    return data as NotificationPreferences;
  } catch {
    // Return defaults on error
    return createNotificationPreferences();
  }
}

/**
 * Updates notification preferences on the server.
 */
export async function updateNotificationPreferences(
  preferences: NotificationPreferences,
  authToken: string
): Promise<NotificationPreferences> {
  try {
    const response = await fetch(`${API_BASE_URL}/api/push/preferences`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${authToken}`,
      },
      body: JSON.stringify(preferences),
    });

    if (!response.ok) {
      return createNotificationPreferences();
    }

    const data = await response.json();
    return data as NotificationPreferences;
  } catch {
    return createNotificationPreferences();
  }
}
