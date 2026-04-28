/**
 * Tests for the builder layout helpers.
 *
 * Covers autoLayout (linear, branching, cyclic graphs) and needsLayout
 * (Rust-stacked input vs user-placed positions).
 */

import { describe, expect, it } from "vitest";
import type { LayoutEdge, LayoutNode } from "./layout";
import { autoLayout, needsLayout } from "./layout";

function makeNode(id: string, x = 0, y = 0): LayoutNode {
  return { id, position: { x, y } };
}

function makeEdge(source: string, target: string): LayoutEdge {
  return { source, target };
}

function xAt(nodes: LayoutNode[], id: string): number {
  const node = nodes.find((n) => n.id === id);
  if (!node) throw new Error(`node ${id} not found`);
  return node.position.x;
}

function yAt(nodes: LayoutNode[], id: string): number {
  const node = nodes.find((n) => n.id === id);
  if (!node) throw new Error(`node ${id} not found`);
  return node.position.y;
}

describe("autoLayout", () => {
  describe("linear chain", () => {
    it("places A → B → C in ascending columns", () => {
      const nodes = [makeNode("a"), makeNode("b"), makeNode("c")];
      const edges = [makeEdge("a", "b"), makeEdge("b", "c")];

      const positioned = autoLayout(nodes, edges);

      expect(xAt(positioned, "a")).toBeLessThan(xAt(positioned, "b"));
      expect(xAt(positioned, "b")).toBeLessThan(xAt(positioned, "c"));
    });
  });

  describe("branching", () => {
    it("places siblings at the same x column, different y", () => {
      const nodes = [makeNode("root"), makeNode("left"), makeNode("right")];
      const edges = [makeEdge("root", "left"), makeEdge("root", "right")];

      const positioned = autoLayout(nodes, edges);

      expect(xAt(positioned, "left")).toBe(xAt(positioned, "right"));
      expect(yAt(positioned, "left")).not.toBe(yAt(positioned, "right"));
      expect(xAt(positioned, "root")).toBeLessThan(xAt(positioned, "left"));
    });
  });

  describe("cycle", () => {
    it("terminates and assigns every node a level", () => {
      const nodes = [makeNode("a"), makeNode("b"), makeNode("c")];
      const edges = [makeEdge("a", "b"), makeEdge("b", "c"), makeEdge("c", "a")];

      const positioned = autoLayout(nodes, edges);

      // Every node must receive a finite position.
      expect(positioned).toHaveLength(3);
      for (const node of positioned) {
        expect(Number.isFinite(node.position.x)).toBe(true);
        expect(Number.isFinite(node.position.y)).toBe(true);
      }
    });
  });

  describe("disconnected nodes", () => {
    it("places an isolated node at level 0 alongside other roots", () => {
      const nodes = [makeNode("a"), makeNode("b"), makeNode("orphan")];
      const edges = [makeEdge("a", "b")];

      const positioned = autoLayout(nodes, edges);

      // `orphan` has in-degree 0, so it's treated as a root just like `a`.
      // Both share level 0 (x = LEVEL_X_START); `b` is one level deeper.
      expect(xAt(positioned, "orphan")).toBe(xAt(positioned, "a"));
      expect(xAt(positioned, "b")).toBeGreaterThan(xAt(positioned, "orphan"));
    });
  });

  describe("empty input", () => {
    it("returns the input unchanged when there are no nodes", () => {
      const result = autoLayout([], []);
      expect(result).toEqual([]);
    });
  });
});

describe("needsLayout", () => {
  it("returns true when every node shares an x-coordinate (Rust-stacked import)", () => {
    const nodes = [makeNode("a", 250, 50), makeNode("b", 250, 250), makeNode("c", 250, 450)];
    expect(needsLayout(nodes)).toBe(true);
  });

  it("returns false when x-coordinates vary (user-placed)", () => {
    const nodes = [makeNode("a", 0, 0), makeNode("b", 280, 0)];
    expect(needsLayout(nodes)).toBe(false);
  });

  it("returns false for a single node", () => {
    expect(needsLayout([makeNode("only", 250, 50)])).toBe(false);
  });

  it("returns false for an empty graph", () => {
    expect(needsLayout([])).toBe(false);
  });
});
