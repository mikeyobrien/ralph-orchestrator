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

import { useCallback, useState, useRef, DragEvent, useMemo } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Controls,
  Background,
  MiniMap,
  addEdge,
  applyNodeChanges,
  applyEdgeChanges,
  type Node,
  type Edge,
  type EdgeTypes,
  type OnNodesChange,
  type OnEdgesChange,
  type OnConnect,
  type NodeTypes,
  BackgroundVariant,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { HatNode, type HatNodeData } from "./HatNode";
import { RerouteNode } from "./RerouteNode";
import { OffsetEdge } from "./OffsetEdge";
import { HatPalette } from "./HatPalette";
import { PropertiesPanel } from "./PropertiesPanel";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { Save, Download } from "lucide-react";
import { v4 as uuidv4 } from "uuid";

/** Custom node types for React Flow - using 'any' to work around strict React Flow types */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const nodeTypes: NodeTypes = {
  hatNode: HatNode as any,
  reroute: RerouteNode as any,
};

const edgeTypes: EdgeTypes = {
  offset: OffsetEdge,
};

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
  className,
}: CollectionBuilderProps) {
  const reactFlowWrapper = useRef<HTMLDivElement>(null);
  const [nodes, setNodes] = useState<Node[]>(initialData?.nodes ?? []);
  const [edges, setEdges] = useState<Edge[]>(initialData?.edges ?? []);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

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
      }
    }
  }, []);

  // Handle edge changes
  const onEdgesChange: OnEdgesChange = useCallback((changes) => {
    setEdges((eds) => applyEdgeChanges(changes, eds));
  }, []);

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
    },
    [nodes, edges, resolveEventLabel]
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
    },
    []
  );

  const onDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }, []);

  // Update node data from properties panel
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
  }, []);

  // Delete node
  const handleDeleteNode = useCallback((nodeId: string) => {
    setNodes((nds) => nds.filter((n) => n.id !== nodeId));
    setEdges((eds) => eds.filter((e) => e.source !== nodeId && e.target !== nodeId));
    setSelectedNodeId(null);
  }, []);

  // Save handler
  const handleSave = useCallback(() => {
    onSave({ nodes, edges, name, description });
  }, [nodes, edges, name, description, onSave]);

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {/* Toolbar */}
      <div className="flex items-center gap-3 p-3 border-b bg-background">
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
        <div className="flex items-center gap-2">
          {onExportYaml && (
            <Button variant="outline" size="sm" onClick={onExportYaml}>
              <Download className="h-4 w-4 mr-2" />
              Export YAML
            </Button>
          )}
          <Button size="sm" onClick={handleSave} disabled={isSaving || !name.trim()}>
            <Save className="h-4 w-4 mr-2" />
            {isSaving ? "Saving..." : "Save"}
          </Button>
        </div>
      </div>

      {/* Main content area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left sidebar - Hat palette */}
        <HatPalette />

        {/* Canvas */}
        <div ref={reactFlowWrapper} className="flex-1" onDrop={onDrop} onDragOver={onDragOver}>
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
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
                const prefix = key.split("-")[0];
                const colors: Record<string, string> = {
                  planner: "#8b5cf6",
                  builder: "#3b82f6",
                  reviewer: "#22c55e",
                  validator: "#f59e0b",
                  confessor: "#ef4444",
                };
                return colors[prefix] ?? "#6b7280";
              }}
              maskColor="rgba(0, 0, 0, 0.1)"
              className="!bg-background/80"
            />
          </ReactFlow>
        </div>

        {/* Right sidebar - Properties panel */}
        <PropertiesPanel
          selectedNode={selectedNode}
          onUpdateNode={handleUpdateNode}
          onDeleteNode={handleDeleteNode}
        />
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
