/**
 * Observation Store
 *
 * Zustand store for live loop observation state in the Builder.
 * Survives navigation via zustand persistence semantics (module-level
 * singleton) so the user can leave /builder and return without losing
 * context.
 *
 * Design: the store is intentionally graph-agnostic. It knows nothing
 * about edges or hat keys beyond the id strings the hook passes in.
 * Topology-aware inference (matching stream events to the collection's
 * edges) lives in `useLoopObservation` where the current `edges` prop
 * is available.
 */

import { create } from "zustand";

export type NodeObservationState = "idle" | "pending" | "active" | "completed";

export interface ObservationStore {
  /** Whether observation mode is active. */
  active: boolean;
  /** The task / collection id being observed. */
  taskId: string | null;
  /** Current iteration number. */
  iteration: number;
  /** Hat id currently executing (excludes the synthetic `loop` framework marker). */
  activeHatId: string | null;
  /** Per-node observation state keyed by node id. */
  nodeStates: Record<string, NodeObservationState>;
  /** Edge id that most recently fired (for pulse animation). */
  lastFiredEdgeId: string | null;
  /** Timestamp of the last edge fire (for timer reset). */
  lastFiredAt: number;

  /** Enter observation mode for a given task / collection. */
  startObserving: (taskId: string) => void;
  /** Exit observation mode and reset all state. */
  stopObserving: () => void;

  /**
   * Mark `hatId` as active. The previously active hat transitions to
   * completed. No-op if `hatId` is already active.
   */
  setHatActive: (hatId: string) => void;

  /**
   * Mark `hatId` as pending ("up next"). Does not change the currently
   * active hat. No-op if `hatId` is already active or completed.
   */
  setHatPending: (hatId: string) => void;

  /** Update the iteration counter (monotonic — ignores lower values). */
  setIteration: (iteration: number) => void;

  /**
   * Terminal transition: every known node → completed, observation exits
   * after a short delay so the user sees the final state.
   */
  completeAll: () => void;

  /** Mark an edge as fired (triggers pulse animation). */
  fireEdge: (edgeId: string) => void;
  /** Clear the fired edge (called after animation completes). */
  clearFiredEdge: () => void;
}

/**
 * Shape of the `loop.orchestration` stream event payload.
 *
 * Ralph-emitted events (loop-level) set all fields. Agent-emitted events
 * (via tool calls) may only set `ts`, `topic`, and `payload` — `hat`
 * defaults to empty string and `iteration` to 0 via `#[serde(default)]`
 * in ralph-core's `EventRecord`.
 */
export interface OrchestrationEventPayload {
  iteration: number;
  hat: string;
  topic: string;
  triggered: string | null;
  ts: string;
}

export const useObservationStore = create<ObservationStore>((set, get) => ({
  active: false,
  taskId: null,
  iteration: 0,
  activeHatId: null,
  nodeStates: {},
  lastFiredEdgeId: null,
  lastFiredAt: 0,

  startObserving: (taskId: string) => {
    set({
      active: true,
      taskId,
      iteration: 0,
      activeHatId: null,
      nodeStates: {},
      lastFiredEdgeId: null,
      lastFiredAt: 0,
    });
  },

  stopObserving: () => {
    set({
      active: false,
      taskId: null,
      iteration: 0,
      activeHatId: null,
      nodeStates: {},
      lastFiredEdgeId: null,
      lastFiredAt: 0,
    });
  },

  setHatActive: (hatId: string) => {
    const state = get();
    if (!state.active || !hatId || hatId === state.activeHatId) return;

    const nextStates = { ...state.nodeStates };
    if (state.activeHatId) {
      nextStates[state.activeHatId] = "completed";
    }
    nextStates[hatId] = "active";
    set({ activeHatId: hatId, nodeStates: nextStates });
  },

  setHatPending: (hatId: string) => {
    const state = get();
    if (!state.active || !hatId) return;
    const current = state.nodeStates[hatId];
    // Never downgrade active or completed → pending.
    if (current === "active" || current === "completed") return;
    set({ nodeStates: { ...state.nodeStates, [hatId]: "pending" } });
  },

  setIteration: (iteration: number) => {
    const state = get();
    if (!state.active || iteration <= state.iteration) return;
    set({ iteration });
  },

  completeAll: () => {
    const state = get();
    if (!state.active) return;
    const next: Record<string, NodeObservationState> = { ...state.nodeStates };
    for (const key of Object.keys(next)) {
      next[key] = "completed";
    }
    if (state.activeHatId) {
      next[state.activeHatId] = "completed";
    }
    set({ nodeStates: next });
    // Delay so the user sees the final "all completed" state.
    setTimeout(() => {
      get().stopObserving();
    }, 2000);
  },

  fireEdge: (edgeId: string) => {
    set({ lastFiredEdgeId: edgeId, lastFiredAt: Date.now() });
  },

  clearFiredEdge: () => {
    set({ lastFiredEdgeId: null });
  },
}));

// Expose the store on `window` outside of production builds so Playwright
// can drive observation state deterministically without a real backend.
// This is a no-op in production bundles (Vite strips `import.meta.env.DEV`
// branches via dead-code elimination).
if (typeof window !== "undefined" && import.meta.env.DEV) {
  (window as unknown as { __observationStore?: typeof useObservationStore }).__observationStore =
    useObservationStore;
}
