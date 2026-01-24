/**
 * TaskSidebar component - displays task counts and event stream.
 *
 * Note: Task API integration is deferred until Step 9 (Loop Manager).
 * For now, this component displays placeholder task counts and the event stream.
 */

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import type { LoopEvent } from '@/hooks/use-loop-websocket';
import { cn } from '@/lib/utils';

interface TaskCounts {
  pending: number;
  inProgress: number;
  completed: number;
}

interface TaskSidebarProps {
  taskCounts?: TaskCounts;
  events: LoopEvent[];
  className?: string;
}

/** Format timestamp for display */
function formatTime(date: Date): string {
  return date.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });
}

/** Get event topic color */
function getEventColor(topic: string): string {
  if (topic.startsWith('build.')) return 'text-blue-600 dark:text-blue-400';
  if (topic.startsWith('task.')) return 'text-purple-600 dark:text-purple-400';
  if (topic.startsWith('validation.')) return 'text-green-600 dark:text-green-400';
  if (topic.startsWith('commit.')) return 'text-orange-600 dark:text-orange-400';
  if (topic.includes('error') || topic.includes('failed')) return 'text-red-600 dark:text-red-400';
  return 'text-muted-foreground';
}

export function TaskSidebar({ taskCounts, events, className }: TaskSidebarProps) {
  // Default counts when API is not available
  const counts = taskCounts || { pending: 0, inProgress: 0, completed: 0 };
  const total = counts.pending + counts.inProgress + counts.completed;

  return (
    <div className={cn('space-y-4', className)} data-testid="task-sidebar">
      {/* Task Counts */}
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm">Tasks</CardTitle>
        </CardHeader>
        <CardContent>
          {total === 0 ? (
            <p className="text-sm text-muted-foreground italic">No tasks</p>
          ) : (
            <div className="space-y-2">
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">Pending</span>
                <span className="font-mono font-medium" data-testid="task-pending">
                  {counts.pending}
                </span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">In Progress</span>
                <span className="font-mono font-medium text-blue-600 dark:text-blue-400" data-testid="task-in-progress">
                  {counts.inProgress}
                </span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">Completed</span>
                <span className="font-mono font-medium text-green-600 dark:text-green-400" data-testid="task-completed">
                  {counts.completed}
                </span>
              </div>
              {/* Progress bar */}
              <div className="h-2 bg-muted rounded-full overflow-hidden mt-2" data-testid="task-progress">
                <div
                  className="h-full bg-green-500 transition-all duration-300"
                  style={{ width: `${total > 0 ? (counts.completed / total) * 100 : 0}%` }}
                />
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Event Stream */}
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm">Events</CardTitle>
        </CardHeader>
        <CardContent>
          <div
            className="max-h-64 overflow-y-auto space-y-1"
            data-testid="event-stream"
          >
            {events.length === 0 ? (
              <p className="text-sm text-muted-foreground italic">No events yet</p>
            ) : (
              events.slice(-20).reverse().map((event, index) => (
                <div
                  key={`${event.timestamp.getTime()}-${index}`}
                  className="flex items-start gap-2 text-xs py-1 border-b border-border last:border-0"
                  data-testid={`event-${index}`}
                >
                  <span className="text-muted-foreground font-mono shrink-0">
                    {formatTime(event.timestamp)}
                  </span>
                  <span className={cn('font-medium', getEventColor(event.topic))}>
                    {event.topic}
                  </span>
                </div>
              ))
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
