import { cn } from "@/lib/utils";

interface FactoryStatsProps {
  summary: any;
  metrics: any;
}

function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null) return "—";
  const s = Math.round(seconds);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  if (m < 60) return rem > 0 ? `${m}m ${rem}s` : `${m}m`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

function StatCard({ title, children, className }: { title: string; children: React.ReactNode; className?: string }) {
  return (
    <div className={cn("rounded-lg border bg-card p-4", className)}>
      <p className="text-sm font-medium text-muted-foreground mb-2">{title}</p>
      {children}
    </div>
  );
}

export function FactoryStats({ summary, metrics }: FactoryStatsProps) {
  const counts = summary?.counts ?? {};
  const metricsSummary = metrics?.summary ?? {};
  const cycleTime = metrics?.cycleTime;
  const queueAge = metrics?.queueAge;

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Workers */}
        <StatCard title="Workers">
          <p className="text-2xl font-bold">
            {metricsSummary.activeWorkers ?? 0}
            <span className="text-sm font-normal text-muted-foreground">/{metricsSummary.aliveWorkers ?? metricsSummary.totalWorkers ?? 0}</span>
          </p>
          {metricsSummary.utilization != null && (
            <p className="text-xs text-muted-foreground mt-1">
              {Math.round(metricsSummary.utilization * 100)}% utilization
            </p>
          )}
          {(metricsSummary.deadWorkers ?? 0) > 0 && (
            <p className="text-xs text-muted-foreground">
              {metricsSummary.deadWorkers} dead
            </p>
          )}
        </StatCard>

        {/* Tasks */}
        <StatCard title="Tasks">
          <p className="text-2xl font-bold">
            {metricsSummary.totalTasks ?? 0}
          </p>
          <div className="flex gap-2 text-xs text-muted-foreground mt-1 flex-wrap">
            {counts.ready > 0 && <span>{counts.ready} ready</span>}
            {counts.in_progress > 0 && <span>{counts.in_progress} active</span>}
            {counts.done > 0 && <span>{counts.done} done</span>}
          </div>
          {metricsSummary.completionRate != null && (
            <p className="text-xs text-muted-foreground">
              {Math.round(metricsSummary.completionRate * 100)}% complete
            </p>
          )}
        </StatCard>

        {/* Cycle Time */}
        <StatCard title="Cycle Time">
          <p className="text-2xl font-bold">
            {formatDuration(cycleTime?.avgSeconds ?? null)}
          </p>
          {cycleTime && (
            <p className="text-xs text-muted-foreground mt-1">
              p50: {formatDuration(cycleTime.p50Seconds)} ({cycleTime.count} tasks)
            </p>
          )}
        </StatCard>

        {/* Queue Health */}
        <StatCard title="Queue Health">
          <p className="text-2xl font-bold">
            {formatDuration(queueAge?.avgSeconds ?? null)}
          </p>
          <div className="text-xs text-muted-foreground mt-1 space-y-0.5">
            {queueAge?.maxSeconds != null && (
              <p>max: {formatDuration(queueAge.maxSeconds)}</p>
            )}
            {(metrics?.reclaimCount ?? 0) > 0 && (
              <p>{metrics.reclaimCount} reclaims</p>
            )}
            {(summary?.staleItems?.length ?? 0) > 0 && (
              <p>{summary.staleItems.length} stale</p>
            )}
          </div>
        </StatCard>
      </div>

      {/* Recommendations */}
      {summary?.recommendations?.length > 0 && (
        <div className="rounded-lg border bg-amber-500/5 border-amber-500/20 p-3">
          <p className="text-sm font-medium text-amber-600 dark:text-amber-400 mb-1">Recommendations</p>
          <ul className="text-sm text-muted-foreground space-y-1">
            {summary.recommendations.map((rec: string, i: number) => (
              <li key={i}>{rec}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
