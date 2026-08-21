//! Map autoloop's `--events` LoopEvent stream onto ralph's JSON-RPC `RpcEvent`
//! contract (#343).
//!
//! `ralph run --rpc` historically emitted [`RpcEvent`]s from the in-house engine
//! so external callers (IDE integrations, the subprocess TUI) could observe a run
//! over stdout. The v3 cutover made autoloop the sole engine, whose native
//! observability channel is the coarser `--events` NDJSON stream
//! ([`AutoloopEvent`]). This mapper bridges the two: it is the `--rpc` counterpart
//! to `ralph-tui`'s `autoloop_source` (which maps the same stream onto TUI state).
//!
//! The `--events` stream updates at iteration *boundaries*, not per token, so the
//! emitted events are correspondingly coarse: [`RpcEvent::IterationStart`], one
//! [`RpcEvent::TextDelta`] carrying the whole iteration's `backend.output`,
//! [`RpcEvent::OrchestrationEvent`] for routing/HITL, and a terminal
//! [`RpcEvent::LoopTerminated`]. Per-token deltas and precise per-iteration
//! `IterationEnd` cost/token accounting are not available from this stream, so
//! those `RpcEvent` variants are not emitted.

use ralph_proto::json_rpc::{RpcEvent, TerminationReason};

use crate::autoloop_events::AutoloopEvent;

/// Current wall-clock time as Unix milliseconds (`0` if the clock is before the
/// epoch, which never happens in practice).
fn now_unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Map an autoloop `stopReason` onto the RPC contract's [`TerminationReason`].
///
/// The RPC enum is deliberately small; autoloop's richer stop vocabulary
/// (`stalled`, `max_runtime`, `cost_budget`, `backend_failed`, …) collapses onto
/// [`TerminationReason::Error`] since there is no closer variant. This mirrors
/// how the in-house engine surfaced non-clean exits over `--rpc`.
fn map_stop_reason(reason: &str) -> TerminationReason {
    match reason {
        "completed" | "verdict_exit" => TerminationReason::Completed,
        "max_iterations" => TerminationReason::MaxIterations,
        "interrupted" => TerminationReason::Interrupted,
        _ => TerminationReason::Error,
    }
}

/// Terminal-result fields captured from a `summary` event, held until the run
/// ends in case no authoritative `loop.finish` follows (older autoloop builds).
#[derive(Debug, Clone)]
struct PendingTerminal {
    reason: TerminationReason,
    total_iterations: u32,
    total_cost_usd: f64,
}

/// Stateful translator from autoloop `--events` [`AutoloopEvent`]s to ralph
/// [`RpcEvent`]s.
///
/// Feed each parsed event through [`map`](AutoloopRpcMapper::map); call
/// [`finalize`](AutoloopRpcMapper::finalize) once after the stream drains to
/// flush a terminal event that only a `summary` (never a `loop.finish`)
/// reported. Tracks the active role (autoloop's role ≈ ralph's hat) so each
/// `iteration.start` is attributed to the hat the preceding `progress` named,
/// matching `ralph-tui`'s `autoloop_source` labelling.
#[derive(Debug)]
pub struct AutoloopRpcMapper {
    /// Loop start time (Unix ms), used to compute `LoopTerminated.duration_ms`.
    started_at: u64,
    /// Backend label placed on `IterationStart` (autoloop's default backend name
    /// when ralph's selection is not forwarded, see #347).
    backend: String,
    /// Display label for the active role, from the most recent
    /// `progress.allowedRoles[0]`. `None` until the first progress event.
    role_label: Option<String>,
    /// The most recently started iteration number; guards against emitting two
    /// `IterationStart`s for a duplicated `iteration.start`.
    current_iteration: Option<u32>,
    /// Iteration budget, from `iteration.start.maxIterations`.
    max_iterations: Option<u32>,
    /// Terminal result carried by a `summary`, pending an authoritative
    /// `loop.finish`.
    pending_terminal: Option<PendingTerminal>,
    /// Whether an authoritative terminal event (`loop.finish`) has been emitted.
    saw_terminal: bool,
}

impl AutoloopRpcMapper {
    /// Create a mapper for a run that started at `started_at` (Unix ms) using
    /// `backend` as the `IterationStart` backend label.
    pub fn new(started_at: u64, backend: impl Into<String>) -> Self {
        Self {
            started_at,
            backend: backend.into(),
            role_label: None,
            current_iteration: None,
            max_iterations: None,
            pending_terminal: None,
            saw_terminal: false,
        }
    }

    /// Translate one autoloop event into zero or more [`RpcEvent`]s.
    pub fn map(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        match event.kind.as_str() {
            "iteration.start" => {
                if let Some(max) = event.max_iterations {
                    self.max_iterations = Some(max);
                }
                let iteration = event.iteration.unwrap_or(0);
                // autoloop emits one iteration.start per iteration; dedupe so we
                // never emit two IterationStarts for the same number.
                if self.current_iteration == Some(iteration) {
                    return Vec::new();
                }
                self.current_iteration = Some(iteration);
                let hat = self.hat_label();
                vec![RpcEvent::IterationStart {
                    iteration,
                    max_iterations: self.max_iterations,
                    hat: hat.clone(),
                    hat_display: hat,
                    backend: self.backend.clone(),
                    started_at: now_unix_millis(),
                }]
            }

            "progress" => {
                // The role named here labels the NEXT iteration (parity with
                // autoloop_source), so update state before building the event.
                if let Some(first) = event.allowed_roles.as_ref().and_then(|r| r.first()) {
                    self.role_label = Some(first.clone());
                }
                let topic = event
                    .emitted_topic
                    .clone()
                    .or_else(|| event.recent_event.clone())
                    .unwrap_or_else(|| "progress".to_string());
                vec![RpcEvent::OrchestrationEvent {
                    topic,
                    payload: event.outcome.clone().unwrap_or_default(),
                    source: self.role_label.clone(),
                    target: None,
                }]
            }

            "backend.output" => match event.output.as_deref() {
                Some(output) if !output.is_empty() => vec![RpcEvent::TextDelta {
                    iteration: self.current_iteration.unwrap_or(0),
                    delta: output.to_string(),
                }],
                _ => Vec::new(),
            },

            // No dedicated human-ask RpcEvent variant exists; carry HITL over the
            // generic OrchestrationEvent channel (forward-compatible with the
            // "replace the contract" option in #343).
            "ask.pending" => vec![RpcEvent::OrchestrationEvent {
                topic: "human.ask".to_string(),
                payload: event.question.clone().unwrap_or_default(),
                source: event.question_id.clone(),
                target: None,
            }],

            "ask.answered" => vec![RpcEvent::OrchestrationEvent {
                topic: "human.answered".to_string(),
                payload: event.answer.clone().unwrap_or_default(),
                source: event.question_id.clone(),
                target: None,
            }],

            // `loop.finish` is authoritative (carries the final cost); emit it.
            "loop.finish" => {
                self.saw_terminal = true;
                vec![self.terminated_event(event)]
            }

            // `summary` precedes `loop.finish` and may omit cost. Stash it as a
            // fallback rather than emitting a duplicate LoopTerminated; flushed by
            // finalize() only if no loop.finish arrives.
            "summary" => {
                self.pending_terminal = Some(PendingTerminal {
                    reason: event
                        .stop_reason
                        .as_deref()
                        .map(map_stop_reason)
                        .unwrap_or(TerminationReason::Error),
                    total_iterations: event.iterations.or(self.current_iteration).unwrap_or(0),
                    total_cost_usd: event.cost_usd.unwrap_or(0.0),
                });
                Vec::new()
            }

            // Unknown event types decode losslessly and produce no RpcEvent.
            _ => Vec::new(),
        }
    }

    /// Flush a terminal [`RpcEvent::LoopTerminated`] if the run ended on a
    /// `summary` with no authoritative `loop.finish`. Returns `None` when a
    /// `loop.finish` was already emitted or the run reported no terminal event.
    pub fn finalize(&mut self) -> Option<RpcEvent> {
        if self.saw_terminal {
            return None;
        }
        let pending = self.pending_terminal.take()?;
        let terminated_at = now_unix_millis();
        Some(RpcEvent::LoopTerminated {
            reason: pending.reason,
            total_iterations: pending.total_iterations,
            duration_ms: terminated_at.saturating_sub(self.started_at),
            total_cost_usd: pending.total_cost_usd,
            terminated_at,
        })
    }

    /// True once a terminal event has been emitted or stashed.
    pub fn saw_terminal(&self) -> bool {
        self.saw_terminal || self.pending_terminal.is_some()
    }

    /// Build a `LoopTerminated` from a `loop.finish` (or `summary`) event.
    fn terminated_event(&self, event: &AutoloopEvent) -> RpcEvent {
        let terminated_at = now_unix_millis();
        RpcEvent::LoopTerminated {
            reason: event
                .stop_reason
                .as_deref()
                .map(map_stop_reason)
                .unwrap_or(TerminationReason::Error),
            total_iterations: event.iterations.or(self.current_iteration).unwrap_or(0),
            duration_ms: terminated_at.saturating_sub(self.started_at),
            total_cost_usd: event.cost_usd.unwrap_or(0.0),
            terminated_at,
        }
    }

    /// The hat/role label to attribute the next iteration to.
    fn hat_label(&self) -> String {
        self.role_label
            .clone()
            .unwrap_or_else(|| "autoloop".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(json: &str) -> AutoloopEvent {
        serde_json::from_str(json).expect("valid AutoloopEvent json")
    }

    fn mapper() -> AutoloopRpcMapper {
        AutoloopRpcMapper::new(1_000, "autoloop")
    }

    #[test]
    fn iteration_start_maps_to_iteration_start_with_budget() {
        let mut m = mapper();
        let out = m.map(&ev(
            r#"{"type":"iteration.start","iteration":2,"maxIterations":7,"runId":"r1"}"#,
        ));
        assert_eq!(out.len(), 1);
        match &out[0] {
            RpcEvent::IterationStart {
                iteration,
                max_iterations,
                hat,
                hat_display,
                backend,
                ..
            } => {
                assert_eq!(*iteration, 2);
                assert_eq!(*max_iterations, Some(7));
                // No progress yet → default role label.
                assert_eq!(hat, "autoloop");
                assert_eq!(hat_display, "autoloop");
                assert_eq!(backend, "autoloop");
            }
            other => panic!("expected IterationStart, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_iteration_start_is_suppressed() {
        let mut m = mapper();
        let line = r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#;
        assert_eq!(m.map(&ev(line)).len(), 1);
        assert_eq!(m.map(&ev(line)).len(), 0, "same iteration number is deduped");
    }

    #[test]
    fn progress_emits_orchestration_event_and_labels_next_iteration() {
        let mut m = mapper();
        m.map(&ev(
            r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#,
        ));
        let out = m.map(&ev(
            r#"{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"tasks.ready","outcome":"continue:routed_event","allowedRoles":["planner"]}"#,
        ));
        assert_eq!(out.len(), 1);
        match &out[0] {
            RpcEvent::OrchestrationEvent {
                topic,
                payload,
                source,
                ..
            } => {
                assert_eq!(topic, "tasks.ready");
                assert_eq!(payload, "continue:routed_event");
                assert_eq!(source.as_deref(), Some("planner"));
            }
            other => panic!("expected OrchestrationEvent, got {other:?}"),
        }

        // The role from progress labels the NEXT iteration's hat.
        let out = m.map(&ev(
            r#"{"type":"iteration.start","iteration":2,"maxIterations":3,"runId":"r1"}"#,
        ));
        match &out[0] {
            RpcEvent::IterationStart { hat, .. } => assert_eq!(hat, "planner"),
            other => panic!("expected IterationStart, got {other:?}"),
        }
    }

    #[test]
    fn backend_output_maps_to_text_delta_for_current_iteration() {
        let mut m = mapper();
        m.map(&ev(r#"{"type":"iteration.start","iteration":4,"runId":"r1"}"#));
        let out = m.map(&ev(
            r#"{"type":"backend.output","output":"line one\nline two"}"#,
        ));
        assert_eq!(out.len(), 1);
        match &out[0] {
            RpcEvent::TextDelta { iteration, delta } => {
                assert_eq!(*iteration, 4);
                assert_eq!(delta, "line one\nline two");
            }
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn empty_backend_output_emits_nothing() {
        let mut m = mapper();
        m.map(&ev(r#"{"type":"iteration.start","iteration":1,"runId":"r1"}"#));
        assert!(m.map(&ev(r#"{"type":"backend.output","output":""}"#)).is_empty());
    }

    #[test]
    fn ask_pending_and_answered_map_to_orchestration_events() {
        let mut m = mapper();
        let out = m.map(&ev(
            r#"{"type":"ask.pending","runId":"r1","iteration":1,"questionId":"q1","question":"Proceed?"}"#,
        ));
        match &out[0] {
            RpcEvent::OrchestrationEvent {
                topic,
                payload,
                source,
                ..
            } => {
                assert_eq!(topic, "human.ask");
                assert_eq!(payload, "Proceed?");
                assert_eq!(source.as_deref(), Some("q1"));
            }
            other => panic!("expected OrchestrationEvent, got {other:?}"),
        }

        let out = m.map(&ev(
            r#"{"type":"ask.answered","runId":"r1","questionId":"q1","answer":"yes"}"#,
        ));
        match &out[0] {
            RpcEvent::OrchestrationEvent { topic, payload, .. } => {
                assert_eq!(topic, "human.answered");
                assert_eq!(payload, "yes");
            }
            other => panic!("expected OrchestrationEvent, got {other:?}"),
        }
    }

    #[test]
    fn loop_finish_maps_to_loop_terminated_with_cost() {
        let mut m = mapper();
        m.map(&ev(
            r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#,
        ));
        let out = m.map(&ev(
            r#"{"type":"loop.finish","iterations":2,"stopReason":"completed","runId":"r1","costUsd":0.08}"#,
        ));
        assert_eq!(out.len(), 1);
        match &out[0] {
            RpcEvent::LoopTerminated {
                reason,
                total_iterations,
                total_cost_usd,
                ..
            } => {
                assert_eq!(*reason, TerminationReason::Completed);
                assert_eq!(*total_iterations, 2);
                assert_eq!(*total_cost_usd, 0.08);
            }
            other => panic!("expected LoopTerminated, got {other:?}"),
        }
        // finalize() must not emit a second terminal event.
        assert!(m.finalize().is_none());
    }

    #[test]
    fn summary_then_loop_finish_emits_one_terminal_with_cost() {
        let mut m = mapper();
        m.map(&ev(r#"{"type":"iteration.start","iteration":1,"runId":"r1"}"#));
        // summary carries no cost and must NOT emit on its own.
        assert!(
            m.map(&ev(
                r#"{"type":"summary","runId":"r1","iterations":2,"stopReason":"max_iterations"}"#
            ))
            .is_empty()
        );
        // loop.finish carries the authoritative cost and emits the single terminal.
        let out = m.map(&ev(
            r#"{"type":"loop.finish","iterations":2,"stopReason":"max_iterations","runId":"r1","costUsd":0.12}"#,
        ));
        assert_eq!(out.len(), 1);
        match &out[0] {
            RpcEvent::LoopTerminated {
                reason,
                total_cost_usd,
                ..
            } => {
                assert_eq!(*reason, TerminationReason::MaxIterations);
                assert_eq!(*total_cost_usd, 0.12);
            }
            other => panic!("expected LoopTerminated, got {other:?}"),
        }
        assert!(m.finalize().is_none(), "no duplicate terminal");
    }

    #[test]
    fn summary_only_is_flushed_by_finalize() {
        let mut m = mapper();
        m.map(&ev(r#"{"type":"iteration.start","iteration":1,"runId":"r1"}"#));
        assert!(
            m.map(&ev(
                r#"{"type":"summary","runId":"r1","iterations":3,"stopReason":"stalled","costUsd":0.2}"#
            ))
            .is_empty()
        );
        let terminal = m.finalize().expect("summary-only run flushes on finalize");
        match terminal {
            RpcEvent::LoopTerminated {
                reason,
                total_iterations,
                total_cost_usd,
                ..
            } => {
                // Unknown-to-RPC reason collapses to Error.
                assert_eq!(reason, TerminationReason::Error);
                assert_eq!(total_iterations, 3);
                assert_eq!(total_cost_usd, 0.2);
            }
            other => panic!("expected LoopTerminated, got {other:?}"),
        }
        assert!(m.finalize().is_none(), "finalize is idempotent");
    }

    #[test]
    fn unknown_events_emit_nothing() {
        let mut m = mapper();
        assert!(m.map(&ev(r#"{"type":"backend.usage","runId":"r1"}"#)).is_empty());
        assert!(!m.saw_terminal());
    }
}
