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

import { useState, useCallback } from "react";
import { trpc } from "../trpc";
import { CollectionBuilder } from "@/components/builder";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Plus,
  FolderOpen,
  Pencil,
  Trash2,
  ArrowLeft,
  Clock,
} from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import type { Node, Edge } from "@xyflow/react";

type ViewMode = "list" | "edit" | "create";

/**
 * CollectionList - shows all saved collections
 */
function CollectionList({
  onSelect,
  onCreate,
}: {
  onSelect: (id: string) => void;
  onCreate: () => void;
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
        <Button onClick={onCreate}>
          <Plus className="h-4 w-4 mr-2" />
          New Collection
        </Button>
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
          {collections.map((collection) => (
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

  // Query for selected collection
  const collectionQuery = trpc.collection.get.useQuery(
    { id: selectedId! },
    { enabled: viewMode === "edit" && !!selectedId }
  );

  // Mutations
  const createMutation = trpc.collection.create.useMutation({
    onSuccess: (data) => {
      setSelectedId(data.id);
      setViewMode("edit");
    },
  });

  const updateMutation = trpc.collection.update.useMutation();

  const exportYamlQuery = trpc.collection.exportYaml.useQuery(
    { id: selectedId! },
    { enabled: false }
  );

  // Handlers
  const handleCreate = useCallback(() => {
    setName("New Collection");
    setDescription("");
    setSelectedId(null);
    setViewMode("create");
  }, []);

  const handleSelect = useCallback((id: string) => {
    setSelectedId(id);
    setViewMode("edit");
  }, []);

  const handleBack = useCallback(() => {
    setViewMode("list");
    setSelectedId(null);
    setName("");
    setDescription("");
  }, []);

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

  // Sync name/description when collection loads
  if (viewMode === "edit" && collectionQuery.data && name !== collectionQuery.data.name) {
    setName(collectionQuery.data.name);
    setDescription(collectionQuery.data.description || "");
  }

  return (
    <div className="h-full flex flex-col">
      {/* Page header */}
      <header className="px-6 py-4 border-b flex items-center justify-between">
        <div className="flex items-center gap-4">
          {viewMode !== "list" && (
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
                : "Drag hats from the palette and connect them"}
            </p>
          </div>
        </div>
      </header>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {viewMode === "list" ? (
          <div className="p-6 max-w-4xl mx-auto">
            <CollectionList onSelect={handleSelect} onCreate={handleCreate} />
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
            onNameChange={setName}
            onDescriptionChange={setDescription}
            onSave={handleSave}
            onExportYaml={selectedId ? handleExportYaml : undefined}
            isSaving={createMutation.isPending || updateMutation.isPending}
            className="h-full"
          />
        )}
      </div>
    </div>
  );
}
