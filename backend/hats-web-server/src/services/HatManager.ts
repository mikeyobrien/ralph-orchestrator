/**
 * HatManager
 *
 * Manages YAML-based hat presets from the filesystem.
 * Hat presets define operational roles with their triggers, event publishing,
 * and optional MCP server configurations.
 *
 * Architecture:
 * - Reads .yml/.yaml files from a presets directory
 * - Validates YAML structure using Zod schemas
 * - Maps snake_case YAML fields to camelCase TypeScript interfaces
 * - Extends HatDefinition with preset-specific fields (mcpServers, defaultPublishes)
 *
 * Usage:
 *   const manager = new HatManager('./presets');
 *   const presets = await manager.loadAll();
 *   const validator = await manager.load('validator');
 */

import * as fs from "fs";
import * as path from "path";
import { parse as parseYaml } from "yaml";
import { z } from "zod";
import type { HatDefinition } from "./SettingsService";

/**
 * MCP server configuration as defined in preset YAML
 */
export interface McpServerConfig {
  /** Command to run the MCP server */
  command: string;
  /** Arguments to pass to the command */
  args?: string[];
  /** Environment variables for the server process */
  env?: Record<string, string>;
}

/**
 * Hat preset loaded from YAML file
 * Extends HatDefinition with preset-specific fields
 */
export interface HatPreset extends HatDefinition {
  /** The filename this preset was loaded from (without extension) */
  filename: string;
  /** Default event to publish when no explicit event is specified */
  defaultPublishes?: string;
  /** MCP servers required by this hat */
  mcpServers?: Record<string, McpServerConfig>;
}

/**
 * Zod schema for MCP server configuration in YAML
 */
const McpServerConfigSchema = z.object({
  command: z.string(),
  args: z.array(z.string()).optional(),
  env: z.record(z.string(), z.string()).optional(),
});

/**
 * Zod schema for hat preset YAML structure
 * Matches the snake_case format used in YAML files
 */
const HatPresetYamlSchema = z.object({
  name: z.string(),
  description: z.string(),
  triggers: z.array(z.string()).default([]),
  publishes: z.array(z.string()).default([]),
  default_publishes: z.string().optional(),
  mcp_servers: z.record(z.string(), McpServerConfigSchema).optional(),
  instructions: z.string().optional(),
});

/**
 * Type for raw parsed YAML (before transformation)
 */
type HatPresetYaml = z.infer<typeof HatPresetYamlSchema>;

/**
 * Error thrown when a preset file cannot be loaded or parsed
 */
export class PresetLoadError extends Error {
  constructor(
    public readonly filename: string,
    public readonly cause: Error
  ) {
    super(`Failed to load preset '${filename}': ${cause.message}`);
    this.name = "PresetLoadError";
  }
}

/**
 * Error thrown when a preset file has invalid structure
 */
export class PresetValidationError extends Error {
  constructor(
    public readonly filename: string,
    public readonly issues: z.ZodIssue[]
  ) {
    const issueList = issues.map((i) => `  - ${i.path.join(".")}: ${i.message}`).join("\n");
    super(`Invalid preset structure in '${filename}':\n${issueList}`);
    this.name = "PresetValidationError";
  }
}

/**
 * HatManager
 *
 * Reads and parses YAML hat presets from a directory.
 */
export class HatManager {
  private readonly presetsDir: string;
  private cache: Map<string, HatPreset> = new Map();

  /**
   * Create a new HatManager
   *
   * @param presetsDir - Path to the directory containing preset YAML files
   */
  constructor(presetsDir: string) {
    this.presetsDir = path.resolve(presetsDir);
  }

  /**
   * Get the presets directory path
   */
  getPresetsDir(): string {
    return this.presetsDir;
  }

  /**
   * List available preset filenames (without extensions)
   *
   * @returns Array of preset names that can be passed to load()
   */
  listPresets(): string[] {
    if (!fs.existsSync(this.presetsDir)) {
      return [];
    }

    const files = fs.readdirSync(this.presetsDir);
    return files
      .filter((f) => f.endsWith(".yml") || f.endsWith(".yaml"))
      .map((f) => f.replace(/\.ya?ml$/, ""));
  }

  /**
   * Load a specific preset by name
   *
   * @param name - Preset name (filename without extension)
   * @param options - Load options
   * @param options.useCache - If true, return cached preset if available (default: true)
   * @returns The loaded preset
   * @throws PresetLoadError if the file cannot be read or parsed
   * @throws PresetValidationError if the YAML structure is invalid
   */
  load(name: string, options: { useCache?: boolean } = {}): HatPreset {
    const { useCache = true } = options;

    // Return cached if available
    if (useCache && this.cache.has(name)) {
      return this.cache.get(name)!;
    }

    // Try both extensions
    const ymlPath = path.join(this.presetsDir, `${name}.yml`);
    const yamlPath = path.join(this.presetsDir, `${name}.yaml`);

    let filePath: string;
    if (fs.existsSync(ymlPath)) {
      filePath = ymlPath;
    } else if (fs.existsSync(yamlPath)) {
      filePath = yamlPath;
    } else {
      throw new PresetLoadError(
        name,
        new Error(`No preset file found (tried ${name}.yml and ${name}.yaml)`)
      );
    }

    const preset = this.loadFile(filePath, name);
    this.cache.set(name, preset);
    return preset;
  }

  /**
   * Load all presets from the directory
   *
   * @param options - Load options
   * @param options.useCache - If true, use cached presets where available (default: true)
   * @returns Array of loaded presets
   */
  loadAll(options: { useCache?: boolean } = {}): HatPreset[] {
    const names = this.listPresets();
    return names.map((name) => this.load(name, options));
  }

  /**
   * Clear the preset cache
   */
  clearCache(): void {
    this.cache.clear();
  }

  /**
   * Check if a preset exists
   *
   * @param name - Preset name to check
   */
  exists(name: string): boolean {
    const ymlPath = path.join(this.presetsDir, `${name}.yml`);
    const yamlPath = path.join(this.presetsDir, `${name}.yaml`);
    return fs.existsSync(ymlPath) || fs.existsSync(yamlPath);
  }

  /**
   * Load and parse a single preset file
   */
  private loadFile(filePath: string, filename: string): HatPreset {
    let content: string;
    try {
      content = fs.readFileSync(filePath, "utf-8");
    } catch (err) {
      throw new PresetLoadError(filename, err as Error);
    }

    let parsed: unknown;
    try {
      parsed = parseYaml(content);
    } catch (err) {
      throw new PresetLoadError(filename, err as Error);
    }

    // Validate with Zod
    const result = HatPresetYamlSchema.safeParse(parsed);
    if (!result.success) {
      throw new PresetValidationError(filename, result.error.issues);
    }

    return this.transformToPreset(result.data, filename);
  }

  /**
   * Transform snake_case YAML to camelCase HatPreset
   */
  private transformToPreset(yaml: HatPresetYaml, filename: string): HatPreset {
    const preset: HatPreset = {
      filename,
      name: yaml.name,
      description: yaml.description,
      triggersOn: yaml.triggers,
      publishes: yaml.publishes,
      instructions: yaml.instructions,
    };

    // Add optional fields if present
    if (yaml.default_publishes) {
      preset.defaultPublishes = yaml.default_publishes;
    }

    if (yaml.mcp_servers) {
      preset.mcpServers = {};
      for (const [key, config] of Object.entries(yaml.mcp_servers)) {
        preset.mcpServers[key] = {
          command: config.command,
          args: config.args,
          env: config.env,
        };
      }
    }

    return preset;
  }
}
