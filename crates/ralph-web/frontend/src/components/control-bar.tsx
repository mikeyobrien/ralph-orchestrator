/**
 * ControlBar component - control buttons for loop management.
 *
 * Provides stop and clear functionality for managing orchestration loops.
 */

import { Button } from '@/components/ui/button';
import type { LoopState } from '@/hooks/use-loop-websocket';
import { cn } from '@/lib/utils';

interface ControlBarProps {
  state: LoopState;
  onStop?: () => void;
  onClear: () => void;
  isStopping?: boolean;
  className?: string;
}

export function ControlBar({ state, onStop, onClear, isStopping = false, className }: ControlBarProps) {
  const isRunning = state === 'running';
  const canStop = isRunning && onStop !== undefined && !isStopping;

  return (
    <div className={cn('flex items-center gap-2', className)} data-testid="control-bar">
      {/* Stop button */}
      <Button
        variant="destructive"
        size="sm"
        onClick={onStop}
        disabled={!canStop}
        data-testid="stop-button"
      >
        {isStopping ? (
          <SpinnerIcon className="w-4 h-4 mr-1 animate-spin" />
        ) : (
          <StopIcon className="w-4 h-4 mr-1" />
        )}
        {isStopping ? 'Stopping...' : 'Stop'}
      </Button>

      {/* Clear button */}
      <Button
        variant="outline"
        size="sm"
        onClick={onClear}
        disabled={isRunning}
        data-testid="clear-button"
      >
        <ClearIcon className="w-4 h-4 mr-1" />
        Clear
      </Button>
    </div>
  );
}

// Simple icon components
function StopIcon({ className }: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="currentColor"
      className={className}
    >
      <rect x="6" y="6" width="12" height="12" rx="1" />
    </svg>
  );
}

function ClearIcon({ className }: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      <path d="M3 6h18" />
      <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
      <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
    </svg>
  );
}

function SpinnerIcon({ className }: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      <path d="M21 12a9 9 0 1 1-6.219-8.56" />
    </svg>
  );
}
