/**
 * Tasks Page
 *
 * Main dashboard showing active tasks as collapsible threads.
 * Features TaskInput for creating new tasks and ThreadList for viewing
 * existing tasks with real-time polling updates.
 */

import { TaskInput, ThreadList } from "@/components/tasks";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export function TasksPage() {
  return (
    <>
      {/* Page header */}
      <header className="mb-6">
        <h1 className="text-2xl font-bold tracking-tight">Tasks</h1>
        <p className="text-muted-foreground text-sm mt-1">Manage and monitor your Hats tasks</p>
      </header>

      {/* Tasks Section */}
      <Card>
        <CardHeader>
          <CardTitle>Tasks</CardTitle>
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
