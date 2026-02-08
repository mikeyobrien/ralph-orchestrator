import axios from "axios";
import * as SecureStore from "expo-secure-store";

const API_URL = process.env.EXPO_PUBLIC_API_URL || "http://localhost:8000";

export const api = axios.create({
  baseURL: API_URL,
  headers: {
    "Content-Type": "application/json",
  },
  timeout: 30000, // 30 second timeout
});

// Request interceptor - Add auth token to all requests
api.interceptors.request.use(
  async (config) => {
    const token = await SecureStore.getItemAsync("token");
    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => {
    return Promise.reject(error);
  }
);

// Response interceptor - Handle 401 errors (token expiry)
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    if (error.response?.status === 401) {
      // Clear token on unauthorized response
      await SecureStore.deleteItemAsync("token");
      await SecureStore.deleteItemAsync("user");
    }
    return Promise.reject(error);
  }
);

// Ralph Orchestrator specific API endpoints
export const orchestratorApi = {
  // Sessions
  getSessions: () => api.get("/api/sessions"),
  getSession: (id: string) => api.get(`/api/sessions/${id}`),
  createSession: (data: { prompt_file: string; config?: object }) =>
    api.post("/api/sessions", data),
  pauseSession: (id: string) => api.post(`/api/sessions/${id}/pause`),
  resumeSession: (id: string) => api.post(`/api/sessions/${id}/resume`),
  stopSession: (id: string) => api.post(`/api/sessions/${id}/stop`),

  // Iterations
  getIterations: (sessionId: string) =>
    api.get(`/api/sessions/${sessionId}/iterations`),
  getIteration: (sessionId: string, iterationId: string) =>
    api.get(`/api/sessions/${sessionId}/iterations/${iterationId}`),

  // Logs
  getLogs: (sessionId: string, params?: { limit?: number; offset?: number }) =>
    api.get(`/api/sessions/${sessionId}/logs`, { params }),

  // Metrics
  getMetrics: (sessionId: string) =>
    api.get(`/api/sessions/${sessionId}/metrics`),
  getTokenUsage: (sessionId: string) =>
    api.get(`/api/sessions/${sessionId}/tokens`),
  getCosts: (sessionId: string) => api.get(`/api/sessions/${sessionId}/costs`),

  // Health check
  healthCheck: () => api.get("/health"),
};

export default api;
