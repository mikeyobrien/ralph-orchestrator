/**
 * API Client for Ralph Orchestrator
 * Handles authentication and API requests
 */

import * as SecureStore from 'expo-secure-store';

// API base URL - configurable via environment variable
const API_BASE_URL = process.env.EXPO_PUBLIC_API_URL || 'http://localhost:8080';

/**
 * API Client configuration
 */
export const apiClient = {
  baseURL: API_BASE_URL,
  defaultHeaders: {
    'Content-Type': 'application/json',
  } as Record<string, string>,
};

/**
 * Login with username and password
 * Stores JWT token in secure storage
 */
export async function login(username: string, password: string): Promise<{ access_token: string; token_type: string }> {
  const response = await fetch(`${apiClient.baseURL}/api/auth/login`, {
    method: 'POST',
    headers: apiClient.defaultHeaders,
    body: JSON.stringify({ username, password }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.detail || 'Login failed');
  }

  const data = await response.json();
  await SecureStore.setItemAsync('token', data.access_token);
  return data;
}

/**
 * Logout - removes token from secure storage
 */
export async function logout(): Promise<void> {
  await SecureStore.deleteItemAsync('token');
}

/**
 * Get authorization headers with JWT token
 */
export async function getAuthHeaders(): Promise<Record<string, string>> {
  const token = await SecureStore.getItemAsync('token');
  if (!token) {
    return {};
  }
  return { Authorization: `Bearer ${token}` };
}

/**
 * Check if user is authenticated
 */
export async function isAuthenticated(): Promise<boolean> {
  const token = await SecureStore.getItemAsync('token');
  return !!token;
}

/**
 * Make authenticated API request
 */
export async function fetchWithAuth(
  endpoint: string,
  options: RequestInit = {}
): Promise<Response> {
  const authHeaders = await getAuthHeaders();
  const response = await fetch(`${apiClient.baseURL}${endpoint}`, {
    ...options,
    headers: {
      ...apiClient.defaultHeaders,
      ...authHeaders,
      ...options.headers,
    },
  });
  return response;
}
