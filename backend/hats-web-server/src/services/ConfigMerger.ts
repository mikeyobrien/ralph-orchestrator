/**
 * ConfigMerger
 *
 * Merges base Hats config with preset hats, preserving all base config settings
 * (max_iterations, backend, guardrails, etc.) while replacing only the hats and
 * events sections from the selected preset.
 *
 * Supports multiple preset formats:
 * - "default" - Use base config unchanged
 * - "builtin:name" - Load from presets/builtin/{name}.yml
 * - "directory:name" - Load from .hats/hats/{name}.yml
 * - UUID - Export from CollectionService
 */

import * as fs from "fs";
import * as path from "path";
import { parse as yamlParse, stringify as yamlStringify } from "yaml";
import { CollectionService } from "./CollectionService";

/**
 * Hat configuration in Hats YAML format
 */
interface HatConfig {
  name: string;
  description: string;
  triggers: string[];
  publishes: string[];
  instructions?: string;
  default_publishes?: string;
}

/**
 * Event metadata in Hats YAML format
 */
interface EventMetadata {
  description?: string;
  on_trigger?: string;
  on_publish?: string;
}

/**
 * Hats config structure (partial - only fields we care about)
 */
interface HatsConfig {
  event_loop?: {
    max_iterations?: number;
    max_runtime_seconds?: number;
    completion_promise?: string;
    starting_event?: string;
    prompt_file?: string;
  };
  cli?: {
    backend?: string;
    prompt_mode?: string;
  };
  core?: {
    specs_dir?: string;
    [key: string]: unknown;
  };
  hats?: Record<string, HatConfig>;
  events?: Record<string, EventMetadata>;
  [key: string]: unknown;
}

/**
 * Result of merging configs
 */
export interface MergeResult {
  config: HatsConfig;
  tempPath: string;
}

/**
 * ConfigMerger options
 */
interface ConfigMergerOptions {
  presetsDir: string;
  directoryPresetsRoot?: string;
  collectionService?: CollectionService;
  tempDir?: string;
}

/**
 * UUID regex for detecting collection IDs
 */
const UUID_REGEX = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Resolved preset data ready for merging
 */
interface PresetData {
  hats: Record<string, HatConfig> | null;
  events: Record<string, EventMetadata> | null;
  eventLoopOverrides: Partial<HatsConfig["event_loop"]>;
}

const EMPTY_PRESET: PresetData = { hats: null, events: null, eventLoopOverrides: {} };

/**
 * ConfigMerger - merges base config with preset hats
 */
export class ConfigMerger {
  private readonly presetsDir: string;
  private readonly directoryPresetsRoot: string;
  private readonly collectionService?: CollectionService;
  private readonly tempDir: string;

  constructor(options: ConfigMergerOptions) {
    this.presetsDir = options.presetsDir;
    this.directoryPresetsRoot = options.directoryPresetsRoot ?? process.cwd();
    this.collectionService = options.collectionService;
    this.tempDir = options.tempDir ?? path.join(process.cwd(), ".hats", "temp");
  }

  /**
   * Merge base config with preset hats
   */
  merge(basePath: string, preset: string): MergeResult {
    const baseConfig = this.loadBaseConfig(basePath);

    if (preset === "default") {
      return { config: baseConfig, tempPath: basePath };
    }

    const { hats, events, eventLoopOverrides } = this.resolvePreset(preset);

    if (!hats) {
      return { config: baseConfig, tempPath: basePath };
    }

    const mergedConfig = this.mergeConfigs(baseConfig, hats, events, eventLoopOverrides);
    const tempPath = this.writeTempConfig(mergedConfig);

    return { config: mergedConfig, tempPath };
  }

  /**
   * Load and validate base config
   */
  private loadBaseConfig(basePath: string): HatsConfig {
    if (!fs.existsSync(basePath)) {
      throw new Error(`Base config not found: ${basePath}`);
    }

    const content = fs.readFileSync(basePath, "utf-8");

    try {
      return yamlParse(content) as HatsConfig;
    } catch {
      throw new Error(`Invalid YAML in base config: ${basePath}`);
    }
  }

  /**
   * Resolve preset identifier to hats, events, and event_loop overrides
   */
  private resolvePreset(preset: string): PresetData {
    if (preset.startsWith("builtin:")) {
      return this.loadFilePreset(
        path.join(this.presetsDir, `${preset.slice(8)}.yml`),
        `Preset not found: ${preset}`
      );
    }

    if (preset.startsWith("directory:")) {
      return this.loadFilePreset(
        path.join(this.directoryPresetsRoot, ".hats", "hats", `${preset.slice(10)}.yml`),
        `Preset not found: ${preset}`
      );
    }

    if (UUID_REGEX.test(preset)) {
      return this.loadCollectionPreset(preset);
    }

    return EMPTY_PRESET;
  }

  /**
   * Load a preset from a YAML file on disk
   */
  private loadFilePreset(presetPath: string, errorMessage: string): PresetData {
    if (!fs.existsSync(presetPath)) {
      throw new Error(errorMessage);
    }

    const content = fs.readFileSync(presetPath, "utf-8");
    return this.extractPresetData(yamlParse(content) as HatsConfig);
  }

  /**
   * Load collection preset by UUID
   */
  private loadCollectionPreset(uuid: string): PresetData {
    if (!this.collectionService) {
      return EMPTY_PRESET;
    }

    const yamlContent = this.collectionService.exportToYaml(uuid);
    if (!yamlContent) {
      return EMPTY_PRESET;
    }

    return this.extractPresetData(yamlParse(yamlContent) as HatsConfig);
  }

  /**
   * Extract hats, events, and event_loop overrides from a parsed preset config
   */
  private extractPresetData(presetConfig: HatsConfig): PresetData {
    const events = presetConfig.events ?? this.deriveEventsFromHats(presetConfig.hats ?? {});

    const eventLoopOverrides: Partial<HatsConfig["event_loop"]> = {};
    if (presetConfig.event_loop?.starting_event) {
      eventLoopOverrides.starting_event = presetConfig.event_loop.starting_event;
    }

    return {
      hats: presetConfig.hats ?? null,
      events,
      eventLoopOverrides,
    };
  }

  /**
   * Derive events from hat triggers/publishes
   */
  private deriveEventsFromHats(hats: Record<string, HatConfig>): Record<string, EventMetadata> {
    const events: Record<string, EventMetadata> = {};

    for (const hatConfig of Object.values(hats)) {
      // Add events from triggers
      for (const trigger of hatConfig.triggers ?? []) {
        if (!events[trigger]) {
          events[trigger] = { description: `Event: ${trigger}` };
        }
      }

      // Add events from publishes
      for (const publish of hatConfig.publishes ?? []) {
        if (!events[publish]) {
          events[publish] = { description: `Event: ${publish}` };
        }
      }
    }

    return events;
  }

  /**
   * Merge base config with preset hats and events
   */
  private mergeConfigs(
    baseConfig: HatsConfig,
    hats: Record<string, HatConfig>,
    events: Record<string, EventMetadata> | null,
    eventLoopOverrides: Partial<HatsConfig["event_loop"]>
  ): HatsConfig {
    const merged: HatsConfig = {
      ...baseConfig,
      hats,
    };

    if (events) {
      merged.events = events;
    }

    // Merge event_loop overrides (like starting_event from preset)
    if (eventLoopOverrides && Object.keys(eventLoopOverrides).length > 0) {
      merged.event_loop = {
        ...baseConfig.event_loop,
        ...eventLoopOverrides,
      };
    }

    return merged;
  }

  /**
   * Write merged config to temp file
   */
  private writeTempConfig(config: HatsConfig): string {
    // Ensure temp directory exists
    if (!fs.existsSync(this.tempDir)) {
      fs.mkdirSync(this.tempDir, { recursive: true });
    }

    // Generate unique filename
    const timestamp = Date.now();
    const random = Math.random().toString(36).substring(2, 8);
    const filename = `merged-config-${timestamp}-${random}.yml`;
    const tempPath = path.join(this.tempDir, filename);

    // Write YAML
    const yamlContent = yamlStringify(config, {
      lineWidth: 100,
      defaultStringType: "PLAIN",
      defaultKeyType: "PLAIN",
    });

    fs.writeFileSync(tempPath, yamlContent, "utf-8");

    return tempPath;
  }
}
