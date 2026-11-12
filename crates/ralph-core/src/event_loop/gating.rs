//! Event gates in the event loop: iteration-start HEAD and blocked rewriting.
//!
//! Gate evaluation itself lives in [`crate::gates`]; this module decides when
//! it runs and what replaces an event that fails.

use super::EventLoop;
use crate::gates::{self, GateContext, StartHead};
use crate::text::truncate_with_ellipsis;
use ralph_proto::Event;
use tracing::{debug, info, warn};

/// Longest shortfall detail a blocked event or diagnostics entry carries.
/// Details are already bounded; this caps the sum.
const SHORTFALL_TEXT_MAX_CHARS: usize = 8 * 1024;

/// Gate outcome of the completion promises emitted this iteration, so the
/// text fallback neither re-gates nor re-reports an emitted completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompletionGate {
    /// No emitted completion promise was gated this iteration.
    NotGated,
    /// Every emitted completion promise passed its gate.
    Passed,
    /// An emitted completion promise was blocked. Sticky for the iteration:
    /// a later passing one does not clear it.
    Blocked,
}

impl EventLoop {
    /// Resets per-iteration gate state and, when a gate checks `new` commits,
    /// records HEAD. Call once per iteration, before the hat runs.
    pub(super) fn begin_gated_iteration(&mut self) {
        self.completion_gate = CompletionGate::NotGated;
        self.iteration_start_head = gates::needs_start_head(&self.config.gates)
            .then(|| gates::record_start_head(&self.config.core.workspace_root));
        if let Some(StartHead::Unknown(why)) = &self.iteration_start_head {
            warn!(
                error = %why,
                "Could not record HEAD at iteration start; `new` commit gates will fail closed"
            );
        }
    }

    /// Checks an event against the gate for its topic.
    ///
    /// Returns the blocked event that replaces it when the gate fails, and
    /// `None` when there is no gate or every requirement passes.
    pub(super) fn gate_event(&mut self, topic: &str, payload: &str) -> Option<Event> {
        let gate = self.config.gates.iter().find(|gate| gate.event == topic)?;

        // Events processed before any iteration recorded HEAD cannot prove a
        // commit is new.
        let not_recorded;
        let start_head = match &self.iteration_start_head {
            Some(head) => head,
            None => {
                not_recorded =
                    StartHead::Unknown("HEAD was not recorded at iteration start".to_string());
                &not_recorded
            }
        };
        let ctx = GateContext {
            workspace_root: &self.config.core.workspace_root,
            iteration_start_head: start_head,
            runner: self.gate_runner.as_ref(),
        };

        let is_completion = topic == self.config.event_loop.completion_promise;
        let shortfall = match gates::evaluate(gate, topic, payload, &ctx) {
            Ok(()) => {
                debug!(topic, "Event passed its gate");
                if is_completion && self.completion_gate == CompletionGate::NotGated {
                    self.completion_gate = CompletionGate::Passed;
                }
                return None;
            }
            Err(shortfall) => shortfall,
        };
        let blocked_topic = gate.blocked_topic();
        // The first line names the gate, requirement, and reason only. It is
        // stable across attempts, so build.blocked's per-task thrashing
        // detection (task id = first line) sees repeated failures as one task.
        let headline = format!(
            "Gate on `{topic}` failed: {}: {}",
            shortfall.requirement, shortfall.reason
        );
        let detail = truncate_with_ellipsis(&shortfall.detail, SHORTFALL_TEXT_MAX_CHARS);

        warn!(
            topic,
            blocked_topic = %blocked_topic,
            requirement = %shortfall.requirement,
            reason = %shortfall.reason,
            detail = %detail,
            "Gate failed: rewriting event to its blocked topic"
        );
        self.diagnostics.log_orchestration(
            self.state.iteration,
            "jsonl",
            crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                reason: format!("{headline}: {detail}"),
            },
        );
        if is_completion {
            self.completion_gate = CompletionGate::Blocked;
        }

        let mut blocked_payload = headline;
        if !detail.is_empty() {
            blocked_payload.push('\n');
            blocked_payload.push_str(&detail);
        }
        blocked_payload.push_str("\n\nOriginal payload:\n");
        blocked_payload.push_str(payload);
        Some(Event::new(blocked_topic, blocked_payload))
    }

    /// Gates a completion promise detected in output text.
    ///
    /// Returns `true` when completion may proceed. When a completion event
    /// emitted this iteration was already gated, its outcome is reused and
    /// nothing runs or is reported again. Otherwise the text carries no
    /// payload, so field requirements fail as missing; command requirements
    /// still run.
    pub(super) fn completion_text_passes_gate(&mut self) -> bool {
        let promise = self.config.event_loop.completion_promise.clone();
        if !self.config.gates.iter().any(|gate| gate.event == promise) {
            return true;
        }
        match self.completion_gate {
            CompletionGate::Blocked => {
                info!(
                    topic = %promise,
                    "Ignoring completion promise in output text: its gate already blocked the emitted event"
                );
                return false;
            }
            CompletionGate::Passed => {
                debug!(
                    topic = %promise,
                    "Completion promise in output text: the emitted event already passed its gate"
                );
                return true;
            }
            CompletionGate::NotGated => {}
        }
        match self.gate_event(&promise, "") {
            None => true,
            Some(blocked) => {
                self.state.record_event(&blocked);
                self.bus.publish(blocked);
                false
            }
        }
    }
}
