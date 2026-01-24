/**
 * LoopStatus component - displays iteration count, elapsed time, and active hat.
 */

import { Card, CardContent } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import type { LoopState } from '@/hooks/use-loop-websocket';

interface LoopStatusProps {
  iteration: number;
  elapsedSeconds: number;
  activeHat: string | null;
  state: LoopState;
  isConnected: boolean;
}

/** Format seconds as MM:SS */
function formatElapsed(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

/** Get hat emoji from hat name */
function getHatEmoji(hat: string): string {
  const emojiMap: Record<string, string> = {
    ralph: '🎩',
    planner: '📋',
    builder: '⚙️',
    committer: '📦',
    validator: '✅',
  };
  return emojiMap[hat.toLowerCase()] || '🎭';
}

/** Get state indicator styles */
function getStateStyles(state: LoopState, isConnected: boolean): { bg: string; text: string } {
  if (!isConnected) {
    return { bg: 'bg-gray-100 dark:bg-gray-800', text: 'text-gray-600 dark:text-gray-400' };
  }

  switch (state) {
    case 'running':
      return { bg: 'bg-blue-100 dark:bg-blue-900', text: 'text-blue-700 dark:text-blue-300' };
    case 'completed':
      return { bg: 'bg-green-100 dark:bg-green-900', text: 'text-green-700 dark:text-green-300' };
    case 'error':
      return { bg: 'bg-red-100 dark:bg-red-900', text: 'text-red-700 dark:text-red-300' };
    default:
      return { bg: 'bg-gray-100 dark:bg-gray-800', text: 'text-gray-600 dark:text-gray-400' };
  }
}

export function LoopStatus({
  iteration,
  elapsedSeconds,
  activeHat,
  state,
  isConnected,
}: LoopStatusProps) {
  const stateStyles = getStateStyles(state, isConnected);

  return (
    <Card data-testid="loop-status">
      <CardContent className="py-4">
        <div className="flex items-center justify-between gap-4 flex-wrap">
          {/* Connection and state indicator */}
          <div className="flex items-center gap-2">
            <div
              className={cn(
                'w-2 h-2 rounded-full',
                isConnected ? 'bg-green-500 animate-pulse' : 'bg-gray-400'
              )}
              data-testid="connection-indicator"
            />
            <span
              className={cn(
                'text-sm font-medium px-2 py-0.5 rounded-full',
                stateStyles.bg,
                stateStyles.text
              )}
              data-testid="loop-state"
            >
              {isConnected ? state : 'disconnected'}
            </span>
          </div>

          {/* Iteration counter */}
          <div className="flex items-center gap-2" data-testid="iteration-counter">
            <span className="text-sm text-muted-foreground">Iteration</span>
            <span className="text-lg font-mono font-bold">{iteration}</span>
          </div>

          {/* Elapsed time */}
          <div className="flex items-center gap-2" data-testid="elapsed-time">
            <span className="text-sm text-muted-foreground">Elapsed</span>
            <span className="text-lg font-mono">{formatElapsed(elapsedSeconds)}</span>
          </div>

          {/* Active hat */}
          {activeHat && (
            <div className="flex items-center gap-2" data-testid="active-hat">
              <span className="text-xl" role="img" aria-label="hat">
                {getHatEmoji(activeHat)}
              </span>
              <span className="text-sm font-medium capitalize">{activeHat}</span>
            </div>
          )}

          {/* Placeholder when no hat */}
          {!activeHat && state === 'idle' && (
            <div className="text-sm text-muted-foreground italic">
              Waiting for loop to start...
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
