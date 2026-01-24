/**
 * LoopSwitcher component - tabs/dropdown to switch between active loops.
 *
 * Shows config name for each active loop and indicates when new output arrives.
 */

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface ActiveLoopInfo {
  sessionId: string;
  configName: string;
  hasNewOutput?: boolean;
}

interface LoopSwitcherProps {
  loops: ActiveLoopInfo[];
  activeSessionId: string | null;
  onSelect: (sessionId: string) => void;
  className?: string;
}

export function LoopSwitcher({
  loops,
  activeSessionId,
  onSelect,
  className,
}: LoopSwitcherProps) {
  if (loops.length === 0) {
    return null;
  }

  if (loops.length === 1) {
    return (
      <div className={cn('text-sm text-muted-foreground', className)} data-testid="loop-switcher">
        <span className="font-medium">{loops[0].configName}</span>
      </div>
    );
  }

  return (
    <div className={cn('flex items-center gap-1', className)} data-testid="loop-switcher">
      {loops.map((loop) => (
        <Button
          key={loop.sessionId}
          variant={loop.sessionId === activeSessionId ? 'default' : 'outline'}
          size="sm"
          onClick={() => onSelect(loop.sessionId)}
          className={cn('relative', loop.hasNewOutput && loop.sessionId !== activeSessionId && 'ring-2 ring-blue-500')}
          data-testid={`loop-tab-${loop.sessionId}`}
        >
          {loop.configName}
          {loop.hasNewOutput && loop.sessionId !== activeSessionId && (
            <span
              className="absolute -top-1 -right-1 w-2 h-2 bg-blue-500 rounded-full"
              data-testid={`new-output-indicator-${loop.sessionId}`}
            />
          )}
        </Button>
      ))}
    </div>
  );
}

export type { ActiveLoopInfo };
