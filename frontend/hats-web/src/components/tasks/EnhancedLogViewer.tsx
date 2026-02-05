/**
 * EnhancedLogViewer Component
 *
 * Real-time log viewer with Tailwind dark theme styling, featuring:
 * - Line numbers for easy reference
 * - stdout/stderr color coding (green/red)
 * - Filtering toggles for stdout/stderr streams
 * - Auto-scroll with smart pause detection
 * - Copy-to-clipboard for individual lines and full log
 *
 * Uses the useTaskWebSocket hook for WebSocket connection management.
 */

import { useRef, useState, useEffect, useCallback, useMemo, type MouseEvent } from "react";
import {
  Copy,
  Check,
  ArrowDownToLine,
  Terminal,
  AlertTriangle,
  Wifi,
  WifiOff,
  Loader2,
  XCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { useTaskWebSocket, type LogEntry, type ConnectionState } from "@/hooks/useTaskWebSocket";

interface EnhancedLogViewerProps {
  /** Task ID to subscribe to */
  taskId: string;
  /** Height of the log viewer (default: '400px') */
  height?: string;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Format a timestamp with milliseconds for log display
 */
function formatTimestamp(timestamp: string | Date): string {
  const d = typeof timestamp === "string" ? new Date(timestamp) : timestamp;
  const hours = d.getHours().toString().padStart(2, "0");
  const mins = d.getMinutes().toString().padStart(2, "0");
  const secs = d.getSeconds().toString().padStart(2, "0");
  const ms = d.getMilliseconds().toString().padStart(3, "0");
  return `${hours}:${mins}:${secs}.${ms}`;
}

/**
 * Connection state indicator configuration
 */
const CONNECTION_CONFIG: Record<
  ConnectionState,
  { icon: typeof Wifi; color: string; label: string }
> = {
  connecting: { icon: Loader2, color: "text-yellow-500", label: "Connecting" },
  connected: { icon: Wifi, color: "text-green-500", label: "Connected" },
  disconnected: { icon: WifiOff, color: "text-zinc-500", label: "Disconnected" },
  error: { icon: XCircle, color: "text-red-500", label: "Error" },
};

export function EnhancedLogViewer({ taskId, height = "400px", className }: EnhancedLogViewerProps) {
  // Filter state
  const [showStdout, setShowStdout] = useState(true);
  const [showStderr, setShowStderr] = useState(true);

  // Auto-scroll state
  const [autoScroll, setAutoScroll] = useState(true);
  const [isAtBottom, setIsAtBottom] = useState(true);

  // Copy feedback state
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [copiedAll, setCopiedAll] = useState(false);

  // Refs
  const containerRef = useRef<HTMLDivElement>(null);
  const userScrolledRef = useRef(false);

  // WebSocket connection - logs are persisted in Zustand store
  // so they survive component unmount when task card is collapsed
  const { entries, connectionState, error, clearEntries } = useTaskWebSocket(taskId);

  // Filter entries and count sources in a single pass
  const { filteredEntries, stdoutCount, stderrCount } = useMemo(() => {
    const filtered: LogEntry[] = [];
    let stdoutTotal = 0;
    let stderrTotal = 0;

    for (const entry of entries) {
      if (entry.source === "stdout") {
        stdoutTotal++;
        if (showStdout) filtered.push(entry);
      } else {
        stderrTotal++;
        if (showStderr) filtered.push(entry);
      }
    }

    return {
      filteredEntries: filtered,
      stdoutCount: stdoutTotal,
      stderrCount: stderrTotal,
    };
  }, [entries, showStdout, showStderr]);

  // Auto-scroll to bottom when new entries arrive
  useEffect(() => {
    if (autoScroll && containerRef.current && !userScrolledRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
    // Reset user scroll flag on next frame
    userScrolledRef.current = false;
  }, [filteredEntries, autoScroll]);

  // Handle scroll events to detect user scrolling
  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;

    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const atBottom = scrollTop + clientHeight >= scrollHeight - 10;

    setIsAtBottom(atBottom);

    // If user scrolled away from bottom, pause auto-scroll
    if (!atBottom && autoScroll) {
      userScrolledRef.current = true;
      setAutoScroll(false);
    }

    // If user scrolled to bottom, resume auto-scroll
    if (atBottom && !autoScroll) {
      setAutoScroll(true);
    }
  }, [autoScroll]);

  // Resume auto-scroll
  const handleResumeScroll = useCallback(() => {
    setAutoScroll(true);
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, []);

  // Copy single line to clipboard
  const handleCopyLine = useCallback(async (entry: LogEntry, index: number, e: MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(entry.line);
      setCopiedIndex(index);
      setTimeout(() => setCopiedIndex(null), 2000);
    } catch {
      // Clipboard API failed silently
    }
  }, []);

  // Copy all logs to clipboard
  const handleCopyAll = useCallback(async () => {
    const text = filteredEntries
      .map((e) => `[${formatTimestamp(e.timestamp)}] ${e.line}`)
      .join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopiedAll(true);
      setTimeout(() => setCopiedAll(false), 2000);
    } catch {
      // Clipboard API failed silently
    }
  }, [filteredEntries]);

  // Connection indicator
  const connectionConfig = CONNECTION_CONFIG[connectionState];
  const ConnectionIcon = connectionConfig.icon;
  const isConnecting = connectionState === "connecting";

  return (
    <div
      className={cn(
        "relative flex flex-col border border-border rounded-md overflow-hidden bg-zinc-950",
        className
      )}
      style={{ height }}
    >
      {/* Header with controls */}
      <div className="flex items-center justify-between px-3 py-2 bg-zinc-900/80 border-b border-border text-xs">
        <div className="flex items-center gap-3">
          {/* Connection status */}
          <div className="flex items-center gap-1.5">
            <ConnectionIcon
              className={cn("h-3.5 w-3.5", connectionConfig.color, isConnecting && "animate-spin")}
            />
            <span className="text-muted-foreground">{connectionConfig.label}</span>
          </div>

          {/* Filter toggles */}
          <div className="flex items-center gap-1.5 ml-3">
            <Button
              variant={showStdout ? "secondary" : "ghost"}
              size="sm"
              className={cn(
                "h-6 px-2 text-xs",
                showStdout ? "text-green-400" : "text-muted-foreground"
              )}
              onClick={() => setShowStdout(!showStdout)}
            >
              <Terminal className="h-3 w-3 mr-1" />
              stdout
              <Badge variant="outline" className="ml-1 h-4 px-1 text-[8px]">
                {stdoutCount}
              </Badge>
            </Button>
            <Button
              variant={showStderr ? "secondary" : "ghost"}
              size="sm"
              className={cn(
                "h-6 px-2 text-xs",
                showStderr ? "text-red-400" : "text-muted-foreground"
              )}
              onClick={() => setShowStderr(!showStderr)}
            >
              <AlertTriangle className="h-3 w-3 mr-1" />
              stderr
              <Badge variant="outline" className="ml-1 h-4 px-1 text-[8px]">
                {stderrCount}
              </Badge>
            </Button>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Auto-scroll indicator/button */}
          {!autoScroll && (
            <Button
              variant="outline"
              size="sm"
              className="h-6 px-2 text-xs text-yellow-500 border-yellow-500/30"
              onClick={handleResumeScroll}
            >
              <ArrowDownToLine className="h-3 w-3 mr-1" />
              Resume scroll
            </Button>
          )}
          {autoScroll && (
            <span className="flex items-center gap-1 text-muted-foreground">
              <ArrowDownToLine className="h-3 w-3" />
              Auto-scroll
            </span>
          )}

          {/* Line count */}
          <span className="text-muted-foreground tabular-nums">{filteredEntries.length} lines</span>

          {/* Copy all button */}
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2"
            onClick={handleCopyAll}
            disabled={filteredEntries.length === 0}
          >
            {copiedAll ? (
              <Check className="h-3 w-3 text-green-500" />
            ) : (
              <Copy className="h-3 w-3" />
            )}
          </Button>

          {/* Clear button */}
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-muted-foreground hover:text-foreground"
            onClick={clearEntries}
            disabled={entries.length === 0}
          >
            Clear
          </Button>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div className="px-3 py-2 bg-red-950/50 text-red-400 text-xs border-b border-red-900/50">
          {error}
        </div>
      )}

      {/* Log entries */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-auto font-mono text-xs"
      >
        {filteredEntries.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
            {connectionState === "connected"
              ? "Waiting for logs..."
              : connectionState === "connecting"
                ? "Connecting..."
                : "Disconnected"}
          </div>
        ) : (
          <div className="min-w-0">
            {filteredEntries.map((entry, index) => {
              const isStderr = entry.source === "stderr";
              const isCopied = copiedIndex === index;

              return (
                <div
                  key={index}
                  className={cn(
                    "group flex items-start gap-2 px-2 py-0.5 hover:bg-zinc-800/50",
                    isStderr && "bg-red-950/20"
                  )}
                >
                  {/* Line number */}
                  <span className="text-zinc-600 select-none tabular-nums text-right shrink-0 w-10">
                    {index + 1}
                  </span>

                  {/* Timestamp - colored by source (green=stdout, red=stderr) */}
                  <span
                    className={cn(
                      "shrink-0 select-none",
                      isStderr ? "text-red-500" : "text-green-500"
                    )}
                  >
                    {formatTimestamp(entry.timestamp)}
                  </span>

                  {/* Log content */}
                  <span
                    className={cn(
                      "flex-1 whitespace-pre-wrap break-all",
                      isStderr ? "text-red-300" : "text-zinc-100"
                    )}
                  >
                    {entry.line}
                  </span>

                  {/* Copy button (shown on hover) */}
                  <button
                    className="opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-zinc-300 transition-opacity shrink-0"
                    onClick={(e) => handleCopyLine(entry, index, e)}
                    aria-label="Copy line"
                  >
                    {isCopied ? (
                      <Check className="h-3.5 w-3.5 text-green-500" />
                    ) : (
                      <Copy className="h-3.5 w-3.5" />
                    )}
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Scroll to bottom indicator (when not at bottom and auto-scroll is off) */}
      {!isAtBottom && !autoScroll && filteredEntries.length > 0 && (
        <div className="absolute bottom-4 right-4">
          <Button
            variant="secondary"
            size="sm"
            className="h-8 px-3 shadow-lg"
            onClick={handleResumeScroll}
          >
            <ArrowDownToLine className="h-4 w-4 mr-1" />
            Scroll to bottom
          </Button>
        </div>
      )}
    </div>
  );
}
