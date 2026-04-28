/**
 * RunPromptDialog
 *
 * Small modal that asks the user what the workflow should do before
 * starting a loop. The prompt is the "what"; the hat collection is the "how."
 */

import { Button } from "@/components/ui/button";
import {
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Play } from "lucide-react";
import { useCallback, useState } from "react";

interface RunPromptDialogProps {
  open: boolean;
  onClose: () => void;
  onRun: (prompt: string) => void;
  isRunning?: boolean;
  collectionName: string;
}

export function RunPromptDialog({
  open,
  onClose,
  onRun,
  isRunning,
  collectionName,
}: RunPromptDialogProps) {
  const [prompt, setPrompt] = useState("");

  const handleRun = useCallback(() => {
    const trimmed = prompt.trim();
    if (!trimmed) return;
    onRun(trimmed);
  }, [prompt, onRun]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      onClick={onClose}
    >
      <Card
        className="w-full max-w-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <CardHeader>
          <CardTitle>Run Workflow</CardTitle>
          <CardDescription>
            What should <strong>{collectionName}</strong> do?
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            <Label htmlFor="run-prompt">Prompt</Label>
            <Textarea
              id="run-prompt"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder='e.g. "Add input validation to the /users endpoint"'
              className="min-h-[100px]"
              autoFocus
              onKeyDown={(e) => {
                if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                  e.preventDefault();
                  handleRun();
                }
              }}
            />
            <p className="text-xs text-muted-foreground">
              ⌘+Enter to run
            </p>
          </div>
        </CardContent>
        <CardFooter className="justify-end gap-2">
          <Button variant="outline" onClick={onClose} disabled={isRunning}>
            Cancel
          </Button>
          <Button
            onClick={handleRun}
            disabled={!prompt.trim() || isRunning}
          >
            <Play className="h-4 w-4 mr-2" />
            {isRunning ? "Starting..." : "Run"}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
