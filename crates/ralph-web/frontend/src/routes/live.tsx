/**
 * Live monitoring page - real-time loop status with streaming output.
 *
 * Features:
 * - Multi-loop support with LoopSwitcher
 * - Iteration navigation (prev/next like TUI)
 * - Live vs historical view toggle
 * - Auto-scroll with following toggle
 * - Task sidebar with event stream
 */

import { useState, useCallback, useMemo, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { LoopSwitcher, type ActiveLoopInfo } from '@/components/loop-switcher';
import { LoopStatus } from '@/components/loop-status';
import { IterationNav } from '@/components/iteration-nav';
import { LiveOutputPane } from '@/components/live-output-pane';
import { TaskSidebar } from '@/components/task-sidebar';
import { ControlBar } from '@/components/control-bar';
import { useLoopWebSocket } from '@/hooks/use-loop-websocket';
import { api } from '@/lib/api';

export function LiveRoute() {
  const { sessionId } = useParams<{ sessionId?: string }>();
  const navigate = useNavigate();

  // Fetch active loops from API
  const {
    data: activeLoops = [],
    isLoading: loopsLoading,
  } = useQuery({
    queryKey: ['activeLoops'],
    queryFn: () => api.listActiveLoops(),
    refetchInterval: 5000, // Poll every 5 seconds for new loops
  });

  // Local state for viewing iteration (null = live)
  const [viewingIteration, setViewingIteration] = useState<number | null>(null);

  // Determine which session to connect to
  const currentSessionId = sessionId || activeLoops[0]?.session_id || null;

  // Connect to WebSocket for current session
  const ws = useLoopWebSocket(currentSessionId ?? undefined);

  // State for initial content loaded from files (before WebSocket connected)
  const [initialOutput, setInitialOutput] = useState<string[]>([]);
  const [initialLoaded, setInitialLoaded] = useState(false);

  // Fetch session data to get current state and initial content
  const { data: sessionData } = useQuery({
    queryKey: ['session', currentSessionId],
    queryFn: () => currentSessionId ? api.getSession(currentSessionId) : Promise.resolve(null),
    enabled: !!currentSessionId,
    // Only fetch once when session changes
    staleTime: Infinity,
  });

  // Fetch initial content for the current iteration when session data is available
  useEffect(() => {
    async function loadInitialContent() {
      if (!currentSessionId || !sessionData || initialLoaded) return;

      // Get the latest iteration number from session data
      const latestIteration = sessionData.iterations.length > 0
        ? Math.max(...sessionData.iterations.map(i => i.number))
        : 1;

      // Initialize WebSocket state from session data
      const latestIterationData = sessionData.iterations.find(i => i.number === latestIteration);
      const isRunning = sessionData.status === 'running';
      ws.initializeFromSession(
        latestIteration,
        latestIterationData?.hat?.id ?? null,
        isRunning
      );

      try {
        const content = await api.getIterationContent(currentSessionId, latestIteration);
        if (content && content.lines.length > 0) {
          setInitialOutput(content.lines.map(line => line.text));
        }
        setInitialLoaded(true);
      } catch {
        // If fetch fails, just mark as loaded so we don't retry
        setInitialLoaded(true);
      }
    }

    loadInitialContent();
  }, [currentSessionId, sessionData, initialLoaded, ws]);

  // Reset initial loaded state when session changes
  useEffect(() => {
    setInitialLoaded(false);
    setInitialOutput([]);
  }, [currentSessionId]);

  // Fetch historical content when viewing past iteration
  const {
    data: historicalContent,
    isLoading: historyLoading,
  } = useQuery({
    queryKey: ['iterationContent', currentSessionId, viewingIteration],
    queryFn: () =>
      currentSessionId && viewingIteration !== null
        ? api.getIterationContent(currentSessionId, viewingIteration)
        : Promise.resolve(null),
    enabled: !!currentSessionId && viewingIteration !== null,
  });

  // Is viewing live or historical?
  const isLive = viewingIteration === null;

  // Output lines to display (live or historical)
  // Merge initial content with WebSocket output, avoiding duplicates
  const displayLines = useMemo(() => {
    if (isLive) {
      // If WebSocket has output, use it; otherwise use initial loaded content
      if (ws.output.length > 0) {
        return ws.output;
      }
      return initialOutput;
    }
    if (historicalContent) {
      return historicalContent.lines.map((line) => line.text);
    }
    return [];
  }, [isLive, ws.output, historicalContent, initialOutput]);

  // Transform active loops for LoopSwitcher
  const loopInfos: ActiveLoopInfo[] = useMemo(() => {
    return activeLoops.map((loop) => ({
      sessionId: loop.session_id,
      configName: extractConfigName(loop.config_path),
      hasNewOutput: false, // Could be enhanced with store tracking
    }));
  }, [activeLoops]);

  // Handle loop selection
  const handleLoopSelect = useCallback(
    (selectedSessionId: string) => {
      navigate(`/live/${selectedSessionId}`);
      setViewingIteration(null); // Reset to live view
    },
    [navigate]
  );

  // Iteration navigation handlers
  const handlePrevIteration = useCallback(() => {
    const current = viewingIteration ?? ws.iteration;
    if (current > 1) {
      setViewingIteration(current - 1);
    }
  }, [viewingIteration, ws.iteration]);

  const handleNextIteration = useCallback(() => {
    const current = viewingIteration ?? ws.iteration;
    if (current < ws.iteration) {
      setViewingIteration(current + 1);
    } else {
      // At or past current iteration, go to live
      setViewingIteration(null);
    }
  }, [viewingIteration, ws.iteration]);

  const handleJumpToIteration = useCallback((iter: number) => {
    if (iter === ws.iteration) {
      setViewingIteration(null); // Jump to live
    } else {
      setViewingIteration(iter);
    }
  }, [ws.iteration]);

  const handleJumpToLive = useCallback(() => {
    setViewingIteration(null);
  }, []);

  // Current iteration for display
  const displayIteration = viewingIteration ?? ws.iteration;

  // Empty state
  if (!loopsLoading && activeLoops.length === 0 && !ws.isConnected) {
    return (
      <div className="space-y-4" data-testid="live-dashboard">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">Live Monitoring</h1>
          <ControlBar
            state="idle"
            onClear={() => {}}
            isStopping={false}
          />
        </div>
        <Card>
          <CardHeader>
            <CardTitle>Output</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-muted-foreground mb-4">
              There are no orchestration loops currently running.
            </p>
            <Button onClick={() => navigate('/start')}>
              Start New Loop
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col space-y-4" data-testid="live-dashboard">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-4">
        <div className="flex items-center gap-4">
          <h1 className="text-2xl font-bold">Live Monitoring</h1>

          {/* Live/Historical badge */}
          {ws.iteration > 0 && (
            <Badge
              variant={isLive ? 'default' : 'secondary'}
              data-testid="view-mode-badge"
            >
              {isLive ? 'Live' : `Viewing iteration ${viewingIteration}`}
            </Badge>
          )}
        </div>

        {/* Loop switcher for multiple loops */}
        {loopInfos.length > 1 && (
          <LoopSwitcher
            loops={loopInfos}
            activeSessionId={currentSessionId}
            onSelect={handleLoopSelect}
          />
        )}
      </div>

      {/* Status bar */}
      <LoopStatus
        iteration={ws.iteration}
        elapsedSeconds={ws.elapsedSeconds}
        activeHat={ws.activeHat}
        state={ws.state}
        isConnected={ws.isConnected}
      />

      {/* Iteration navigation and controls */}
      {ws.iteration > 0 && (
        <div className="flex items-center justify-between flex-wrap gap-4">
          <IterationNav
            current={displayIteration}
            total={ws.iteration}
            onPrev={handlePrevIteration}
            onNext={handleNextIteration}
            onJump={handleJumpToIteration}
          />

          <div className="flex items-center gap-2">
            {/* Jump to live button when viewing history */}
            {!isLive && (
              <Button
                variant="outline"
                size="sm"
                onClick={handleJumpToLive}
                data-testid="jump-to-live"
              >
                <LiveIcon className="w-4 h-4 mr-1 text-green-500" />
                Jump to Live
              </Button>
            )}

            <ControlBar
              state={ws.state}
              onStop={ws.stop}
              onClear={ws.clear}
              isStopping={ws.isStopping}
            />
          </div>
        </div>
      )}

      {/* Main content area */}
      <div className="flex-1 grid grid-cols-1 lg:grid-cols-4 gap-4 min-h-0">
        {/* Output pane (takes 3/4 on large screens) */}
        <div className="lg:col-span-3 min-h-[400px] lg:min-h-0">
          {historyLoading && !isLive ? (
            <Card className="h-full flex items-center justify-center">
              <CardContent>
                <p className="text-muted-foreground">Loading iteration content...</p>
              </CardContent>
            </Card>
          ) : (
            <LiveOutputPane lines={displayLines} className="h-full" />
          )}
        </div>

        {/* Task sidebar (takes 1/4 on large screens) */}
        <div className="lg:col-span-1">
          <TaskSidebar events={ws.events} />
        </div>
      </div>

      {/* Error display */}
      {ws.error && (
        <Card className="border-red-500">
          <CardContent className="py-3">
            <p className="text-red-600 dark:text-red-400 text-sm">{ws.error}</p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

/** Extract config name from path */
function extractConfigName(configPath: string): string {
  const parts = configPath.split('/');
  const filename = parts[parts.length - 1] || configPath;
  return filename.replace(/\.(yml|yaml)$/, '');
}

/** Live indicator icon */
function LiveIcon({ className }: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="currentColor"
      className={className}
    >
      <circle cx="12" cy="12" r="6" />
    </svg>
  );
}
