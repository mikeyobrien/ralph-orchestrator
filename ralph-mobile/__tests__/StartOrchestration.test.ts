/**
 * @fileoverview TDD Tests for Start Orchestration functionality
 * Plan 06-01: Start Orchestration UI
 *
 * Tests helper functions and API for starting new orchestrations
 */

import {
  validatePromptPath,
  validateMaxIterations,
  validateMaxRuntime,
  formatDuration,
  getDefaultConfig,
  StartOrchestratorConfig,
} from '../lib/startOrchestratorHelpers';

import { startOrchestrator, StartOrchestratorRequest } from '../lib/orchestratorControlApi';

// Mock the API module
jest.mock('../lib/api', () => ({
  apiClient: {
    baseURL: 'http://localhost:8080',
    defaultHeaders: { 'Content-Type': 'application/json' },
  },
  getAuthHeaders: jest.fn().mockResolvedValue({ Authorization: 'Bearer test-token' }),
}));

// Mock fetch
global.fetch = jest.fn();

describe('Start Orchestration Helpers', () => {
  describe('validatePromptPath', () => {
    it('returns true for valid prompt file paths', () => {
      expect(validatePromptPath('test.md')).toBe(true);
      expect(validatePromptPath('prompts/test.md')).toBe(true);
      expect(validatePromptPath('/absolute/path/to/prompt.md')).toBe(true);
    });

    it('returns false for empty paths', () => {
      expect(validatePromptPath('')).toBe(false);
      expect(validatePromptPath('   ')).toBe(false);
    });

    it('returns false for paths without .md extension', () => {
      expect(validatePromptPath('test.txt')).toBe(false);
      expect(validatePromptPath('test')).toBe(false);
      expect(validatePromptPath('test.markdown')).toBe(false);
    });

    it('returns false for paths with invalid characters', () => {
      expect(validatePromptPath('test<>.md')).toBe(false);
      expect(validatePromptPath('test|.md')).toBe(false);
      expect(validatePromptPath('test*.md')).toBe(false);
    });
  });

  describe('validateMaxIterations', () => {
    it('returns true for valid iteration counts', () => {
      expect(validateMaxIterations(1)).toBe(true);
      expect(validateMaxIterations(50)).toBe(true);
      expect(validateMaxIterations(100)).toBe(true);
      expect(validateMaxIterations(1000)).toBe(true);
    });

    it('returns false for zero or negative values', () => {
      expect(validateMaxIterations(0)).toBe(false);
      expect(validateMaxIterations(-1)).toBe(false);
      expect(validateMaxIterations(-100)).toBe(false);
    });

    it('returns false for non-integer values', () => {
      expect(validateMaxIterations(1.5)).toBe(false);
      expect(validateMaxIterations(50.99)).toBe(false);
    });

    it('returns false for values exceeding maximum', () => {
      expect(validateMaxIterations(10001)).toBe(false);
      expect(validateMaxIterations(100000)).toBe(false);
    });
  });

  describe('validateMaxRuntime', () => {
    it('returns true for valid runtime values in seconds', () => {
      expect(validateMaxRuntime(60)).toBe(true); // 1 minute
      expect(validateMaxRuntime(3600)).toBe(true); // 1 hour
      expect(validateMaxRuntime(86400)).toBe(true); // 24 hours
    });

    it('returns false for zero or negative values', () => {
      expect(validateMaxRuntime(0)).toBe(false);
      expect(validateMaxRuntime(-1)).toBe(false);
    });

    it('returns false for values exceeding maximum (7 days)', () => {
      expect(validateMaxRuntime(604801)).toBe(false); // > 7 days
      expect(validateMaxRuntime(1000000)).toBe(false);
    });

    it('returns true for edge case at exactly 7 days', () => {
      expect(validateMaxRuntime(604800)).toBe(true); // exactly 7 days
    });
  });

  describe('formatDuration', () => {
    it('formats seconds as human readable duration', () => {
      expect(formatDuration(60)).toBe('1m');
      expect(formatDuration(3600)).toBe('1h');
      expect(formatDuration(3660)).toBe('1h 1m');
      expect(formatDuration(86400)).toBe('24h');
    });

    it('handles complex durations', () => {
      expect(formatDuration(7200)).toBe('2h');
      expect(formatDuration(7320)).toBe('2h 2m');
      expect(formatDuration(90)).toBe('1m 30s');
    });

    it('shows seconds only for short durations', () => {
      expect(formatDuration(30)).toBe('30s');
      expect(formatDuration(45)).toBe('45s');
    });

    it('handles zero', () => {
      expect(formatDuration(0)).toBe('0s');
    });
  });

  describe('getDefaultConfig', () => {
    it('returns sensible default configuration', () => {
      const config = getDefaultConfig();
      expect(config.max_iterations).toBe(50);
      expect(config.max_runtime).toBe(3600);
      expect(config.auto_commit).toBe(true);
    });

    it('returns a new object each time', () => {
      const config1 = getDefaultConfig();
      const config2 = getDefaultConfig();
      expect(config1).not.toBe(config2);
      expect(config1).toEqual(config2);
    });
  });
});

describe('Start Orchestrator API', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('startOrchestrator', () => {
    it('makes POST request to start orchestration', async () => {
      const mockResponse = {
        instance_id: 'abc12345',
        status: 'started',
        port: 8081,
      };

      (global.fetch as jest.Mock).mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const request: StartOrchestratorRequest = {
        prompt_file: 'test.md',
        max_iterations: 50,
        max_runtime: 3600,
      };

      const result = await startOrchestrator(request);

      expect(global.fetch).toHaveBeenCalledWith(
        'http://localhost:8080/api/orchestrators',
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify(request),
          headers: expect.objectContaining({
            'Content-Type': 'application/json',
            Authorization: 'Bearer test-token',
          }),
        })
      );

      expect(result.instance_id).toBe('abc12345');
      expect(result.status).toBe('started');
    });

    it('throws error on API failure', async () => {
      (global.fetch as jest.Mock).mockResolvedValueOnce({
        ok: false,
        json: () => Promise.resolve({ detail: 'Prompt file not found' }),
      });

      const request: StartOrchestratorRequest = {
        prompt_file: 'nonexistent.md',
      };

      await expect(startOrchestrator(request)).rejects.toThrow('Prompt file not found');
    });

    it('includes optional parameters when provided', async () => {
      const mockResponse = {
        instance_id: 'xyz98765',
        status: 'started',
      };

      (global.fetch as jest.Mock).mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const request: StartOrchestratorRequest = {
        prompt_file: 'test.md',
        max_iterations: 100,
        max_runtime: 7200,
        auto_commit: false,
      };

      await startOrchestrator(request);

      const fetchCall = (global.fetch as jest.Mock).mock.calls[0];
      const body = JSON.parse(fetchCall[1].body);

      expect(body.max_iterations).toBe(100);
      expect(body.max_runtime).toBe(7200);
      expect(body.auto_commit).toBe(false);
    });

    it('handles network errors gracefully', async () => {
      (global.fetch as jest.Mock).mockRejectedValueOnce(new Error('Network error'));

      const request: StartOrchestratorRequest = {
        prompt_file: 'test.md',
      };

      await expect(startOrchestrator(request)).rejects.toThrow('Network error');
    });
  });
});
