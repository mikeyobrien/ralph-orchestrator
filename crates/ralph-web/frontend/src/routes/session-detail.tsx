/**
 * Session detail page - displays session with iteration browser and search
 */

import { useState, useCallback, useMemo } from 'react';
import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { IterationNav } from '@/components/iteration-nav';
import { OutputPane } from '@/components/output-pane';
import { SearchBar } from '@/components/search-bar';
import { api, type SessionStatus, type OutputLine } from '@/lib/api';
import { cn } from '@/lib/utils';

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

function SessionDetailLoading() {
  return (
    <div className="space-y-4" data-testid="session-detail-loading">
      <div className="flex items-center justify-between">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-6 w-20 rounded-full" />
      </div>
      <Skeleton className="h-4 w-32" />
      <Card>
        <CardHeader>
          <Skeleton className="h-6 w-40" />
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-3/4" />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function SessionDetailError({ error }: { error: Error }) {
  return (
    <Card data-testid="session-detail-error">
      <CardContent className="py-8 text-center">
        <p className="text-destructive font-medium">Failed to load session</p>
        <p className="text-sm text-muted-foreground mt-1">{error.message}</p>
      </CardContent>
    </Card>
  );
}

interface SearchMatch {
  lineIndex: number;
  text: string;
}

function findMatches(lines: OutputLine[], query: string): SearchMatch[] {
  if (!query) return [];
  const lowerQuery = query.toLowerCase();
  return lines
    .map((line, index) => ({ lineIndex: index, text: line.text }))
    .filter(({ text }) => text.toLowerCase().includes(lowerQuery));
}

export function SessionDetailRoute() {
  const { id } = useParams<{ id: string }>();
  const [currentIteration, setCurrentIteration] = useState(1);
  const [searchQuery, setSearchQuery] = useState('');
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);

  const {
    data: session,
    isLoading: sessionLoading,
    error: sessionError,
  } = useQuery({
    queryKey: ['session', id],
    queryFn: () => api.getSession(id!),
    enabled: !!id,
  });

  const {
    data: iterationContent,
    isLoading: contentLoading,
  } = useQuery({
    queryKey: ['iteration-content', id, currentIteration],
    queryFn: () => api.getIterationContent(id!, currentIteration),
    enabled: !!id && !!session,
  });

  const currentIterationData = session?.iterations[currentIteration - 1];

  const searchMatches = useMemo(() => {
    if (!iterationContent) return [];
    return findMatches(iterationContent.lines, searchQuery);
  }, [iterationContent, searchQuery]);

  const handlePrevIteration = useCallback(() => {
    setCurrentIteration((prev) => Math.max(1, prev - 1));
    setSearchQuery('');
    setCurrentMatchIndex(0);
  }, []);

  const handleNextIteration = useCallback(() => {
    if (!session) return;
    setCurrentIteration((prev) => Math.min(session.iterations.length, prev + 1));
    setSearchQuery('');
    setCurrentMatchIndex(0);
  }, [session]);

  const handleJumpToIteration = useCallback((iteration: number) => {
    setCurrentIteration(iteration);
    setSearchQuery('');
    setCurrentMatchIndex(0);
  }, []);

  const handleSearch = useCallback((query: string) => {
    setSearchQuery(query);
    setCurrentMatchIndex(0);
  }, []);

  const handleNextMatch = useCallback(() => {
    if (searchMatches.length === 0) return;
    setCurrentMatchIndex((prev) => (prev + 1) % searchMatches.length);
  }, [searchMatches.length]);

  const handlePrevMatch = useCallback(() => {
    if (searchMatches.length === 0) return;
    setCurrentMatchIndex((prev) =>
      prev === 0 ? searchMatches.length - 1 : prev - 1
    );
  }, [searchMatches.length]);

  const scrollToLine = searchMatches[currentMatchIndex]?.lineIndex;

  if (sessionLoading) {
    return <SessionDetailLoading />;
  }

  if (sessionError) {
    return <SessionDetailError error={sessionError as Error} />;
  }

  if (!session) {
    return null;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold">{session.started_at}</h1>
          <p className="text-sm text-muted-foreground">
            {session.iterations.length} iterations
          </p>
        </div>
        <StatusBadge status={session.status} />
      </div>

      <div className="flex items-center justify-between gap-4 flex-wrap">
        <IterationNav
          current={currentIteration}
          total={session.iterations.length}
          onPrev={handlePrevIteration}
          onNext={handleNextIteration}
          onJump={handleJumpToIteration}
        />

        {currentIterationData?.hat && (
          <div className="flex items-center gap-2 text-sm">
            <span className="text-muted-foreground">Hat:</span>
            <span className="font-medium">{currentIterationData.hat.display}</span>
          </div>
        )}
      </div>

      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between gap-4">
            <CardTitle className="text-base">Output</CardTitle>
            <div className="w-64">
              <SearchBar
                onSearch={handleSearch}
                resultCount={searchQuery ? searchMatches.length : undefined}
                currentResult={searchQuery ? currentMatchIndex : undefined}
                onNextResult={handleNextMatch}
                onPrevResult={handlePrevMatch}
              />
            </div>
          </div>
          {currentIterationData?.events && currentIterationData.events.length > 0 && (
            <CardDescription>
              Events: {currentIterationData.events.map((e) => e.topic).join(', ')}
            </CardDescription>
          )}
        </CardHeader>
        <CardContent>
          {contentLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-3/4" />
            </div>
          ) : (
            <div className="border rounded-md overflow-hidden h-[500px]">
              <OutputPane
                lines={iterationContent?.lines || []}
                searchQuery={searchQuery}
                scrollToLine={scrollToLine}
              />
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
