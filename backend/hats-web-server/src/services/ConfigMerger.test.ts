/**
 * ConfigMerger Tests
 *
 * Tests for the config merging logic that combines base config with preset hats.
 * The merger preserves all base config settings (max_iterations, backend, etc.)
 * while replacing only the hats and events sections from the preset.
 */

import { test, describe, beforeEach, afterEach } from "node:test";
import assert from "node:assert";
import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import { parse as yamlParse, stringify as yamlStringify } from "yaml";
import { ConfigMerger, MergeResult } from "./ConfigMerger";
import { CollectionService } from "./CollectionService";
import { CollectionRepository } from "../repositories/CollectionRepository";
import { initializeTestDatabase, getTestDatabase, closeTestDatabase } from "../db/testUtils";

describe("ConfigMerger", () => {
  let tempDir: string;
  let baseConfigPath: string;
  let presetsDir: string;

  beforeEach(() => {
    // Create temp directory structure
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "config-merger-test-"));
    presetsDir = path.join(tempDir, "presets", "builtin");
    fs.mkdirSync(presetsDir, { recursive: true });

    // Create a base config file
    baseConfigPath = path.join(tempDir, "hats.yml");
    const baseConfig = {
      event_loop: {
        max_iterations: 25,
        max_runtime_seconds: 3600,
        completion_promise: "LOOP_COMPLETE",
      },
      cli: {
        backend: "claude",
        prompt_mode: "arg",
      },
      core: {
        specs_dir: "./specs/",
      },
      hats: {
        planner: {
          name: "Planner",
          description: "Plans the work",
          triggers: ["task.start"],
          publishes: ["plan.done"],
        },
      },
    };
    fs.writeFileSync(baseConfigPath, yamlStringify(baseConfig), "utf-8");

    // Create a builtin preset
    const tddPreset = {
      event_loop: {
        starting_event: "tdd.start",
      },
      hats: {
        test_writer: {
          name: "Test Writer",
          description: "Writes failing tests first",
          triggers: ["tdd.start"],
          publishes: ["test.written"],
        },
        implementer: {
          name: "Implementer",
          description: "Makes failing tests pass",
          triggers: ["test.written"],
          publishes: ["test.passing"],
        },
      },
      events: {
        "tdd.start": { description: "TDD cycle begins" },
        "test.written": { description: "Test has been written" },
      },
    };
    fs.writeFileSync(
      path.join(presetsDir, "tdd-red-green.yml"),
      yamlStringify(tddPreset),
      "utf-8"
    );

    // Create a preset without events (to test deriveEventsFromHats)
    const noEventsPreset = {
      hats: {
        builder: {
          name: "Builder",
          description: "Builds code",
          triggers: ["build.start"],
          publishes: ["build.done"],
        },
      },
    };
    fs.writeFileSync(
      path.join(presetsDir, "no-events.yml"),
      yamlStringify(noEventsPreset),
      "utf-8"
    );

    // Initialize test database for collection tests
    initializeTestDatabase();
  });

  afterEach(() => {
    // Cleanup
    fs.rmSync(tempDir, { recursive: true, force: true });
    closeTestDatabase();
  });

  describe("merge with default preset", () => {
    test("returns base config unchanged when preset is 'default'", () => {
      const merger = new ConfigMerger({ presetsDir });

      const result = merger.merge(baseConfigPath, "default");

      // Should return base config path, not a temp file
      assert.strictEqual(result.tempPath, baseConfigPath, "Should return base config path");

      // Config should be unchanged
      const parsedBase = yamlParse(fs.readFileSync(baseConfigPath, "utf-8"));
      assert.deepStrictEqual(result.config, parsedBase, "Config should match base config");
    });
  });

  describe("merge with builtin preset", () => {
    test("preserves base config settings while replacing hats", () => {
      const merger = new ConfigMerger({ presetsDir });

      const result = merger.merge(baseConfigPath, "builtin:tdd-red-green");

      // Should create temp file
      assert.ok(result.tempPath !== baseConfigPath, "Should create temp file");
      assert.ok(fs.existsSync(result.tempPath), "Temp file should exist");

      // Base settings should be preserved
      assert.strictEqual(
        result.config.event_loop?.max_iterations,
        25,
        "max_iterations should be preserved from base"
      );
      assert.strictEqual(
        result.config.event_loop?.max_runtime_seconds,
        3600,
        "max_runtime_seconds should be preserved from base"
      );
      assert.strictEqual(
        result.config.cli?.backend,
        "claude",
        "backend should be preserved from base"
      );
      assert.strictEqual(
        result.config.core?.specs_dir,
        "./specs/",
        "core.specs_dir should be preserved from base"
      );

      // Hats should come from preset
      assert.ok(result.config.hats?.test_writer, "Should have test_writer hat from preset");
      assert.ok(result.config.hats?.implementer, "Should have implementer hat from preset");
      assert.ok(!result.config.hats?.planner, "Should NOT have planner hat from base");

      // Events should come from preset
      assert.ok(result.config.events?.["tdd.start"], "Should have tdd.start event");
      assert.ok(result.config.events?.["test.written"], "Should have test.written event");
    });

    test("merges starting_event from preset into event_loop", () => {
      const merger = new ConfigMerger({ presetsDir });

      const result = merger.merge(baseConfigPath, "builtin:tdd-red-green");

      // starting_event should come from preset
      assert.strictEqual(
        result.config.event_loop?.starting_event,
        "tdd.start",
        "starting_event should come from preset"
      );

      // But other event_loop settings should be from base
      assert.strictEqual(
        result.config.event_loop?.max_iterations,
        25,
        "max_iterations should be from base"
      );
    });

    test("derives events from hats when preset has no events section", () => {
      const merger = new ConfigMerger({ presetsDir });

      const result = merger.merge(baseConfigPath, "builtin:no-events");

      // Events should be auto-derived from hat triggers/publishes
      assert.ok(result.config.events, "Should have events section");
      assert.ok(
        result.config.events?.["build.start"] || result.config.events?.["build.done"],
        "Should derive events from hat triggers/publishes"
      );
    });
  });

  describe("merge with directory preset", () => {
    test("resolves directory preset to .hats/hats/ path", () => {
      // Create .hats/hats directory with a preset
      const hatsDir = path.join(tempDir, ".hats", "hats");
      fs.mkdirSync(hatsDir, { recursive: true });

      const customPreset = {
        hats: {
          custom_hat: {
            name: "Custom Hat",
            description: "A custom hat",
            triggers: ["custom.start"],
            publishes: ["custom.done"],
          },
        },
      };
      fs.writeFileSync(
        path.join(hatsDir, "my-custom.yml"),
        yamlStringify(customPreset),
        "utf-8"
      );

      const merger = new ConfigMerger({
        presetsDir,
        directoryPresetsRoot: tempDir,
      });

      const result = merger.merge(baseConfigPath, "directory:my-custom");

      // Should have custom hat
      assert.ok(result.config.hats?.custom_hat, "Should have custom_hat from directory preset");
      assert.ok(!result.config.hats?.planner, "Should NOT have planner from base");

      // Base settings preserved
      assert.strictEqual(result.config.event_loop?.max_iterations, 25);
    });
  });

  describe("merge with collection preset (UUID)", () => {
    test("exports collection to YAML and merges with base config", () => {
      const db = getTestDatabase();
      const collectionRepository = new CollectionRepository(db);
      const collectionService = new CollectionService(collectionRepository);

      // Create a collection
      const collection = collectionRepository.create({
        name: "Test Collection",
        description: "A test collection",
        graph: {
          nodes: [
            {
              id: "hat1",
              type: "hatNode",
              position: { x: 0, y: 0 },
              data: {
                key: "collection_hat",
                name: "Collection Hat",
                description: "Hat from collection",
                triggersOn: ["collection.start"],
                publishes: ["collection.done"],
              },
            },
          ],
          edges: [],
          viewport: { x: 0, y: 0, zoom: 1 },
        },
      });

      const merger = new ConfigMerger({
        presetsDir,
        collectionService,
        tempDir: path.join(tempDir, ".hats", "temp"),
      });

      const result = merger.merge(baseConfigPath, collection.id);

      // Should have collection hat
      assert.ok(
        result.config.hats?.collection_hat,
        "Should have collection_hat from collection"
      );
      assert.ok(!result.config.hats?.planner, "Should NOT have planner from base");

      // Base settings preserved
      assert.strictEqual(result.config.event_loop?.max_iterations, 25);
      assert.strictEqual(result.config.cli?.backend, "claude");
    });

    test("falls back to base config when collection UUID not found", () => {
      const db = getTestDatabase();
      const collectionRepository = new CollectionRepository(db);
      const collectionService = new CollectionService(collectionRepository);

      const merger = new ConfigMerger({
        presetsDir,
        collectionService,
        tempDir: path.join(tempDir, ".hats", "temp"),
      });

      const result = merger.merge(baseConfigPath, "550e8400-e29b-41d4-a716-446655440000");

      // Should return base config unchanged
      assert.strictEqual(result.tempPath, baseConfigPath);

      // Should have base hats (fallback behavior)
      assert.ok(result.config.hats?.planner, "Should have planner from base config");
    });
  });

  describe("temp file management", () => {
    test("writes merged config to temp directory", () => {
      const tempOutputDir = path.join(tempDir, ".hats", "temp");

      const merger = new ConfigMerger({
        presetsDir,
        tempDir: tempOutputDir,
      });

      const result = merger.merge(baseConfigPath, "builtin:tdd-red-green");

      // Temp file should be in temp directory
      assert.ok(
        result.tempPath.startsWith(tempOutputDir),
        `Temp file should be in temp dir: ${result.tempPath}`
      );

      // Temp file should be valid YAML
      const content = fs.readFileSync(result.tempPath, "utf-8");
      const parsed = yamlParse(content);
      assert.ok(parsed.hats, "Temp file should contain valid YAML with hats");
    });

    test("creates temp directory if it does not exist", () => {
      const tempOutputDir = path.join(tempDir, "nonexistent", "temp");

      const merger = new ConfigMerger({
        presetsDir,
        tempDir: tempOutputDir,
      });

      const result = merger.merge(baseConfigPath, "builtin:tdd-red-green");

      // Temp directory should have been created
      assert.ok(fs.existsSync(tempOutputDir), "Temp directory should be created");
      assert.ok(fs.existsSync(result.tempPath), "Temp file should exist");
    });
  });

  describe("error handling", () => {
    test("throws when base config does not exist", () => {
      const merger = new ConfigMerger({ presetsDir });

      assert.throws(
        () => merger.merge("/nonexistent/config.yml", "default"),
        /Base config not found/,
        "Should throw when base config missing"
      );
    });

    test("throws when builtin preset does not exist", () => {
      const merger = new ConfigMerger({ presetsDir });

      assert.throws(
        () => merger.merge(baseConfigPath, "builtin:nonexistent"),
        /Preset not found/,
        "Should throw when preset missing"
      );
    });

    test("throws when base config is invalid YAML", () => {
      const invalidConfigPath = path.join(tempDir, "invalid.yml");
      fs.writeFileSync(invalidConfigPath, "{ invalid yaml :", "utf-8");

      const merger = new ConfigMerger({ presetsDir });

      assert.throws(
        () => merger.merge(invalidConfigPath, "default"),
        /Invalid YAML/,
        "Should throw when YAML is invalid"
      );
    });
  });
});
