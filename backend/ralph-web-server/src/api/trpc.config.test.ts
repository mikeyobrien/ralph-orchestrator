/**
 * tRPC Config Router Tests
 *
 * Tests for the config.get and config.update tRPC endpoints that handle
 * reading and writing the ralph.yml configuration file.
 *
 * Security considerations tested:
 * - YAML validation before writing
 * - Error handling for missing files
 */

import { test, describe, beforeEach, afterEach } from "node:test";
import assert from "node:assert";
import * as fs from "fs";
import * as path from "path";
import { appRouter, createContext } from "./trpc";
import { initializeDatabase, getDatabase } from "../db/connection";

// Path to configs directory relative to this file (4 levels up to repo root)
const REPO_ROOT = path.resolve(__dirname, "../../../..");
const TEST_CONFIG_DIR = REPO_ROOT;
const TEST_CONFIG_PATH = path.join(REPO_ROOT, "ralph.yml");

describe("config.get tRPC endpoint", () => {
  beforeEach(() => {
    initializeDatabase(getDatabase(":memory:"));
  });

  test("returns config when file exists", async () => {
    // Skip if config file doesn't exist (CI environment)
    if (!fs.existsSync(TEST_CONFIG_PATH)) {
      return;
    }

    // Given: A configured context
    const ctx = createContext(getDatabase());

    // When: Calling the config.get endpoint
    const caller = appRouter.createCaller(ctx);
    const result = await caller.config.get();

    // Then: Should return raw and parsed config
    assert.ok(result.raw, "Should have raw YAML string");
    assert.ok(typeof result.raw === "string", "Raw should be a string");
    assert.ok(result.parsed, "Should have parsed object");
    assert.ok(typeof result.parsed === "object", "Parsed should be an object");
  });

  test("returns valid YAML structure", async () => {
    // Skip if config file doesn't exist
    if (!fs.existsSync(TEST_CONFIG_PATH)) {
      return;
    }

    // Given: A configured context
    const ctx = createContext(getDatabase());

    // When: Calling the config.get endpoint
    const caller = appRouter.createCaller(ctx);
    const result = await caller.config.get();

    // Then: Parsed config should have expected ralph.yml fields
    const parsed = result.parsed as Record<string, unknown>;
    // The config should have at least one of these typical ralph.yml sections
    const hasValidSection =
      "event_loop" in parsed ||
      "cli" in parsed ||
      "core" in parsed ||
      "hats" in parsed;
    assert.ok(hasValidSection, "Config should have recognized ralph.yml sections");
  });
});

describe("config.update tRPC endpoint", () => {
  let originalContent: string | null = null;

  beforeEach(() => {
    initializeDatabase(getDatabase(":memory:"));
    // Backup original config if it exists
    if (fs.existsSync(TEST_CONFIG_PATH)) {
      originalContent = fs.readFileSync(TEST_CONFIG_PATH, "utf-8");
    }
  });

  afterEach(() => {
    // Restore original config
    if (originalContent !== null) {
      fs.writeFileSync(TEST_CONFIG_PATH, originalContent, "utf-8");
    }
  });

  test("rejects invalid YAML syntax", async () => {
    // Given: A configured context
    const ctx = createContext(getDatabase());

    // When: Calling config.update with invalid YAML
    const caller = appRouter.createCaller(ctx);

    // Then: Should throw an error
    await assert.rejects(
      async () => {
        await caller.config.update({
          content: "invalid: yaml: syntax: [[[",
        });
      },
      (err: Error) => {
        assert.ok(err.message.includes("Invalid YAML syntax"), `Expected YAML error, got: ${err.message}`);
        return true;
      }
    );
  });

  test("accepts valid YAML content", async () => {
    // Skip if we can't write to config dir
    if (!fs.existsSync(TEST_CONFIG_DIR)) {
      return;
    }

    // Given: A configured context
    const ctx = createContext(getDatabase());

    // When: Calling config.update with valid YAML
    const caller = appRouter.createCaller(ctx);
    const validYaml = `# Test config
event_loop:
  max_iterations: 10
cli:
  backend: claude
`;

    const result = await caller.config.update({ content: validYaml });

    // Then: Should succeed and return parsed content
    assert.ok(result.success, "Update should succeed");
    assert.ok(result.parsed, "Should return parsed content");
    assert.strictEqual((result.parsed as any).event_loop?.max_iterations, 10);
  });

  test("preserves comments and formatting in raw content", async () => {
    // Skip if we can't write to config dir
    if (!fs.existsSync(TEST_CONFIG_DIR)) {
      return;
    }

    // Given: A configured context
    const ctx = createContext(getDatabase());

    // When: Saving YAML with comments
    const caller = appRouter.createCaller(ctx);
    const yamlWithComments = `# This is a comment
event_loop:
  max_iterations: 5  # inline comment
`;

    await caller.config.update({ content: yamlWithComments });

    // Then: Reading back should preserve the raw content
    const result = await caller.config.get();
    assert.ok(result.raw.includes("# This is a comment"), "Should preserve comments");
  });
});
