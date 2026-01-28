/**
 * Sidebar Component
 *
 * Collapsible navigation sidebar with nav items and toggle button.
 * Uses Zustand store for state persistence across page refreshes.
 * Navigation items use React Router NavLink for proper routing.
 */

import { ListTodo, PanelLeftClose, PanelLeft, Terminal, Lightbulb, Workflow, Settings } from "lucide-react";
import { NavItem } from "./NavItem";
import { useUIStore } from "@/store";
import { cn } from "@/lib/utils";

/** Navigation items configuration with route paths */
const NAV_ITEMS = [
  { to: "/tasks", icon: ListTodo, label: "Tasks" },
  { to: "/builder", icon: Workflow, label: "Builder" },
  { to: "/plan", icon: Lightbulb, label: "Plan" },
  { to: "/settings", icon: Settings, label: "Settings" },
] as const;

export function Sidebar() {
  const { sidebarOpen, toggleSidebar } = useUIStore();

  return (
    <aside
      className={cn(
        "flex flex-col h-full bg-card border-r border-border transition-all duration-200",
        sidebarOpen ? "w-56" : "w-14"
      )}
    >
      {/* Logo and brand */}
      <div
        className={cn(
          "flex items-center h-14 px-3 border-b border-border",
          sidebarOpen ? "gap-3" : "justify-center"
        )}
      >
        <Terminal className="h-6 w-6 text-primary flex-shrink-0" />
        {sidebarOpen && <span className="font-bold text-lg tracking-tight truncate">Ralph</span>}
      </div>

      {/* Navigation items */}
      <nav className="flex-1 p-2 space-y-1">
        {NAV_ITEMS.map((item) => (
          <NavItem
            key={item.to}
            to={item.to}
            icon={item.icon}
            label={item.label}
            collapsed={!sidebarOpen}
          />
        ))}
      </nav>

      {/* Toggle button at bottom */}
      <div className="p-2 border-t border-border">
        <button
          onClick={toggleSidebar}
          className={cn(
            "flex items-center gap-3 w-full px-3 py-2 rounded-md text-sm font-medium transition-colors",
            "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            !sidebarOpen && "justify-center px-2"
          )}
          title={sidebarOpen ? "Collapse sidebar" : "Expand sidebar"}
        >
          {sidebarOpen ? (
            <>
              <PanelLeftClose className="h-5 w-5 flex-shrink-0" />
              <span className="truncate">Collapse</span>
            </>
          ) : (
            <PanelLeft className="h-5 w-5 flex-shrink-0" />
          )}
        </button>
      </div>
    </aside>
  );
}
