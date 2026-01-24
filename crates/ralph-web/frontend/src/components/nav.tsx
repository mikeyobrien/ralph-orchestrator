/**
 * Navigation links component for sidebar
 */

import { NavLink } from 'react-router-dom';
import { List, Eye, Play } from 'lucide-react';
import {
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
} from '@/components/ui/sidebar';

const navItems = [
  {
    title: 'Sessions',
    icon: List,
    to: '/sessions',
  },
  {
    title: 'Live',
    icon: Eye,
    to: '/live',
  },
  {
    title: 'Start Loop',
    icon: Play,
    to: '/start',
  },
];

export function Nav() {
  return (
    <SidebarMenu>
      {navItems.map((item) => (
        <SidebarMenuItem key={item.title}>
          <SidebarMenuButton asChild>
            <NavLink
              to={item.to}
              className={({ isActive }) =>
                isActive ? 'bg-sidebar-accent' : ''
              }
            >
              <item.icon className="h-4 w-4" />
              <span>{item.title}</span>
            </NavLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      ))}
    </SidebarMenu>
  );
}
