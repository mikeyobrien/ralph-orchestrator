/**
 * useLoopObservation
 *
 * Subscribes to `loop.orchestration` stream events and drives the
 * observation zustand store. Owns the topology-aware inference that
 * lets us handle both event shapes Ralph produces:
 *
 *  1. Loop-level events (Ralph itself): carry `hat`, `iteration`, and
 *     `triggered` — explicit transition.
 *  2. Agent-level events (kiro-acp, claude, etc. via tool calls): only
 *     `ts`, `topic`, `payload`. We infer the transition by matching
 *     `topic` to an edge label in the current graph and applying
 *     `edge.source → completed`, `edge.target → active`.
 *
 * The store stays graph-agnostic; the hook owns edge topology.
 */

import { buildStreamWebSocketUrl, rpcSubscribe, rpcUnsubscribe, type StreamEventEnvelope } from "@/rpc/client";
import {
    useObservationStore,
    type OrchestrationEventPayload,
} from "@/stores/observationStore";
import type { Edge } from "@xyflow/react";
import { useCallback, useEffect, useRef } from "react";

const OBSERVATION_TOPICS = ["loop.orchestration", "stream.keepalive"];
const EDGE_ANIMATION_MS = 1500;
const TERMINAL_TOPICS = new Set(["LOOP_COMPLETE", "loop.terminate"]);

/**
 * Subscribes to loop orchestration events when observation is active.
 *
 * @param edges - The current graph edges. Used to map topics (agent-level
 *   events that lack a `hat` field) to source/target hats.
 */
export function useLoopObservation(edges: Edge[]) {
  const active = useObservationStore((s) => s.active);
  const taskId = useObservationStore((s) => s.taskId);
  const setHatActive = useObservationStore((s) => s.setHatActive);
  const setHatPending = useObservationStore((s) => s.setHatPending);
  const setIteration = useObservationStore((s) => s.setIteration);
  const completeAll = useObservationStore((s) => s.completeAll);
  const fireEdge = useObservationStore((s) => s.fireEdge);
  const clearFiredEdge = useObservationStore((s) => s.clearFiredEdge);

  const wsRef = useRef<WebSocket | null>(null);
  const subscriptionIdRef = useRef<string | null>(null);
  const edgeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Keep latest edges accessible to the WebSocket message handler without
  // re-subscribing on every edge change (which would drop replay position).
  const edgesRef = useRef(edges);
  edgesRef.current = edges;

  /** Return all edges whose label exactly matches `topic`. */
  const edgesForTopic = useCallback((topic: string): Edge[] => {
    return edgesRef.current.filter(
      (e) => typeof e.label === "string" && e.label === topic
    );
  }, []);

  /** Start the edge pulse animation, canceling any in-flight pulse. */
  const pulseEdge = useCallback(
    (edgeId: string) => {
      if (edgeTimerRef.current) {
        clearTimeout(edgeTimerRef.current);
      }
      fireEdge(edgeId);
      edgeTimerRef.current = setTimeout(() => {
        clearFiredEdge();
        edgeTimerRef.current = null;
      }, EDGE_ANIMATION_MS);
    },
    [fireEdge, clearFiredEdge]
  );

  /**
   * Apply an event to the observation state.
   *
   * Exported via ref + useEffect so the WebSocket handler always uses the
   * latest closure without re-subscribing.
   */
  const dispatchEvent = useCallback(
    (payload: OrchestrationEventPayload) => {
      // Iteration counter updates regardless of hat context.
      if (typeof payload.iteration === "number") {
        setIteration(payload.iteration);
      }

      // Terminal events always override hat-level logic.
      if (payload.topic && TERMINAL_TOPICS.has(payload.topic)) {
        completeAll();
        return;
      }

      // Case 1: Loop-level event — `hat: "loop"` with a `triggered` target.
      // The framework is handing control to `triggered`.
      if (payload.hat === "loop" && payload.triggered) {
        setHatActive(payload.triggered);
        return;
      }

      // Case 2: Hat-level event — a real hat is emitting.
      // It is currently active; its `triggered` target (if any) is pending.
      if (payload.hat && payload.hat !== "loop") {
        setHatActive(payload.hat);
        if (payload.triggered) {
          setHatPending(payload.triggered);
        }
        // Also fire the matching edge so the UI shows the handoff path.
        if (payload.topic) {
          for (const edge of edgesForTopic(payload.topic)) {
            pulseEdge(edge.id);
          }
        }
        return;
      }

      // Case 3: Agent-level event — no `hat`, only `topic`. Infer from graph.
      if (payload.topic) {
        const matching = edgesForTopic(payload.topic);
        for (const edge of matching) {
          setHatActive(edge.target);
          pulseEdge(edge.id);
        }
      }
    },
    [completeAll, edgesForTopic, pulseEdge, setHatActive, setHatPending, setIteration]
  );

  // Stable ref to the latest dispatcher — lets the WebSocket handler
  // run through an `useEffect` that only cares about (active, taskId).
  const dispatchRef = useRef(dispatchEvent);
  dispatchRef.current = dispatchEvent;

  useEffect(() => {
    if (!active || !taskId) return;

    let cancelled = false;

    void (async () => {
      try {
        const subscription = await rpcSubscribe({
          topics: OBSERVATION_TOPICS,
          replayLimit: 100,
        });

        if (cancelled) {
          void rpcUnsubscribe(subscription.subscriptionId).catch(() => {});
          return;
        }

        subscriptionIdRef.current = subscription.subscriptionId;

        const ws = new WebSocket(buildStreamWebSocketUrl(subscription.subscriptionId));
        wsRef.current = ws;

        ws.onmessage = (message) => {
          if (cancelled) return;

          let event: StreamEventEnvelope;
          try {
            event = JSON.parse(String(message.data)) as StreamEventEnvelope;
          } catch {
            return;
          }

          if (event.topic !== "loop.orchestration") return;

          const payload = event.payload as unknown as OrchestrationEventPayload;
          if (!payload || typeof payload !== "object") return;

          dispatchRef.current(payload);
        };

        ws.onerror = () => {
          // Reconnection is not implemented for MVP — the user can
          // stop and re-run if the connection drops.
        };
      } catch {
        // Subscription failed — observation stays active but no events arrive.
      }
    })();

    return () => {
      cancelled = true;

      if (edgeTimerRef.current) {
        clearTimeout(edgeTimerRef.current);
        edgeTimerRef.current = null;
      }

      if (wsRef.current) {
        wsRef.current.onclose = null;
        wsRef.current.close();
        wsRef.current = null;
      }

      const subId = subscriptionIdRef.current;
      subscriptionIdRef.current = null;
      if (subId) {
        void rpcUnsubscribe(subId).catch(() => {});
      }
    };
  }, [active, taskId]);
}
