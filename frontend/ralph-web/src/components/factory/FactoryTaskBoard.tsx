import { useState, useCallback, type KeyboardEvent } from "react";
import { useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { trpc } from "@/trpc";

interface FactoryTaskBoardProps {
  summary: any;
}

const STATUS_VARIANT: Record<string, "outline" | "default" | "destructive" | "secondary"> = {
  backlog: "secondary",
  ready: "outline",
  in_progress: "default",
  in_review: "outline",
  blocked: "destructive",
  done: "secondary",
};

const STATUS_ORDER: Record<string, number> = {
  in_progress: 0,
  in_review: 1,
  ready: 2,
  backlog: 3,
  blocked: 4,
  done: 5,
};

function generateTaskId(): string {
  return `task-${Date.now()}-${Math.random().toString(16).slice(2, 6)}`;
}

function relativeTime(iso?: string): string {
  if (!iso) return "—";
  const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

function AddTaskInput() {
  const [title, setTitle] = useState("");
  const utils = trpc.useUtils();

  const mutation = trpc.factory.addTask.useMutation({
    onSuccess: () => {
      setTitle("");
      utils.factory.summary.invalidate();
      utils.factory.metrics.invalidate();
    },
  });

  const handleSubmit = useCallback(() => {
    const trimmed = title.trim();
    if (!trimmed || mutation.isPending) return;
    mutation.mutate({ id: generateTaskId(), title: trimmed });
  }, [title, mutation]);

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="space-y-1">
      <div className="flex gap-2">
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Add a task to the board..."
          disabled={mutation.isPending}
          className="flex-1"
        />
        <Button
          onClick={handleSubmit}
          disabled={mutation.isPending || !title.trim()}
          size="sm"
        >
          {mutation.isPending ? "Adding..." : "Add"}
        </Button>
      </div>
      {mutation.isError && (
        <p className="text-xs text-destructive">{mutation.error.message}</p>
      )}
    </div>
  );
}

export function FactoryTaskBoard({ summary }: FactoryTaskBoardProps) {
  const navigate = useNavigate();
  const workers = summary?.workers ?? [];
  const staleIds = new Set((summary?.staleItems ?? []).map((s: any) => s.id));

  // Build a task list from workers' currentTask + board items
  // board.summary includes enriched workers with currentTask
  const taskMap = new Map<string, any>();

  // Collect tasks from various summary fields
  for (const w of workers) {
    if (w.currentTask) {
      taskMap.set(w.currentTask.id, { ...w.currentTask, assigneeWorkerId: w.workerId });
    }
  }
  for (const item of summary?.recentCompletions ?? []) {
    if (!taskMap.has(item.id)) taskMap.set(item.id, item);
  }
  for (const item of summary?.blockedItems ?? []) {
    if (!taskMap.has(item.id)) taskMap.set(item.id, item);
  }
  for (const item of summary?.inReviewItems ?? []) {
    if (!taskMap.has(item.id)) taskMap.set(item.id, item);
  }
  for (const item of summary?.staleItems ?? []) {
    if (!taskMap.has(item.id)) taskMap.set(item.id, item);
  }
  for (const item of summary?.readyItems ?? []) {
    if (!taskMap.has(item.id)) taskMap.set(item.id, item);
  }
  for (const item of summary?.backlogItems ?? []) {
    if (!taskMap.has(item.id)) taskMap.set(item.id, item);
  }

  const tasks = Array.from(taskMap.values()).sort(
    (a, b) => (STATUS_ORDER[a.status] ?? 9) - (STATUS_ORDER[b.status] ?? 9)
  );

  return (
    <div className="space-y-4">
      <AddTaskInput />

      {tasks.length === 0 ? (
        <p className="text-sm text-muted-foreground text-center py-8">No tasks on the board.</p>
      ) : (
        <div className="border rounded-lg overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-muted/50">
                <th className="text-left px-3 py-2 font-medium">Status</th>
                <th className="text-left px-3 py-2 font-medium">Title</th>
                <th className="text-left px-3 py-2 font-medium">Worker</th>
                <th className="text-left px-3 py-2 font-medium">Files</th>
                <th className="text-left px-3 py-2 font-medium">Error</th>
                <th className="text-left px-3 py-2 font-medium">Age</th>
              </tr>
            </thead>
            <tbody>
              {tasks.map((task: any) => {
                const isStale = staleIds.has(task.id);
                const variant = STATUS_VARIANT[task.status] ?? "outline";
                const files = task.scope_files ?? [];

                return (
                  <tr
                    key={task.id}
                    onClick={() => navigate(`/tasks/${task.id}`)}
                    className={cn(
                      "border-b last:border-0 cursor-pointer hover:bg-muted/50 transition-colors",
                      isStale && "border-l-2 border-l-amber-500"
                    )}
                  >
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-1.5">
                        <Badge variant={isStale ? "destructive" : variant}>
                          {isStale ? "stale" : task.status}
                        </Badge>
                      </div>
                    </td>
                    <td className="px-3 py-2 max-w-[300px] truncate" title={task.title}>
                      {task.title ?? task.id}
                    </td>
                    <td className="px-3 py-2 text-muted-foreground">
                      {task.assigneeWorkerId ?? "—"}
                    </td>
                    <td className="px-3 py-2 text-muted-foreground" title={files.join(", ")}>
                      {files.length > 0 ? files.length : "—"}
                    </td>
                    <td className="px-3 py-2 max-w-[200px] truncate">
                      {isStale ? (
                        <span className="text-destructive" title="Worker lease expired — task needs reclaiming">
                          Worker died
                        </span>
                      ) : task.error_message ? (
                        <span className="text-destructive" title={task.error_message}>
                          {task.error_message}
                        </span>
                      ) : "—"}
                    </td>
                    <td className="px-3 py-2 text-muted-foreground whitespace-nowrap">
                      {relativeTime(task.createdAt)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
