/**
 * TDD Tests for API Client (Plan 04-04 Authentication)
 * Following TDD: Write tests first, watch them fail, then implement
 */

import { apiClient, login, logout, getAuthHeaders, isAuthenticated } from '../lib/api';

// Mock expo-secure-store
jest.mock('expo-secure-store', () => ({
  getItemAsync: jest.fn(),
  setItemAsync: jest.fn(),
  deleteItemAsync: jest.fn(),
}));

// Mock fetch
global.fetch = jest.fn();

import * as SecureStore from 'expo-secure-store';

describe('API Client', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    (global.fetch as jest.Mock).mockReset();
  });

  describe('apiClient', () => {
    it('has base URL configured', () => {
      expect(apiClient.baseURL).toBeDefined();
    });

    it('sets JSON content type by default', () => {
      expect(apiClient.defaultHeaders['Content-Type']).toBe('application/json');
    });
  });

  describe('login', () => {
    it('calls auth endpoint with credentials', async () => {
      const mockResponse = { access_token: 'test-token', token_type: 'bearer' };
      (global.fetch as jest.Mock).mockResolvedValueOnce({
        ok: true,
        json: async () => mockResponse,
      });

      await login('testuser', 'testpass');

      expect(global.fetch).toHaveBeenCalledWith(
        expect.stringContaining('/api/auth/login'),
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify({ username: 'testuser', password: 'testpass' }),
        })
      );
    });

    it('stores token in secure storage', async () => {
      const mockResponse = { access_token: 'test-token', token_type: 'bearer' };
      (global.fetch as jest.Mock).mockResolvedValueOnce({
        ok: true,
        json: async () => mockResponse,
      });

      await login('testuser', 'testpass');

      expect(SecureStore.setItemAsync).toHaveBeenCalledWith('token', 'test-token');
    });

    it('throws error on login failure', async () => {
      (global.fetch as jest.Mock).mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: async () => ({ detail: 'Invalid credentials' }),
      });

      await expect(login('wrong', 'creds')).rejects.toThrow('Invalid credentials');
    });
  });

  describe('logout', () => {
    it('removes token from secure storage', async () => {
      await logout();

      expect(SecureStore.deleteItemAsync).toHaveBeenCalledWith('token');
    });
  });

  describe('getAuthHeaders', () => {
    it('returns authorization header with token', async () => {
      (SecureStore.getItemAsync as jest.Mock).mockResolvedValueOnce('stored-token');

      const headers = await getAuthHeaders();

      expect(headers).toEqual({ Authorization: 'Bearer stored-token' });
    });

    it('returns empty object when no token', async () => {
      (SecureStore.getItemAsync as jest.Mock).mockResolvedValueOnce(null);

      const headers = await getAuthHeaders();

      expect(headers).toEqual({});
    });
  });

  describe('isAuthenticated', () => {
    it('returns true when token exists', async () => {
      (SecureStore.getItemAsync as jest.Mock).mockResolvedValueOnce('token');

      const result = await isAuthenticated();

      expect(result).toBe(true);
    });

    it('returns false when no token', async () => {
      (SecureStore.getItemAsync as jest.Mock).mockResolvedValueOnce(null);

      const result = await isAuthenticated();

      expect(result).toBe(false);
    });
  });
});
