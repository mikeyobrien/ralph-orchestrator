/**
 * App shell with sidebar navigation
 */

import { Outlet } from 'react-router-dom';
import { Bot } from 'lucide-react';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarProvider,
  SidebarInset,
  SidebarTrigger,
} from '@/components/ui/sidebar';
import { Separator } from '@/components/ui/separator';
import { Nav } from '@/components/nav';
import { ThemeToggle } from '@/components/theme-toggle';

export function AppShell() {
  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader>
          <div className="flex items-center gap-2 px-2 py-2">
            <Bot className="h-6 w-6" />
            <span className="font-semibold">Ralph</span>
          </div>
        </SidebarHeader>
        <SidebarContent>
          <Nav />
        </SidebarContent>
        <SidebarFooter>
          <div className="flex items-center justify-between px-2 py-2">
            <span className="text-xs text-muted-foreground">Theme</span>
            <ThemeToggle />
          </div>
        </SidebarFooter>
      </Sidebar>
      <SidebarInset>
        <header className="flex h-14 items-center gap-2 border-b px-4">
          <SidebarTrigger className="-ml-1" />
          <Separator orientation="vertical" className="h-4" />
          <div className="flex-1" />
        </header>
        <main className="flex-1 p-4">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
