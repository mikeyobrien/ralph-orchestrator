// Mock expo-secure-store for tests
jest.mock("expo-secure-store", () => ({
  getItemAsync: jest.fn(),
  setItemAsync: jest.fn(),
  deleteItemAsync: jest.fn(),
}));

// Mock expo-task-manager for tests
jest.mock("expo-task-manager", () => ({
  defineTask: jest.fn(),
  isTaskRegisteredAsync: jest.fn(),
  unregisterTaskAsync: jest.fn(),
  isAvailableAsync: jest.fn(),
  TaskManagerTaskBody: {},
}));

// Mock expo-background-fetch for tests
jest.mock("expo-background-fetch", () => ({
  registerTaskAsync: jest.fn(),
  unregisterTaskAsync: jest.fn(),
  getStatusAsync: jest.fn(),
  setMinimumIntervalAsync: jest.fn(),
  BackgroundFetchStatus: {
    Available: 3,
    Restricted: 1,
    Denied: 2,
  },
  BackgroundFetchResult: {
    NewData: 2,
    NoData: 1,
    Failed: 3,
  },
}));

// Mock expo-notifications for tests
jest.mock("expo-notifications", () => ({
  requestPermissionsAsync: jest.fn(),
  getPermissionsAsync: jest.fn(),
  scheduleNotificationAsync: jest.fn(),
  cancelScheduledNotificationAsync: jest.fn(),
  cancelAllScheduledNotificationsAsync: jest.fn(),
  setNotificationHandler: jest.fn(),
  addNotificationReceivedListener: jest.fn(),
  addNotificationResponseReceivedListener: jest.fn(),
  setBadgeCountAsync: jest.fn(),
  getBadgeCountAsync: jest.fn(),
}));

// Mock WebSocket for tests
global.WebSocket = class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  readyState = MockWebSocket.CONNECTING;
  onopen = null;
  onclose = null;
  onmessage = null;
  onerror = null;

  constructor(url) {
    this.url = url;
    // Simulate connection opening
    setTimeout(() => {
      this.readyState = MockWebSocket.OPEN;
      if (this.onopen) this.onopen({ type: "open" });
    }, 10);
  }

  send(data) {
    if (this.readyState !== MockWebSocket.OPEN) {
      throw new Error("WebSocket is not open");
    }
  }

  close() {
    this.readyState = MockWebSocket.CLOSED;
    if (this.onclose) this.onclose({ type: "close" });
  }

  // Test helper to simulate incoming messages
  _simulateMessage(data) {
    if (this.onmessage) {
      this.onmessage({ data: JSON.stringify(data) });
    }
  }

  // Test helper to simulate errors
  _simulateError(error) {
    if (this.onerror) {
      this.onerror({ error });
    }
  }
};
