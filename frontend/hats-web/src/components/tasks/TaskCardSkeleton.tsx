/**
 * TaskCardSkeleton Component
 *
 * A skeleton loading placeholder that matches the two-row TaskCard structure.
 * No animation per spec, just static gray placeholder rectangles.
 * Used during loading states before task data is available.
 */

import { cn } from "@/lib/utils";

interface TaskCardSkeletonProps {
  /** Additional CSS classes */
  className?: string;
}

export function TaskCardSkeleton({ className }: TaskCardSkeletonProps) {
  return (
    <div
      data-testid="task-card-skeleton"
      aria-hidden="true"
      className={cn(
        "rounded-lg border border-border bg-card p-4 space-y-3",
        className
      )}
    >
      {/* Row 1: Icon + Title */}
      <div
        data-testid="skeleton-row-1"
        className="flex items-center gap-3"
      >
        {/* Icon placeholder */}
        <div
          data-testid="skeleton-icon"
          className="h-5 w-5 rounded bg-muted shrink-0"
        />
        {/* Title placeholder */}
        <div
          data-testid="skeleton-title"
          className="h-4 w-3/4 rounded bg-muted"
        />
      </div>

      {/* Row 2: Badges + Time */}
      <div
        data-testid="skeleton-row-2"
        className="flex items-center justify-between"
      >
        {/* Badge placeholders */}
        <div className="flex items-center gap-2">
          <div
            data-testid="skeleton-badge-1"
            className="h-5 w-16 rounded-full bg-muted/50"
          />
          <div
            data-testid="skeleton-badge-2"
            className="h-5 w-12 rounded-full bg-muted/50"
          />
        </div>
        {/* Time placeholder */}
        <div
          data-testid="skeleton-time"
          className="h-3 w-14 rounded bg-muted/50"
        />
      </div>
    </div>
  );
}
