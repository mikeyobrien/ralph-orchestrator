/**
 * @fileoverview API functions for controlling orchestrators
 * Plan 06-01: Start Orchestration UI
 * Plan 06-02: Stop/Pause/Resume Controls
 *
 * Provides functions to start, stop, pause, and resume orchestrations
 */

import { apiClient, getAuthHeaders } from './api';

/**
 * Request to start a new orchestrator
 */
export interface StartOrchestratorRequest {
  prompt_file: string;
  max_iterations?: number;
  max_runtime?: number;
  auto_commit?: boolean;
}

/**
 * Response from starting an orchestrator
 */
export interface StartOrchestratorResponse {
  instance_id: string;
  status: string;
  port?: number;
}

/**
 * Start a new orchestrator
 */
export async function startOrchestrator(
  request: StartOrchestratorRequest
): Promise<StartOrchestratorResponse> {
  const authHeaders = await getAuthHeaders();

  const response = await fetch(`${apiClient.baseURL}/api/orchestrators`, {
    method: 'POST',
    headers: {
      ...apiClient.defaultHeaders,
      ...authHeaders,
    },
    body: JSON.stringify(request),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.detail || 'Failed to start orchestrator');
  }

  return response.json();
}
