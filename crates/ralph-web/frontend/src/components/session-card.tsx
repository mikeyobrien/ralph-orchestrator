/**
 * SessionCard component - displays a session summary in a card format
 */

import { Link } from 'react-router-dom';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import type { SessionSummary, SessionStatus } from '@/lib/api';
import { cn } from '@/lib/utils';

interface SessionCardProps {
  session: SessionSummary;
}

const statusStyles: Record<SessionStatus, string> = {
  running: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
  completed: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
  failed: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
  cancelled: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
};

function StatusBadge({ status }: { status: SessionStatus }) {
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium',
        statusStyles[status]
      )}
    >
      {status}
    </span>
  );
}

export function SessionCard({ session }: SessionCardProps) {
  const iterationText =
    session.iteration_count === 1
      ? '1 iteration'
      : `${session.iteration_count} iterations`;

  return (
    <Link
      to={`/sessions/${session.id}`}
      className="block transition-colors hover:bg-accent/50 rounded-xl"
      data-testid="session-card"
    >
      <Card className="hover:border-primary/50 transition-colors">
        <CardHeader className="pb-2">
          <div className="flex items-start justify-between gap-4">
            <CardTitle className="text-base font-medium">
              {session.started_at}
            </CardTitle>
            <StatusBadge status={session.status} />
          </div>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">{iterationText}</p>
        </CardContent>
      </Card>
    </Link>
  );
}
