/**
 * HatNode Component
 *
 * Custom React Flow node representing a hat in the visual builder.
 * Displays hat metadata with connection handles for triggers (inputs)
 * and publishes (outputs).
 *
 * Features:
 * - Visual card display with hat name and description
 * - Dynamic input handles for trigger events
 * - Dynamic output handles for publish events
 * - Selected state highlighting
 * - Compact layout optimized for workflow canvas
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import { cn } from "@/lib/utils";

/**
 * Data structure for a hat node
 */
export interface HatNodeData {
  key: string;
  name: string;
  description: string;
  triggersOn: string[];
  publishes: string[];
  instructions?: string;
}

/**
 * Props for HatNode component
 */
interface HatNodeProps {
  data: HatNodeData;
  selected?: boolean;
}

/**
 * HatNode - visual representation of a hat in the workflow
 */
function HatNodeComponent({ data, selected }: HatNodeProps) {
  const hatData = data;

  return (
    <div
      className={cn(
        "bg-card border rounded-lg shadow-md min-w-[180px] max-w-[280px]",
        "transition-all duration-200",
        selected
          ? "border-primary ring-2 ring-primary/30 shadow-lg"
          : "border-border hover:border-muted-foreground/50"
      )}
    >
      {/* Input handles (triggers) - placed on left side */}
      <div className="absolute -left-[9px] top-0 h-full flex flex-col justify-center gap-2 py-3">
        {hatData.triggersOn.map((trigger, index) => (
          <div key={trigger} className="relative group">
            <Handle
              type="target"
              position={Position.Left}
              id={trigger}
              className={cn(
                "!w-[18px] !h-[18px] !bg-blue-500 !border-2 !border-blue-700",
                "hover:!bg-blue-400 transition-colors cursor-crosshair"
              )}
              style={{ top: `${30 + index * 24}px`, position: "absolute" }}
            />
            <span className="absolute left-5 whitespace-nowrap text-xs text-muted-foreground bg-background/80 px-1 rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none">
              {trigger}
            </span>
          </div>
        ))}
        {/* Default input handle if no triggers defined */}
        {hatData.triggersOn.length === 0 && (
          <Handle
            type="target"
            position={Position.Left}
            id="default-in"
            className={cn(
              "!w-[18px] !h-[18px] !bg-gray-400 !border-2 !border-gray-600",
              "hover:!bg-gray-300 transition-colors cursor-crosshair"
            )}
          />
        )}
      </div>

      {/* Node content */}
      <div className="p-3">
        {/* Header with name */}
        <div className="flex items-center gap-2 mb-1">
          <span className="text-lg">🎩</span>
          <h3 className="font-semibold text-sm truncate">{hatData.name}</h3>
        </div>

        {/* Description */}
        <p className="text-xs text-muted-foreground line-clamp-2 mb-2">
          {hatData.description}
        </p>

        {/* Triggers/Publishes summary */}
        <div className="flex items-center justify-between text-xs">
          <span className="text-blue-500">
            {hatData.triggersOn.length} trigger{hatData.triggersOn.length !== 1 ? "s" : ""}
          </span>
          <span className="text-green-500">
            {hatData.publishes.length} publish{hatData.publishes.length !== 1 ? "es" : ""}
          </span>
        </div>
      </div>

      {/* Output handles (publishes) - placed on right side */}
      <div className="absolute -right-[9px] top-0 h-full flex flex-col justify-center gap-2 py-3">
        {hatData.publishes.map((publish, index) => (
          <div key={publish} className="relative group">
            <Handle
              type="source"
              position={Position.Right}
              id={publish}
              className={cn(
                "!w-[18px] !h-[18px] !bg-green-500 !border-2 !border-green-700",
                "hover:!bg-green-400 transition-colors cursor-crosshair"
              )}
              style={{ top: `${30 + index * 24}px`, position: "absolute" }}
            />
            <span className="absolute right-5 whitespace-nowrap text-xs text-muted-foreground bg-background/80 px-1 rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none">
              {publish}
            </span>
          </div>
        ))}
        {/* Default output handle if no publishes defined */}
        {hatData.publishes.length === 0 && (
          <Handle
            type="source"
            position={Position.Right}
            id="default-out"
            className={cn(
              "!w-[18px] !h-[18px] !bg-gray-400 !border-2 !border-gray-600",
              "hover:!bg-gray-300 transition-colors cursor-crosshair"
            )}
          />
        )}
      </div>
    </div>
  );
}

export const HatNode = memo(HatNodeComponent);
