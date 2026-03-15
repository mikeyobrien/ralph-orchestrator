import { useState } from "react";
import { trpc } from "@/trpc";
import { Button } from "@/components/ui/button";
import { FactoryStats } from "@/components/factory/FactoryStats";
import { WorkerCard } from "@/components/factory/WorkerCard";
import { FactoryTaskBoard } from "@/components/factory/FactoryTaskBoard";
import { GitStatusPanel } from "@/components/factory/GitStatusPanel";
import { AlertTriangle, Loader2, RefreshCw, Eye, EyeOff } from "lucide-react";

export function FactoryPage() {
  const workers = trpc.factory.workers.useQuery(undefined, { refetchInterval: 5000 });
  const summary = trpc.factory.summary.useQuery(undefined, { refetchInterval: 5000 });
  const metrics = trpc.factory.metrics.useQuery(undefined, { refetchInterval: 5000 });

  const utils = trpc.useUtils();
  const reclaimMutation = trpc.factory.reclaimStale.useMutation({
    onSuccess: () => {
      utils.factory.summary.invalidate();
      utils.factory.workers.invalidate();
      utils.factory.metrics.invalidate();
    },
  });

  const [showDead, setShowDead] = useState(false);

  const isLoading = workers.isLoading && summary.isLoading && metrics.isLoading;
  const hasError = workers.isError && summary.isError && metrics.isError;
  const workerList = workers.data ?? [];
  const aliveWorkers = workerList.filter((w: any) => w.status !== "dead");
  const deadWorkers = workerList.filter((w: any) => w.status === "dead");
  const staleCount = (summary.data?.staleItems ?? []).length;

  // Build a map of workerId -> currentTask from summary data
  const workerTaskMap = new Map<string, any>();
  for (const w of summary.data?.workers ?? []) {
    if (w.currentTask) {
      workerTaskMap.set(w.workerId, w.currentTask);
    }
  }

  return (
    <>
      <header className="mb-6">
        <h1 className="text-2xl font-bold tracking-tight">Factory</h1>
        <p className="text-muted-foreground text-sm mt-1">Monitor parallel workers and board tasks</p>
      </header>

      {hasError ? (
        <p className="text-sm text-muted-foreground">Could not connect to backend. Start the server with <code className="text-xs bg-muted px-1 py-0.5 rounded">ralph web</code>.</p>
      ) : isLoading ? (
        <p className="text-sm text-muted-foreground">Loading factory data...</p>
      ) : (
        <div className="space-y-6">
          <FactoryStats summary={summary.data} metrics={metrics.data} />

          <GitStatusPanel />

          {/* Workers */}
          <div>
            <div className="flex items-center gap-3 mb-3">
              <h2 className="text-lg font-semibold">Workers</h2>
              {deadWorkers.length > 0 && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setShowDead(!showDead)}
                  className="text-xs text-muted-foreground"
                >
                  {showDead ? (
                    <EyeOff className="h-3.5 w-3.5 mr-1" />
                  ) : (
                    <Eye className="h-3.5 w-3.5 mr-1" />
                  )}
                  {deadWorkers.length} dead
                </Button>
              )}
            </div>
            {aliveWorkers.length === 0 && !showDead ? (
              <p className="text-sm text-muted-foreground">No workers registered.</p>
            ) : (
              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                {aliveWorkers.map((w: any) => (
                  <WorkerCard
                    key={w.workerId}
                    worker={w}
                    currentTask={workerTaskMap.get(w.workerId)}
                  />
                ))}
                {showDead && deadWorkers.map((w: any) => (
                  <WorkerCard
                    key={w.workerId}
                    worker={w}
                    currentTask={workerTaskMap.get(w.workerId)}
                  />
                ))}
              </div>
            )}
          </div>

          {/* Stale tasks alert */}
          {staleCount > 0 && (
            <div className="border border-amber-500/30 bg-amber-500/10 rounded-lg p-4 flex items-center gap-3">
              <AlertTriangle className="h-5 w-5 text-amber-500 shrink-0" />
              <div className="flex-1">
                <span className="text-sm font-medium text-amber-700 dark:text-amber-400">
                  {staleCount} stale {staleCount === 1 ? "task" : "tasks"} — worker lease expired
                </span>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => reclaimMutation.mutate()}
                disabled={reclaimMutation.isPending}
              >
                {reclaimMutation.isPending ? (
                  <Loader2 className="h-4 w-4 mr-1 animate-spin" />
                ) : (
                  <RefreshCw className="h-4 w-4 mr-1" />
                )}
                Reclaim
              </Button>
            </div>
          )}

          {/* Task Board */}
          <div>
            <h2 className="text-lg font-semibold mb-3">Task Board</h2>
            <FactoryTaskBoard summary={summary.data} />
          </div>
        </div>
      )}
    </>
  );
}
