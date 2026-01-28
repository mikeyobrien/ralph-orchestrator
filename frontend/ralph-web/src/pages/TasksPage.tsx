/**
 * Tasks Page
 *
 * Main dashboard showing active tasks as collapsible threads.
 * Features TaskInput for creating new tasks and ThreadList for viewing
 * existing tasks with real-time polling updates.
 */

import { TaskInput, ThreadList } from "@/components/tasks";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

export function TasksPage() {
  return (
    <>
      {/* Page header */}
      <header className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Tasks</h1>
          <p className="text-muted-foreground text-sm mt-1">Manage and monitor your Ralph tasks</p>
        </div>
        <Badge variant="secondary">v0.1.0</Badge>
      </header>

      {/* Tasks Section */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Tasks
            <Badge variant="outline" className="ml-2">
              Live
            </Badge>
          </CardTitle>
          <CardDescription>Active and recent task threads</CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          <TaskInput />
          <ThreadList pollingInterval={5000} />
        </CardContent>
      </Card>
    </>
  );
}
