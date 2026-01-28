/**
 * RerouteNode Component
 *
 * A tiny pass-through node for routing connections around the canvas.
 * One input handle, one output handle — a small dot with visible grab areas.
 * Inspired by ComfyUI's Reroute node.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import { cn } from "@/lib/utils";

interface RerouteNodeProps {
  selected?: boolean;
}

function RerouteNodeComponent({ selected }: RerouteNodeProps) {
  return (
    <div
      className={cn(
        "w-5 h-5 rounded-full border-2 transition-all relative",
        selected
          ? "bg-primary border-primary shadow-lg shadow-primary/40"
          : "bg-muted-foreground/60 border-muted-foreground hover:bg-muted-foreground/80"
      )}
    >
      <Handle
        type="target"
        position={Position.Left}
        className="!w-3 !h-3 !bg-blue-500 !border-2 !border-blue-700 !-left-[7px] hover:!bg-blue-400 !cursor-crosshair"
      />
      <Handle
        type="source"
        position={Position.Right}
        className="!w-3 !h-3 !bg-green-500 !border-2 !border-green-700 !-right-[7px] hover:!bg-green-400 !cursor-crosshair"
      />
    </div>
  );
}

export const RerouteNode = memo(RerouteNodeComponent);
