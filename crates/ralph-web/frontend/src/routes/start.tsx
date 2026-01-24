/**
 * Start loop page - form to kick off new orchestration loops
 *
 * Features:
 * - Config selection with grouped dropdown (local files + presets)
 * - Prompt textarea with Cmd/Ctrl+Enter submit
 * - Working directory input with validation
 * - Redirect to /live on successful start
 */

import { useState, useCallback, type KeyboardEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Input } from '@/components/ui/input';
import { Select } from '@/components/ui/select';
import { api, type ConfigGroup, type ConfigOption } from '@/lib/api';

export function StartRoute() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // Form state - default working dir to current directory
  const [selectedConfig, setSelectedConfig] = useState<string>('');
  const [prompt, setPrompt] = useState<string>('');
  const [workingDir, setWorkingDir] = useState<string>('.');
  const [dirError, setDirError] = useState<string>('');
  // Track whether form has been submitted to show validation errors
  const [submitted, setSubmitted] = useState<boolean>(false);

  // Fetch available configs
  const {
    data: configsResponse,
    isLoading: configsLoading,
    error: configsError,
  } = useQuery({
    queryKey: ['configs', workingDir],
    queryFn: () => api.listConfigs(workingDir || undefined),
    enabled: true,
  });

  // Start loop mutation
  const startLoopMutation = useMutation({
    mutationFn: async () => {
      if (!selectedConfig || !prompt.trim()) {
        throw new Error('Please select a config and enter a prompt');
      }

      return api.startLoop({
        config_path: selectedConfig,
        prompt: prompt.trim(),
        working_dir: workingDir || '.',
      });
    },
    onSuccess: (data) => {
      // Invalidate active loops query
      queryClient.invalidateQueries({ queryKey: ['activeLoops'] });
      // Navigate to live view with the new session
      navigate(`/live/${data.session_id}`);
    },
  });

  // Handle form submission
  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      setSubmitted(true);
      if (selectedConfig && prompt.trim() && !dirError) {
        startLoopMutation.mutate();
      }
    },
    [startLoopMutation, selectedConfig, prompt, dirError]
  );

  // Handle Cmd/Ctrl+Enter in textarea
  const handlePromptKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        if (selectedConfig && prompt.trim() && !startLoopMutation.isPending) {
          startLoopMutation.mutate();
        }
      }
    },
    [selectedConfig, prompt, startLoopMutation]
  );

  // Validate working directory (basic check)
  const handleWorkingDirChange = useCallback((value: string) => {
    setWorkingDir(value);
    // Basic validation - could be enhanced with server-side check
    if (value && value.includes('..')) {
      setDirError('Path cannot contain ".."');
    } else if (value && value.startsWith('/') && value.length > 100) {
      setDirError('Path seems too long');
    } else {
      setDirError('');
    }
  }, []);

  return (
    <div className="space-y-4 max-w-2xl">
      <h1 className="text-2xl font-bold">Start Loop</h1>

      <form onSubmit={handleSubmit}>
        <Card>
          <CardHeader>
            <CardTitle>Kickoff Configuration</CardTitle>
            <CardDescription>
              Configure and start a new orchestration loop. Use{' '}
              <kbd className="px-1.5 py-0.5 text-xs border rounded bg-muted">
                {navigator.platform.includes('Mac') ? '⌘' : 'Ctrl'}+Enter
              </kbd>{' '}
              in the prompt field to submit.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {/* Config Select */}
            <div className="space-y-2">
              <Label htmlFor="config">Configuration</Label>
              {configsError && (
                <div className="text-sm text-red-500">
                  Failed to load configs: {String(configsError)}
                </div>
              )}
              <Select
                id="config"
                data-testid="config-select"
                value={selectedConfig}
                onChange={(e) => setSelectedConfig(e.target.value)}
                aria-describedby="config-description"
                disabled={configsLoading}
              >
                <option value="">{configsLoading ? 'Loading configs...' : 'Select a configuration...'}</option>
                {configsResponse?.groups.map((group: ConfigGroup) => (
                  <optgroup key={group.source} label={group.source}>
                    {group.configs.map((config: ConfigOption) => (
                      <option key={config.path} value={config.path}>
                        {config.name}
                        {config.description ? ` - ${config.description}` : ''}
                      </option>
                    ))}
                  </optgroup>
                ))}
              </Select>
              {submitted && !selectedConfig && (
                <p className="text-xs text-red-500">Configuration is required</p>
              )}
              <p id="config-description" className="text-xs text-muted-foreground">
                Choose a local config file or a built-in preset
              </p>
            </div>

            {/* Prompt Input */}
            <div className="space-y-2">
              <Label htmlFor="prompt">Prompt</Label>
              <Textarea
                id="prompt"
                data-testid="prompt-input"
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                onKeyDown={handlePromptKeyDown}
                placeholder="Describe what you want Ralph to accomplish..."
                className="min-h-[150px] font-mono text-sm"
                aria-describedby="prompt-description"
              />
              {submitted && !prompt.trim() && (
                <p className="text-xs text-red-500">Prompt is required</p>
              )}
              <p id="prompt-description" className="text-xs text-muted-foreground">
                Enter the task or objective for the orchestration loop
              </p>
            </div>

            {/* Working Directory */}
            <div className="space-y-2">
              <Label htmlFor="working-dir">Working Directory</Label>
              <Input
                id="working-dir"
                data-testid="working-dir-input"
                type="text"
                value={workingDir}
                onChange={(e) => handleWorkingDirChange(e.target.value)}
                placeholder="."
                aria-invalid={!!dirError}
                aria-describedby="dir-description dir-error"
              />
              {dirError && (
                <p id="dir-error" className="text-xs text-red-500">
                  {dirError}
                </p>
              )}
              <p id="dir-description" className="text-xs text-muted-foreground">
                Directory where Ralph will run (relative paths are relative to the server)
              </p>
            </div>

            {/* Selected config info */}
            {selectedConfig && (
              <div className="p-3 rounded-md bg-muted/50 text-sm">
                <span className="font-medium">Selected: </span>
                <code className="text-xs bg-muted px-1 py-0.5 rounded">{selectedConfig}</code>
              </div>
            )}

            {/* Error display */}
            {startLoopMutation.error && (
              <div className="p-3 rounded-md bg-red-100 dark:bg-red-900/20 text-red-700 dark:text-red-300 text-sm">
                {String(startLoopMutation.error)}
              </div>
            )}

            {/* Submit button */}
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                onClick={() => navigate('/live')}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                data-testid="submit-button"
                disabled={startLoopMutation.isPending}
              >
                {startLoopMutation.isPending ? 'Starting...' : 'Start Loop'}
              </Button>
            </div>
          </CardContent>
        </Card>
      </form>
    </div>
  );
}
