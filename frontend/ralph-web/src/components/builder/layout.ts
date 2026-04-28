/**
 * Pure layout utilities for the collection builder.
 *
 * `autoLayout` runs a Kahn-style BFS to assign each node a level based on
 * its distance from the graph's roots, then positions nodes in a left-to-right
 * column-per-level grid. Handles cycles by ignoring back-edges (first-seen
 * level wins) and handles pure-cycle graphs by seeding level 0 with the
 * first node. `needsLayout` is a heuristic that detects the Rust YAML
 * importer's stacked output (all nodes at the same x-coordinate).
 *
 * The structural `LayoutNode` / `LayoutEdge` types keep this module free of
 * a runtime dependency on `@xyflow/react` so it can be imported in vitest
 * without pulling the React Flow bundle (which breaks the test runner
 * under the @tailwindcss/vite plugin).
 */

export interface LayoutNode {
  id: string;
  position: { x: number; y: number };
}

export interface LayoutEdge {
  source: string;
  target: string;
}


const LEVEL_X_SPACING = 280;
const LEVEL_Y_SPACING = 160;
const LEVEL_X_START = 40;
const LEVEL_Y_START = 40;

/**
 * Detect the "freshly imported from YAML" layout pattern where every node
 * is stacked at the same x-coordinate. Used to decide whether to run
 * autoLayout on mount without disturbing user-placed graphs.
 */
export function needsLayout(nodes: LayoutNode[]): boolean {
  if (nodes.length < 2) return false;
  const xs = new Set(nodes.map((n) => n.position.x));
  return xs.size === 1;
}

/**
 * Assign a level (x-column) to every node via BFS from in-degree-0 roots.
 * Returns a Map<nodeId, level>. Nodes unreachable from any root are placed
 * after the deepest reachable level.
 */
function computeLevels(nodes: LayoutNode[], edges: LayoutEdge[]): Map<string, number> {
  const adjacency = new Map<string, string[]>();
  const inDegree = new Map<string, number>();

  for (const node of nodes) {
    adjacency.set(node.id, []);
    inDegree.set(node.id, 0);
  }

  for (const edge of edges) {
    const neighbors = adjacency.get(edge.source);
    if (neighbors) {
      neighbors.push(edge.target);
    }
    inDegree.set(edge.target, (inDegree.get(edge.target) ?? 0) + 1);
  }

  const levels = new Map<string, number>();
  const queue: string[] = [];

  for (const [id, degree] of inDegree) {
    if (degree === 0) {
      levels.set(id, 0);
      queue.push(id);
    }
  }

  // Pure-cycle fallback: no roots at all.
  if (queue.length === 0 && nodes.length > 0) {
    const seedId = nodes[0].id;
    levels.set(seedId, 0);
    queue.push(seedId);
  }

  while (queue.length > 0) {
    const currentId = queue.shift() as string;
    const currentLevel = levels.get(currentId) ?? 0;
    for (const neighborId of adjacency.get(currentId) ?? []) {
      // First-seen wins: if the neighbor already has a level, this is a
      // back-edge in a cycle — skip it to guarantee termination.
      if (levels.has(neighborId)) continue;
      levels.set(neighborId, currentLevel + 1);
      queue.push(neighborId);
    }
  }

  // Unreached nodes (disconnected subgraphs that were cycles without an
  // entry point, or islands) go after the deepest reached level.
  let maxLevel = -1;
  for (const level of levels.values()) {
    if (level > maxLevel) maxLevel = level;
  }
  for (const node of nodes) {
    if (!levels.has(node.id)) {
      levels.set(node.id, maxLevel + 1);
    }
  }

  return levels;
}

/**
 * Position nodes in a left-to-right layered grid based on graph topology.
 * Node identities are preserved; only `position` is rewritten.
 */
export function autoLayout<N extends LayoutNode>(nodes: N[], edges: LayoutEdge[]): N[] {
  if (nodes.length === 0) return nodes;

  const levels = computeLevels(nodes, edges);

  // Group nodes by their level so we can stack them vertically within each column.
  const byLevel = new Map<number, N[]>();
  for (const node of nodes) {
    const level = levels.get(node.id) ?? 0;
    const bucket = byLevel.get(level);
    if (bucket) {
      bucket.push(node);
    } else {
      byLevel.set(level, [node]);
    }
  }

  const positioned: N[] = [];
  for (const [level, bucket] of byLevel) {
    bucket.forEach((node, indexInLevel) => {
      positioned.push({
        ...node,
        position: {
          x: LEVEL_X_START + level * LEVEL_X_SPACING,
          y: LEVEL_Y_START + indexInLevel * LEVEL_Y_SPACING,
        },
      });
    });
  }

  // Preserve the original node ordering so downstream consumers (React Flow)
  // receive a stable list.
  const order = new Map(nodes.map((node, index) => [node.id, index]));
  positioned.sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0));

  return positioned;
}
