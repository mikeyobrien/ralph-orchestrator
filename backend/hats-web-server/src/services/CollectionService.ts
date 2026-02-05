/**
 * CollectionService
 *
 * Business logic for hat collections including:
 * - CRUD operations via repository
 * - Exporting collections to Hats YAML preset format
 * - Importing YAML presets as collections
 * - Deriving event flow from visual connections
 */

import { stringify as yamlStringify, parse as yamlParse } from "yaml";
import {
  CollectionRepository,
  CollectionWithGraph,
  GraphData,
  GraphNode,
  GraphEdge,
  HatNodeData,
} from "../repositories/CollectionRepository";

/**
 * Hat configuration in Hats YAML format
 */
interface YamlHatConfig {
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
interface YamlEventMetadata {
  description?: string;
  on_trigger?: string;
  on_publish?: string;
}

/**
 * Complete Hats preset YAML structure
 */
interface HatsPresetYaml {
  event_loop?: {
    prompt_file?: string;
    completion_promise?: string;
    starting_event?: string;
    max_iterations?: number;
  };
  cli?: {
    backend?: string;
    prompt_mode?: string;
  };
  hats: Record<string, YamlHatConfig>;
  events?: Record<string, YamlEventMetadata>;
}

/**
 * CollectionService - manages hat collections and YAML conversion
 */
export class CollectionService {
  private readonly repository: CollectionRepository;

  constructor(repository: CollectionRepository) {
    this.repository = repository;
  }

  /**
   * List all collections (metadata only)
   */
  listCollections() {
    return this.repository.findAll();
  }

  /**
   * Get a single collection with full graph data
   */
  getCollection(id: string): CollectionWithGraph | null {
    return this.repository.findById(id);
  }

  /**
   * Create a new collection
   */
  createCollection(data: { name: string; description?: string; graph?: GraphData }) {
    return this.repository.create(data);
  }

  /**
   * Update an existing collection
   */
  updateCollection(id: string, data: { name?: string; description?: string; graph?: GraphData }) {
    return this.repository.update(id, data);
  }

  /**
   * Delete a collection
   */
  deleteCollection(id: string): boolean {
    return this.repository.delete(id);
  }

  /**
   * Export a collection to Hats YAML preset format
   *
   * The export process:
   * 1. Extract hats from graph nodes
   * 2. Derive triggers/publishes from edges (connections)
   * 3. Auto-generate event metadata descriptions
   * 4. Format as YAML with sensible defaults
   */
  exportToYaml(id: string): string | null {
    const collection = this.repository.findById(id);
    if (!collection) return null;

    const { nodes, edges } = collection.graph;

    // Build trigger/publish maps from edges
    const hatTriggers = new Map<string, Set<string>>();
    const hatPublishes = new Map<string, Set<string>>();
    const allEvents = new Set<string>();

    // Initialize maps for all hats
    for (const node of nodes) {
      hatTriggers.set(node.id, new Set(node.data.triggersOn));
      hatPublishes.set(node.id, new Set(node.data.publishes));
    }

    // Process edges to derive event flow
    for (const edge of edges) {
      const eventName = edge.label || `${edge.source}_to_${edge.target}`;
      allEvents.add(eventName);

      // Source hat publishes this event
      const sourcePublishes = hatPublishes.get(edge.source);
      if (sourcePublishes) {
        sourcePublishes.add(eventName);
      }

      // Target hat triggers on this event
      const targetTriggers = hatTriggers.get(edge.target);
      if (targetTriggers) {
        targetTriggers.add(eventName);
      }
    }

    // Build hats config
    const hats: Record<string, YamlHatConfig> = {};
    for (const node of nodes) {
      const triggers = Array.from(hatTriggers.get(node.id) ?? []);
      const publishes = Array.from(hatPublishes.get(node.id) ?? []);

      const hatConfig: YamlHatConfig = {
        name: node.data.name,
        description: node.data.description,
        triggers,
        publishes,
      };

      if (node.data.instructions) {
        hatConfig.instructions = node.data.instructions;
      }

      // Set default_publishes to first publish event if multiple exist
      if (publishes.length > 0) {
        hatConfig.default_publishes = publishes[0];
      }

      hats[node.data.key] = hatConfig;
    }

    // Build events metadata
    const events: Record<string, YamlEventMetadata> = {};
    for (const eventName of allEvents) {
      events[eventName] = {
        description: `Event: ${eventName}`,
      };
    }

    // Determine starting event (first hat's first trigger or default)
    const startingEvent = nodes.length > 0 ? "task.start" : "task.start";

    // Build complete preset
    const preset: HatsPresetYaml = {
      event_loop: {
        completion_promise: "LOOP_COMPLETE",
        starting_event: startingEvent,
        max_iterations: 50,
      },
      cli: {
        backend: "claude",
        prompt_mode: "arg",
      },
      hats,
    };

    // Only include events if we have any
    if (Object.keys(events).length > 0) {
      preset.events = events;
    }

    // Generate YAML with header comment
    const yamlContent = yamlStringify(preset, {
      lineWidth: 100,
      defaultStringType: "PLAIN",
      defaultKeyType: "PLAIN",
    });

    const header = `# ${collection.name}\n# ${collection.description || "Generated by Hats Hat Collection Builder"}\n# Generated at: ${new Date().toISOString()}\n\n`;

    return header + yamlContent;
  }

  /**
   * Import a YAML preset as a new collection
   *
   * The import process:
   * 1. Parse YAML structure
   * 2. Create nodes for each hat with auto-layout
   * 3. Create edges based on trigger/publish relationships
   * 4. Save as new collection
   */
  importFromYaml(yamlContent: string, name: string, description?: string): CollectionWithGraph {
    const preset = yamlParse(yamlContent) as HatsPresetYaml;

    const nodes: GraphNode[] = [];
    const edges: GraphEdge[] = [];

    // Track event publishers and subscribers for edge creation
    const eventPublishers = new Map<string, string[]>(); // event -> [nodeIds]
    const eventSubscribers = new Map<string, string[]>(); // event -> [nodeIds]

    // Create nodes from hats with auto-layout (vertical arrangement)
    const hatEntries = Object.entries(preset.hats || {});
    let yPosition = 50;

    for (const [hatKey, hatConfig] of hatEntries) {
      const nodeId = hatKey;

      const nodeData: HatNodeData = {
        key: hatKey,
        name: hatConfig.name,
        description: hatConfig.description,
        triggersOn: hatConfig.triggers || [],
        publishes: hatConfig.publishes || [],
        instructions: hatConfig.instructions,
      };

      nodes.push({
        id: nodeId,
        type: "hatNode",
        position: { x: 250, y: yPosition },
        data: nodeData,
      });

      yPosition += 200;

      // Track event relationships
      for (const event of hatConfig.publishes || []) {
        if (!eventPublishers.has(event)) {
          eventPublishers.set(event, []);
        }
        eventPublishers.get(event)!.push(nodeId);
      }

      for (const event of hatConfig.triggers || []) {
        if (!eventSubscribers.has(event)) {
          eventSubscribers.set(event, []);
        }
        eventSubscribers.get(event)!.push(nodeId);
      }
    }

    // Create edges for event relationships
    let edgeIndex = 0;
    for (const [event, publishers] of eventPublishers) {
      const subscribers = eventSubscribers.get(event) || [];
      for (const publisher of publishers) {
        for (const subscriber of subscribers) {
          if (publisher !== subscriber) {
            edges.push({
              id: `edge-${edgeIndex++}`,
              source: publisher,
              target: subscriber,
              label: event,
              sourceHandle: event,
              targetHandle: event,
            });
          }
        }
      }
    }

    const graph: GraphData = {
      nodes,
      edges,
      viewport: { x: 0, y: 0, zoom: 0.8 },
    };

    return this.repository.create({
      name,
      description,
      graph,
    });
  }
}
