import { trpc } from "@/trpc";
import { cn } from "@/lib/utils";
import { GitBranch, CheckCircle2 } from "lucide-react";

const STATUS_LABELS: Record<string, { label: string; color: string }> = {
  M: { label: "Modified", color: "text-amber-500" },
  A: { label: "Added", color: "text-green-500" },
  D: { label: "Deleted", color: "text-red-500" },
  R: { label: "Renamed", color: "text-blue-500" },
  "??": { label: "Untracked", color: "text-muted-foreground" },
};

function statusInfo(code: string) {
  return STATUS_LABELS[code] ?? { label: code, color: "text-muted-foreground" };
}

export function GitStatusPanel() {
  const { data, isLoading, isError } = trpc.factory.gitStatus.useQuery(undefined, {
    refetchInterval: 10_000,
  });

  if (isLoading) return null;
  if (isError) return null;
  if (!data) return null;

  return (
    <div className="rounded-lg border bg-card p-4">
      <div className="flex items-center gap-2 mb-3">
        <GitBranch className="h-4 w-4 text-muted-foreground" />
        <h3 className="text-sm font-medium">Git Status</h3>
        {data.branch && (
          <code className="text-xs bg-muted px-1.5 py-0.5 rounded">{data.branch}</code>
        )}
      </div>

      {data.clean ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <CheckCircle2 className="h-4 w-4 text-green-500" />
          <span>Working tree clean</span>
        </div>
      ) : (
        <ul className="space-y-1 max-h-48 overflow-y-auto">
          {data.files.map((f: { status: string; path: string }) => {
            const info = statusInfo(f.status);
            return (
              <li key={f.path} className="flex items-center gap-2 text-xs font-mono">
                <span className={cn("w-16 shrink-0", info.color)}>{info.label}</span>
                <span className="truncate text-muted-foreground" title={f.path}>{f.path}</span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
