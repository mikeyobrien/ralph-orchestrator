/**
 * IterationNav component - navigation between iterations
 */

import { Button } from '@/components/ui/button';
import { ChevronLeft, ChevronRight } from 'lucide-react';

interface IterationNavProps {
  current: number;
  total: number;
  onPrev: () => void;
  onNext: () => void;
  onJump: (iteration: number) => void;
}

export function IterationNav({
  current,
  total,
  onPrev,
  onNext,
  onJump,
}: IterationNavProps) {
  return (
    <div className="flex items-center gap-2" data-testid="iteration-nav">
      <Button
        variant="outline"
        size="sm"
        onClick={onPrev}
        disabled={current <= 1}
        aria-label="prev"
      >
        <ChevronLeft className="h-4 w-4" />
        Prev
      </Button>

      <div className="flex items-center gap-1">
        <span className="text-sm font-medium">Iteration {current}</span>
        <span className="text-sm text-muted-foreground">/ {total}</span>
      </div>

      <Button
        variant="outline"
        size="sm"
        onClick={onNext}
        disabled={current >= total}
        aria-label="next"
      >
        Next
        <ChevronRight className="h-4 w-4" />
      </Button>

      {total > 5 && (
        <select
          className="ml-2 h-8 rounded-md border border-input bg-background px-2 text-sm"
          value={current}
          onChange={(e) => onJump(Number(e.target.value))}
          aria-label="jump to iteration"
        >
          {Array.from({ length: total }, (_, i) => i + 1).map((num) => (
            <option key={num} value={num}>
              Iteration {num}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}
