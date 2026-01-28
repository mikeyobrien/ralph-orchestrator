/**
 * HatPalette Component
 *
 * Sidebar component showing available hat templates that can be
 * dragged onto the canvas. Also shows preset templates.
 *
 * Features:
 * - Draggable hat templates (blank hat to customize)
 * - Preset templates from common patterns
 * - Search/filter functionality
 * - Collapsed/expanded state
 */

import { useState, DragEvent } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { Search, ChevronLeft, ChevronRight, GripVertical, Circle } from "lucide-react";
import type { HatNodeData } from "./HatNode";

/**
 * Preset hat templates for common roles
 */
const HAT_TEMPLATES: HatNodeData[] = [
  {
    key: "planner",
    name: "Planner",
    description: "Analyzes tasks and creates implementation plans",
    triggersOn: ["work.start", "build.blocked"],
    publishes: ["build.task"],
  },
  {
    key: "builder",
    name: "Builder",
    description: "Implements code, runs tests, creates commits",
    triggersOn: ["build.task"],
    publishes: ["build.done", "build.blocked"],
  },
  {
    key: "reviewer",
    name: "Reviewer",
    description: "Reviews code for quality and correctness",
    triggersOn: ["build.done"],
    publishes: ["review.approved", "review.changes_requested"],
  },
  {
    key: "validator",
    name: "Validator",
    description: "Runs validation checks and tests",
    triggersOn: ["build.done"],
    publishes: ["validation.passed", "validation.failed"],
  },
  {
    key: "confessor",
    name: "Confessor",
    description: "Self-assesses work and identifies issues",
    triggersOn: ["build.done"],
    publishes: ["confession.clean", "confession.issues_found"],
  },
  {
    key: "custom",
    name: "Custom Hat",
    description: "A blank hat to customize",
    triggersOn: [],
    publishes: [],
  },
];

interface HatPaletteProps {
  /** Optional className */
  className?: string;
}

/**
 * PaletteItem - a draggable hat template
 */
function PaletteItem({ template }: { template: HatNodeData }) {
  const onDragStart = (event: DragEvent<HTMLDivElement>) => {
    // Store the template data in the drag event for the drop handler
    event.dataTransfer.setData("application/reactflow", JSON.stringify(template));
    event.dataTransfer.effectAllowed = "move";
  };

  return (
    <div
      draggable
      onDragStart={onDragStart}
      className={cn(
        "group flex items-start gap-2 p-2 rounded-md border border-transparent",
        "bg-muted/30 hover:bg-muted/50 hover:border-border",
        "cursor-grab active:cursor-grabbing transition-colors"
      )}
    >
      <GripVertical className="h-4 w-4 mt-0.5 text-muted-foreground/50 group-hover:text-muted-foreground" />
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm">🎩</span>
          <span className="font-medium text-sm truncate">{template.name}</span>
        </div>
        <p className="text-xs text-muted-foreground line-clamp-2 mt-0.5">
          {template.description}
        </p>
        <div className="flex items-center gap-2 mt-1">
          {template.triggersOn.length > 0 && (
            <Badge variant="secondary" className="text-xs px-1 py-0">
              {template.triggersOn.length} in
            </Badge>
          )}
          {template.publishes.length > 0 && (
            <Badge variant="outline" className="text-xs px-1 py-0">
              {template.publishes.length} out
            </Badge>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * RerouteItem - draggable reroute waypoint
 */
function RerouteItem() {
  const onDragStart = (event: DragEvent<HTMLDivElement>) => {
    event.dataTransfer.setData("application/reroute", "true");
    event.dataTransfer.effectAllowed = "move";
  };

  return (
    <div
      draggable
      onDragStart={onDragStart}
      className={cn(
        "group flex items-center gap-2 p-2 rounded-md border border-transparent",
        "bg-muted/30 hover:bg-muted/50 hover:border-border",
        "cursor-grab active:cursor-grabbing transition-colors"
      )}
    >
      <GripVertical className="h-4 w-4 text-muted-foreground/50 group-hover:text-muted-foreground" />
      <Circle className="h-3 w-3 text-muted-foreground fill-muted-foreground" />
      <div>
        <span className="font-medium text-sm">Reroute</span>
        <p className="text-xs text-muted-foreground">Waypoint for connection routing</p>
      </div>
    </div>
  );
}

/**
 * HatPalette - sidebar with draggable hat templates
 */
export function HatPalette({ className }: HatPaletteProps) {
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const filteredTemplates = HAT_TEMPLATES.filter(
    (template) =>
      template.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      template.description.toLowerCase().includes(searchQuery.toLowerCase())
  );

  if (isCollapsed) {
    return (
      <div className={cn("w-10 bg-background border-r flex flex-col items-center py-2", className)}>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setIsCollapsed(false)}
          className="w-8 h-8 p-0"
        >
          <ChevronRight className="h-4 w-4" />
        </Button>
        <div className="flex-1 flex flex-col items-center justify-center">
          <span className="text-lg rotate-90 whitespace-nowrap text-muted-foreground text-xs tracking-wider">
            HAT PALETTE
          </span>
        </div>
      </div>
    );
  }

  return (
    <Card className={cn("w-64 rounded-none border-t-0 border-b-0 border-l-0", className)}>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm font-medium">Hat Palette</CardTitle>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setIsCollapsed(true)}
            className="h-6 w-6 p-0"
          >
            <ChevronLeft className="h-4 w-4" />
          </Button>
        </div>
        <div className="relative mt-2">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
          <Input
            placeholder="Search templates..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-7 h-8 text-xs"
          />
        </div>
      </CardHeader>
      <CardContent className="space-y-1 overflow-y-auto max-h-[calc(100vh-200px)]">
        <p className="text-xs text-muted-foreground mb-2">
          Drag a hat template onto the canvas to add it
        </p>
        {filteredTemplates.map((template) => (
          <PaletteItem key={template.key} template={template} />
        ))}
        {filteredTemplates.length === 0 && (
          <p className="text-xs text-muted-foreground text-center py-4">
            No matching templates
          </p>
        )}

        {/* Utilities */}
        <div className="border-t pt-2 mt-2">
          <p className="text-xs text-muted-foreground mb-1.5">Utilities</p>
          <RerouteItem />
        </div>
      </CardContent>
    </Card>
  );
}
