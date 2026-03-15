import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

interface Worker {
  workerId: string;
  workerName?: string;
  loopId?: string;
  backend?: string;
  status: string;
  currentTaskId?: string;
  currentHat?: string;
  lastHeartbeatAt?: string;
  iteration?: number;
  maxIterations?: number;
  registeredAt?: string;
}

interface WorkerCardProps {
  worker: Worker;
  currentTask?: { title?: string; claimedAt?: string } | null;
}

const STATUS_VARIANT: Record<string, "secondary" | "default" | "destructive" | "outline"> = {
  idle: "secondary",
  busy: "default",
  blocked: "destructive",
  dead: "outline",
};

function relativeTime(iso?: string): string {
  if (!iso) return "—";
  const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

function elapsedSince(iso?: string): string {
  if (!iso) return "";
  const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

export function WorkerCard({ worker, currentTask }: WorkerCardProps) {
  const variant = STATUS_VARIANT[worker.status] ?? "outline";
  const isBusy = worker.status === "busy";

  return (
    <div className={cn(
      "rounded-lg border bg-card p-4 space-y-2",
      worker.status === "dead" && "opacity-60"
    )}>
      <div className="flex items-center justify-between">
        <div className="min-w-0">
          <p className="font-medium text-sm truncate">{worker.workerName ?? worker.workerId}</p>
          {worker.workerName && (
            <p className="text-xs text-muted-foreground truncate">{worker.workerId}</p>
          )}
        </div>
        <div className="flex items-center gap-1.5">
          {isBusy && worker.iteration != null && worker.maxIterations != null && (
            <span className="text-xs font-mono px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
              {worker.iteration}/{worker.maxIterations}
            </span>
          )}
          <Badge variant={variant}>{worker.status}</Badge>
        </div>
      </div>

      <div className="text-sm text-muted-foreground truncate">
        {worker.currentTaskId && currentTask?.title ? (
          <Link
            to={`/tasks/${worker.currentTaskId}`}
            className="hover:text-foreground hover:underline transition-colors"
          >
            {currentTask.title}
          </Link>
        ) : (
          "—"
        )}
      </div>

      <div className="flex items-center gap-2 flex-wrap">
        {worker.currentHat && (
          <span className="text-xs px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium">
            {worker.currentHat}
          </span>
        )}
        {worker.backend && (
          <span className="text-xs text-muted-foreground">{worker.backend}</span>
        )}
        {isBusy && currentTask?.claimedAt && (
          <span className="text-xs text-muted-foreground" title="Task elapsed time">
            task {elapsedSince(currentTask.claimedAt)}
          </span>
        )}
        {worker.registeredAt && (
          <span className="text-xs text-muted-foreground" title="Worker uptime">
            up {elapsedSince(worker.registeredAt)}
          </span>
        )}
        <span className="text-xs text-muted-foreground ml-auto">
          {relativeTime(worker.lastHeartbeatAt)}
        </span>
      </div>
    </div>
  );
}
