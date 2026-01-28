/**
 * SettingsPage
 *
 * Settings page for editing ralph.yml configuration.
 * Features:
 * - YAML editor showing the config
 * - Save button to persist changes
 * - Hat collection dropdown (only affects hat collection, not backend args)
 */

import { useState, useEffect } from "react";
import { trpc } from "../trpc";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Save, AlertCircle, CheckCircle2, RefreshCw } from "lucide-react";

export function SettingsPage() {
  const [content, setContent] = useState("");
  const [isDirty, setIsDirty] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "success" | "error">("idle");

  const configQuery = trpc.config.get.useQuery();
  const presetsQuery = trpc.presets.list.useQuery();
  const updateMutation = trpc.config.update.useMutation({
    onSuccess: () => {
      setIsDirty(false);
      setSaveStatus("success");
      configQuery.refetch();
      setTimeout(() => setSaveStatus("idle"), 3000);
    },
    onError: () => {
      setSaveStatus("error");
    },
  });

  // Initialize content from query
  useEffect(() => {
    if (configQuery.data?.raw && !isDirty) {
      setContent(configQuery.data.raw);
    }
  }, [configQuery.data?.raw, isDirty]);

  const handleContentChange = (value: string) => {
    setContent(value);
    setIsDirty(true);
    setSaveStatus("idle");
  };

  const handleSave = () => {
    updateMutation.mutate({ content });
  };

  const handleReset = () => {
    if (configQuery.data?.raw) {
      setContent(configQuery.data.raw);
      setIsDirty(false);
      setSaveStatus("idle");
    }
  };

  // Extract current hat collection from config
  const currentHatCollection = configQuery.data?.parsed?.hats
    ? "default"
    : undefined;

  // Get available presets for the dropdown
  const presets = presetsQuery.data ?? [];

  return (
    <>
      {/* Page header */}
      <header className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
          <p className="text-muted-foreground text-sm mt-1">
            Configure your Ralph orchestrator
          </p>
        </div>
        <Badge variant="secondary">ralph.yml</Badge>
      </header>

      {/* Hat Collection Selector */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-lg">Hat Collection</CardTitle>
          <CardDescription>
            Select a preset hat collection. This only affects the hat workflow,
            not backend settings like CLI arguments.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-4">
            <Label htmlFor="hat-collection" className="min-w-[120px]">
              Active Collection
            </Label>
            <select
              id="hat-collection"
              className="flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm"
              value={currentHatCollection ?? ""}
              disabled={presetsQuery.isLoading}
            >
              {currentHatCollection && (
                <option value="default">Default (from config)</option>
              )}
              {presets.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.name} ({preset.source})
                </option>
              ))}
            </select>
          </div>
          <p className="text-xs text-muted-foreground mt-2">
            The dropdown selection is read-only. Edit the YAML below to change the hat collection.
          </p>
        </CardContent>
      </Card>

      {/* Configuration Editor */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2">
                Configuration
                {isDirty && (
                  <Badge variant="outline" className="ml-2 text-yellow-600 border-yellow-600">
                    Unsaved changes
                  </Badge>
                )}
              </CardTitle>
              <CardDescription>
                Edit your ralph.yml configuration directly
              </CardDescription>
            </div>
            <div className="flex items-center gap-2">
              {saveStatus === "success" && (
                <span className="flex items-center gap-1 text-sm text-green-600">
                  <CheckCircle2 className="h-4 w-4" />
                  Saved
                </span>
              )}
              {saveStatus === "error" && (
                <span className="flex items-center gap-1 text-sm text-destructive">
                  <AlertCircle className="h-4 w-4" />
                  Error saving
                </span>
              )}
              <Button
                variant="outline"
                size="sm"
                onClick={handleReset}
                disabled={!isDirty || updateMutation.isPending}
              >
                <RefreshCw className="h-4 w-4 mr-2" />
                Reset
              </Button>
              <Button
                size="sm"
                onClick={handleSave}
                disabled={!isDirty || updateMutation.isPending}
              >
                <Save className="h-4 w-4 mr-2" />
                {updateMutation.isPending ? "Saving..." : "Save"}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {configQuery.isLoading ? (
            <div className="flex items-center justify-center h-64 text-muted-foreground">
              Loading configuration...
            </div>
          ) : configQuery.isError ? (
            <div className="flex flex-col items-center justify-center h-64 gap-4">
              <AlertCircle className="h-8 w-8 text-destructive" />
              <p className="text-destructive">
                {configQuery.error.message}
              </p>
              <Button variant="outline" onClick={() => configQuery.refetch()}>
                Retry
              </Button>
            </div>
          ) : (
            <div className="space-y-4">
              <Textarea
                value={content}
                onChange={(e) => handleContentChange(e.target.value)}
                className="font-mono text-sm min-h-[500px] resize-y"
                placeholder="# Ralph configuration"
                spellCheck={false}
              />
              {updateMutation.isError && (
                <div className="flex items-center gap-2 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
                  <AlertCircle className="h-4 w-4 flex-shrink-0" />
                  <span>{updateMutation.error.message}</span>
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>
    </>
  );
}
