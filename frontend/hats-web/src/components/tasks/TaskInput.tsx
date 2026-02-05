/**
 * TaskInput Component
 *
 * Auto-resizing textarea with Cmd/Ctrl+Enter submit for creating new tasks.
 * Integrates with tRPC task.create mutation for backend persistence.
 */

import { useRef, useState, useEffect, useCallback, type KeyboardEvent } from "react";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { trpc } from "@/trpc";

interface TaskInputProps {
  /** Callback fired after successful task creation */
  onTaskCreated?: (taskId: string) => void;
  /** Placeholder text for the textarea */
  placeholder?: string;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Generate a unique task ID in the format task-{timestamp}-{random}
 */
function generateTaskId(): string {
  const timestamp = Date.now();
  const random = Math.random().toString(16).slice(2, 6);
  return `task-${timestamp}-${random}`;
}

/**
 * Detect if running on Mac for keyboard shortcut display
 */
const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

export function TaskInput({
  onTaskCreated,
  placeholder = "Describe your task...",
  className,
}: TaskInputProps) {
  const [value, setValue] = useState("");
  const [selectedPreset, setSelectedPreset] = useState<string | undefined>(() => {
    // Restore session selection if available (resets on page refresh via sessionStorage)
    return sessionStorage.getItem("hats-preset-selection") ?? undefined;
  });
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const utils = trpc.useUtils();
  const presetsQuery = trpc.presets.list.useQuery();

  // Default to "default" (from config) when no session selection and data is loaded
  useEffect(() => {
    if (presetsQuery.data && !selectedPreset) {
      setSelectedPreset("default");
    }
  }, [presetsQuery.data, selectedPreset]);

  const handlePresetChange = (value: string) => {
    setSelectedPreset(value);
    sessionStorage.setItem("hats-preset-selection", value);
  };

  const createMutation = trpc.task.create.useMutation({
    onSuccess: (task) => {
      setValue("");
      // Reset textarea height after clearing
      if (textareaRef.current) {
        textareaRef.current.style.height = "auto";
      }
      // Invalidate task list to show new task
      utils.task.list.invalidate();
      utils.task.ready.invalidate();
      onTaskCreated?.(task.id);
    },
  });

  // Auto-resize textarea as content changes
  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    // Reset height to auto to get correct scrollHeight
    textarea.style.height = "auto";
    // Set to scrollHeight to fit content
    textarea.style.height = `${textarea.scrollHeight}px`;
  }, [value]);

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed || createMutation.isPending) return;

    createMutation.mutate({
      id: generateTaskId(),
      title: trimmed,
      status: "open",
      priority: 2,
      preset: selectedPreset,
    });
  }, [value, createMutation, selectedPreset]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      // Cmd+Enter on Mac, Ctrl+Enter on other platforms
      const isSubmitKey = (isMac ? e.metaKey : e.ctrlKey) && e.key === "Enter";
      if (isSubmitKey) {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit]
  );

  const isDisabled = createMutation.isPending;
  const hasValue = value.trim().length > 0;
  const presetsDisabled = presetsQuery.isLoading || !presetsQuery.data;

  return (
    <div className={cn("space-y-3", className)}>
      <select
        aria-label="Preset"
        value={selectedPreset ?? "default"}
        onChange={(e) => handlePresetChange(e.target.value)}
        disabled={presetsDisabled}
        className={cn(
          "w-full rounded-md border border-input bg-background px-3 py-2 text-sm",
          presetsDisabled && "opacity-50 cursor-not-allowed"
        )}
      >
        <option value="default">Default (from config)</option>
        {presetsQuery.data?.map((preset) => (
          <option key={preset.id} value={preset.id}>
            {preset.name} ({preset.source})
          </option>
        ))}
      </select>

      <Textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={isDisabled}
        className={cn(
          "resize-none min-h-[80px] max-h-[300px] overflow-y-auto",
          isDisabled && "opacity-50 cursor-not-allowed"
        )}
        aria-label="Task description"
      />

      <div className="flex items-center justify-between">
        <span className="text-xs text-muted-foreground">
          {isMac ? "⌘" : "Ctrl"}+Enter to submit
        </span>

        <Button onClick={handleSubmit} disabled={isDisabled || !hasValue} size="sm">
          {isDisabled ? (
            <>
              <span
                className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
                role="status"
                aria-label="Creating task"
              />
              Creating...
            </>
          ) : (
            "Create Task"
          )}
        </Button>
      </div>

      {createMutation.isError && (
        <div className="text-sm text-destructive" role="alert">
          Error: {createMutation.error.message}
        </div>
      )}
    </div>
  );
}
