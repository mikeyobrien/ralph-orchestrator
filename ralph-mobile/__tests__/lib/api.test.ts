/**
 * Tests for api.ts - API client with auth interceptors
 * TDD: Write tests first, watch them fail, then verify
 */
import { InternalAxiosRequestConfig } from "axios";
import * as SecureStore from "expo-secure-store";

// Store interceptors for testing
let requestInterceptor: (
  config: InternalAxiosRequestConfig
) => Promise<InternalAxiosRequestConfig>;
let requestErrorInterceptor: (error: any) => Promise<any>;
let responseInterceptor: (response: any) => any;
let responseErrorInterceptor: (error: any) => Promise<any>;

// Mock axios with captured interceptors
const mockAxiosInstance = {
  get: jest.fn(),
  post: jest.fn(),
  put: jest.fn(),
  delete: jest.fn(),
  interceptors: {
    request: {
      use: jest.fn((onFulfilled, onRejected) => {
        requestInterceptor = onFulfilled;
        requestErrorInterceptor = onRejected;
      }),
    },
    response: {
      use: jest.fn((onFulfilled, onRejected) => {
        responseInterceptor = onFulfilled;
        responseErrorInterceptor = onRejected;
      }),
    },
  },
};

// Mock axios before importing api
jest.mock("axios", () => ({
  create: jest.fn(() => mockAxiosInstance),
}));

// Import axios for assertion, require api after mock
const axios = require("axios");
const { api, orchestratorApi } = require("../../lib/api");

// Test axios.create config before clearAllMocks resets it
describe("axios instance creation", () => {
  it("should have created axios instance with correct configuration", () => {
    // axios.create was called at module load time
    const createMock = axios.create as jest.Mock;
    const calls = createMock.mock.calls;
    expect(calls.length).toBeGreaterThanOrEqual(1);

    // Check first call had correct config
    const config = calls[0][0];
    expect(config).toMatchObject({
      baseURL: expect.any(String),
      headers: {
        "Content-Type": "application/json",
      },
      timeout: 30000,
    });
  });
});

describe("api", () => {
  let mockSecureStore: jest.Mocked<typeof SecureStore>;

  beforeEach(() => {
    jest.clearAllMocks();
    mockSecureStore = SecureStore as jest.Mocked<typeof SecureStore>;
  });

  describe("request interceptor", () => {
    it("should add Authorization header when token exists", async () => {
      mockSecureStore.getItemAsync.mockResolvedValue("test-token-123");

      const config = {
        headers: {},
      } as InternalAxiosRequestConfig;

      const result = await requestInterceptor(config);

      expect(mockSecureStore.getItemAsync).toHaveBeenCalledWith("token");
      expect(result.headers.Authorization).toBe("Bearer test-token-123");
    });

    it("should not add Authorization header when no token exists", async () => {
      mockSecureStore.getItemAsync.mockResolvedValue(null);

      const config = {
        headers: {},
      } as InternalAxiosRequestConfig;

      const result = await requestInterceptor(config);

      expect(result.headers.Authorization).toBeUndefined();
    });

    it("should reject request errors", async () => {
      const error = new Error("Request error");

      await expect(requestErrorInterceptor(error)).rejects.toThrow(
        "Request error"
      );
    });
  });

  describe("response interceptor", () => {
    it("should pass through successful responses", () => {
      const response = { data: { success: true }, status: 200 };

      const result = responseInterceptor(response);

      expect(result).toEqual(response);
    });

    it("should clear auth on 401 response", async () => {
      mockSecureStore.deleteItemAsync.mockResolvedValue(undefined);

      const error = {
        response: { status: 401 },
        message: "Unauthorized",
      };

      await expect(responseErrorInterceptor(error)).rejects.toEqual(error);

      expect(mockSecureStore.deleteItemAsync).toHaveBeenCalledWith("token");
      expect(mockSecureStore.deleteItemAsync).toHaveBeenCalledWith("user");
    });

    it("should not clear auth on non-401 errors", async () => {
      const error = {
        response: { status: 500 },
        message: "Server error",
      };

      await expect(responseErrorInterceptor(error)).rejects.toEqual(error);

      expect(mockSecureStore.deleteItemAsync).not.toHaveBeenCalled();
    });

    it("should handle errors without response object", async () => {
      const error = {
        message: "Network error",
      };

      await expect(responseErrorInterceptor(error)).rejects.toEqual(error);

      expect(mockSecureStore.deleteItemAsync).not.toHaveBeenCalled();
    });
  });

  describe("orchestratorApi", () => {
    describe("Sessions endpoints", () => {
      it("getSessions calls correct endpoint", () => {
        orchestratorApi.getSessions();
        expect(mockAxiosInstance.get).toHaveBeenCalledWith("/api/sessions");
      });

      it("getSession calls correct endpoint with id", () => {
        orchestratorApi.getSession("session-123");
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123"
        );
      });

      it("createSession posts to correct endpoint with data", () => {
        const data = { prompt_file: "test.md", config: { key: "value" } };
        orchestratorApi.createSession(data);
        expect(mockAxiosInstance.post).toHaveBeenCalledWith(
          "/api/sessions",
          data
        );
      });

      it("pauseSession posts to correct endpoint", () => {
        orchestratorApi.pauseSession("session-123");
        expect(mockAxiosInstance.post).toHaveBeenCalledWith(
          "/api/sessions/session-123/pause"
        );
      });

      it("resumeSession posts to correct endpoint", () => {
        orchestratorApi.resumeSession("session-123");
        expect(mockAxiosInstance.post).toHaveBeenCalledWith(
          "/api/sessions/session-123/resume"
        );
      });

      it("stopSession posts to correct endpoint", () => {
        orchestratorApi.stopSession("session-123");
        expect(mockAxiosInstance.post).toHaveBeenCalledWith(
          "/api/sessions/session-123/stop"
        );
      });
    });

    describe("Iterations endpoints", () => {
      it("getIterations calls correct endpoint", () => {
        orchestratorApi.getIterations("session-123");
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123/iterations"
        );
      });

      it("getIteration calls correct endpoint with both ids", () => {
        orchestratorApi.getIteration("session-123", "iteration-456");
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123/iterations/iteration-456"
        );
      });
    });

    describe("Logs endpoint", () => {
      it("getLogs calls correct endpoint without params", () => {
        orchestratorApi.getLogs("session-123");
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123/logs",
          { params: undefined }
        );
      });

      it("getLogs calls correct endpoint with pagination params", () => {
        orchestratorApi.getLogs("session-123", { limit: 50, offset: 100 });
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123/logs",
          { params: { limit: 50, offset: 100 } }
        );
      });
    });

    describe("Metrics endpoints", () => {
      it("getMetrics calls correct endpoint", () => {
        orchestratorApi.getMetrics("session-123");
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123/metrics"
        );
      });

      it("getTokenUsage calls correct endpoint", () => {
        orchestratorApi.getTokenUsage("session-123");
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123/tokens"
        );
      });

      it("getCosts calls correct endpoint", () => {
        orchestratorApi.getCosts("session-123");
        expect(mockAxiosInstance.get).toHaveBeenCalledWith(
          "/api/sessions/session-123/costs"
        );
      });
    });

    describe("Health check endpoint", () => {
      it("healthCheck calls correct endpoint", () => {
        orchestratorApi.healthCheck();
        expect(mockAxiosInstance.get).toHaveBeenCalledWith("/health");
      });
    });
  });
});
