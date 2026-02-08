/**
 * Integration tests for API client - Tests actual HTTP flows with mock server
 * Following TDD: These tests verify complete request/response cycles
 */
import axios from "axios";
import MockAdapter from "axios-mock-adapter";
import * as SecureStore from "expo-secure-store";

// Import fresh api instance for each test suite
// Note: We need to test the actual axios instance, not mocked version
describe("API Integration Tests", () => {
  let mockApi: MockAdapter;
  let apiClient: typeof axios;

  // Create a fresh axios instance for integration tests
  beforeAll(() => {
    apiClient = axios.create({
      baseURL: "http://localhost:8000",
      headers: { "Content-Type": "application/json" },
      timeout: 30000,
    });

    // Add same interceptors as production api.ts
    apiClient.interceptors.request.use(
      async (config) => {
        const token = await SecureStore.getItemAsync("token");
        if (token) {
          config.headers.Authorization = `Bearer ${token}`;
        }
        return config;
      },
      (error) => Promise.reject(error)
    );

    apiClient.interceptors.response.use(
      (response) => response,
      async (error) => {
        if (error.response?.status === 401) {
          await SecureStore.deleteItemAsync("token");
          await SecureStore.deleteItemAsync("user");
        }
        return Promise.reject(error);
      }
    );
  });

  beforeEach(() => {
    mockApi = new MockAdapter(apiClient);
    jest.clearAllMocks();
  });

  afterEach(() => {
    mockApi.reset();
  });

  afterAll(() => {
    mockApi.restore();
  });

  describe("Complete request/response cycles", () => {
    it("should make GET request and return data", async () => {
      const mockSessions = [
        { id: "sess-1", status: "running" },
        { id: "sess-2", status: "completed" },
      ];

      mockApi.onGet("/api/sessions").reply(200, mockSessions);

      const response = await apiClient.get("/api/sessions");

      expect(response.status).toBe(200);
      expect(response.data).toEqual(mockSessions);
    });

    it("should make POST request with body and return created resource", async () => {
      const requestData = { prompt_file: "test.md", config: { max_iterations: 5 } };
      const responseData = { id: "sess-new", status: "created", ...requestData };

      mockApi.onPost("/api/sessions", requestData).reply(201, responseData);

      const response = await apiClient.post("/api/sessions", requestData);

      expect(response.status).toBe(201);
      expect(response.data).toEqual(responseData);
    });

    it("should handle query parameters correctly", async () => {
      const mockLogs = [
        { timestamp: "2024-01-01T00:00:00Z", message: "Log 1" },
        { timestamp: "2024-01-01T00:01:00Z", message: "Log 2" },
      ];

      mockApi
        .onGet("/api/sessions/sess-1/logs", { params: { limit: 50, offset: 100 } })
        .reply(200, mockLogs);

      const response = await apiClient.get("/api/sessions/sess-1/logs", {
        params: { limit: 50, offset: 100 },
      });

      expect(response.status).toBe(200);
      expect(response.data).toEqual(mockLogs);
    });
  });

  describe("Authentication flow integration", () => {
    it("should add Bearer token to request when authenticated", async () => {
      const mockToken = "integration-test-token-12345";
      (SecureStore.getItemAsync as jest.Mock).mockResolvedValue(mockToken);

      mockApi.onGet("/api/sessions").reply((config) => {
        // Verify token was added to request
        expect(config.headers?.Authorization).toBe(`Bearer ${mockToken}`);
        return [200, []];
      });

      await apiClient.get("/api/sessions");
    });

    it("should not add Authorization header when no token exists", async () => {
      (SecureStore.getItemAsync as jest.Mock).mockResolvedValue(null);

      mockApi.onGet("/api/sessions").reply((config) => {
        expect(config.headers?.Authorization).toBeUndefined();
        return [200, []];
      });

      await apiClient.get("/api/sessions");
    });

    it("should clear stored credentials on 401 response", async () => {
      (SecureStore.getItemAsync as jest.Mock).mockResolvedValue("expired-token");
      (SecureStore.deleteItemAsync as jest.Mock).mockResolvedValue(undefined);

      mockApi.onGet("/api/sessions").reply(401, { error: "Unauthorized" });

      await expect(apiClient.get("/api/sessions")).rejects.toThrow();

      // Verify credentials were cleared
      expect(SecureStore.deleteItemAsync).toHaveBeenCalledWith("token");
      expect(SecureStore.deleteItemAsync).toHaveBeenCalledWith("user");
    });
  });

  describe("Error handling integration", () => {
    it("should handle 400 Bad Request with error message", async () => {
      const errorResponse = { error: "Invalid prompt file", code: "INVALID_INPUT" };

      mockApi.onPost("/api/sessions").reply(400, errorResponse);

      try {
        await apiClient.post("/api/sessions", {});
        fail("Should have thrown an error");
      } catch (error: any) {
        expect(error.response.status).toBe(400);
        expect(error.response.data).toEqual(errorResponse);
      }
    });

    it("should handle 404 Not Found", async () => {
      mockApi.onGet("/api/sessions/nonexistent").reply(404, { error: "Session not found" });

      try {
        await apiClient.get("/api/sessions/nonexistent");
        fail("Should have thrown an error");
      } catch (error: any) {
        expect(error.response.status).toBe(404);
      }
    });

    it("should handle 500 Internal Server Error", async () => {
      mockApi.onGet("/api/sessions").reply(500, { error: "Internal server error" });

      try {
        await apiClient.get("/api/sessions");
        fail("Should have thrown an error");
      } catch (error: any) {
        expect(error.response.status).toBe(500);
      }
    });

    it("should handle network timeout", async () => {
      mockApi.onGet("/api/sessions").timeout();

      try {
        await apiClient.get("/api/sessions");
        fail("Should have thrown an error");
      } catch (error: any) {
        expect(error.code).toBe("ECONNABORTED");
      }
    });

    it("should handle network error", async () => {
      mockApi.onGet("/api/sessions").networkError();

      try {
        await apiClient.get("/api/sessions");
        fail("Should have thrown an error");
      } catch (error: any) {
        expect(error.message).toBe("Network Error");
      }
    });
  });

  describe("Session lifecycle integration", () => {
    it("should complete full session lifecycle: create -> pause -> resume -> stop", async () => {
      const sessionId = "lifecycle-test-session";

      // 1. Create session
      mockApi.onPost("/api/sessions").reply(201, { id: sessionId, status: "running" });
      const createResponse = await apiClient.post("/api/sessions", {
        prompt_file: "test.md",
      });
      expect(createResponse.data.id).toBe(sessionId);
      expect(createResponse.data.status).toBe("running");

      // 2. Pause session
      mockApi
        .onPost(`/api/sessions/${sessionId}/pause`)
        .reply(200, { id: sessionId, status: "paused" });
      const pauseResponse = await apiClient.post(`/api/sessions/${sessionId}/pause`);
      expect(pauseResponse.data.status).toBe("paused");

      // 3. Resume session
      mockApi
        .onPost(`/api/sessions/${sessionId}/resume`)
        .reply(200, { id: sessionId, status: "running" });
      const resumeResponse = await apiClient.post(`/api/sessions/${sessionId}/resume`);
      expect(resumeResponse.data.status).toBe("running");

      // 4. Stop session
      mockApi
        .onPost(`/api/sessions/${sessionId}/stop`)
        .reply(200, { id: sessionId, status: "stopped" });
      const stopResponse = await apiClient.post(`/api/sessions/${sessionId}/stop`);
      expect(stopResponse.data.status).toBe("stopped");
    });

    it("should fetch session details with iterations", async () => {
      const sessionId = "detail-test-session";
      const mockSession = {
        id: sessionId,
        status: "running",
        iteration_count: 3,
        created_at: "2024-01-01T00:00:00Z",
      };
      const mockIterations = [
        { id: "iter-1", status: "completed", metrics: { tokens: 1000 } },
        { id: "iter-2", status: "completed", metrics: { tokens: 1500 } },
        { id: "iter-3", status: "running", metrics: { tokens: 500 } },
      ];

      mockApi.onGet(`/api/sessions/${sessionId}`).reply(200, mockSession);
      mockApi.onGet(`/api/sessions/${sessionId}/iterations`).reply(200, mockIterations);

      // Fetch both in parallel (as real app would)
      const [sessionResponse, iterationsResponse] = await Promise.all([
        apiClient.get(`/api/sessions/${sessionId}`),
        apiClient.get(`/api/sessions/${sessionId}/iterations`),
      ]);

      expect(sessionResponse.data).toEqual(mockSession);
      expect(iterationsResponse.data).toHaveLength(3);
      expect(iterationsResponse.data[0].status).toBe("completed");
    });
  });

  describe("Metrics and logs integration", () => {
    it("should fetch metrics, tokens, and costs for a session", async () => {
      const sessionId = "metrics-test-session";
      const mockMetrics = { duration_seconds: 3600, iterations: 10 };
      const mockTokens = { input_tokens: 50000, output_tokens: 25000 };
      const mockCosts = { total_cost: 0.75, currency: "USD" };

      mockApi.onGet(`/api/sessions/${sessionId}/metrics`).reply(200, mockMetrics);
      mockApi.onGet(`/api/sessions/${sessionId}/tokens`).reply(200, mockTokens);
      mockApi.onGet(`/api/sessions/${sessionId}/costs`).reply(200, mockCosts);

      const [metricsRes, tokensRes, costsRes] = await Promise.all([
        apiClient.get(`/api/sessions/${sessionId}/metrics`),
        apiClient.get(`/api/sessions/${sessionId}/tokens`),
        apiClient.get(`/api/sessions/${sessionId}/costs`),
      ]);

      expect(metricsRes.data.duration_seconds).toBe(3600);
      expect(tokensRes.data.input_tokens).toBe(50000);
      expect(costsRes.data.total_cost).toBe(0.75);
    });

    it("should fetch paginated logs", async () => {
      const sessionId = "logs-test-session";
      const page1Logs = [
        { timestamp: "2024-01-01T00:00:00Z", message: "Start" },
        { timestamp: "2024-01-01T00:01:00Z", message: "Processing" },
      ];
      const page2Logs = [
        { timestamp: "2024-01-01T00:02:00Z", message: "Completed" },
      ];

      mockApi
        .onGet(`/api/sessions/${sessionId}/logs`, { params: { limit: 2, offset: 0 } })
        .reply(200, { logs: page1Logs, total: 3, hasMore: true });

      mockApi
        .onGet(`/api/sessions/${sessionId}/logs`, { params: { limit: 2, offset: 2 } })
        .reply(200, { logs: page2Logs, total: 3, hasMore: false });

      const page1 = await apiClient.get(`/api/sessions/${sessionId}/logs`, {
        params: { limit: 2, offset: 0 },
      });
      expect(page1.data.logs).toHaveLength(2);
      expect(page1.data.hasMore).toBe(true);

      const page2 = await apiClient.get(`/api/sessions/${sessionId}/logs`, {
        params: { limit: 2, offset: 2 },
      });
      expect(page2.data.logs).toHaveLength(1);
      expect(page2.data.hasMore).toBe(false);
    });
  });

  describe("Health check integration", () => {
    it("should verify server health", async () => {
      mockApi.onGet("/health").reply(200, { status: "healthy", version: "1.0.0" });

      const response = await apiClient.get("/health");

      expect(response.status).toBe(200);
      expect(response.data.status).toBe("healthy");
    });

    it("should detect unhealthy server", async () => {
      mockApi.onGet("/health").reply(503, { status: "unhealthy", error: "Database connection failed" });

      try {
        await apiClient.get("/health");
        fail("Should have thrown an error");
      } catch (error: any) {
        expect(error.response.status).toBe(503);
        expect(error.response.data.status).toBe("unhealthy");
      }
    });
  });
});
