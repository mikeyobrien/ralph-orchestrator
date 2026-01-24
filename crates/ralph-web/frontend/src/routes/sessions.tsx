/**
 * Sessions list page - displays all past orchestration sessions
 */

import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { SessionCard } from '@/components/session-card';
import { api, type SessionStatus, type SessionSummary } from '@/lib/api';
import { cn } from '@/lib/utils';

const statusFilters: Array<{ value: SessionStatus | 'all'; label: string }> = [
  { value: 'all', label: 'All' },
  { value: 'completed', label: 'Completed' },
  { value: 'running', label: 'Running' },
  { value: 'failed', label: 'Failed' },
  { value: 'cancelled', label: 'Cancelled' },
];

function SessionsLoading() {
  return (
    <div className="space-y-4" data-testid="sessions-loading">
      {[1, 2, 3].map((i) => (
        <Card key={i}>
          <CardHeader className="pb-2">
            <div className="flex items-start justify-between gap-4">
              <Skeleton className="h-5 w-40" />
              <Skeleton className="h-5 w-20 rounded-full" />
            </div>
          </CardHeader>
          <CardContent>
            <Skeleton className="h-4 w-24" />
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

function SessionsEmpty() {
  return (
    <Card data-testid="sessions-empty">
      <CardContent className="py-8 text-center">
        <p className="text-muted-foreground">No sessions found.</p>
        <p className="text-sm text-muted-foreground mt-1">
          Start a new loop to see sessions here.
        </p>
      </CardContent>
    </Card>
  );
}

function SessionsError({ error }: { error: Error }) {
  return (
    <Card data-testid="sessions-error">
      <CardContent className="py-8 text-center">
        <p className="text-destructive font-medium">Failed to load sessions</p>
        <p className="text-sm text-muted-foreground mt-1">{error.message}</p>
      </CardContent>
    </Card>
  );
}

interface StatusFilterProps {
  value: SessionStatus | 'all';
  onChange: (value: SessionStatus | 'all') => void;
}

function StatusFilter({ value, onChange }: StatusFilterProps) {
  return (
    <div className="flex gap-2 flex-wrap" data-testid="status-filter">
      {statusFilters.map((filter) => (
        <Button
          key={filter.value}
          variant={value === filter.value ? 'default' : 'outline'}
          size="sm"
          onClick={() => onChange(filter.value)}
          className={cn(
            value === filter.value && 'pointer-events-none'
          )}
        >
          {filter.label}
        </Button>
      ))}
    </div>
  );
}

function sortSessionsByDate(sessions: SessionSummary[]): SessionSummary[] {
  return [...sessions].sort((a, b) => {
    return new Date(b.started_at).getTime() - new Date(a.started_at).getTime();
  });
}

export function SessionsRoute() {
  const [statusFilter, setStatusFilter] = useState<SessionStatus | 'all'>('all');

  const {
    data: sessions,
    isLoading,
    error,
  } = useQuery({
    queryKey: ['sessions'],
    queryFn: () => api.listSessions(),
  });

  const filteredSessions = useMemo(() => {
    if (!sessions) return [];

    const filtered =
      statusFilter === 'all'
        ? sessions
        : sessions.filter((s) => s.status === statusFilter);

    return sortSessionsByDate(filtered);
  }, [sessions, statusFilter]);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Sessions</h1>
        <p className="text-muted-foreground">Browse past orchestration sessions</p>
      </div>

      <StatusFilter value={statusFilter} onChange={setStatusFilter} />

      {isLoading && <SessionsLoading />}

      {error && <SessionsError error={error as Error} />}

      {!isLoading && !error && filteredSessions.length === 0 && <SessionsEmpty />}

      {!isLoading && !error && filteredSessions.length > 0 && (
        <div className="space-y-4">
          {filteredSessions.map((session) => (
            <SessionCard key={session.id} session={session} />
          ))}
        </div>
      )}
    </div>
  );
}
