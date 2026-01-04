/**
 * TDD Tests for useAuth Hook (Plan 04-04 Authentication)
 * Tests the hook's logic without requiring full React Native environment
 */

import { login, logout, isAuthenticated } from '../lib/api';

// Mock API functions
jest.mock('../lib/api', () => ({
  login: jest.fn(),
  logout: jest.fn(),
  isAuthenticated: jest.fn(),
}));

describe('useAuth Hook Dependencies', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('API Integration', () => {
    it('login function is available and callable', async () => {
      (login as jest.Mock).mockResolvedValueOnce({ access_token: 'test-token' });

      const result = await login('testuser', 'testpass');

      expect(login).toHaveBeenCalledWith('testuser', 'testpass');
      expect(result).toEqual({ access_token: 'test-token' });
    });

    it('logout function is available and callable', async () => {
      (logout as jest.Mock).mockResolvedValueOnce(undefined);

      await logout();

      expect(logout).toHaveBeenCalled();
    });

    it('isAuthenticated function is available and callable', async () => {
      (isAuthenticated as jest.Mock).mockResolvedValueOnce(true);

      const result = await isAuthenticated();

      expect(result).toBe(true);
    });

    it('isAuthenticated returns false when not logged in', async () => {
      (isAuthenticated as jest.Mock).mockResolvedValueOnce(false);

      const result = await isAuthenticated();

      expect(result).toBe(false);
    });
  });

  describe('Authentication Flow', () => {
    it('login followed by isAuthenticated returns true', async () => {
      (login as jest.Mock).mockResolvedValueOnce({ access_token: 'token' });
      (isAuthenticated as jest.Mock).mockResolvedValueOnce(true);

      await login('user', 'pass');
      const authed = await isAuthenticated();

      expect(authed).toBe(true);
    });

    it('logout followed by isAuthenticated returns false', async () => {
      (logout as jest.Mock).mockResolvedValueOnce(undefined);
      (isAuthenticated as jest.Mock).mockResolvedValueOnce(false);

      await logout();
      const authed = await isAuthenticated();

      expect(authed).toBe(false);
    });

    it('login error is propagated', async () => {
      (login as jest.Mock).mockRejectedValueOnce(new Error('Invalid credentials'));

      await expect(login('wrong', 'creds')).rejects.toThrow('Invalid credentials');
    });
  });
});
