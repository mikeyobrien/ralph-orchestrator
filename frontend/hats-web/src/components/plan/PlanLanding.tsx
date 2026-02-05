/**
 * PlanLanding Component
 *
 * Landing page for the planning workflow.
 * Shows existing planning sessions and allows starting a new session.
 */

import { useState } from "react";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { trpc } from "@/trpc";
import { Lightbulb, Clock, MessageSquare, Trash2, Play, CheckCircle2, AlertCircle } from "lucide-react";
import { formatDistanceToNow } from "date-fns";

/**
 * Session status badge variant mapping
 */
function getStatusVariant(status: string): "default" | "secondary" | "outline" {
  switch (status) {
    case "active":
      return "default";
    case "paused":
      return "secondary";
    case "completed":
      return "outline";
    default:
      return "outline";
  }
}

/**
 * Format timestamp to relative time
 */
function formatTimestamp(timestamp: string | undefined): string {
  if (!timestamp) return "Unknown";
  try {
    return formatDistanceToNow(new Date(timestamp), { addSuffix: true });
  } catch {
    return "Unknown";
  }
}

interface PlanLandingProps {
  /** Callback when a new session is started */
  onStart: (sessionId: string) => void;
}

export function PlanLanding({ onStart }: PlanLandingProps) {
  const [prompt, setPrompt] = useState("");
  const utils = trpc.useUtils();

  // Query for existing sessions
  const { data: sessions, isLoading } = trpc.planning.list.useQuery();

  // Start session mutation
  const startMutation = trpc.planning.start.useMutation({
    onSuccess: (result) => {
      onStart(result.sessionId);
      utils.planning.list.invalidate();
    },
  });

  // Resume session mutation
  const resumeMutation = trpc.planning.resume.useMutation({
    onSuccess: () => {
      utils.planning.list.invalidate();
    },
  });

  // Delete session mutation
  const deleteMutation = trpc.planning.delete.useMutation({
    onSuccess: () => {
      utils.planning.list.invalidate();
    },
  });

  const handleStartSession = () => {
    const trimmed = prompt.trim();
    if (!trimmed || startMutation.isPending) return;

    startMutation.mutate({ prompt: trimmed });
    setPrompt("");
  };

  const handleResumeSession = (sessionId: string) => {
    resumeMutation.mutate({ id: sessionId });
    onStart(sessionId);
  };

  const handleDeleteSession = (sessionId: string) => {
    if (confirm("Are you sure you want to delete this planning session?")) {
      deleteMutation.mutate({ id: sessionId });
    }
  };

  const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

  return (
    <div className="space-y-6">
      {/* Page header */}
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Planning</h1>
          <p className="text-muted-foreground text-sm mt-1">
            Collaborative design through interactive Q&A
          </p>
        </div>
        <Badge variant="secondary" className="gap-1">
          <Lightbulb className="h-3 w-3" />
          Planning Mode
        </Badge>
      </header>

      {/* New Session Card */}
      <Card>
        <CardHeader>
          <CardTitle>Start New Planning Session</CardTitle>
          <CardDescription>
            Describe what you want to plan or design. Hats will guide you through
            clarifying questions to create a comprehensive design document.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
              const isSubmitKey = (isMac ? e.metaKey : e.ctrlKey) && e.key === "Enter";
              if (isSubmitKey) {
                e.preventDefault();
                handleStartSession();
              }
            }}
            placeholder="What would you like to plan? (e.g., 'Build a REST API for task management')"
            className="min-h-[100px] resize-none"
            disabled={startMutation.isPending}
          />
          <div className="flex items-center justify-between">
            <span className="text-xs text-muted-foreground">
              {isMac ? "⌘" : "Ctrl"}+Enter to start
            </span>
            <Button
              onClick={handleStartSession}
              disabled={!prompt.trim() || startMutation.isPending}
            >
              {startMutation.isPending ? (
                <>
                  <span className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent mr-2" />
                  Starting...
                </>
              ) : (
                "Start Planning"
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Existing Sessions */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Planning Sessions
            {sessions && sessions.length > 0 && (
              <Badge variant="outline">{sessions.length}</Badge>
            )}
          </CardTitle>
          <CardDescription>
            Resume previous planning sessions or review past conversations
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="text-center py-8 text-muted-foreground">
              Loading sessions...
            </div>
          ) : !sessions || sessions.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              <Lightbulb className="h-12 w-12 mx-auto mb-3 opacity-50" />
              <p>No planning sessions yet. Start your first one above!</p>
            </div>
          ) : (
            <div className="space-y-3">
              {sessions.map((session) => (
                <div
                  key={session.id}
                  className={cn(
                    "flex items-start gap-4 p-4 rounded-lg border border-border",
                    "hover:bg-accent/50 transition-colors"
                  )}
                >
                  {/* Session icon/status */}
                  <div className="flex-shrink-0 mt-1">
                    {session.status === "active" ? (
                      <div className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                    ) : (
                      <MessageSquare className="h-4 w-4 text-muted-foreground" />
                    )}
                  </div>

                  {/* Session info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <h3 className="font-medium truncate">{session.title || "Untitled Session"}</h3>
                      <Badge variant={getStatusVariant(session.status)} className="text-xs">
                        {session.status}
                      </Badge>
                    </div>
                    <div className="flex items-center gap-3 text-xs text-muted-foreground">
                      <span className="flex items-center gap-1">
                        <Clock className="h-3 w-3" />
                        {formatTimestamp(session.createdAt)}
                      </span>
                      {session.messageCount !== undefined && (
                        <span className="flex items-center gap-1">
                          <MessageSquare className="h-3 w-3" />
                          {session.messageCount} {session.messageCount === 1 ? "message" : "messages"}
                        </span>
                      )}
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2">
                    {session.status === "paused" && (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleResumeSession(session.id)}
                        disabled={resumeMutation.isPending}
                      >
                        <Play className="h-3 w-3 mr-1" />
                        Resume
                      </Button>
                    )}
                    {session.status === "active" && (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => onStart(session.id)}
                      >
                        <Play className="h-3 w-3 mr-1" />
                        View
                      </Button>
                    )}
                    {session.status === "completed" && (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => onStart(session.id)}
                      >
                        <CheckCircle2 className="h-3 w-3 mr-1" />
                        View
                      </Button>
                    )}
                    {session.status === "failed" && (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => onStart(session.id)}
                      >
                        <AlertCircle className="h-3 w-3 mr-1" />
                        View
                      </Button>
                    )}
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => handleDeleteSession(session.id)}
                      disabled={deleteMutation.isPending}
                      className="text-destructive hover:text-destructive"
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
