/**
 * AppShell Component
 *
 * Main application layout with fixed sidebar and scrollable content area.
 * Uses React Router's Outlet for nested route rendering.
 * Provides the structural shell for the entire application.
 */

import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";

export function AppShell() {
  return (
    <div className="flex h-screen overflow-hidden bg-background">
      {/* Fixed sidebar */}
      <Sidebar />

      {/* Main content area - renders active route via Outlet */}
      <main className="flex-1 overflow-auto">
        <div className="p-6">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
