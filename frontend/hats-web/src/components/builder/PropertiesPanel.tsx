/**
 * PropertiesPanel Component
 *
 * Right sidebar panel for editing the properties of a selected hat node.
 * Allows editing name, description, triggers, publishes, and instructions.
 *
 * Features:
 * - Form-based editing for selected node
 * - Tag input for triggers and publishes
 * - Real-time updates to the node
 * - Delete node button
 */

import { useState, useEffect, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { X, Plus, Trash2, ChevronLeft, ChevronRight } from "lucide-react";
import type { HatNodeData } from "./HatNode";

interface PropertiesPanelProps {
  /** Selected node data (null if no selection) */
  selectedNode: { id: string; data: HatNodeData } | null;
  /** Callback when node data is updated */
  onUpdateNode: (nodeId: string, data: Partial<HatNodeData>) => void;
  /** Callback when node is deleted */
  onDeleteNode: (nodeId: string) => void;
  /** Optional className */
  className?: string;
}

/**
 * TagEditor - inline editor for string arrays (triggers/publishes)
 */
function TagEditor({
  value,
  onChange,
  placeholder,
  label,
  variant,
}: {
  value: string[];
  onChange: (tags: string[]) => void;
  placeholder: string;
  label: string;
  variant: "input" | "output";
}) {
  const [inputValue, setInputValue] = useState("");
  const [error, setError] = useState<string | null>(null);

  const addTag = useCallback(() => {
    const tag = inputValue.trim();
    if (!tag || value.includes(tag)) return;

    // task.* events are reserved for the orchestrator (triggers only)
    if (variant === "input" && tag.startsWith("task.")) {
      setError("task.* events are reserved for the orchestrator");
      return;
    }

    setError(null);
    onChange([...value, tag]);
    setInputValue("");
  }, [inputValue, value, onChange, variant]);

  const removeTag = useCallback(
    (tagToRemove: string) => {
      onChange(value.filter((tag) => tag !== tagToRemove));
    },
    [value, onChange]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        addTag();
      } else if (e.key === "Backspace" && !inputValue && value.length > 0) {
        removeTag(value[value.length - 1]);
      }
    },
    [addTag, inputValue, value, removeTag]
  );

  const badgeVariant = variant === "input" ? "secondary" : "outline";
  const colorClass = variant === "input" ? "bg-blue-500/10 border-blue-500/30" : "bg-green-500/10 border-green-500/30";

  return (
    <div className="space-y-1.5">
      <Label className="text-xs">{label}</Label>
      <div
        className={cn(
          "flex flex-wrap gap-1 p-1.5 rounded-md border bg-transparent min-h-[32px]",
          "focus-within:ring-1 focus-within:ring-ring",
          error && "border-destructive"
        )}
      >
        {value.map((tag) => (
          <Badge
            key={tag}
            variant={badgeVariant}
            className={cn("text-xs font-mono flex items-center gap-1 py-0", colorClass)}
          >
            {tag}
            <button
              type="button"
              onClick={() => removeTag(tag)}
              className="hover:text-destructive transition-colors"
            >
              <X className="h-2.5 w-2.5" />
            </button>
          </Badge>
        ))}
        <input
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={addTag}
          placeholder={value.length === 0 ? placeholder : ""}
          className="flex-1 min-w-[80px] bg-transparent outline-none text-xs"
        />
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={addTag}
          disabled={!inputValue.trim()}
          className="h-5 w-5 p-0"
        >
          <Plus className="h-3 w-3" />
        </Button>
      </div>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

/**
 * PropertiesPanel - form for editing selected node
 */
export function PropertiesPanel({
  selectedNode,
  onUpdateNode,
  onDeleteNode,
  className,
}: PropertiesPanelProps) {
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [localData, setLocalData] = useState<HatNodeData | null>(null);

  // Sync local state when selection changes
  useEffect(() => {
    if (selectedNode) {
      setLocalData({ ...selectedNode.data });
    } else {
      setLocalData(null);
    }
  }, [selectedNode]);

  // Update field handler
  const updateField = useCallback(
    <K extends keyof HatNodeData>(field: K, value: HatNodeData[K]) => {
      if (!selectedNode || !localData) return;
      const updated = { ...localData, [field]: value };
      setLocalData(updated);
      onUpdateNode(selectedNode.id, { [field]: value });
    },
    [selectedNode, localData, onUpdateNode]
  );

  const handleDelete = useCallback(() => {
    if (selectedNode && confirm("Delete this hat? This cannot be undone.")) {
      onDeleteNode(selectedNode.id);
    }
  }, [selectedNode, onDeleteNode]);

  if (isCollapsed) {
    return (
      <div className={cn("w-10 bg-background border-l flex flex-col items-center py-2", className)}>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setIsCollapsed(false)}
          className="w-8 h-8 p-0"
        >
          <ChevronLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1 flex flex-col items-center justify-center">
          <span className="text-lg -rotate-90 whitespace-nowrap text-muted-foreground text-xs tracking-wider">
            PROPERTIES
          </span>
        </div>
      </div>
    );
  }

  return (
    <Card className={cn("w-72 rounded-none border-t-0 border-b-0 border-r-0", className)}>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm font-medium">Properties</CardTitle>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setIsCollapsed(true)}
            className="h-6 w-6 p-0"
          >
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3 overflow-y-auto max-h-[calc(100vh-200px)]">
        {!selectedNode || !localData ? (
          <p className="text-xs text-muted-foreground text-center py-8">
            Select a hat on the canvas to edit its properties
          </p>
        ) : (
          <>
            {/* Key (read-only) */}
            <div className="space-y-1">
              <Label className="text-xs">Key</Label>
              <Input
                value={localData.key}
                disabled
                className="h-8 text-xs font-mono opacity-60"
              />
            </div>

            {/* Name */}
            <div className="space-y-1">
              <Label className="text-xs">Name</Label>
              <Input
                value={localData.name}
                onChange={(e) => updateField("name", e.target.value)}
                className="h-8 text-xs"
                placeholder="Hat name"
              />
            </div>

            {/* Description */}
            <div className="space-y-1">
              <Label className="text-xs">Description</Label>
              <Textarea
                value={localData.description}
                onChange={(e) => updateField("description", e.target.value)}
                className="text-xs min-h-[60px]"
                placeholder="What does this hat do?"
              />
            </div>

            {/* Triggers */}
            <TagEditor
              value={localData.triggersOn}
              onChange={(tags) => updateField("triggersOn", tags)}
              placeholder="Add trigger events..."
              label="Triggers On (Inputs)"
              variant="input"
            />

            {/* Publishes */}
            <TagEditor
              value={localData.publishes}
              onChange={(tags) => updateField("publishes", tags)}
              placeholder="Add publish events..."
              label="Publishes (Outputs)"
              variant="output"
            />

            {/* Instructions */}
            <div className="space-y-1">
              <Label className="text-xs">Instructions</Label>
              <Textarea
                value={localData.instructions || ""}
                onChange={(e) => updateField("instructions", e.target.value || undefined)}
                className="text-xs font-mono min-h-[80px]"
                placeholder="Optional instructions for this hat..."
              />
            </div>

            {/* Delete button */}
            <div className="pt-3 border-t">
              <Button
                variant="destructive"
                size="sm"
                onClick={handleDelete}
                className="w-full"
              >
                <Trash2 className="h-3.5 w-3.5 mr-2" />
                Delete Hat
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
