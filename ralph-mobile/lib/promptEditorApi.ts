/**
 * @fileoverview API functions for prompt viewing and editing
 * Plan 06-03: Inline Prompt Editor
 *
 * Provides functions to fetch and update orchestrator prompts
 */

import { apiClient, getAuthHeaders } from './api';

/**
 * Response from fetching prompt content
 */
export interface PromptContentResponse {
  content: string;
  path: string;
  last_modified: string;
}

/**
 * Response from updating prompt content
 */
export interface UpdatePromptResponse {
  success: boolean;
  path: string;
  last_modified: string;
}

/**
 * A version of a prompt in history
 */
export interface PromptVersion {
  version: number;
  timestamp: string;
  preview: string;
}

/**
 * Response from fetching prompt versions
 */
export interface PromptVersionsResponse {
  versions: PromptVersion[];
}

/**
 * Fetch the current prompt content for an orchestrator
 */
export async function getPromptContent(
  instanceId: string
): Promise<PromptContentResponse> {
  const authHeaders = await getAuthHeaders();

  const response = await fetch(
    `${apiClient.baseURL}/api/orchestrators/${instanceId}/prompt`,
    {
      method: 'GET',
      headers: {
        ...apiClient.defaultHeaders,
        ...authHeaders,
      },
    }
  );

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.detail || 'Failed to fetch prompt content');
  }

  return response.json();
}

/**
 * Update the prompt content for an orchestrator
 */
export async function updatePromptContent(
  instanceId: string,
  content: string
): Promise<UpdatePromptResponse> {
  const authHeaders = await getAuthHeaders();

  const response = await fetch(
    `${apiClient.baseURL}/api/orchestrators/${instanceId}/prompt`,
    {
      method: 'PUT',
      headers: {
        ...apiClient.defaultHeaders,
        ...authHeaders,
      },
      body: JSON.stringify({ content }),
    }
  );

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.detail || 'Failed to update prompt content');
  }

  return response.json();
}

/**
 * Fetch version history for a prompt
 * Returns empty versions array on failure (non-critical feature)
 */
export async function getPromptVersions(
  instanceId: string
): Promise<PromptVersionsResponse> {
  const authHeaders = await getAuthHeaders();

  const response = await fetch(
    `${apiClient.baseURL}/api/orchestrators/${instanceId}/prompt/versions`,
    {
      method: 'GET',
      headers: {
        ...apiClient.defaultHeaders,
        ...authHeaders,
      },
    }
  );

  if (!response.ok) {
    // Version history is optional - return empty on failure
    return { versions: [] };
  }

  return response.json();
}
