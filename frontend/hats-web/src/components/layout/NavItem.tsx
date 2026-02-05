/**
 * NavItem Component
 *
 * Individual navigation item for the sidebar using React Router's NavLink.
 * Supports icons, labels, active state highlighting, and collapsed mode.
 * Uses NavLink for automatic active class management based on current route.
 */

import { NavLink } from "react-router-dom";
import { type LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

interface NavItemProps {
  /** Icon component from lucide-react */
  icon: LucideIcon;
  /** Navigation item label */
  label: string;
  /** Route path to navigate to */
  to: string;
  /** Whether the sidebar is collapsed (icon-only mode) */
  collapsed?: boolean;
}

export function NavItem({ icon: Icon, label, to, collapsed = false }: NavItemProps) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn(
          "flex items-center gap-3 w-full px-3 py-2 rounded-md text-sm font-medium transition-colors",
          "hover:bg-accent hover:text-accent-foreground",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          isActive && "bg-accent text-accent-foreground",
          !isActive && "text-muted-foreground",
          collapsed && "justify-center px-2"
        )
      }
      title={collapsed ? label : undefined}
    >
      <Icon className="h-5 w-5 flex-shrink-0" />
      {!collapsed && <span className="truncate">{label}</span>}
    </NavLink>
  );
}
