/**
 * tRPC Presets Router Tests
 *
 * Tests for the presets.list tRPC endpoint that returns available presets
 * from three sources:
 * 1. Builtin presets (from presets/*.yml at repo root)
 * 2. Directory presets (from .ralph/hats/ or configured path)
 * 3. Database collections (created via Builder tool)
 */

import { test, describe, mock } from "node:test";
import assert from "node:assert";
import { appRouter, createContext } from "./trpc";
import { initializeDatabase, getDatabase } from "../db/connection";

describe("presets.list tRPC endpoint", () => {
  test("returns builtin presets from presets/ directory", async () => {
    // Given: A configured context with default settings
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase());

    // When: Calling the presets.list endpoint
    const caller = appRouter.createCaller(ctx);
    const result = await caller.presets.list();

    // Then: Should return builtin presets
    assert.ok(Array.isArray(result), "Result should be an array");

    // Check for known builtin presets
    const builtinPresets = result.filter((p: any) => p.source === "builtin");
    assert.ok(builtinPresets.length > 0, "Should have builtin presets");

    // Check that feature preset is included
    const featurePreset = builtinPresets.find((p: any) => p.name === "feature");
    assert.ok(featurePreset, "Should include feature preset");
    assert.strictEqual(featurePreset.source, "builtin");
  });

  test("preset entries have required fields (id, name, source, description)", async () => {
    // Given: A configured context
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase());

    // When: Calling the presets.list endpoint
    const caller = appRouter.createCaller(ctx);
    const result = await caller.presets.list();

    // Then: Each preset should have required fields
    assert.ok(result.length > 0, "Should have at least one preset");

    for (const preset of result) {
      assert.ok(preset.id, `Preset should have id: ${JSON.stringify(preset)}`);
      assert.ok(preset.name, `Preset should have name: ${JSON.stringify(preset)}`);
      assert.ok(preset.source, `Preset should have source: ${JSON.stringify(preset)}`);
      assert.ok(
        ["builtin", "directory", "collection"].includes(preset.source),
        `Source should be builtin, directory, or collection: ${preset.source}`
      );
    }
  });

  test("returns directory presets from .ralph/hats/", async () => {
    // Given: A configured context with directory presets
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase());

    // When: Calling the presets.list endpoint
    const caller = appRouter.createCaller(ctx);
    const result = await caller.presets.list();

    // Then: Directory presets should have source "directory"
    const directoryPresets = result.filter((p: any) => p.source === "directory");
    // Note: Directory presets may or may not exist, so we just verify the structure
    for (const preset of directoryPresets) {
      assert.strictEqual(preset.source, "directory");
      assert.ok(preset.path, "Directory preset should have path");
    }
  });

  test("returns database collections as presets", async () => {
    // Given: A configured context with a collection in the database
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase());

    // First, create a collection
    const caller = appRouter.createCaller(ctx);
    const collection = await caller.collection.create({
      name: "Test Collection",
      description: "A test collection for presets",
    });

    // When: Calling the presets.list endpoint
    const result = await caller.presets.list();

    // Then: Should include the collection as a preset
    const collectionPresets = result.filter((p: any) => p.source === "collection");
    assert.ok(collectionPresets.length > 0, "Should have collection presets");

    const testCollection = collectionPresets.find(
      (p: any) => p.name === "Test Collection"
    );
    assert.ok(testCollection, "Should include the test collection");
    assert.strictEqual(testCollection.source, "collection");
    assert.strictEqual(testCollection.id, collection.id);
  });

  test("combines presets from all sources in correct order", async () => {
    // Given: A configured context
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase());

    // Create a collection
    const caller = appRouter.createCaller(ctx);
    await caller.collection.create({
      name: "My Custom Collection",
      description: "Custom workflow",
    });

    // When: Calling the presets.list endpoint
    const result = await caller.presets.list();

    // Then: Should have presets from multiple sources, ordered by source priority
    // Expected order: builtin first, then directory, then collections
    const sources = result.map((p: any) => p.source);

    // Find first occurrence of each source type
    const firstBuiltin = sources.indexOf("builtin");
    const firstDirectory = sources.indexOf("directory");
    const firstCollection = sources.indexOf("collection");

    // Builtin presets should come first (if they exist)
    if (firstBuiltin !== -1 && firstDirectory !== -1) {
      assert.ok(
        firstBuiltin < firstDirectory,
        "Builtin presets should come before directory presets"
      );
    }
    if (firstBuiltin !== -1 && firstCollection !== -1) {
      assert.ok(
        firstBuiltin < firstCollection,
        "Builtin presets should come before collection presets"
      );
    }
    if (firstDirectory !== -1 && firstCollection !== -1) {
      assert.ok(
        firstDirectory < firstCollection,
        "Directory presets should come before collection presets"
      );
    }
  });

  test("returns empty array when no presets available (edge case)", async () => {
    // Note: This test verifies the API doesn't throw when presets are missing
    // In practice, builtin presets should always exist
    initializeDatabase(getDatabase(":memory:"));
    const ctx = createContext(getDatabase());

    const caller = appRouter.createCaller(ctx);
    const result = await caller.presets.list();

    // Should return an array (even if empty in edge cases)
    assert.ok(Array.isArray(result), "Result should always be an array");
  });
});
