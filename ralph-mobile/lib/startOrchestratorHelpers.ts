/**
 * @fileoverview Helper functions for starting orchestrations
 * Plan 06-01: Start Orchestration UI
 *
 * Provides validation, formatting, and configuration utilities
 */

/**
 * Configuration for starting an orchestrator
 */
export interface StartOrchestratorConfig {
  max_iterations: number;
  max_runtime: number;
  auto_commit: boolean;
}

// Constants
const MAX_ITERATIONS_LIMIT = 10000;
const MAX_RUNTIME_LIMIT = 604800; // 7 days in seconds
const INVALID_PATH_CHARS = /[<>|*"]/;

/**
 * Validates a prompt file path
 * Must be non-empty, end with .md, and not contain invalid characters
 */
export function validatePromptPath(path: string): boolean {
  // Check for empty or whitespace-only
  const trimmed = path.trim();
  if (!trimmed) {
    return false;
  }

  // Check for .md extension
  if (!trimmed.endsWith('.md')) {
    return false;
  }

  // Check for invalid characters
  if (INVALID_PATH_CHARS.test(trimmed)) {
    return false;
  }

  return true;
}

/**
 * Validates max iterations value
 * Must be a positive integer not exceeding 10000
 */
export function validateMaxIterations(value: number): boolean {
  // Must be positive
  if (value <= 0) {
    return false;
  }

  // Must be an integer
  if (!Number.isInteger(value)) {
    return false;
  }

  // Must not exceed limit
  if (value > MAX_ITERATIONS_LIMIT) {
    return false;
  }

  return true;
}

/**
 * Validates max runtime value in seconds
 * Must be positive and not exceed 7 days
 */
export function validateMaxRuntime(value: number): boolean {
  // Must be positive
  if (value <= 0) {
    return false;
  }

  // Must not exceed 7 days
  if (value > MAX_RUNTIME_LIMIT) {
    return false;
  }

  return true;
}

/**
 * Formats a duration in seconds as human-readable string
 * Examples: "30s", "1m", "1h 30m", "2h"
 */
export function formatDuration(seconds: number): string {
  if (seconds === 0) {
    return '0s';
  }

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainingSeconds = seconds % 60;

  const parts: string[] = [];

  if (hours > 0) {
    parts.push(`${hours}h`);
  }

  if (minutes > 0) {
    parts.push(`${minutes}m`);
  }

  // Only show seconds if no hours and duration includes seconds
  if (hours === 0 && remainingSeconds > 0) {
    parts.push(`${remainingSeconds}s`);
  }

  return parts.join(' ');
}

/**
 * Returns default configuration for starting an orchestrator
 * Returns a new object each time to prevent mutation issues
 */
export function getDefaultConfig(): StartOrchestratorConfig {
  return {
    max_iterations: 50,
    max_runtime: 3600, // 1 hour
    auto_commit: true,
  };
}
