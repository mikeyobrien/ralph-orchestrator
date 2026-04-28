/**
 * CollectionBuilder Component
 *
 * Main visual workflow builder for hat collections. Uses React Flow
 * to provide a canvas where users can:
 * - Drag and drop hat nodes from the palette
 * - Connect hats via event edges (publishes → triggers)
 * - Edit node properties in the side panel
 * - Save the collection to the database
 * - Export as YAML preset
 *
 * This is the n8n-style builder the user requested.
 */

import {
    Background,
    BackgroundVariant,
    Controls,
    MiniMap,
    ReactFlow,
    ReactFlowProvider,
    addEdge,
    applyEdgeChanges,
    applyNodeChanges,
    type Edge,
    type EdgeTypes,
    type Node,
    type NodeTypes,
    type OnConnect,
    type OnEdgesChange,
    type OnNodesChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { DragEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useLoopObservation } from "@/hooks/useLoopObservation";
import { cn } from "@/lib/utils";
import { useObservationStore } from "@/stores/observationStore";
import { AlertCircle, CheckCircle2, Download, Save } from "lucide-react";
import { v4 as uuidv4 } from "uuid";
import { HatNode, type HatNodeData } from "./HatNode";
import { HatPalette } from "./HatPalette";
import { autoLayout, needsLayout } from "./layout";
import { OffsetEdge } from "./OffsetEdge";
import { PropertiesPanel } from "./PropertiesPanel";
import { RerouteNode } from "./RerouteNode";
import { getRoleMeta } from "./roles";

/** Custom node types for React Flow - using 'any' to work around strict React Flow types */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const nodeTypes: NodeTypes = {
  hatNode: HatNode as any,
  reroute: RerouteNode as any,
};

const edgeTypes: EdgeTypes = {
  offset: OffsetEdge,
};

/**
 * Inline legend shown in the observation toolbar. Explains what each
 * node-ring color means so the user doesn't have to guess.
 */
function ObservationLegend() {
  return (
    <div className="flex items-center gap-3 text-xs text-muted-foreground">
      <span className="flex items-center gap-1.5">
        <span className="inline-block w-3 h-3 rounded border-2 border-sky-400 bg-transparent" aria-hidden />
        pending
      </span>
      <span className="flex items-center gap-1.5">
        <span className="inline-block w-3 h-3 rounded border-2 border-red-500 bg-transparent animate-border-pulse" aria-hidden />
        active
      </span>
      <span className="flex items-center gap-1.5">
        <span className="inline-block w-3 h-3 rounded border-2 border-teal-400 bg-transparent" aria-hidden />
        done ✓
      </span>
    </div>
  );
}

interface CollectionBuilderProps {
  /** Collection ID (null for new collection) */
  collectionId: string | null;
  /** Initial graph data (from API or empty) */
  initialData?: {
    nodes: Node[];
    edges: Edge[];
  };
  /** Collection metadata */
  name: string;
  description: string;
  /** Callback when save is requested */
  onSave: (data: { nodes: Node[]; edges: Edge[]; name: string; description: string }) => void;
  /** Callback to export as YAML */
  onExportYaml?: () => void;
  /** Callback when name changes */
  onNameChange: (name: string) => void;
  /** Callback when description changes */
  onDescriptionChange: (description: string) => void;
  /** Whether save is in progress */
  isSaving?: boolean;
  /** Parent-owned dirty flag. Controls Badge + beforeunload. */
  isDirty?: boolean;
  /** Called whenever an internal change occurs so the parent can flip isDirty. */
  onMarkDirty?: () => void;
  /** Post-save feedback driven by the parent's mutation result. */
  saveStatus?: "idle" | "success" | "error";
  /** True when the collection was just imported — triggers auto-layout on mount. */
  justImported?: boolean;
  /** Optional className */
  className?: string;
}

/**
 * CollectionBuilder - main workflow canvas component
 */
function CollectionBuilderInner({
  initialData,
  name,
  description,
  onSave,
  onExportYaml,
  onNameChange,
  onDescriptionChange,
  isSaving,
  isDirty,
  onMarkDirty,
  saveStatus,
  justImported,
  className,
}: CollectionBuilderProps) {
  const reactFlowWrapper = useRef<HTMLDivElement>(null);
  // On mount, auto-layout freshly imported collections so the user lands on a
  // sensible graph instead of the Rust importer's stacked column. Uses lazy
  // initial state so autoLayout only runs once (useState initialiser semantics).
  const [initialNodes] = useState<Node[]>(() => {
    const raw = initialData?.nodes ?? [];
    if (justImported && needsLayout(raw)) {
      return autoLayout(raw, initialData?.edges ?? []);
    }
    return raw;
  });
  const [nodes, setNodes] = useState<Node[]>(initialNodes);
  const [edges, setEdges] = useState<Edge[]>(initialData?.edges ?? []);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  // If auto-layout ran, the rendered graph differs from the persisted one —
  // mark dirty so the user saves the nicer positions.
  useEffect(() => {
    if (justImported && initialData && needsLayout(initialData.nodes)) {
      onMarkDirty?.();
    }
    // Intentionally only on mount: subsequent re-renders should not re-fire.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Browser-level guard for tab close / reload while there are unsaved changes.
  useEffect(() => {
    if (!isDirty) return;
    const handler = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [isDirty]);

  // Get selected node with data (only hat nodes have editable properties)
  const selectedNode = useMemo(() => {
    if (!selectedNodeId) return null;
    const node = nodes.find((n) => n.id === selectedNodeId);
    if (!node || node.type !== "hatNode") return null;
    return { id: node.id, data: node.data as unknown as HatNodeData };
  }, [selectedNodeId, nodes]);

  // Handle node changes (position, selection, etc.)
  const onNodesChange: OnNodesChange = useCallback((changes) => {
    setNodes((nds) => applyNodeChanges(changes, nds));

    // Track selection changes
    for (const change of changes) {
      if (change.type === "select") {
        setSelectedNodeId(change.selected ? change.id : null);
      } else if (change.type === "position" || change.type === "add" || change.type === "remove") {
        onMarkDirty?.();
      }
    }
  }, [onMarkDirty]);

  // Handle edge changes
  const onEdgesChange: OnEdgesChange = useCallback((changes) => {
    setEdges((eds) => applyEdgeChanges(changes, eds));
    for (const change of changes) {
      if (change.type === "add" || change.type === "remove") {
        onMarkDirty?.();
      }
    }
  }, [onMarkDirty]);

  // Keyboard-driven node deletion removes the node via onNodesChange but
  // leaves connected edges orphaned — clean them up here.
  const handleNodesDelete = useCallback((deleted: Node[]) => {
    const ids = new Set(deleted.map((n) => n.id));
    setEdges((eds) => eds.filter((e) => !ids.has(e.source) && !ids.has(e.target)));
    onMarkDirty?.();
  }, [onMarkDirty]);

  // Trace back through reroute nodes to find the original event name
  const resolveEventLabel = useCallback(
    (sourceId: string, sourceHandle: string | null, currentEdges: Edge[]): string => {
      // If sourceHandle is a real event name (from a hat node), use it
      if (sourceHandle && sourceHandle !== "default-out") return sourceHandle;
      // Source is a reroute node — find any edge feeding into it
      const incoming = currentEdges.find((e) => e.target === sourceId);
      if (incoming) return String(incoming.label ?? "event");
      return "event";
    },
    []
  );

  // Handle new connections
  const onConnect: OnConnect = useCallback(
    (connection) => {
      const sourceNode = nodes.find((n) => n.id === connection.source);
      const isRerouteSource = sourceNode?.type === "reroute";

      const label = isRerouteSource
        ? resolveEventLabel(connection.source!, connection.sourceHandle ?? null, edges)
        : connection.sourceHandle || "event";

      const newEdge: Edge = {
        id: `edge-${uuidv4()}`,
        source: connection.source!,
        target: connection.target!,
        sourceHandle: connection.sourceHandle,
        targetHandle: connection.targetHandle,
        label,
        type: "offset",
      };
      setEdges((eds) => addEdge(newEdge, eds));
      onMarkDirty?.();
    },
    [nodes, edges, resolveEventLabel, onMarkDirty]
  );

  // Handle drop from palette
  const onDrop = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();

      const reactFlowBounds = reactFlowWrapper.current?.getBoundingClientRect();
      if (!reactFlowBounds) return;

      // Reroute node drop
      if (event.dataTransfer.getData("application/reroute")) {
        const position = {
          x: event.clientX - reactFlowBounds.left - 8,
          y: event.clientY - reactFlowBounds.top - 8,
        };
        const nodeId = `reroute-${uuidv4().slice(0, 8)}`;
        setNodes((nds) => [
          ...nds,
          { id: nodeId, type: "reroute", position, data: {} },
        ]);
        onMarkDirty?.();
        return;
      }

      const dataStr = event.dataTransfer.getData("application/reactflow");
      if (!dataStr) return;

      const templateData = JSON.parse(dataStr) as HatNodeData;

      // Calculate drop position
      const position = {
        x: event.clientX - reactFlowBounds.left - 90, // Center the node
        y: event.clientY - reactFlowBounds.top - 40,
      };

      // Create unique key for this instance
      const nodeId = `${templateData.key}-${uuidv4().slice(0, 8)}`;
      const newNode: Node = {
        id: nodeId,
        type: "hatNode",
        position,
        data: {
          ...templateData,
          key: nodeId,
        },
      };

      setNodes((nds) => [...nds, newNode]);
      setSelectedNodeId(nodeId);
      onMarkDirty?.();
    },
    [onMarkDirty]
  );

  const onDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }, []);

  // Update node data from properties panel. When publishes/triggersOn change,
  // remove any edges that reference handles the node no longer exposes.
  const handleUpdateNode = useCallback((nodeId: string, data: Partial<HatNodeData>) => {
    setNodes((nds) =>
      nds.map((node) => {
        if (node.id === nodeId) {
          return {
            ...node,
            data: { ...node.data, ...data },
          };
        }
        return node;
      })
    );

    if (data.publishes !== undefined) {
      const pubs = new Set(data.publishes);
      setEdges((eds) => {
        const before = eds.length;
        const filtered = eds.filter(
          (e) => !(e.source === nodeId && e.sourceHandle && !pubs.has(e.sourceHandle))
        );
        const removed = before - filtered.length;
        if (removed > 0) console.info(`Removed ${removed} invalid edge(s)`);
        return filtered;
      });
    }

    if (data.triggersOn !== undefined) {
      const trigs = new Set(data.triggersOn);
      setEdges((eds) => {
        const before = eds.length;
        const filtered = eds.filter(
          (e) => !(e.target === nodeId && e.targetHandle && !trigs.has(e.targetHandle))
        );
        const removed = before - filtered.length;
        if (removed > 0) console.info(`Removed ${removed} invalid edge(s)`);
        return filtered;
      });
    }

    onMarkDirty?.();
  }, [onMarkDirty]);

  // Delete node
  const handleDeleteNode = useCallback((nodeId: string) => {
    setNodes((nds) => nds.filter((n) => n.id !== nodeId));
    setEdges((eds) => eds.filter((e) => e.source !== nodeId && e.target !== nodeId));
    setSelectedNodeId(null);
    onMarkDirty?.();
  }, [onMarkDirty]);

  // ── Live observation overlay ──────────────────────────────────────────
  const observing = useObservationStore((s) => s.active);
  const obsNodeStates = useObservationStore((s) => s.nodeStates);
  const lastFiredEdgeId = useObservationStore((s) => s.lastFiredEdgeId);
  const obsIteration = useObservationStore((s) => s.iteration);
  const obsActiveHat = useObservationStore((s) => s.activeHatId);

  // Subscribe to loop.orchestration events when observing.
  useLoopObservation(edges);

  // Overlay observation state onto nodes for rendering.
  const displayNodes = useMemo(() => {
    if (!observing) return nodes;
    return nodes.map((node) => {
      const obsState = obsNodeStates[node.id];
      if (!obsState || obsState === "idle") return node;
      return {
        ...node,
        data: { ...node.data, observationState: obsState },
      };
    });
  }, [nodes, observing, obsNodeStates]);

  // Overlay fired state onto edges for rendering.
  const displayEdges = useMemo(() => {
    if (!observing || !lastFiredEdgeId) return edges;
    return edges.map((edge) => {
      if (edge.id !== lastFiredEdgeId) return edge;
      return { ...edge, data: { ...edge.data, fired: true } };
    });
  }, [edges, observing, lastFiredEdgeId]);

  // Save handler
  const handleSave = useCallback(() => {
    onSave({ nodes, edges, name, description });
  }, [nodes, edges, name, description, onSave]);

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {/* Toolbar */}
      <div className="flex items-center gap-3 p-3 border-b bg-background flex-wrap">
        {observing ? (
          <>
            <Badge variant="secondary" className="bg-indigo-500/10 text-indigo-400 border-indigo-500/30">
              ● Observing
            </Badge>
            <span className="text-sm text-muted-foreground">
              Iteration <span className="font-mono font-bold text-foreground">{obsIteration}</span>
            </span>
            {obsActiveHat && obsActiveHat !== "loop" && (
              <Badge variant="outline" className="text-indigo-400 border-indigo-400/40">
                {getRoleMeta(obsActiveHat).emoji} {obsActiveHat}
              </Badge>
            )}
            <div className="flex-1" />
            <ObservationLegend />
          </>
        ) : (
          <>
            <Input
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder="Collection name"
              className="w-48 h-8"
            />
            <Input
              value={description}
              onChange={(e) => onDescriptionChange(e.target.value)}
              placeholder="Description"
              className="flex-1 h-8"
            />
          </>
        )}
        <div className="flex items-center gap-2">
          {!observing && isDirty && (
            <Badge variant="outline" className="text-yellow-600 border-yellow-600">
              Unsaved changes
            </Badge>
          )}
          {!observing && saveStatus === "success" && (
            <span className="flex items-center gap-1 text-sm text-green-600">
              <CheckCircle2 className="h-4 w-4" />
              Saved
            </span>
          )}
          {!observing && saveStatus === "error" && (
            <span className="flex items-center gap-1 text-sm text-destructive">
              <AlertCircle className="h-4 w-4" />
              Error saving
            </span>
          )}
          {!observing && onExportYaml && (
            <Button variant="outline" size="sm" onClick={onExportYaml}>
              <Download className="h-4 w-4 mr-2" />
              Export YAML
            </Button>
          )}
          {!observing && (
            <Button size="sm" onClick={handleSave} disabled={isSaving || !name.trim()}>
              <Save className="h-4 w-4 mr-2" />
              {isSaving ? "Saving..." : "Save"}
            </Button>
          )}
        </div>
      </div>

      {/* Main content area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left sidebar - Hat palette (hidden during observation) */}
        {!observing && <HatPalette />}

        {/* Canvas */}
        <div ref={reactFlowWrapper} className="flex-1" onDrop={observing ? undefined : onDrop} onDragOver={observing ? undefined : onDragOver}>
          <ReactFlow
            nodes={displayNodes}
            edges={displayEdges}
            onNodesChange={observing ? undefined : onNodesChange}
            onEdgesChange={observing ? undefined : onEdgesChange}
            onConnect={observing ? undefined : onConnect}
            onNodesDelete={observing ? undefined : handleNodesDelete}
            deleteKeyCode={observing ? [] : ["Backspace", "Delete"]}
            nodeTypes={nodeTypes}
            fitView
            minZoom={0.1}
            maxZoom={4}
            snapToGrid
            snapGrid={[15, 15]}
            edgeTypes={edgeTypes}
            defaultEdgeOptions={{
              type: "offset",
            }}
            colorMode="dark"
            className="bg-muted/20"
          >
            <Background variant={BackgroundVariant.Dots} gap={20} size={1} />
            <Controls position="bottom-left" />
            <MiniMap
              position="bottom-right"
              nodeColor={(node) => {
                const key = (node.data as unknown as HatNodeData)?.key ?? "";
                return getRoleMeta(key).color;
              }}
              maskColor="rgba(0, 0, 0, 0.1)"
              className="!bg-background/80"
            />
          </ReactFlow>
        </div>

        {/* Right sidebar - Properties panel (hidden during observation) */}
        {!observing && (
          <PropertiesPanel
            selectedNode={selectedNode}
            onUpdateNode={handleUpdateNode}
            onDeleteNode={handleDeleteNode}
          />
        )}
      </div>
    </div>
  );
}

/**
 * CollectionBuilder wrapped with ReactFlowProvider
 */
export function CollectionBuilder(props: CollectionBuilderProps) {
  return (
    <ReactFlowProvider>
      <CollectionBuilderInner {...props} />
    </ReactFlowProvider>
  );
}
