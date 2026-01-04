/**
 * Plan 06-04: Push Notifications Tests (TDD)
 *
 * Tests for push notification registration, permissions, and handling.
 * Covers: token management, permission requests, notification types, preferences.
 */

import {
  validatePushToken,
  formatNotificationMessage,
  getNotificationType,
  shouldShowNotification,
  parseNotificationPayload,
  createNotificationPreferences,
  updateNotificationPreference,
  isNotificationEnabled,
  getEnabledNotificationTypes,
  formatNotificationTime,
} from '../lib/pushNotificationHelpers';

import {
  registerPushToken,
  unregisterPushToken,
  getNotificationPreferences,
  updateNotificationPreferences,
} from '../lib/pushNotificationApi';

// ============================================================================
// Push Token Validation Tests
// ============================================================================

describe('validatePushToken', () => {
  it('returns valid for Expo push token format', () => {
    const result = validatePushToken('ExponentPushToken[xxxxxxxxxxxxxxxxxxxxxx]');
    expect(result.valid).toBe(true);
    expect(result.error).toBeUndefined();
  });

  it('returns valid for APNs token format (64 hex chars)', () => {
    const apnsToken = 'a'.repeat(64);
    const result = validatePushToken(apnsToken);
    expect(result.valid).toBe(true);
  });

  it('returns invalid for empty token', () => {
    const result = validatePushToken('');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('Token cannot be empty');
  });

  it('returns invalid for null/undefined', () => {
    expect(validatePushToken(null as any).valid).toBe(false);
    expect(validatePushToken(undefined as any).valid).toBe(false);
  });

  it('returns invalid for token exceeding max length', () => {
    const longToken = 'a'.repeat(500);
    const result = validatePushToken(longToken);
    expect(result.valid).toBe(false);
    expect(result.error).toBe('Token exceeds maximum length');
  });

  it('returns invalid for token with invalid characters', () => {
    const result = validatePushToken('token with spaces!@#');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('Token contains invalid characters');
  });
});

// ============================================================================
// Notification Message Formatting Tests
// ============================================================================

describe('formatNotificationMessage', () => {
  it('formats orchestration_complete message', () => {
    const result = formatNotificationMessage('orchestration_complete', {
      orchestratorId: 'abc123',
      iterations: 50,
      duration: 3600,
    });
    expect(result.title).toBe('Orchestration Complete');
    expect(result.body).toContain('abc123');
    expect(result.body).toContain('50 iterations');
  });

  it('formats orchestration_error message', () => {
    const result = formatNotificationMessage('orchestration_error', {
      orchestratorId: 'abc123',
      error: 'Max iterations exceeded',
    });
    expect(result.title).toBe('Orchestration Error');
    expect(result.body).toContain('Max iterations exceeded');
  });

  it('formats validation_required message', () => {
    const result = formatNotificationMessage('validation_required', {
      orchestratorId: 'abc123',
      prompt: 'Review the changes?',
    });
    expect(result.title).toBe('Validation Required');
    expect(result.body).toContain('Review the changes?');
  });

  it('formats orchestration_paused message', () => {
    const result = formatNotificationMessage('orchestration_paused', {
      orchestratorId: 'abc123',
      reason: 'User requested',
    });
    expect(result.title).toBe('Orchestration Paused');
    expect(result.body).toContain('User requested');
  });

  it('returns default message for unknown type', () => {
    const result = formatNotificationMessage('unknown_type' as any, {});
    expect(result.title).toBe('Ralph Orchestrator');
    expect(result.body).toBe('You have a new notification');
  });
});

// ============================================================================
// Notification Type Detection Tests
// ============================================================================

describe('getNotificationType', () => {
  it('returns correct type for completion payload', () => {
    const payload = { type: 'orchestration_complete', data: {} };
    expect(getNotificationType(payload)).toBe('orchestration_complete');
  });

  it('returns correct type for error payload', () => {
    const payload = { type: 'orchestration_error', data: {} };
    expect(getNotificationType(payload)).toBe('orchestration_error');
  });

  it('returns correct type for validation payload', () => {
    const payload = { type: 'validation_required', data: {} };
    expect(getNotificationType(payload)).toBe('validation_required');
  });

  it('returns "unknown" for missing type', () => {
    const payload = { data: {} };
    expect(getNotificationType(payload)).toBe('unknown');
  });

  it('returns "unknown" for null payload', () => {
    expect(getNotificationType(null)).toBe('unknown');
  });
});

// ============================================================================
// Notification Display Logic Tests
// ============================================================================

describe('shouldShowNotification', () => {
  const defaultPrefs = createNotificationPreferences();

  it('returns true when notification type is enabled', () => {
    const prefs = { ...defaultPrefs, orchestration_complete: true };
    expect(shouldShowNotification('orchestration_complete', prefs)).toBe(true);
  });

  it('returns false when notification type is disabled', () => {
    const prefs = { ...defaultPrefs, orchestration_complete: false };
    expect(shouldShowNotification('orchestration_complete', prefs)).toBe(false);
  });

  it('returns false when all notifications are muted', () => {
    const prefs = { ...defaultPrefs, muted: true, orchestration_complete: true };
    expect(shouldShowNotification('orchestration_complete', prefs)).toBe(false);
  });

  it('returns true for unknown type when allow_unknown is true', () => {
    const prefs = { ...defaultPrefs, allow_unknown: true };
    expect(shouldShowNotification('unknown', prefs)).toBe(true);
  });

  it('returns false for unknown type by default', () => {
    expect(shouldShowNotification('unknown', defaultPrefs)).toBe(false);
  });
});

// ============================================================================
// Notification Payload Parsing Tests
// ============================================================================

describe('parseNotificationPayload', () => {
  it('parses valid JSON payload', () => {
    const json = JSON.stringify({
      type: 'orchestration_complete',
      data: { orchestratorId: 'abc123' },
    });
    const result = parseNotificationPayload(json);
    expect(result.type).toBe('orchestration_complete');
    expect(result.data.orchestratorId).toBe('abc123');
  });

  it('handles object payload directly', () => {
    const payload = {
      type: 'orchestration_error',
      data: { error: 'Something went wrong' },
    };
    const result = parseNotificationPayload(payload);
    expect(result.type).toBe('orchestration_error');
  });

  it('returns default payload for invalid JSON', () => {
    const result = parseNotificationPayload('invalid json {{{');
    expect(result.type).toBe('unknown');
    expect(result.data).toEqual({});
  });

  it('returns default payload for null', () => {
    const result = parseNotificationPayload(null);
    expect(result.type).toBe('unknown');
  });

  it('extracts nested data from expo notification format', () => {
    const expoPayload = {
      notification: {
        request: {
          content: {
            data: {
              type: 'validation_required',
              orchestratorId: 'xyz789',
            },
          },
        },
      },
    };
    const result = parseNotificationPayload(expoPayload);
    expect(result.type).toBe('validation_required');
  });
});

// ============================================================================
// Notification Preferences Tests
// ============================================================================

describe('createNotificationPreferences', () => {
  it('creates default preferences with all types enabled', () => {
    const prefs = createNotificationPreferences();
    expect(prefs.orchestration_complete).toBe(true);
    expect(prefs.orchestration_error).toBe(true);
    expect(prefs.validation_required).toBe(true);
    expect(prefs.orchestration_paused).toBe(true);
    expect(prefs.muted).toBe(false);
  });

  it('allows overriding defaults', () => {
    const prefs = createNotificationPreferences({
      orchestration_complete: false,
      muted: true,
    });
    expect(prefs.orchestration_complete).toBe(false);
    expect(prefs.muted).toBe(true);
    expect(prefs.orchestration_error).toBe(true); // Still default
  });
});

describe('updateNotificationPreference', () => {
  it('updates a single preference', () => {
    const prefs = createNotificationPreferences();
    const updated = updateNotificationPreference(prefs, 'orchestration_complete', false);
    expect(updated.orchestration_complete).toBe(false);
    expect(updated.orchestration_error).toBe(true); // Unchanged
  });

  it('does not mutate original preferences', () => {
    const prefs = createNotificationPreferences();
    const updated = updateNotificationPreference(prefs, 'muted', true);
    expect(prefs.muted).toBe(false);
    expect(updated.muted).toBe(true);
  });
});

describe('isNotificationEnabled', () => {
  it('returns true for enabled type', () => {
    const prefs = createNotificationPreferences();
    expect(isNotificationEnabled(prefs, 'orchestration_complete')).toBe(true);
  });

  it('returns false for disabled type', () => {
    const prefs = createNotificationPreferences({ orchestration_error: false });
    expect(isNotificationEnabled(prefs, 'orchestration_error')).toBe(false);
  });

  it('returns false when globally muted regardless of type setting', () => {
    const prefs = createNotificationPreferences({ muted: true });
    expect(isNotificationEnabled(prefs, 'orchestration_complete')).toBe(false);
  });
});

describe('getEnabledNotificationTypes', () => {
  it('returns all types when all enabled', () => {
    const prefs = createNotificationPreferences();
    const types = getEnabledNotificationTypes(prefs);
    expect(types).toContain('orchestration_complete');
    expect(types).toContain('orchestration_error');
    expect(types).toContain('validation_required');
    expect(types).toContain('orchestration_paused');
  });

  it('returns empty array when muted', () => {
    const prefs = createNotificationPreferences({ muted: true });
    const types = getEnabledNotificationTypes(prefs);
    expect(types).toEqual([]);
  });

  it('excludes disabled types', () => {
    const prefs = createNotificationPreferences({
      orchestration_complete: false,
      orchestration_error: false,
    });
    const types = getEnabledNotificationTypes(prefs);
    expect(types).not.toContain('orchestration_complete');
    expect(types).not.toContain('orchestration_error');
    expect(types).toContain('validation_required');
  });
});

// ============================================================================
// Time Formatting Tests
// ============================================================================

describe('formatNotificationTime', () => {
  it('formats recent time as "Just now"', () => {
    const now = Date.now();
    expect(formatNotificationTime(now)).toBe('Just now');
  });

  it('formats time a few minutes ago', () => {
    const fiveMinAgo = Date.now() - 5 * 60 * 1000;
    expect(formatNotificationTime(fiveMinAgo)).toBe('5m ago');
  });

  it('formats time hours ago', () => {
    const twoHoursAgo = Date.now() - 2 * 60 * 60 * 1000;
    expect(formatNotificationTime(twoHoursAgo)).toBe('2h ago');
  });

  it('formats time days ago', () => {
    const threeDaysAgo = Date.now() - 3 * 24 * 60 * 60 * 1000;
    expect(formatNotificationTime(threeDaysAgo)).toBe('3d ago');
  });

  it('formats timestamp from ISO string', () => {
    const date = new Date(Date.now() - 60 * 1000).toISOString();
    expect(formatNotificationTime(date)).toBe('1m ago');
  });
});

// ============================================================================
// API Client Tests
// ============================================================================

describe('registerPushToken', () => {
  beforeEach(() => {
    global.fetch = jest.fn();
  });

  afterEach(() => {
    jest.resetAllMocks();
  });

  it('sends token registration request', async () => {
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      json: async () => ({ success: true }),
    });

    const result = await registerPushToken('ExponentPushToken[xxx]', 'test-jwt');

    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining('/api/push/register'),
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({
          Authorization: 'Bearer test-jwt',
        }),
      })
    );
    expect(result.success).toBe(true);
  });

  it('handles registration failure', async () => {
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: false,
      status: 400,
      json: async () => ({ error: 'Invalid token' }),
    });

    const result = await registerPushToken('invalid', 'test-jwt');
    expect(result.success).toBe(false);
    expect(result.error).toBe('Invalid token');
  });

  it('handles network error', async () => {
    (global.fetch as jest.Mock).mockRejectedValue(new Error('Network error'));

    const result = await registerPushToken('token', 'test-jwt');
    expect(result.success).toBe(false);
    expect(result.error).toContain('Network error');
  });
});

describe('unregisterPushToken', () => {
  beforeEach(() => {
    global.fetch = jest.fn();
  });

  afterEach(() => {
    jest.resetAllMocks();
  });

  it('sends unregister request', async () => {
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      json: async () => ({ success: true }),
    });

    const result = await unregisterPushToken('ExponentPushToken[xxx]', 'test-jwt');

    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining('/api/push/unregister'),
      expect.objectContaining({ method: 'DELETE' })
    );
    expect(result.success).toBe(true);
  });
});

describe('getNotificationPreferences', () => {
  beforeEach(() => {
    global.fetch = jest.fn();
  });

  afterEach(() => {
    jest.resetAllMocks();
  });

  it('fetches preferences from API', async () => {
    const mockPrefs = createNotificationPreferences({ muted: true });
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      json: async () => mockPrefs,
    });

    const result = await getNotificationPreferences('test-jwt');

    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining('/api/push/preferences'),
      expect.objectContaining({ method: 'GET' })
    );
    expect(result.muted).toBe(true);
  });

  it('returns default preferences on error', async () => {
    (global.fetch as jest.Mock).mockRejectedValue(new Error('Failed'));

    const result = await getNotificationPreferences('test-jwt');
    expect(result.orchestration_complete).toBe(true);
    expect(result.muted).toBe(false);
  });
});

describe('updateNotificationPreferences', () => {
  beforeEach(() => {
    global.fetch = jest.fn();
  });

  afterEach(() => {
    jest.resetAllMocks();
  });

  it('sends update request with preferences', async () => {
    const prefs = createNotificationPreferences({ muted: true });
    (global.fetch as jest.Mock).mockResolvedValue({
      ok: true,
      json: async () => prefs,
    });

    const result = await updateNotificationPreferences(prefs, 'test-jwt');

    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining('/api/push/preferences'),
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify(prefs),
      })
    );
    expect(result.muted).toBe(true);
  });
});
