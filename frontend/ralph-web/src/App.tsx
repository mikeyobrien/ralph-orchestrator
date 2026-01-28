/**
 * App Component
 *
 * Application routing configuration using React Router.
 * Defines routes with AppShell layout and page components.
 */

import { Routes, Route, Navigate } from "react-router-dom";
import { AppShell } from "./components/layout";
import { TasksPage, PlanPage, BuilderPage, TaskDetailPage, SettingsPage } from "./pages";

export function App() {
  return (
    <Routes>
      {/* AppShell provides the layout, Outlet renders the matched route */}
      <Route element={<AppShell />}>
        <Route path="/tasks" element={<TasksPage />} />
        <Route path="/tasks/:id" element={<TaskDetailPage />} />
        <Route path="/builder" element={<BuilderPage />} />
        <Route path="/plan" element={<PlanPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        {/* Redirect root to tasks */}
        <Route path="/" element={<Navigate to="/tasks" replace />} />
        {/* Catch-all redirect to tasks */}
        <Route path="*" element={<Navigate to="/tasks" replace />} />
      </Route>
    </Routes>
  );
}
