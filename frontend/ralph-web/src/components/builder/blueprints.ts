/**
 * Pre-built workflow blueprints.
 *
 * Each blueprint is a complete hat collection with nodes, edges, and
 * metadata. When the user picks a blueprint, a new collection is created
 * with these nodes and edges pre-populated and auto-laid-out.
 */

import type { HatNodeData } from "./HatNode";

export interface Blueprint {
  id: string;
  name: string;
  description: string;
  emoji: string;
  hats: HatNodeData[];
  /** Edges as [sourceKey, targetKey, eventName] tuples. */
  edges: [string, string, string][];
}

export const BLUEPRINTS: Blueprint[] = [
  {
    id: "code-assist",
    name: "Code Assist",
    description: "Plan → Build → Review → Finalize. The standard development workflow.",
    emoji: "⚡",
    hats: [
      {
        key: "planner",
        name: "📋 Planner",
        description: "Breaks the task into ordered sub-tasks",
        triggersOn: ["work.start", "subtask.done"],
        publishes: ["subtask.ready", "all_steps.done"],
      },
      {
        key: "builder",
        name: "⚡ Builder",
        description: "Implements one sub-task at a time, runs tests",
        triggersOn: ["subtask.ready", "review.changes_requested"],
        publishes: ["subtask.done", "implementation.done"],
      },
      {
        key: "reviewer",
        name: "👀 Reviewer",
        description: "Reviews code quality and AGENTS.md compliance",
        triggersOn: ["all_steps.done", "implementation.done"],
        publishes: ["review.approved", "review.changes_requested"],
      },
      {
        key: "finalizer",
        name: "📝 Finalizer",
        description: "Documents changes and emits LOOP_COMPLETE",
        triggersOn: ["review.approved"],
        publishes: ["LOOP_COMPLETE"],
      },
    ],
    edges: [
      ["planner", "builder", "subtask.ready"],
      ["builder", "planner", "subtask.done"],
      ["planner", "reviewer", "all_steps.done"],
      ["builder", "reviewer", "implementation.done"],
      ["reviewer", "finalizer", "review.approved"],
      ["reviewer", "builder", "review.changes_requested"],
    ],
  },
  {
    id: "debug",
    name: "Debug",
    description: "Investigate → Test → Fix → Verify. For hunting bugs.",
    emoji: "🔍",
    hats: [
      {
        key: "investigator",
        name: "🔍 Investigator",
        description: "Reproduces the bug and identifies root cause",
        triggersOn: ["work.start", "fix.failed"],
        publishes: ["hypothesis.ready"],
      },
      {
        key: "fixer",
        name: "🔧 Fixer",
        description: "Applies the fix based on the hypothesis",
        triggersOn: ["hypothesis.ready"],
        publishes: ["fix.done", "fix.failed"],
      },
      {
        key: "verifier",
        name: "✅ Verifier",
        description: "Runs tests to confirm the fix works",
        triggersOn: ["fix.done"],
        publishes: ["LOOP_COMPLETE", "fix.failed"],
      },
    ],
    edges: [
      ["investigator", "fixer", "hypothesis.ready"],
      ["fixer", "verifier", "fix.done"],
      ["fixer", "investigator", "fix.failed"],
      ["verifier", "investigator", "fix.failed"],
    ],
  },
  {
    id: "research",
    name: "Research",
    description: "Explore → Synthesize. For code exploration without changes.",
    emoji: "📚",
    hats: [
      {
        key: "researcher",
        name: "🔬 Researcher",
        description: "Explores the codebase and gathers information",
        triggersOn: ["work.start", "synthesis.needs_more"],
        publishes: ["research.done"],
      },
      {
        key: "synthesizer",
        name: "📝 Synthesizer",
        description: "Summarizes findings into a coherent report",
        triggersOn: ["research.done"],
        publishes: ["LOOP_COMPLETE", "synthesis.needs_more"],
      },
    ],
    edges: [
      ["researcher", "synthesizer", "research.done"],
      ["synthesizer", "researcher", "synthesis.needs_more"],
    ],
  },
];
