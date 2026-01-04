/**
 * @fileoverview Helper functions for prompt viewing and editing
 * Plan 06-03: Inline Prompt Editor
 *
 * Provides validation, formatting, and metadata extraction for prompts
 */

/**
 * Maximum allowed prompt size in characters (100KB)
 */
const MAX_PROMPT_SIZE = 100000;

/**
 * Result of prompt content validation
 */
export interface ValidationResult {
  valid: boolean;
  error?: string;
}

/**
 * Metadata extracted from prompt content
 */
export interface PromptMetadata {
  title: string;
  lineCount: number;
  characterCount: number;
  sections: string[];
}

/**
 * Validate prompt content for correctness
 */
export function validatePromptContent(content: string): ValidationResult {
  // Check for empty or whitespace-only content
  if (!content || !content.trim()) {
    return { valid: false, error: 'Prompt content cannot be empty' };
  }

  // Check size limit
  if (content.length > MAX_PROMPT_SIZE) {
    return { valid: false, error: 'Prompt content exceeds maximum size (100KB)' };
  }

  return { valid: true };
}

/**
 * Sanitize prompt content for safe storage
 */
export function sanitizePromptContent(content: string): string {
  return content
    // Remove null characters
    .replace(/\x00/g, '')
    // Normalize line endings to LF
    .replace(/\r\n/g, '\n')
    .replace(/\r/g, '\n')
    // Trim leading and trailing whitespace
    .trim();
}

/**
 * Format prompt content for preview display
 */
export function formatPromptPreview(content: string, maxLength: number): string {
  if (!content) {
    return '';
  }

  // Strip markdown headers for cleaner preview
  let preview = content.replace(/^#{1,6}\s+/gm, '');

  // Collapse multiple newlines/whitespace to single space
  preview = preview.replace(/\s+/g, ' ').trim();

  // Truncate if necessary
  if (preview.length > maxLength) {
    // Trim trailing space before adding ellipsis
    return preview.substring(0, maxLength).trimEnd() + '...';
  }

  return preview;
}

/**
 * Extract metadata from prompt content
 */
export function getPromptMetadata(content: string): PromptMetadata {
  // Extract title from H1 header or first line
  const h1Match = content.match(/^#\s+(.+)$/m);
  let title: string;

  if (h1Match) {
    title = h1Match[1].trim();
  } else {
    const firstLine = content.split('\n')[0] || '';
    title = firstLine.trim();
  }

  // Extract section headers (H2)
  const sectionMatches = content.matchAll(/^##\s+(.+)$/gm);
  const sections: string[] = [];
  for (const match of sectionMatches) {
    sections.push(match[1].trim());
  }

  return {
    title,
    lineCount: countPromptLines(content),
    characterCount: countPromptCharacters(content),
    sections,
  };
}

/**
 * Check if current content differs from original (has unsaved changes)
 */
export function hasUnsavedChanges(original: string, current: string): boolean {
  // Normalize both by trimming trailing whitespace from each line
  const normalizeContent = (str: string): string =>
    str
      .split('\n')
      .map((line) => line.trimEnd())
      .join('\n')
      .trim();

  return normalizeContent(original) !== normalizeContent(current);
}

/**
 * Count the number of lines in prompt content
 */
export function countPromptLines(content: string): number {
  if (!content) {
    return 0;
  }
  return content.split('\n').length;
}

/**
 * Count the number of characters in prompt content
 */
export function countPromptCharacters(content: string): number {
  return content.length;
}
