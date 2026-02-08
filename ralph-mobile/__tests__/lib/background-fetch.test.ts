/**
 * Tests for BackgroundFetchService
 * TDD: Write tests first, watch them fail, then implement
 */
import * as TaskManager from "expo-task-manager";
import * as BackgroundFetch from "expo-background-fetch";
import {
  BackgroundFetchService,
  BACKGROUND_FETCH_TASK,
  BackgroundFetchStatus,
} from "../../lib/background-fetch";

// Mock axios for API calls
jest.mock("axios", () => ({
  create: jest.fn(() => ({
    get: jest.fn(),
    interceptors: {
      request: { use: jest.fn() },
      response: { use: jest.fn() },
    },
  })),
}));

describe("BackgroundFetchService", () => {
  let service: BackgroundFetchService;
  let mockTaskManager: jest.Mocked<typeof TaskManager>;
  let mockBackgroundFetch: jest.Mocked<typeof BackgroundFetch>;

  beforeEach(() => {
    jest.clearAllMocks();
    service = new BackgroundFetchService();
    mockTaskManager = TaskManager as jest.Mocked<typeof TaskManager>;
    mockBackgroundFetch = BackgroundFetch as jest.Mocked<typeof BackgroundFetch>;
  });

  describe("constructor", () => {
    it("should create a service instance with BACKGROUND_FETCH_TASK constant", () => {
      expect(service).toBeInstanceOf(BackgroundFetchService);
      expect(BACKGROUND_FETCH_TASK).toBe("ralph-session-status-fetch");
    });
  });

  describe("isAvailable", () => {
    it("should return true when background fetch is available", async () => {
      mockBackgroundFetch.getStatusAsync.mockResolvedValue(
        BackgroundFetch.BackgroundFetchStatus.Available
      );

      const result = await service.isAvailable();
      expect(result).toBe(true);
      expect(mockBackgroundFetch.getStatusAsync).toHaveBeenCalled();
    });

    it("should return false when background fetch is restricted", async () => {
      mockBackgroundFetch.getStatusAsync.mockResolvedValue(
        BackgroundFetch.BackgroundFetchStatus.Restricted
      );

      const result = await service.isAvailable();
      expect(result).toBe(false);
    });

    it("should return false when background fetch is denied", async () => {
      mockBackgroundFetch.getStatusAsync.mockResolvedValue(
        BackgroundFetch.BackgroundFetchStatus.Denied
      );

      const result = await service.isAvailable();
      expect(result).toBe(false);
    });
  });

  describe("getStatus", () => {
    it("should return 'available' when status is Available", async () => {
      mockBackgroundFetch.getStatusAsync.mockResolvedValue(
        BackgroundFetch.BackgroundFetchStatus.Available
      );

      const status = await service.getStatus();
      expect(status).toBe("available");
    });

    it("should return 'restricted' when status is Restricted", async () => {
      mockBackgroundFetch.getStatusAsync.mockResolvedValue(
        BackgroundFetch.BackgroundFetchStatus.Restricted
      );

      const status = await service.getStatus();
      expect(status).toBe("restricted");
    });

    it("should return 'denied' when status is Denied", async () => {
      mockBackgroundFetch.getStatusAsync.mockResolvedValue(
        BackgroundFetch.BackgroundFetchStatus.Denied
      );

      const status = await service.getStatus();
      expect(status).toBe("denied");
    });
  });

  describe("isRegistered", () => {
    it("should return true when task is registered", async () => {
      mockTaskManager.isTaskRegisteredAsync.mockResolvedValue(true);

      const result = await service.isRegistered();
      expect(result).toBe(true);
      expect(mockTaskManager.isTaskRegisteredAsync).toHaveBeenCalledWith(
        BACKGROUND_FETCH_TASK
      );
    });

    it("should return false when task is not registered", async () => {
      mockTaskManager.isTaskRegisteredAsync.mockResolvedValue(false);

      const result = await service.isRegistered();
      expect(result).toBe(false);
    });
  });

  describe("register", () => {
    it("should register the background fetch task with default interval", async () => {
      mockTaskManager.isTaskRegisteredAsync.mockResolvedValue(false);
      mockBackgroundFetch.registerTaskAsync.mockResolvedValue(undefined);

      await service.register();

      expect(mockBackgroundFetch.registerTaskAsync).toHaveBeenCalledWith(
        BACKGROUND_FETCH_TASK,
        expect.objectContaining({
          minimumInterval: 15 * 60, // 15 minutes default
          stopOnTerminate: false,
          startOnBoot: true,
        })
      );
    });

    it("should register with custom interval", async () => {
      mockTaskManager.isTaskRegisteredAsync.mockResolvedValue(false);
      mockBackgroundFetch.registerTaskAsync.mockResolvedValue(undefined);

      await service.register({ intervalMinutes: 30 });

      expect(mockBackgroundFetch.registerTaskAsync).toHaveBeenCalledWith(
        BACKGROUND_FETCH_TASK,
        expect.objectContaining({
          minimumInterval: 30 * 60,
        })
      );
    });

    it("should not re-register if already registered", async () => {
      mockTaskManager.isTaskRegisteredAsync.mockResolvedValue(true);

      await service.register();

      expect(mockBackgroundFetch.registerTaskAsync).not.toHaveBeenCalled();
    });
  });

  describe("unregister", () => {
    it("should unregister the background fetch task when registered", async () => {
      mockTaskManager.isTaskRegisteredAsync.mockResolvedValue(true);
      mockBackgroundFetch.unregisterTaskAsync.mockResolvedValue(undefined);

      await service.unregister();

      expect(mockBackgroundFetch.unregisterTaskAsync).toHaveBeenCalledWith(
        BACKGROUND_FETCH_TASK
      );
    });

    it("should do nothing when task is not registered", async () => {
      mockTaskManager.isTaskRegisteredAsync.mockResolvedValue(false);

      await service.unregister();

      expect(mockBackgroundFetch.unregisterTaskAsync).not.toHaveBeenCalled();
    });
  });

  describe("defineTask", () => {
    it("should define the task with TaskManager", () => {
      const handler = jest.fn();

      service.defineTask(handler);

      expect(mockTaskManager.defineTask).toHaveBeenCalledWith(
        BACKGROUND_FETCH_TASK,
        expect.any(Function)
      );
    });

    it("should wrap handler to return BackgroundFetchResult", async () => {
      const handler = jest.fn().mockResolvedValue({ hasNewData: true });
      let capturedTaskFn: (body: any) => Promise<any>;

      mockTaskManager.defineTask.mockImplementation((name, fn) => {
        capturedTaskFn = fn;
      });

      service.defineTask(handler);

      // Simulate task execution
      const result = await capturedTaskFn!({ data: {}, error: null });

      expect(handler).toHaveBeenCalled();
      expect(result).toBe(BackgroundFetch.BackgroundFetchResult.NewData);
    });

    it("should return NoData when handler returns no new data", async () => {
      const handler = jest.fn().mockResolvedValue({ hasNewData: false });
      let capturedTaskFn: (body: any) => Promise<any>;

      mockTaskManager.defineTask.mockImplementation((name, fn) => {
        capturedTaskFn = fn;
      });

      service.defineTask(handler);

      const result = await capturedTaskFn!({ data: {}, error: null });

      expect(result).toBe(BackgroundFetch.BackgroundFetchResult.NoData);
    });

    it("should return Failed when handler throws error", async () => {
      const handler = jest.fn().mockRejectedValue(new Error("Network error"));
      let capturedTaskFn: (body: any) => Promise<any>;

      mockTaskManager.defineTask.mockImplementation((name, fn) => {
        capturedTaskFn = fn;
      });

      service.defineTask(handler);

      const result = await capturedTaskFn!({ data: {}, error: null });

      expect(result).toBe(BackgroundFetch.BackgroundFetchResult.Failed);
    });
  });

  describe("setMinimumInterval", () => {
    it("should set minimum interval in seconds", async () => {
      mockBackgroundFetch.setMinimumIntervalAsync.mockResolvedValue(undefined);

      await service.setMinimumInterval(30); // 30 minutes

      expect(mockBackgroundFetch.setMinimumIntervalAsync).toHaveBeenCalledWith(
        30 * 60
      );
    });
  });
});
