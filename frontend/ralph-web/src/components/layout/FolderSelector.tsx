/**
 * FolderSelector Component
 *
 * Dropdown in the sidebar that lets users switch between registered folder domains.
 * Calls folder.list on mount and when the selector is opened.
 * Updates the Zustand store so all RPC calls target the selected folder.
 */

import { useCallback, useEffect, useState } from "react";
import { FolderOpen, ChevronDown } from "lucide-react";
import { rpcCall } from "@/rpc/client";
import { useUIStore } from "@/store";
import { cn } from "@/lib/utils";

interface FolderDomain {
  slug: string;
  path: string;
  displayName: string;
}

interface FolderListResult {
  folders: FolderDomain[];
  defaultSlug: string | null;
}

export function FolderSelector({ collapsed }: { collapsed: boolean }) {
  const { activeFolderSlug, setActiveFolder } = useUIStore();
  const [folders, setFolders] = useState<FolderDomain[]>([]);
  const [open, setOpen] = useState(false);

  const fetchFolders = useCallback(async () => {
    try {
      const result = await rpcCall<FolderListResult>("folder.list");
      setFolders(result.folders);

      // If no folder is selected yet, use the default
      if (!activeFolderSlug && result.defaultSlug) {
        setActiveFolder(result.defaultSlug);
      }
    } catch {
      // folder.list may not be available in single-runtime mode
    }
  }, [activeFolderSlug, setActiveFolder]);

  useEffect(() => {
    fetchFolders();
  }, [fetchFolders]);

  // Don't render if only one folder (or none)
  if (folders.length <= 1) return null;

  const activeFolder = folders.find((f) => f.slug === activeFolderSlug) ?? folders[0];

  if (collapsed) {
    return (
      <div className="px-2 py-1.5">
        <button
          onClick={() => setOpen(!open)}
          className={cn(
            "flex items-center justify-center w-full py-2 rounded-md text-sm",
            "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          )}
          title={activeFolder?.displayName ?? "Select folder"}
        >
          <FolderOpen className="h-4 w-4" />
        </button>
      </div>
    );
  }

  return (
    <div className="px-2 py-1.5 relative">
      <button
        onClick={() => {
          if (!open) fetchFolders();
          setOpen(!open);
        }}
        className={cn(
          "flex items-center gap-2 w-full px-2.5 py-1.5 rounded-md text-sm",
          "bg-muted/50 hover:bg-muted transition-colors",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        )}
        title={activeFolder?.path}
      >
        <FolderOpen className="h-4 w-4 text-muted-foreground flex-shrink-0" />
        <span className="truncate flex-1 text-left font-medium">
          {activeFolder?.displayName ?? "Select folder"}
        </span>
        <ChevronDown className={cn("h-3 w-3 text-muted-foreground transition-transform", open && "rotate-180")} />
      </button>

      {open && (
        <div className="absolute left-2 right-2 top-full mt-1 z-50 bg-popover border border-border rounded-md shadow-md py-1">
          {folders.map((folder) => (
            <button
              key={folder.slug}
              onClick={() => {
                setActiveFolder(folder.slug);
                setOpen(false);
              }}
              className={cn(
                "flex flex-col w-full px-3 py-1.5 text-left text-sm hover:bg-accent transition-colors",
                folder.slug === activeFolderSlug && "bg-accent/50"
              )}
              title={folder.path}
            >
              <span className="font-medium truncate">{folder.displayName}</span>
              <span className="text-xs text-muted-foreground truncate">{folder.path}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
