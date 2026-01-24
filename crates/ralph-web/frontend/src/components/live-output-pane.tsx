/**
 * LiveOutputPane component - streaming output with auto-scroll.
 *
 * Features:
 * - Auto-scrolls to bottom when new content arrives
 * - Stops auto-scroll when user scrolls up
 * - "Jump to bottom" button when not following
 */

import { useEffect, useRef, useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface LiveOutputPaneProps {
  lines: string[];
  className?: string;
}

export function LiveOutputPane({ lines, className }: LiveOutputPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isFollowing, setIsFollowing] = useState(true);
  const [showJumpButton, setShowJumpButton] = useState(false);
  const lastLineCountRef = useRef(0);

  // Handle scroll events to detect user scrolling up
  const handleScroll = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    const { scrollTop, scrollHeight, clientHeight } = container;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

    if (isAtBottom) {
      setIsFollowing(true);
      setShowJumpButton(false);
    } else {
      setIsFollowing(false);
      setShowJumpButton(true);
    }
  }, []);

  // Auto-scroll when new lines arrive and following is enabled
  useEffect(() => {
    if (isFollowing && lines.length > lastLineCountRef.current) {
      const container = containerRef.current;
      if (container) {
        container.scrollTop = container.scrollHeight;
      }
    }
    lastLineCountRef.current = lines.length;
  }, [lines, isFollowing]);

  // Jump to bottom handler
  const jumpToBottom = useCallback(() => {
    const container = containerRef.current;
    if (container) {
      container.scrollTop = container.scrollHeight;
      setIsFollowing(true);
      setShowJumpButton(false);
    }
  }, []);

  return (
    <div className={cn('relative', className)} data-testid="live-output-pane">
      {/* Output container */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="h-full overflow-y-auto bg-muted/30 font-mono text-sm p-4 rounded-md"
        data-testid="output-container"
      >
        {lines.length === 0 ? (
          <div className="text-muted-foreground italic" data-testid="output-empty">
            Waiting for output...
          </div>
        ) : (
          lines.map((line, index) => (
            <div
              key={index}
              className="py-0.5 whitespace-pre-wrap break-all"
              data-testid={`output-line-${index}`}
            >
              {line || '\u00A0'}
            </div>
          ))
        )}
      </div>

      {/* Following indicator */}
      <div
        className={cn(
          'absolute bottom-4 right-4 flex items-center gap-2 transition-opacity',
          isFollowing ? 'opacity-100' : 'opacity-50'
        )}
      >
        {isFollowing && (
          <span
            className="text-xs text-muted-foreground bg-background/80 px-2 py-1 rounded"
            data-testid="following-indicator"
          >
            Following
          </span>
        )}
      </div>

      {/* Jump to bottom button */}
      {showJumpButton && (
        <Button
          onClick={jumpToBottom}
          size="sm"
          variant="secondary"
          className="absolute bottom-4 left-1/2 transform -translate-x-1/2 shadow-lg"
          data-testid="jump-to-bottom"
        >
          Jump to bottom
        </Button>
      )}
    </div>
  );
}
