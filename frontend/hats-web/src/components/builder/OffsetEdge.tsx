/**
 * OffsetEdge Component
 *
 * Custom smoothstep edge styled like ComfyUI: thick, solid, color-coded
 * by event name. Offsets parallel edges so they don't overlap.
 */

import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  useStore,
  type EdgeProps,
} from "@xyflow/react";

/** Assign a stable color based on the event/label string */
const EVENT_COLORS: Record<string, string> = {
  "build.task": "#60a5fa",       // blue-400
  "build.done": "#4ade80",       // green-400
  "build.blocked": "#f87171",    // red-400
  "review.approved": "#a78bfa",  // violet-400
  "review.changes_requested": "#fb923c", // orange-400
  "validation.passed": "#2dd4bf", // teal-400
  "validation.failed": "#f472b6", // pink-400
  "confession.clean": "#a3e635",  // lime-400
  "confession.issues_found": "#fbbf24", // amber-400
  "work.start": "#38bdf8",       // sky-400
};

const FALLBACK_COLORS = [
  "#60a5fa", "#4ade80", "#f87171", "#a78bfa", "#fb923c",
  "#2dd4bf", "#f472b6", "#a3e635", "#fbbf24", "#38bdf8",
];

function getEdgeColor(label?: string): string {
  if (!label) return "#94a3b8"; // slate-400
  const str = String(label);
  if (EVENT_COLORS[str]) return EVENT_COLORS[str];
  // Hash fallback for unknown events
  let hash = 0;
  for (let i = 0; i < str.length; i++) hash = (hash * 31 + str.charCodeAt(i)) | 0;
  return FALLBACK_COLORS[Math.abs(hash) % FALLBACK_COLORS.length];
}

export function OffsetEdge({
  id,
  source,
  target,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  label,
  markerEnd,
}: EdgeProps) {
  // Find sibling edges (same source→target pair, either direction)
  const siblingInfo = useStore((s) => {
    const siblings = s.edges.filter(
      (e) =>
        (e.source === source && e.target === target) ||
        (e.source === target && e.target === source)
    );
    const index = siblings.findIndex((e) => e.id === id);
    return { count: siblings.length, index };
  });

  // Offset the control point to separate parallel edges
  const spacing = 25;
  const curvature = 0.25;
  const offsetY = siblingInfo.count <= 1 ? 0 : (siblingInfo.index - (siblingInfo.count - 1) / 2) * spacing;

  const [path, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY: sourceY + offsetY,
    sourcePosition,
    targetX,
    targetY: targetY + offsetY,
    targetPosition,
    curvature,
  });

  const color = getEdgeColor(label as string | undefined);

  return (
    <>
      {/* Glow layer */}
      <path
        d={path}
        fill="none"
        stroke={color}
        strokeWidth={10}
        strokeOpacity={0.15}
      />
      {/* Main edge — solid, thick */}
      <BaseEdge
        path={path}
        markerEnd={markerEnd}
        style={{ stroke: color, strokeWidth: 4 }}
      />
      {label && (
        <EdgeLabelRenderer>
          <div
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX}px,${labelY}px)`,
              pointerEvents: "all",
              borderLeft: `3px solid ${color}`,
            }}
            className="text-xs font-semibold text-slate-100 bg-slate-900/95 px-2 py-1 rounded"
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}
