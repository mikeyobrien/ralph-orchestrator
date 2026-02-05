/**
 * CollectionRepository
 *
 * Data access layer for hat collections. Collections are named groups of
 * interconnected hats that form a visual workflow (similar to n8n).
 *
 * Stores React Flow graph state (nodes, edges, viewport) for persistence.
 */

import { eq } from "drizzle-orm";
import { BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import * as schema from "../db/schema";
import { collections, Collection, NewCollection } from "../db/schema";
import { v4 as uuidv4 } from "uuid";

/**
 * Node position in the visual canvas
 */
export interface NodePosition {
  x: number;
  y: number;
}

/**
 * Hat node data stored within the graph
 */
export interface HatNodeData {
  key: string;
  name: string;
  description: string;
  triggersOn: string[];
  publishes: string[];
  instructions?: string;
}

/**
 * React Flow node structure
 */
export interface GraphNode {
  id: string;
  type: string;
  position: NodePosition;
  data: HatNodeData;
}

/**
 * React Flow edge structure (connection between nodes)
 */
export interface GraphEdge {
  id: string;
  source: string; // Node ID
  target: string; // Node ID
  sourceHandle?: string; // Output handle (event name)
  targetHandle?: string; // Input handle (event name)
  label?: string; // Event name displayed on edge
}

/**
 * React Flow viewport state
 */
export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

/**
 * Complete graph state persisted to database
 */
export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
  viewport: Viewport;
}

/**
 * Collection with parsed graph data
 */
export interface CollectionWithGraph extends Omit<Collection, "graphData"> {
  graph: GraphData;
}

/**
 * CollectionRepository - CRUD operations for hat collections
 */
export class CollectionRepository {
  private readonly db: BetterSQLite3Database<typeof schema>;

  constructor(db: BetterSQLite3Database<typeof schema>) {
    this.db = db;
  }

  /**
   * Get all collections (without graph data for listing)
   */
  findAll(): Omit<Collection, "graphData">[] {
    const rows = this.db.select().from(collections).all();
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    return rows.map(({ graphData, ...rest }) => rest);
  }

  /**
   * Get a single collection by ID with parsed graph data
   */
  findById(id: string): CollectionWithGraph | null {
    const row = this.db.select().from(collections).where(eq(collections.id, id)).get();
    if (!row) return null;

    return {
      id: row.id,
      name: row.name,
      description: row.description,
      createdAt: row.createdAt,
      updatedAt: row.updatedAt,
      graph: JSON.parse(row.graphData) as GraphData,
    };
  }

  /**
   * Create a new collection
   */
  create(data: {
    name: string;
    description?: string;
    graph?: GraphData;
  }): CollectionWithGraph {
    const id = uuidv4();
    const now = new Date();
    const graph: GraphData = data.graph ?? {
      nodes: [],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };

    const newCollection: NewCollection = {
      id,
      name: data.name,
      description: data.description ?? null,
      graphData: JSON.stringify(graph),
      createdAt: now,
      updatedAt: now,
    };

    this.db.insert(collections).values(newCollection).run();

    return {
      id,
      name: data.name,
      description: data.description ?? null,
      createdAt: now,
      updatedAt: now,
      graph,
    };
  }

  /**
   * Update an existing collection
   */
  update(
    id: string,
    data: {
      name?: string;
      description?: string;
      graph?: GraphData;
    }
  ): CollectionWithGraph | null {
    const existing = this.findById(id);
    if (!existing) return null;

    const now = new Date();
    const updates: Partial<NewCollection> = {
      updatedAt: now,
    };

    if (data.name !== undefined) {
      updates.name = data.name;
    }
    if (data.description !== undefined) {
      updates.description = data.description;
    }
    if (data.graph !== undefined) {
      updates.graphData = JSON.stringify(data.graph);
    }

    this.db.update(collections).set(updates).where(eq(collections.id, id)).run();

    return this.findById(id);
  }

  /**
   * Delete a collection
   */
  delete(id: string): boolean {
    const result = this.db.delete(collections).where(eq(collections.id, id)).run();
    return result.changes > 0;
  }
}
