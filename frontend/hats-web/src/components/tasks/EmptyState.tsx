/**
 * EmptyState Component
 *
 * A reusable empty state component that displays a centered message
 * with an icon, title, and description. Useful for showing empty lists,
 * no results states, or placeholder content.
 */

import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  /** Lucide icon component to display */
  icon: LucideIcon;
  /** Main title text */
  title: string;
  /** Description text below the title */
  description: string;
  /** Additional CSS classes */
  className?: string;
  /** Optional children (e.g., action buttons) */
  children?: React.ReactNode;
}

export function EmptyState({
  icon: Icon,
  title,
  description,
  className,
  children,
}: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center py-12 text-center",
        className
      )}
    >
      <Icon className="h-12 w-12 text-muted-foreground mb-4" />
      <p className="text-lg font-medium mb-1">{title}</p>
      <p className="text-sm text-muted-foreground">{description}</p>
      {children && <div className="mt-4">{children}</div>}
    </div>
  );
}
