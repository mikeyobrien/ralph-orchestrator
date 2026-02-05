/**
 * PlanSession Component
 *
 * Active planning session with chat-style Q&A interface.
 * Displays agent questions and collects user responses.
 */

import { useState, useEffect, useRef, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import { trpc } from "@/trpc";
import { ArrowLeft, Send, Bot, User, Loader2, CheckCircle2, Clock, AlertCircle, FileText, Download, X, ChevronDown, ChevronRight } from "lucide-react";
import { formatDistanceToNow } from "date-fns";

interface ConversationEntry {
  type: "prompt" | "response";
  id: string;
  content: string;
  timestamp: string;
}

interface PlanSessionProps {
  /** The planning session ID */
  sessionId: string;
  /** Callback to return to landing page */
  onBack: () => void;
}

/**
 * Get status icon for prompt
 */
function getStatusIcon(isAnswered: boolean) {
  if (isAnswered) {
    return <CheckCircle2 className="h-4 w-4 text-green-500" />;
  }
  return <Clock className="h-4 w-4 text-muted-foreground" />;
}

/**
 * Format timestamp to relative time
 */
function formatTimestamp(timestamp: string | undefined): string {
  if (!timestamp) return "";
  try {
    return formatDistanceToNow(new Date(timestamp), { addSuffix: true });
  } catch {
    return "";
  }
}

export function PlanSession({ sessionId, onBack }: PlanSessionProps) {
  const utils = trpc.useUtils();
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // State for artifact viewing
  const [selectedArtifact, setSelectedArtifact] = useState<string | null>(null);
  const [artifactsExpanded, setArtifactsExpanded] = useState(true);

  // Fetch session details
  const { data: session, isLoading: isLoadingSession, error: sessionError } =
    trpc.planning.get.useQuery(
      { id: sessionId },
      {
        refetchInterval: 2000, // Poll every 2 seconds
      }
    );

  // Fetch artifact content when selected
  const { data: artifactData, isLoading: isLoadingArtifact } =
    trpc.planning.getArtifact.useQuery(
      { sessionId, filename: selectedArtifact || "" },
      { enabled: !!selectedArtifact }
    );

  // Submit response mutation
  const submitMutation = trpc.planning.respond.useMutation({
    onSuccess: () => {
      utils.planning.get.invalidate({ id: sessionId });
      utils.planning.list.invalidate();
    },
  });

  // Track current pending question (first unanswered prompt)
  const pendingPrompt = session?.conversation?.find((entry) => entry.type === "prompt");
  const isAnswered = session?.conversation?.some(
    (entry) => entry.type === "response" && entry.id === pendingPrompt?.id
  );

  // Auto-scroll to bottom when conversation updates
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [session?.conversation]);

  // Local state for response input
  const [response, setResponse] = useState("");

  // Clear input when pending prompt changes
  useEffect(() => {
    setResponse("");
  }, [pendingPrompt?.id]);

  const handleSubmit = useCallback(() => {
    const trimmed = response.trim();
    if (!trimmed || !pendingPrompt || submitMutation.isPending) return;

    submitMutation.mutate(
      {
        sessionId,
        promptId: pendingPrompt.id,
        response: trimmed,
      },
      {
        onSuccess: () => {
          setResponse("");
        },
      }
    );
  }, [response, pendingPrompt, submitMutation, sessionId]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      const isMac = navigator.userAgent.includes("Mac");
      const isSubmitKey = (isMac ? e.metaKey : e.ctrlKey) && e.key === "Enter";
      if (isSubmitKey) {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit]
  );

  // Group conversation into message pairs
  const conversationGroups = session?.conversation?.reduce<
    Array<{
      prompt?: ConversationEntry;
      response?: ConversationEntry;
    }>
  >((acc, entry) => {
    if (entry.type === "prompt") {
      acc.push({ prompt: entry });
    } else if (entry.type === "response" && acc.length > 0) {
      const lastGroup = acc[acc.length - 1];
      if (lastGroup.prompt && !lastGroup.response) {
        lastGroup.response = entry;
      } else {
        acc.push({ response: entry });
      }
    }
    return acc;
  }, []);

  const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

  if (isLoadingSession) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (sessionError || !session) {
    return (
      <Card className="border-destructive">
        <CardContent className="pt-6">
          <div className="flex items-center gap-3 text-destructive">
            <AlertCircle className="h-5 w-5" />
            <div>
              <p className="font-medium">Failed to load session</p>
              <p className="text-sm text-muted-foreground">
                {sessionError?.message || "Session not found"}
              </p>
            </div>
          </div>
          <Button variant="outline" onClick={onBack} className="mt-4">
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <header className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <Button variant="ghost" size="sm" onClick={onBack}>
            <ArrowLeft className="h-4 w-4 mr-1" />
            Back
          </Button>
          <div>
            <h2 className="text-lg font-semibold">{session.title || "Planning Session"}</h2>
            <div className="flex items-center gap-2">
              <Badge
                variant={session.status === "active" ? "default" : "secondary"}
                className="text-xs"
              >
                {session.status}
              </Badge>
              {session.completedAt && (
                <span className="text-xs text-muted-foreground">
                  Completed {formatTimestamp(session.completedAt)}
                </span>
              )}
            </div>
          </div>
        </div>
        {session.artifacts && session.artifacts.length > 0 && (
          <Badge variant="outline" className="text-xs">
            {session.artifacts.length} {session.artifacts.length === 1 ? "artifact" : "artifacts"}
          </Badge>
        )}
      </header>

      {/* Session prompt */}
      {session.prompt && (
        <Card className="mb-4 bg-accent/50">
          <CardHeader className="pb-3">
            <CardDescription>Original Prompt</CardDescription>
          </CardHeader>
          <CardContent className="pt-0">
            <p className="text-sm">{session.prompt}</p>
          </CardContent>
        </Card>
      )}

      {/* Messages */}
      <Card className="flex-1 flex flex-col min-h-0">
        <CardHeader className="pb-3">
          <CardTitle className="text-base">Conversation</CardTitle>
          <CardDescription>
            {session.status === "active"
              ? "Hats is asking questions to understand your requirements"
              : session.status === "completed"
              ? "Planning session completed"
              : "Session paused"}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex-1 overflow-y-auto">
          {!conversationGroups || conversationGroups.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-40 text-muted-foreground">
              <Bot className="h-10 w-10 mb-2 opacity-50" />
              <p className="text-sm">Waiting for Hats to ask a question...</p>
            </div>
          ) : (
            <div className="space-y-4">
              {conversationGroups.map((group, idx) => (
                <div key={`group-${idx}`} className="space-y-2">
                  {/* Agent question */}
                  {group.prompt && (
                    <div className="flex gap-3">
                      <div className="flex-shrink-0">
                        <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                          <Bot className="h-4 w-4 text-primary" />
                        </div>
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1">
                          <span className="text-sm font-medium">Hats</span>
                          <span className="text-xs text-muted-foreground">
                            {formatTimestamp(group.prompt.timestamp)}
                          </span>
                          {getStatusIcon(
                            !!group.response || session.status === "completed"
                          )}
                        </div>
                        <div className="bg-muted rounded-lg px-3 py-2 text-sm">
                          {group.prompt.content}
                        </div>
                      </div>
                    </div>
                  )}

                  {/* User response */}
                  {group.response && (
                    <div className="flex gap-3 justify-end">
                      <div className="flex-1 min-w-0 max-w-[80%]">
                        <div className="flex items-center gap-2 mb-1 justify-end">
                          {getStatusIcon(true)}
                          <span className="text-xs text-muted-foreground">
                            {formatTimestamp(group.response.timestamp)}
                          </span>
                          <span className="text-sm font-medium">You</span>
                        </div>
                        <div className="bg-primary text-primary-foreground rounded-lg px-3 py-2 text-sm ml-auto">
                          {group.response.content}
                        </div>
                      </div>
                      <div className="flex-shrink-0">
                        <div className="h-8 w-8 rounded-full bg-primary flex items-center justify-center">
                          <User className="h-4 w-4 text-primary-foreground" />
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              ))}

              {/* Pending question - show if exists and not answered */}
              {pendingPrompt && !isAnswered && !conversationGroups.some(g => g.prompt?.id === pendingPrompt.id) && (
                <div className="flex gap-3">
                  <div className="flex-shrink-0">
                    <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                      <Bot className="h-4 w-4 text-primary" />
                    </div>
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-sm font-medium">Hats</span>
                      <span className="text-xs text-muted-foreground">
                        {formatTimestamp(pendingPrompt.timestamp)}
                      </span>
                      <Clock className="h-4 w-4 text-muted-foreground" />
                    </div>
                    <div className="bg-muted rounded-lg px-3 py-2 text-sm">
                      {pendingPrompt.content}
                    </div>
                  </div>
                </div>
              )}

              {/* Session completed message with interactive artifacts */}
              {session.status === "completed" && session.artifacts && session.artifacts.length > 0 && (
                <div className="flex gap-3">
                  <div className="flex-shrink-0">
                    <div className="h-8 w-8 rounded-full bg-green-500/10 flex items-center justify-center">
                      <CheckCircle2 className="h-4 w-4 text-green-500" />
                    </div>
                  </div>
                  <div className="flex-1">
                    <div className="bg-green-500/10 border border-green-500/20 rounded-lg px-3 py-2 text-sm">
                      <p className="font-medium text-green-700 dark:text-green-400 mb-1">
                        Planning Complete!
                      </p>
                      <div className="mt-2">
                        <button
                          onClick={() => setArtifactsExpanded(!artifactsExpanded)}
                          className="flex items-center gap-1 text-muted-foreground hover:text-foreground transition-colors"
                        >
                          {artifactsExpanded ? (
                            <ChevronDown className="h-4 w-4" />
                          ) : (
                            <ChevronRight className="h-4 w-4" />
                          )}
                          <span>Design artifacts ({session.artifacts.length})</span>
                        </button>
                        {artifactsExpanded && (
                          <div className="mt-2 space-y-1">
                            {session.artifacts.map((artifact) => (
                              <button
                                key={artifact}
                                onClick={() => setSelectedArtifact(artifact)}
                                className="flex items-center gap-2 w-full text-left px-2 py-1.5 rounded hover:bg-green-500/10 text-muted-foreground hover:text-foreground transition-colors"
                              >
                                <FileText className="h-4 w-4" />
                                <span className="truncate">{artifact}</span>
                              </button>
                            ))}
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* Artifacts also shown for non-completed sessions with artifacts */}
              {session.status !== "completed" && session.artifacts && session.artifacts.length > 0 && (
                <div className="flex gap-3">
                  <div className="flex-shrink-0">
                    <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                      <FileText className="h-4 w-4 text-primary" />
                    </div>
                  </div>
                  <div className="flex-1">
                    <div className="bg-muted border rounded-lg px-3 py-2 text-sm">
                      <button
                        onClick={() => setArtifactsExpanded(!artifactsExpanded)}
                        className="flex items-center gap-1 text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {artifactsExpanded ? (
                          <ChevronDown className="h-4 w-4" />
                        ) : (
                          <ChevronRight className="h-4 w-4" />
                        )}
                        <span>Generated artifacts ({session.artifacts.length})</span>
                      </button>
                      {artifactsExpanded && (
                        <div className="mt-2 space-y-1">
                          {session.artifacts.map((artifact) => (
                            <button
                              key={artifact}
                              onClick={() => setSelectedArtifact(artifact)}
                              className="flex items-center gap-2 w-full text-left px-2 py-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                            >
                              <FileText className="h-4 w-4" />
                              <span className="truncate">{artifact}</span>
                            </button>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* Loading indicator for active session */}
              {session.status === "active" && isAnswered && (
                <div className="flex gap-3">
                  <div className="flex-shrink-0">
                    <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                      <Bot className="h-4 w-4 text-primary" />
                    </div>
                  </div>
                  <div className="flex-1">
                    <div className="bg-muted rounded-lg px-3 py-2 text-sm text-muted-foreground flex items-center gap-2">
                      <Loader2 className="h-3 w-3 animate-spin" />
                      Thinking...
                    </div>
                  </div>
                </div>
              )}

              <div ref={messagesEndRef} />
            </div>
          )}
        </CardContent>

        {/* Response input */}
        {pendingPrompt && !isAnswered && session.status === "active" && (
          <div className="border-t p-4 space-y-3">
            <label className="text-sm font-medium">Your Response</label>
            <Textarea
              value={response}
              onChange={(e) => setResponse(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type your answer here..."
              className="min-h-[80px] resize-none"
              disabled={submitMutation.isPending}
            />
            <div className="flex items-center justify-between">
              <span className="text-xs text-muted-foreground">
                {isMac ? "⌘" : "Ctrl"}+Enter to send
              </span>
              <Button
                onClick={handleSubmit}
                disabled={!response.trim() || submitMutation.isPending}
                size="sm"
              >
                {submitMutation.isPending ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    Sending...
                  </>
                ) : (
                  <>
                    <Send className="h-4 w-4 mr-2" />
                    Send Response
                  </>
                )}
              </Button>
            </div>
          </div>
        )}

        {/* Session controls */}
        <div className="border-t p-4 flex items-center justify-between">
          <span className="text-xs text-muted-foreground">
            Session ID: <code className="text-xs">{sessionId.slice(0, 8)}</code>
          </span>
          {session.status === "paused" && (
            <Button variant="outline" size="sm">
              Resume Session
            </Button>
          )}
        </div>
      </Card>

      {/* Artifact Viewer Modal */}
      {selectedArtifact && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
          <Card className="w-full max-w-4xl max-h-[90vh] flex flex-col">
            <CardHeader className="flex-shrink-0 flex flex-row items-center justify-between space-y-0 pb-2">
              <div className="flex items-center gap-2">
                <FileText className="h-5 w-5" />
                <CardTitle className="text-lg">{selectedArtifact}</CardTitle>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    if (artifactData?.content) {
                      const blob = new Blob([artifactData.content], { type: "text/plain" });
                      const url = URL.createObjectURL(blob);
                      const a = document.createElement("a");
                      a.href = url;
                      a.download = selectedArtifact;
                      a.click();
                      URL.revokeObjectURL(url);
                    }
                  }}
                  disabled={!artifactData?.content}
                >
                  <Download className="h-4 w-4 mr-1" />
                  Download
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setSelectedArtifact(null)}>
                  <X className="h-4 w-4" />
                </Button>
              </div>
            </CardHeader>
            <CardContent className="flex-1 overflow-auto">
              {isLoadingArtifact ? (
                <div className="flex items-center justify-center h-40">
                  <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
                </div>
              ) : artifactData?.content ? (
                <pre className="text-sm bg-muted p-4 rounded-lg overflow-auto whitespace-pre-wrap font-mono">
                  {artifactData.content}
                </pre>
              ) : (
                <div className="text-center text-muted-foreground py-8">
                  Failed to load artifact content
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
