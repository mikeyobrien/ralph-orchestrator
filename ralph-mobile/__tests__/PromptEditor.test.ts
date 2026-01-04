/**
 * @fileoverview Tests for Plan 06-03: Inline Prompt Editor
 *
 * TDD: Write tests first for prompt viewing, editing, and saving functionality
 */

import {
  validatePromptContent,
  sanitizePromptContent,
  formatPromptPreview,
  getPromptMetadata,
  hasUnsavedChanges,
  countPromptLines,
  countPromptCharacters,
} from '../lib/promptEditorHelpers';
import {
  getPromptContent,
  updatePromptContent,
  getPromptVersions,
} from '../lib/promptEditorApi';
import { apiClient, getAuthHeaders } from '../lib/api';

// Mock the API module
jest.mock('../lib/api', () => ({
  apiClient: {
    baseURL: 'http://test-api.example.com',
    defaultHeaders: { 'Content-Type': 'application/json' },
  },
  getAuthHeaders: jest.fn(),
}));

describe('Plan 06-03: Prompt Editor Helpers', () => {
  describe('validatePromptContent', () => {
    it('should accept valid markdown content', () => {
      const content = '# My Prompt\n\nThis is a valid prompt.';
      const result = validatePromptContent(content);
      expect(result.valid).toBe(true);
      expect(result.error).toBeUndefined();
    });

    it('should reject empty content', () => {
      const result = validatePromptContent('');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Prompt content cannot be empty');
    });

    it('should reject whitespace-only content', () => {
      const result = validatePromptContent('   \n\t  ');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Prompt content cannot be empty');
    });

    it('should reject content exceeding max length', () => {
      const content = 'a'.repeat(100001); // Max 100KB
      const result = validatePromptContent(content);
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Prompt content exceeds maximum size (100KB)');
    });

    it('should accept content at max length boundary', () => {
      const content = 'a'.repeat(100000);
      const result = validatePromptContent(content);
      expect(result.valid).toBe(true);
    });
  });

  describe('sanitizePromptContent', () => {
    it('should trim leading and trailing whitespace', () => {
      const content = '  \n# Prompt\n  ';
      expect(sanitizePromptContent(content)).toBe('# Prompt');
    });

    it('should normalize line endings to LF', () => {
      const content = 'Line 1\r\nLine 2\r\nLine 3';
      expect(sanitizePromptContent(content)).toBe('Line 1\nLine 2\nLine 3');
    });

    it('should remove null characters', () => {
      const content = 'Hello\x00World';
      expect(sanitizePromptContent(content)).toBe('HelloWorld');
    });

    it('should preserve markdown formatting', () => {
      const content = '# Header\n\n- Item 1\n- Item 2\n\n```code```';
      expect(sanitizePromptContent(content)).toBe(content);
    });
  });

  describe('formatPromptPreview', () => {
    it('should truncate long content with ellipsis', () => {
      const content = 'This is a very long prompt that should be truncated for preview.';
      const preview = formatPromptPreview(content, 20);
      expect(preview).toBe('This is a very long...');
      expect(preview.length).toBe(22); // 19 chars trimmed + '...'
    });

    it('should not truncate short content', () => {
      const content = 'Short prompt';
      const preview = formatPromptPreview(content, 50);
      expect(preview).toBe('Short prompt');
    });

    it('should strip markdown headers for preview', () => {
      const content = '# My Title\n\nContent here';
      const preview = formatPromptPreview(content, 50);
      expect(preview).not.toContain('#');
    });

    it('should handle empty content', () => {
      const preview = formatPromptPreview('', 50);
      expect(preview).toBe('');
    });
  });

  describe('getPromptMetadata', () => {
    it('should extract title from H1 header', () => {
      const content = '# My Prompt Title\n\nContent here';
      const metadata = getPromptMetadata(content);
      expect(metadata.title).toBe('My Prompt Title');
    });

    it('should use first line as title if no H1', () => {
      const content = 'This is the first line.\n\nMore content';
      const metadata = getPromptMetadata(content);
      expect(metadata.title).toBe('This is the first line.');
    });

    it('should return character and line counts', () => {
      const content = 'Line 1\nLine 2\nLine 3';
      const metadata = getPromptMetadata(content);
      expect(metadata.lineCount).toBe(3);
      expect(metadata.characterCount).toBe(20);
    });

    it('should detect markdown sections', () => {
      const content = '# Title\n\n## Section 1\n\nContent\n\n## Section 2\n\nMore';
      const metadata = getPromptMetadata(content);
      expect(metadata.sections).toContain('Section 1');
      expect(metadata.sections).toContain('Section 2');
    });
  });

  describe('hasUnsavedChanges', () => {
    it('should return false when content matches original', () => {
      const original = '# Prompt\n\nContent';
      const current = '# Prompt\n\nContent';
      expect(hasUnsavedChanges(original, current)).toBe(false);
    });

    it('should return true when content differs', () => {
      const original = '# Prompt\n\nContent';
      const current = '# Prompt\n\nModified content';
      expect(hasUnsavedChanges(original, current)).toBe(true);
    });

    it('should ignore trailing whitespace differences', () => {
      const original = '# Prompt\n\nContent';
      const current = '# Prompt\n\nContent  \n';
      expect(hasUnsavedChanges(original, current)).toBe(false);
    });
  });

  describe('countPromptLines', () => {
    it('should count lines correctly', () => {
      expect(countPromptLines('Line 1\nLine 2\nLine 3')).toBe(3);
    });

    it('should return 1 for single line', () => {
      expect(countPromptLines('Single line')).toBe(1);
    });

    it('should return 0 for empty content', () => {
      expect(countPromptLines('')).toBe(0);
    });
  });

  describe('countPromptCharacters', () => {
    it('should count characters correctly', () => {
      expect(countPromptCharacters('Hello')).toBe(5);
    });

    it('should count including newlines', () => {
      expect(countPromptCharacters('Hi\nThere')).toBe(8);
    });

    it('should return 0 for empty content', () => {
      expect(countPromptCharacters('')).toBe(0);
    });
  });
});

describe('Plan 06-03: Prompt Editor API', () => {
  let mockFetch: jest.SpyInstance;

  beforeEach(() => {
    mockFetch = jest.spyOn(global, 'fetch').mockImplementation();
    (getAuthHeaders as jest.Mock).mockResolvedValue({
      Authorization: 'Bearer test-token',
    });
  });

  afterEach(() => {
    mockFetch.mockRestore();
    jest.clearAllMocks();
  });

  describe('getPromptContent', () => {
    it('should fetch prompt content for an orchestrator', async () => {
      const mockContent = {
        content: '# Test Prompt\n\nContent here',
        path: '/path/to/prompt.md',
        last_modified: '2025-01-04T10:00:00Z',
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockContent),
      });

      const result = await getPromptContent('instance-123');

      expect(mockFetch).toHaveBeenCalledWith(
        'http://test-api.example.com/api/orchestrators/instance-123/prompt',
        expect.objectContaining({
          method: 'GET',
          headers: expect.objectContaining({
            Authorization: 'Bearer test-token',
          }),
        })
      );
      expect(result.content).toBe('# Test Prompt\n\nContent here');
      expect(result.path).toBe('/path/to/prompt.md');
    });

    it('should throw error on fetch failure', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        json: () => Promise.resolve({ detail: 'Orchestrator not found' }),
      });

      await expect(getPromptContent('invalid-id')).rejects.toThrow(
        'Orchestrator not found'
      );
    });
  });

  describe('updatePromptContent', () => {
    it('should update prompt content successfully', async () => {
      const mockResponse = {
        success: true,
        path: '/path/to/prompt.md',
        last_modified: '2025-01-04T11:00:00Z',
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const newContent = '# Updated Prompt\n\nNew content';
      const result = await updatePromptContent('instance-123', newContent);

      expect(mockFetch).toHaveBeenCalledWith(
        'http://test-api.example.com/api/orchestrators/instance-123/prompt',
        expect.objectContaining({
          method: 'PUT',
          headers: expect.objectContaining({
            Authorization: 'Bearer test-token',
            'Content-Type': 'application/json',
          }),
          body: JSON.stringify({ content: newContent }),
        })
      );
      expect(result.success).toBe(true);
    });

    it('should throw error on update failure', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        json: () => Promise.resolve({ detail: 'Invalid content' }),
      });

      await expect(
        updatePromptContent('instance-123', 'bad content')
      ).rejects.toThrow('Invalid content');
    });
  });

  describe('getPromptVersions', () => {
    it('should fetch version history for a prompt', async () => {
      const mockVersions = {
        versions: [
          { version: 1, timestamp: '2025-01-04T09:00:00Z', preview: 'Initial...' },
          { version: 2, timestamp: '2025-01-04T10:00:00Z', preview: 'Updated...' },
        ],
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockVersions),
      });

      const result = await getPromptVersions('instance-123');

      expect(mockFetch).toHaveBeenCalledWith(
        'http://test-api.example.com/api/orchestrators/instance-123/prompt/versions',
        expect.objectContaining({
          method: 'GET',
        })
      );
      expect(result.versions).toHaveLength(2);
      expect(result.versions[1].version).toBe(2);
    });

    it('should return empty versions on failure', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        json: () => Promise.resolve({ detail: 'No versions available' }),
      });

      const result = await getPromptVersions('instance-123');
      expect(result.versions).toEqual([]);
    });
  });
});
