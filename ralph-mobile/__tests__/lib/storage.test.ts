/**
 * Tests for storage.ts - secure storage utilities
 * TDD: Write tests first, watch them fail, then verify
 */
import * as SecureStore from "expo-secure-store";
import {
  saveToken,
  getToken,
  removeToken,
  saveUser,
  getUser,
  removeUser,
  clearAuth,
  saveSettings,
  getSettings,
  resetSettings,
  User,
  AppSettings,
} from "../../lib/storage";

describe("storage", () => {
  let mockSecureStore: jest.Mocked<typeof SecureStore>;

  beforeEach(() => {
    jest.clearAllMocks();
    mockSecureStore = SecureStore as jest.Mocked<typeof SecureStore>;
  });

  describe("Token Management", () => {
    describe("saveToken", () => {
      it("should save token to secure storage", async () => {
        mockSecureStore.setItemAsync.mockResolvedValue(undefined);

        await saveToken("test-jwt-token-123");

        expect(mockSecureStore.setItemAsync).toHaveBeenCalledWith(
          "token",
          "test-jwt-token-123"
        );
      });
    });

    describe("getToken", () => {
      it("should retrieve token from secure storage", async () => {
        mockSecureStore.getItemAsync.mockResolvedValue("stored-token");

        const token = await getToken();

        expect(mockSecureStore.getItemAsync).toHaveBeenCalledWith("token");
        expect(token).toBe("stored-token");
      });

      it("should return null when no token exists", async () => {
        mockSecureStore.getItemAsync.mockResolvedValue(null);

        const token = await getToken();

        expect(token).toBeNull();
      });
    });

    describe("removeToken", () => {
      it("should delete token from secure storage", async () => {
        mockSecureStore.deleteItemAsync.mockResolvedValue(undefined);

        await removeToken();

        expect(mockSecureStore.deleteItemAsync).toHaveBeenCalledWith("token");
      });
    });
  });

  describe("User Management", () => {
    const testUser: User = {
      id: "user-123",
      name: "Test User",
      email: "test@example.com",
    };

    describe("saveUser", () => {
      it("should save user as JSON to secure storage", async () => {
        mockSecureStore.setItemAsync.mockResolvedValue(undefined);

        await saveUser(testUser);

        expect(mockSecureStore.setItemAsync).toHaveBeenCalledWith(
          "user",
          JSON.stringify(testUser)
        );
      });
    });

    describe("getUser", () => {
      it("should retrieve and parse user from secure storage", async () => {
        mockSecureStore.getItemAsync.mockResolvedValue(
          JSON.stringify(testUser)
        );

        const user = await getUser();

        expect(mockSecureStore.getItemAsync).toHaveBeenCalledWith("user");
        expect(user).toEqual(testUser);
      });

      it("should return null when no user exists", async () => {
        mockSecureStore.getItemAsync.mockResolvedValue(null);

        const user = await getUser();

        expect(user).toBeNull();
      });
    });

    describe("removeUser", () => {
      it("should delete user from secure storage", async () => {
        mockSecureStore.deleteItemAsync.mockResolvedValue(undefined);

        await removeUser();

        expect(mockSecureStore.deleteItemAsync).toHaveBeenCalledWith("user");
      });
    });
  });

  describe("clearAuth", () => {
    it("should clear both token and user from storage", async () => {
      mockSecureStore.deleteItemAsync.mockResolvedValue(undefined);

      await clearAuth();

      expect(mockSecureStore.deleteItemAsync).toHaveBeenCalledWith("token");
      expect(mockSecureStore.deleteItemAsync).toHaveBeenCalledWith("user");
      expect(mockSecureStore.deleteItemAsync).toHaveBeenCalledTimes(2);
    });
  });

  describe("Settings Management", () => {
    const defaultSettings: AppSettings = {
      notificationsEnabled: true,
      darkMode: true,
      autoRefreshInterval: 30,
    };

    describe("getSettings", () => {
      it("should return default settings when none are saved", async () => {
        mockSecureStore.getItemAsync.mockResolvedValue(null);

        const settings = await getSettings();

        expect(mockSecureStore.getItemAsync).toHaveBeenCalledWith("settings");
        expect(settings).toEqual(defaultSettings);
      });

      it("should return stored settings merged with defaults", async () => {
        const storedSettings = {
          notificationsEnabled: false,
          apiUrl: "http://custom-api.com",
        };
        mockSecureStore.getItemAsync.mockResolvedValue(
          JSON.stringify(storedSettings)
        );

        const settings = await getSettings();

        expect(settings).toEqual({
          ...defaultSettings,
          notificationsEnabled: false,
          apiUrl: "http://custom-api.com",
        });
      });
    });

    describe("saveSettings", () => {
      it("should merge new settings with existing and save", async () => {
        // First call to getSettings (inside saveSettings)
        mockSecureStore.getItemAsync.mockResolvedValue(null);
        mockSecureStore.setItemAsync.mockResolvedValue(undefined);

        await saveSettings({ darkMode: false });

        expect(mockSecureStore.setItemAsync).toHaveBeenCalledWith(
          "settings",
          JSON.stringify({
            ...defaultSettings,
            darkMode: false,
          })
        );
      });

      it("should preserve existing custom settings when updating", async () => {
        const existingSettings = {
          ...defaultSettings,
          apiUrl: "http://existing-api.com",
        };
        mockSecureStore.getItemAsync.mockResolvedValue(
          JSON.stringify(existingSettings)
        );
        mockSecureStore.setItemAsync.mockResolvedValue(undefined);

        await saveSettings({ autoRefreshInterval: 60 });

        expect(mockSecureStore.setItemAsync).toHaveBeenCalledWith(
          "settings",
          JSON.stringify({
            ...existingSettings,
            autoRefreshInterval: 60,
          })
        );
      });
    });

    describe("resetSettings", () => {
      it("should reset settings to defaults", async () => {
        mockSecureStore.setItemAsync.mockResolvedValue(undefined);

        await resetSettings();

        expect(mockSecureStore.setItemAsync).toHaveBeenCalledWith(
          "settings",
          JSON.stringify(defaultSettings)
        );
      });
    });
  });
});
