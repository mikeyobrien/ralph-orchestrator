import {
  NotificationService,
  NotificationConfig,
  SessionNotification,
} from "../../lib/notifications";

describe("NotificationService", () => {
  let service: NotificationService;

  beforeEach(() => {
    jest.clearAllMocks();
    service = new NotificationService();
  });

  describe("initialization", () => {
    test("requests notification permissions on initialize", async () => {
      const mockRequestPermissions = require("expo-notifications")
        .requestPermissionsAsync as jest.Mock;
      mockRequestPermissions.mockResolvedValueOnce({ status: "granted" });

      const result = await service.initialize();

      expect(mockRequestPermissions).toHaveBeenCalled();
      expect(result).toBe(true);
    });

    test("returns false when permissions are denied", async () => {
      const mockRequestPermissions = require("expo-notifications")
        .requestPermissionsAsync as jest.Mock;
      mockRequestPermissions.mockResolvedValueOnce({ status: "denied" });

      const result = await service.initialize();

      expect(result).toBe(false);
    });

    test("returns true when permissions already granted", async () => {
      const mockRequestPermissions = require("expo-notifications")
        .requestPermissionsAsync as jest.Mock;
      mockRequestPermissions.mockResolvedValueOnce({ status: "granted" });

      const result = await service.initialize();

      expect(result).toBe(true);
    });
  });

  describe("permission status", () => {
    test("checks current permission status", async () => {
      const mockGetPermissions = require("expo-notifications")
        .getPermissionsAsync as jest.Mock;
      mockGetPermissions.mockResolvedValueOnce({ status: "granted" });

      const status = await service.getPermissionStatus();

      expect(mockGetPermissions).toHaveBeenCalled();
      expect(status).toBe("granted");
    });

    test("returns denied when not granted", async () => {
      const mockGetPermissions = require("expo-notifications")
        .getPermissionsAsync as jest.Mock;
      mockGetPermissions.mockResolvedValueOnce({ status: "denied" });

      const status = await service.getPermissionStatus();

      expect(status).toBe("denied");
    });
  });

  describe("scheduling notifications", () => {
    test("schedules session status notification", async () => {
      const mockSchedule = require("expo-notifications")
        .scheduleNotificationAsync as jest.Mock;
      mockSchedule.mockResolvedValueOnce("notification-id-123");

      const notification: SessionNotification = {
        sessionId: "session-123",
        title: "Session Complete",
        body: "Orchestration completed successfully",
        data: { sessionId: "session-123", status: "complete" },
      };

      const id = await service.scheduleNotification(notification);

      expect(mockSchedule).toHaveBeenCalledWith({
        content: {
          title: "Session Complete",
          body: "Orchestration completed successfully",
          data: { sessionId: "session-123", status: "complete" },
        },
        trigger: null,
      });
      expect(id).toBe("notification-id-123");
    });

    test("schedules notification with delay", async () => {
      const mockSchedule = require("expo-notifications")
        .scheduleNotificationAsync as jest.Mock;
      mockSchedule.mockResolvedValueOnce("notification-id-456");

      const notification: SessionNotification = {
        sessionId: "session-456",
        title: "Iteration Update",
        body: "Iteration 5 completed",
        data: { sessionId: "session-456", iteration: 5 },
      };

      await service.scheduleNotification(notification, { seconds: 5 });

      expect(mockSchedule).toHaveBeenCalledWith({
        content: {
          title: "Iteration Update",
          body: "Iteration 5 completed",
          data: { sessionId: "session-456", iteration: 5 },
        },
        trigger: { seconds: 5 },
      });
    });
  });

  describe("canceling notifications", () => {
    test("cancels a specific notification by id", async () => {
      const mockCancel = require("expo-notifications")
        .cancelScheduledNotificationAsync as jest.Mock;
      mockCancel.mockResolvedValueOnce(undefined);

      await service.cancelNotification("notification-id-123");

      expect(mockCancel).toHaveBeenCalledWith("notification-id-123");
    });

    test("cancels all notifications", async () => {
      const mockCancelAll = require("expo-notifications")
        .cancelAllScheduledNotificationsAsync as jest.Mock;
      mockCancelAll.mockResolvedValueOnce(undefined);

      await service.cancelAllNotifications();

      expect(mockCancelAll).toHaveBeenCalled();
    });
  });

  describe("notification handlers", () => {
    test("sets notification handler for foreground display", () => {
      const mockSetHandler = require("expo-notifications")
        .setNotificationHandler as jest.Mock;

      service.configureForegroundHandler();

      expect(mockSetHandler).toHaveBeenCalledWith({
        handleNotification: expect.any(Function),
      });
    });

    test("adds notification received listener", () => {
      const mockAddListener = require("expo-notifications")
        .addNotificationReceivedListener as jest.Mock;
      const mockSubscription = { remove: jest.fn() };
      mockAddListener.mockReturnValueOnce(mockSubscription);

      const callback = jest.fn();
      const subscription = service.onNotificationReceived(callback);

      expect(mockAddListener).toHaveBeenCalledWith(callback);
      expect(subscription).toBe(mockSubscription);
    });

    test("adds notification response listener for taps", () => {
      const mockAddListener = require("expo-notifications")
        .addNotificationResponseReceivedListener as jest.Mock;
      const mockSubscription = { remove: jest.fn() };
      mockAddListener.mockReturnValueOnce(mockSubscription);

      const callback = jest.fn();
      const subscription = service.onNotificationTapped(callback);

      expect(mockAddListener).toHaveBeenCalledWith(callback);
      expect(subscription).toBe(mockSubscription);
    });
  });

  describe("badge management", () => {
    test("sets badge count", async () => {
      const mockSetBadge = require("expo-notifications")
        .setBadgeCountAsync as jest.Mock;
      mockSetBadge.mockResolvedValueOnce(true);

      const result = await service.setBadgeCount(5);

      expect(mockSetBadge).toHaveBeenCalledWith(5);
      expect(result).toBe(true);
    });

    test("gets current badge count", async () => {
      const mockGetBadge = require("expo-notifications")
        .getBadgeCountAsync as jest.Mock;
      mockGetBadge.mockResolvedValueOnce(3);

      const count = await service.getBadgeCount();

      expect(mockGetBadge).toHaveBeenCalled();
      expect(count).toBe(3);
    });

    test("clears badge count", async () => {
      const mockSetBadge = require("expo-notifications")
        .setBadgeCountAsync as jest.Mock;
      mockSetBadge.mockResolvedValueOnce(true);

      await service.clearBadge();

      expect(mockSetBadge).toHaveBeenCalledWith(0);
    });
  });
});
