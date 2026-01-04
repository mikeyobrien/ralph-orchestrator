/**
 * Push Notification Helpers
 *
 * Utilities for push notification token validation, message formatting,
 * payload parsing, and preference management.
 */

// ============================================================================
// Types
// ============================================================================

export type NotificationType =
  | 'orchestration_complete'
  | 'orchestration_error'
  | 'validation_required'
  | 'orchestration_paused'
  | 'unknown';

export interface NotificationMessage {
  title: string;
  body: string;
}

export interface NotificationPayload {
  type: NotificationType;
  data: Record<string, any>;
}

export interface NotificationPreferences {
  orchestration_complete: boolean;
  orchestration_error: boolean;
  validation_required: boolean;
  orchestration_paused: boolean;
  muted: boolean;
  allow_unknown: boolean;
}

export interface TokenValidationResult {
  valid: boolean;
  error?: string;
}

// ============================================================================
// Constants
// ============================================================================

const MAX_TOKEN_LENGTH = 256;
const EXPO_TOKEN_REGEX = /^ExponentPushToken\[.+\]$/;
const APNS_TOKEN_REGEX = /^[a-fA-F0-9]{64}$/;
const VALID_TOKEN_CHARS = /^[a-zA-Z0-9\[\]_\-]+$/;

// ============================================================================
// Token Validation
// ============================================================================

/**
 * Validates a push notification token format.
 */
export function validatePushToken(token: string | null | undefined): TokenValidationResult {
  if (token === null || token === undefined) {
    return { valid: false, error: 'Token cannot be empty' };
  }

  if (typeof token !== 'string' || token.trim() === '') {
    return { valid: false, error: 'Token cannot be empty' };
  }

  if (token.length > MAX_TOKEN_LENGTH) {
    return { valid: false, error: 'Token exceeds maximum length' };
  }

  // Check for Expo or APNs format
  if (EXPO_TOKEN_REGEX.test(token) || APNS_TOKEN_REGEX.test(token)) {
    return { valid: true };
  }

  // Check for invalid characters
  if (!VALID_TOKEN_CHARS.test(token)) {
    return { valid: false, error: 'Token contains invalid characters' };
  }

  return { valid: true };
}

// ============================================================================
// Message Formatting
// ============================================================================

/**
 * Formats a notification message for display based on type and data.
 */
export function formatNotificationMessage(
  type: NotificationType,
  data: Record<string, any>
): NotificationMessage {
  switch (type) {
    case 'orchestration_complete':
      return {
        title: 'Orchestration Complete',
        body: `Orchestrator ${data.orchestratorId} finished with ${data.iterations} iterations`,
      };

    case 'orchestration_error':
      return {
        title: 'Orchestration Error',
        body: `Orchestrator ${data.orchestratorId}: ${data.error}`,
      };

    case 'validation_required':
      return {
        title: 'Validation Required',
        body: data.prompt || 'Your input is needed',
      };

    case 'orchestration_paused':
      return {
        title: 'Orchestration Paused',
        body: `Orchestrator ${data.orchestratorId}: ${data.reason}`,
      };

    default:
      return {
        title: 'Ralph Orchestrator',
        body: 'You have a new notification',
      };
  }
}

// ============================================================================
// Notification Type Detection
// ============================================================================

/**
 * Extracts the notification type from a payload.
 */
export function getNotificationType(payload: any): NotificationType {
  if (!payload) {
    return 'unknown';
  }

  if (typeof payload.type === 'string') {
    const validTypes: NotificationType[] = [
      'orchestration_complete',
      'orchestration_error',
      'validation_required',
      'orchestration_paused',
    ];
    if (validTypes.includes(payload.type)) {
      return payload.type;
    }
  }

  return 'unknown';
}

// ============================================================================
// Display Logic
// ============================================================================

/**
 * Determines if a notification should be displayed based on preferences.
 */
export function shouldShowNotification(
  type: NotificationType,
  preferences: NotificationPreferences
): boolean {
  // Global mute takes precedence
  if (preferences.muted) {
    return false;
  }

  // Check specific type preference
  if (type === 'unknown') {
    return preferences.allow_unknown === true;
  }

  return preferences[type] === true;
}

// ============================================================================
// Payload Parsing
// ============================================================================

/**
 * Parses a notification payload from various formats.
 */
export function parseNotificationPayload(payload: any): NotificationPayload {
  const defaultPayload: NotificationPayload = { type: 'unknown', data: {} };

  if (!payload) {
    return defaultPayload;
  }

  // Handle string payload (JSON)
  if (typeof payload === 'string') {
    try {
      const parsed = JSON.parse(payload);
      return parseNotificationPayload(parsed);
    } catch {
      return defaultPayload;
    }
  }

  // Handle Expo notification format
  if (payload.notification?.request?.content?.data) {
    const expoData = payload.notification.request.content.data;
    return {
      type: getNotificationType(expoData),
      data: expoData,
    };
  }

  // Handle direct object payload
  if (typeof payload === 'object') {
    return {
      type: getNotificationType(payload),
      data: payload.data || payload,
    };
  }

  return defaultPayload;
}

// ============================================================================
// Preferences Management
// ============================================================================

/**
 * Creates default notification preferences with optional overrides.
 */
export function createNotificationPreferences(
  overrides?: Partial<NotificationPreferences>
): NotificationPreferences {
  return {
    orchestration_complete: true,
    orchestration_error: true,
    validation_required: true,
    orchestration_paused: true,
    muted: false,
    allow_unknown: false,
    ...overrides,
  };
}

/**
 * Updates a single preference immutably.
 */
export function updateNotificationPreference(
  preferences: NotificationPreferences,
  key: keyof NotificationPreferences,
  value: boolean
): NotificationPreferences {
  return {
    ...preferences,
    [key]: value,
  };
}

/**
 * Checks if a notification type is enabled (considering mute state).
 */
export function isNotificationEnabled(
  preferences: NotificationPreferences,
  type: NotificationType
): boolean {
  if (preferences.muted) {
    return false;
  }

  if (type === 'unknown') {
    return preferences.allow_unknown;
  }

  return preferences[type] === true;
}

/**
 * Returns array of enabled notification types.
 */
export function getEnabledNotificationTypes(
  preferences: NotificationPreferences
): NotificationType[] {
  if (preferences.muted) {
    return [];
  }

  const types: NotificationType[] = [];
  const checkTypes: NotificationType[] = [
    'orchestration_complete',
    'orchestration_error',
    'validation_required',
    'orchestration_paused',
  ];

  for (const type of checkTypes) {
    if (preferences[type]) {
      types.push(type);
    }
  }

  return types;
}

// ============================================================================
// Time Formatting
// ============================================================================

/**
 * Formats a notification timestamp as relative time.
 */
export function formatNotificationTime(timestamp: number | string): string {
  const time = typeof timestamp === 'string' ? new Date(timestamp).getTime() : timestamp;
  const now = Date.now();
  const diffMs = now - time;

  const seconds = Math.floor(diffMs / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (seconds < 60) {
    return 'Just now';
  }

  if (minutes < 60) {
    return `${minutes}m ago`;
  }

  if (hours < 24) {
    return `${hours}h ago`;
  }

  return `${days}d ago`;
}
