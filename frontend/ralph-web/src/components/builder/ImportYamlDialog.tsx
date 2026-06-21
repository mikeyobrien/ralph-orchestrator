/**
 * ImportYamlDialog
 *
 * Modal for importing a preset YAML as a new collection. Accepts either a
 * file upload or pasted YAML text, plus a required name and optional
 * description. Delegates parsing to the backend via collection.import RPC.
 * On success, calls onImported with the created collection.
 *
 * Follows the inline-modal pattern from PlanSession (fixed overlay + Card)
 * so no new dependency is introduced for a dialog primitive.
 */

import { Button } from "@/components/ui/button";
import {
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { trpc } from "@/trpc";
import { AlertCircle, Upload, X } from "lucide-react";
import { useCallback, useRef, useState, type ChangeEvent } from "react";

interface ImportYamlDialogProps {
  open: boolean;
  onClose: () => void;
  /** Called with the created collection record after a successful import. */
  onImported: (collection: { id: string; name: string }) => void;
}

export function ImportYamlDialog({ open, onClose, onImported }: ImportYamlDialogProps) {
  const [yaml, setYaml] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [fileError, setFileError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const importMutation = trpc.collection.importYaml.useMutation({
    onSuccess: (collection: { id: string; name: string }) => {
      onImported(collection);
      // Reset local state after propagation so re-opening the dialog starts clean.
      setYaml("");
      setName("");
      setDescription("");
      setFileError(null);
    },
  });

  const handleFile = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    setFileError(null);
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        setFileError("Selected file could not be read as text.");
        return;
      }
      setYaml(result);
      // Seed name from filename if the user hasn't entered one yet.
      if (!name.trim()) {
        const base = file.name.replace(/\.(ya?ml)$/i, "");
        if (base) setName(base);
      }
    };
    reader.onerror = () => {
      setFileError(reader.error?.message ?? "Failed to read file.");
    };
    reader.readAsText(file);
  }, [name]);

  const handleImport = useCallback(() => {
    const trimmedYaml = yaml.trim();
    const trimmedName = name.trim();
    if (!trimmedYaml || !trimmedName) return;
    importMutation.mutate({
      yaml: trimmedYaml,
      name: trimmedName,
      description: description.trim() || undefined,
    });
  }, [yaml, name, description, importMutation]);

  if (!open) return null;

  const canImport =
    yaml.trim().length > 0 && name.trim().length > 0 && !importMutation.isPending;

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      onClick={onClose}
    >
      <Card
        className="w-full max-w-2xl max-h-[90vh] flex flex-col"
        onClick={(event) => event.stopPropagation()}
      >
        <CardHeader className="flex-shrink-0 flex flex-row items-center justify-between space-y-0 pb-2">
          <div>
            <CardTitle>Import YAML</CardTitle>
            <CardDescription>
              Paste a preset YAML or upload a file to create a new collection.
            </CardDescription>
          </div>
          <Button variant="ghost" size="icon" onClick={onClose} aria-label="Close">
            <X className="h-4 w-4" />
          </Button>
        </CardHeader>
        <CardContent className="flex-1 overflow-y-auto space-y-4">
          <div className="space-y-2">
            <Label htmlFor="import-name">Name</Label>
            <Input
              id="import-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Collection name"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="import-description">Description (optional)</Label>
            <Input
              id="import-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              placeholder="What is this collection for?"
            />
          </div>
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label htmlFor="import-yaml">YAML</Label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => fileInputRef.current?.click()}
              >
                <Upload className="h-4 w-4 mr-2" />
                Upload file
              </Button>
              <input
                ref={fileInputRef}
                type="file"
                accept=".yml,.yaml,text/yaml,application/x-yaml"
                className="hidden"
                onChange={handleFile}
              />
            </div>
            <Textarea
              id="import-yaml"
              value={yaml}
              onChange={(event) => setYaml(event.target.value)}
              placeholder={"hats:\n  planner:\n    name: Planner\n    triggers: [work.start]\n    publishes: [build.task]"}
              className="min-h-[240px] font-mono text-xs"
              spellCheck={false}
            />
          </div>
          {fileError && (
            <div
              className="flex items-center gap-2 p-3 rounded-md bg-destructive/10 text-destructive text-sm"
              role="alert"
            >
              <AlertCircle className="h-4 w-4 flex-shrink-0" />
              <span>{fileError}</span>
            </div>
          )}
          {importMutation.isError && (
            <div
              className="flex items-center gap-2 p-3 rounded-md bg-destructive/10 text-destructive text-sm"
              role="alert"
            >
              <AlertCircle className="h-4 w-4 flex-shrink-0" />
              <span>{importMutation.error.message}</span>
            </div>
          )}
        </CardContent>
        <CardFooter className="flex-shrink-0 justify-end gap-2 pt-4 border-t">
          <Button variant="outline" onClick={onClose} disabled={importMutation.isPending}>
            Cancel
          </Button>
          <Button onClick={handleImport} disabled={!canImport}>
            {importMutation.isPending ? "Importing..." : "Import"}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
