/**
 * BuilderPage
 *
 * Page for the visual hat collection builder. Provides:
 * - List of existing collections
 * - Create new collection
 * - Edit existing collection
 * - Export collection as YAML
 * - Import YAML as collection
 *
 * This implements the n8n-style builder for hat collections.
 */

import { CollectionBuilder, ImportYamlDialog } from "@/components/builder";
import { BLUEPRINTS, type Blueprint } from "@/components/builder/blueprints";
import { RunPromptDialog } from "@/components/builder/RunPromptDialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useObservationStore } from "@/stores/observationStore";
import type { Edge, Node } from "@xyflow/react";
import { formatDistanceToNow } from "date-fns";
import {
    AlertCircle,
    ArrowLeft,
    Clock,
    FolderOpen,
    Pencil,
    Play,
    Plus,
    Square,
    Trash2,
    Upload
} from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { trpc } from "../trpc";

type ViewMode = "list" | "edit" | "create";

/**
 * CollectionList - shows all saved collections
 */
function CollectionList({
  onSelect,
  onCreate,
  onImport,
  onBlueprint,
}: {
  onSelect: (id: string) => void;
  onCreate: () => void;
  onImport: () => void;
  onBlueprint: (blueprint: Blueprint) => void;
}) {
  const collectionsQuery = trpc.collection.list.useQuery();
  const deleteMutation = trpc.collection.delete.useMutation({
    onSuccess: () => collectionsQuery.refetch(),
  });

  const handleDelete = (id: string, name: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (confirm(`Delete collection "${name}"? This cannot be undone.`)) {
      deleteMutation.mutate({ id });
    }
  };

  if (collectionsQuery.isLoading) {
    return <div className="p-8 text-center text-muted-foreground">Loading collections...</div>;
  }

  if (collectionsQuery.isError) {
    return (
      <div className="p-8 text-center">
        <p className="text-destructive mb-2">Error loading collections</p>
        <Button variant="outline" onClick={() => collectionsQuery.refetch()}>
          Retry
        </Button>
      </div>
    );
  }

  const collections = collectionsQuery.data ?? [];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold">Your Collections</h2>
          <p className="text-sm text-muted-foreground">
            Visual hat workflows you've created
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={onImport}>
            <Upload className="h-4 w-4 mr-2" />
            Import YAML
          </Button>
          <Button onClick={onCreate}>
            <Plus className="h-4 w-4 mr-2" />
            New Collection
          </Button>
        </div>
      </div>

      {/* Blueprints — pre-built workflow templates */}
      <div className="mb-6">
        <h3 className="text-sm font-medium text-muted-foreground mb-2">Start from a blueprint</h3>
        <div className="grid grid-cols-3 gap-3">
          {BLUEPRINTS.map((bp) => (
            <Card
              key={bp.id}
              className="cursor-pointer hover:border-primary/50 transition-colors"
              onClick={() => onBlueprint(bp)}
            >
              <CardContent className="p-4">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-lg">{bp.emoji}</span>
                  <span className="font-medium text-sm">{bp.name}</span>
                </div>
                <p className="text-xs text-muted-foreground">{bp.description}</p>
                <div className="flex items-center gap-2 mt-2 text-xs text-muted-foreground">
                  <span>{bp.hats.length} hats</span>
                  <span>·</span>
                  <span>{bp.edges.length} connections</span>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      </div>

      {collections.length === 0 ? (
        <Card className="border-dashed">
          <CardContent className="flex flex-col items-center justify-center py-12">
            <FolderOpen className="h-12 w-12 text-muted-foreground/50 mb-4" />
            <p className="text-muted-foreground mb-4">No collections yet</p>
            <Button onClick={onCreate}>
              <Plus className="h-4 w-4 mr-2" />
              Create Your First Collection
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-3">
          {collections.map((collection: any) => (
            <Card
              key={collection.id}
              className="cursor-pointer hover:border-primary/50 transition-colors"
              onClick={() => onSelect(collection.id)}
            >
              <CardHeader className="pb-2">
                <div className="flex items-start justify-between">
                  <div>
                    <CardTitle className="text-base">{collection.name}</CardTitle>
                    {collection.description && (
                      <CardDescription className="text-xs mt-0.5">
                        {collection.description}
                      </CardDescription>
                    )}
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-8 w-8 p-0"
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelect(collection.id);
                      }}
                    >
                      <Pencil className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-8 w-8 p-0 text-destructive hover:text-destructive"
                      onClick={(e) => handleDelete(collection.id, collection.name, e)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="pt-0">
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <Clock className="h-3 w-3" />
                  <span>
                    Updated {formatDistanceToNow(new Date(collection.updatedAt), { addSuffix: true })}
                  </span>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * BuilderPage - main page component
 */
export function BuilderPage() {
  const [viewMode, setViewMode] = useState<ViewMode>("list");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [isDirty, setIsDirty] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "success" | "error">("idle");
  const [showImport, setShowImport] = useState(false);
  const justImportedId = useRef<string | null>(null);

  // Query for selected collection
  const collectionQuery = trpc.collection.get.useQuery(
    { id: selectedId! },
    { enabled: viewMode === "edit" && !!selectedId }
  );
  const collectionsQuery = trpc.collection.list.useQuery();

  // Mutations
  const createMutation = trpc.collection.create.useMutation({
    onSuccess: (data: any) => {
      setSelectedId(data.id);
      setViewMode("edit");
      setIsDirty(false);
      setSaveStatus("success");
      setTimeout(() => setSaveStatus("idle"), 3000);
      collectionsQuery.refetch();
    },
    onError: () => {
      setSaveStatus("error");
    },
  });

  const updateMutation = trpc.collection.update.useMutation({
    onSuccess: () => {
      setIsDirty(false);
      setSaveStatus("success");
      justImportedId.current = null;
      setTimeout(() => setSaveStatus("idle"), 3000);
      collectionsQuery.refetch();
    },
    onError: () => {
      setSaveStatus("error");
    },
  });

  const exportYamlQuery = trpc.collection.exportYaml.useQuery(
    { id: selectedId! },
    { enabled: false }
  );

  const markDirty = useCallback(() => {
    setIsDirty(true);
    setSaveStatus("idle");
  }, []);

  // Handlers
  const handleCreate = useCallback(() => {
    setName("New Collection");
    setDescription("");
    setSelectedId(null);
    setIsDirty(false);
    setSaveStatus("idle");
    setViewMode("create");
  }, []);

  const handleBlueprint = useCallback((blueprint: Blueprint) => {
    // Convert blueprint hats to graph nodes with temporary positions.
    const nodes = blueprint.hats.map((hat, i) => ({
      id: hat.key,
      type: "hatNode",
      position: { x: 250, y: 50 + i * 200 },
      data: { ...hat },
    }));

    // Convert edge tuples to graph edges.
    const edges = blueprint.edges.map(([source, target, event], i) => ({
      id: `edge-${i}`,
      source,
      target,
      sourceHandle: event,
      targetHandle: event,
      label: event,
      type: "offset",
    }));

    // Create the collection with the blueprint graph.
    const graph = {
      nodes,
      edges,
      viewport: { x: 0, y: 0, zoom: 1 },
    };

    setName(blueprint.name);
    setDescription(blueprint.description);
    createMutation.mutate({
      name: blueprint.name,
      description: blueprint.description,
      graph,
    });
  }, [createMutation]);

  const handleSelect = useCallback((id: string) => {
    setSelectedId(id);
    setIsDirty(false);
    setSaveStatus("idle");
    setViewMode("edit");
  }, []);

  const handleImported = useCallback((collection: { id: string; name: string }) => {
    justImportedId.current = collection.id;
    setShowImport(false);
    setSelectedId(collection.id);
    setIsDirty(false);
    setSaveStatus("idle");
    setViewMode("edit");
    collectionsQuery.refetch();
  }, [collectionsQuery]);

  const handleBack = useCallback(() => {
    if (isDirty && !confirm("Discard unsaved changes?")) return;
    setViewMode("list");
    setSelectedId(null);
    setName("");
    setDescription("");
    setIsDirty(false);
    setSaveStatus("idle");
    justImportedId.current = null;
  }, [isDirty]);

  const handleNameChange = useCallback((value: string) => {
    setName(value);
    markDirty();
  }, [markDirty]);

  const handleDescriptionChange = useCallback((value: string) => {
    setDescription(value);
    markDirty();
  }, [markDirty]);

  const handleSave = useCallback(
    (data: { nodes: Node[]; edges: Edge[]; name: string; description: string }) => {
      // Transform React Flow nodes/edges to our schema
      const graph = {
        nodes: data.nodes.map((n) => ({
          id: n.id,
          type: n.type ?? "hatNode",
          position: { x: n.position.x, y: n.position.y },
          // Cast data to our expected structure
          data: n.data as {
            key: string;
            name: string;
            description: string;
            triggersOn: string[];
            publishes: string[];
            instructions?: string;
          },
        })),
        edges: data.edges.map((e) => ({
          id: e.id,
          source: e.source,
          target: e.target,
          sourceHandle: e.sourceHandle ?? undefined,
          targetHandle: e.targetHandle ?? undefined,
          label: typeof e.label === "string" ? e.label : undefined,
        })),
        viewport: { x: 0, y: 0, zoom: 1 },
      };

      if (viewMode === "create") {
        createMutation.mutate({
          name: data.name,
          description: data.description,
          graph,
        });
      } else if (selectedId) {
        updateMutation.mutate({
          id: selectedId,
          name: data.name,
          description: data.description,
          graph,
        });
      }
    },
    [viewMode, selectedId, createMutation, updateMutation]
  );

  const handleExportYaml = useCallback(async () => {
    if (!selectedId) return;
    const result = await exportYamlQuery.refetch();
    if (result.data?.yaml) {
      // Create a download link
      const blob = new Blob([result.data.yaml], { type: "text/yaml" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${name || "collection"}.yml`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    }
  }, [selectedId, exportYamlQuery, name]);

  // ── Run / Stop loop from the Builder ──────────────────────────────────
  const observing = useObservationStore((s) => s.active);
  const startObserving = useObservationStore((s) => s.startObserving);
  const stopObserving = useObservationStore((s) => s.stopObserving);

  const setHatActive = useObservationStore((s) => s.setHatActive);

  const [showRunPrompt, setShowRunPrompt] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);
  const collectionRunMutation = trpc.collection.run.useMutation();
  const stopLoopMutation = trpc.loops.stop.useMutation();

  const handleRunWithPrompt = useCallback(async (prompt: string) => {
    if (!selectedId) return;
    setShowRunPrompt(false);
    setRunError(null);
    try {
      const result = await collectionRunMutation.mutateAsync({ id: selectedId, prompt });
      startObserving(selectedId);
      // Immediately highlight the entry hat so the user sees feedback
      // before the first WebSocket event arrives (timing-race fix).
      if (result.startingHat) {
        setHatActive(result.startingHat);
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : "Failed to start loop";
      setRunError(message);
    }
  }, [selectedId, collectionRunMutation, startObserving, setHatActive]);

  const handleStop = useCallback(async () => {
    stopObserving();
    // loop.stop reads the PID from .ralph/loop.lock and kills the process.
    try {
      await stopLoopMutation.mutateAsync({ id: "primary" });
    } catch {
      // Best effort — loop may have already terminated.
    }
  }, [stopObserving, stopLoopMutation]);

  // Sync name/description when collection loads. Only fire when we don't yet
  // have the collection's name populated (avoids flipping dirty on every render).
  if (
    viewMode === "edit" &&
    collectionQuery.data &&
    name === "" &&
    collectionQuery.data.name
  ) {
    setName(collectionQuery.data.name);
    setDescription(collectionQuery.data.description || "");
  }

  return (
    <div className="h-full flex flex-col">
      {/* Page header */}
      <header className="px-6 py-4 border-b flex items-center justify-between">
        <div className="flex items-center gap-4">
          {viewMode !== "list" && !observing && (
            <Button variant="ghost" size="sm" onClick={handleBack}>
              <ArrowLeft className="h-4 w-4 mr-2" />
              Back
            </Button>
          )}
          <div>
            <h1 className="text-xl font-bold tracking-tight">
              {viewMode === "list" ? "Hat Builder" : viewMode === "create" ? "New Collection" : name}
            </h1>
            <p className="text-muted-foreground text-sm">
              {viewMode === "list"
                ? "Create visual workflows for hat collections"
                : observing
                  ? "Observing live loop execution"
                  : "Drag hats from the palette and connect them"}
            </p>
          </div>
        </div>
        {viewMode !== "list" && (
          <div className="flex items-center gap-2">
            {observing ? (
              <Button variant="destructive" size="sm" onClick={handleStop}>
                <Square className="h-4 w-4 mr-2" />
                Stop
              </Button>
            ) : (
              <Button
                size="sm"
                onClick={() => setShowRunPrompt(true)}
                disabled={isDirty || !name.trim() || collectionRunMutation.isPending}
                title={isDirty ? "Save the collection before running" : "Start a loop with this collection"}
              >
                <Play className="h-4 w-4 mr-2" />
                {collectionRunMutation.isPending ? "Starting..." : "Run"}
              </Button>
            )}
          </div>
        )}
      </header>

      {/* Run error banner */}
      {runError && (
        <div className="mx-6 mt-3 flex items-center gap-2 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
          <AlertCircle className="h-4 w-4 flex-shrink-0" />
          <span className="flex-1">{runError}</span>
          <button className="text-xs underline" onClick={() => setRunError(null)}>Dismiss</button>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {viewMode === "list" ? (
          <div className="p-6 max-w-4xl mx-auto">
            <CollectionList
              onSelect={handleSelect}
              onCreate={handleCreate}
              onImport={() => setShowImport(true)}
              onBlueprint={handleBlueprint}
            />
          </div>
        ) : viewMode === "edit" && collectionQuery.isLoading ? (
          <div className="flex items-center justify-center h-full">
            <p className="text-muted-foreground">Loading collection...</p>
          </div>
        ) : viewMode === "edit" && collectionQuery.isError ? (
          <div className="flex flex-col items-center justify-center h-full gap-4">
            <p className="text-destructive">Failed to load collection</p>
            <Button variant="outline" onClick={handleBack}>
              Back to list
            </Button>
          </div>
        ) : (
          <CollectionBuilder
            collectionId={selectedId}
            initialData={
              collectionQuery.data?.graph
                ? {
                    nodes: collectionQuery.data.graph.nodes as Node[],
                    edges: collectionQuery.data.graph.edges as Edge[],
                  }
                : undefined
            }
            name={name}
            description={description}
            onNameChange={handleNameChange}
            onDescriptionChange={handleDescriptionChange}
            onSave={handleSave}
            onExportYaml={selectedId ? handleExportYaml : undefined}
            isSaving={createMutation.isPending || updateMutation.isPending}
            isDirty={isDirty}
            onMarkDirty={markDirty}
            saveStatus={saveStatus}
            justImported={!!selectedId && justImportedId.current === selectedId}
            className="h-full"
          />
        )}
      </div>

      <ImportYamlDialog
        open={showImport}
        onClose={() => setShowImport(false)}
        onImported={handleImported}
      />

      <RunPromptDialog
        open={showRunPrompt}
        onClose={() => setShowRunPrompt(false)}
        onRun={handleRunWithPrompt}
        isRunning={collectionRunMutation.isPending}
        collectionName={name}
      />
    </div>
  );
}
