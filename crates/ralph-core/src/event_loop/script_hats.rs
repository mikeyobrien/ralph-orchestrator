//! Script hats in the event loop: starting a script iteration without a
//! prompt, and keeping model iterations off a script hat's triggers.
//!
//! Running the command is the runner's job. Accounting lives in `LoopState`:
//! a script iteration gets an iteration number and counts toward
//! `max_script_runs`, max_activations, and stale-loop detection, but not
//! `max_iterations`. Its events are always scope-enforced.

use super::EventLoop;
use crate::config::HatConfig;
use ralph_proto::{Event, HatId};
use std::collections::HashSet;
use std::fmt;
use tracing::{debug, warn};

/// How much of a failed script's stderr goes into its `<hat>.failed` event.
pub const SCRIPT_STDERR_TAIL_BYTES: usize = 4 * 1024;

/// Why a script hat run did not produce a normal iteration. Its `Display`
/// is the first line of the `<hat>.failed` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptFailure {
    /// The command exited non-zero, or was killed (`code: None`).
    Exited { code: Option<i32>, stderr: String },
    /// The command ran past the hat's timeout and was killed.
    TimedOut { seconds: u64, stderr: String },
    /// The command could not be started (not found, not executable, ...).
    CouldNotRun { error: String },
    /// The command exited 0 without emitting, and the hat has no
    /// `default_publishes`.
    NoEvents,
}

impl ScriptFailure {
    /// Whether the run counts toward `max_consecutive_failures`. A command
    /// that exits 0 without emitting succeeded as a process.
    pub fn counts_as_failure(&self) -> bool {
        !matches!(self, Self::NoEvents)
    }

    fn stderr(&self) -> &str {
        match self {
            Self::Exited { stderr, .. } | Self::TimedOut { stderr, .. } => stderr,
            Self::CouldNotRun { .. } | Self::NoEvents => "",
        }
    }
}

impl fmt::Display for ScriptFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exited {
                code: Some(code), ..
            } => write!(f, "command exited {code}"),
            Self::Exited { code: None, .. } => write!(f, "command was killed by a signal"),
            Self::TimedOut { seconds, .. } => write!(f, "command timed out after {seconds}s"),
            Self::CouldNotRun { error } => write!(f, "could not run command: {error}"),
            Self::NoEvents => write!(f, "command exited 0 without emitting"),
        }
    }
}

/// The last `max_bytes` of `s`, starting on a char boundary.
fn tail_on_char_boundary(s: &str, max_bytes: usize) -> &str {
    let mut start = s.len().saturating_sub(max_bytes);
    while !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

impl EventLoop {
    /// True when `hat_id` is a configured hat with `run`.
    pub fn is_script_hat(&self, hat_id: &HatId) -> bool {
        self.registry
            .get_config(hat_id)
            .is_some_and(HatConfig::is_script)
    }

    /// Starts a script iteration for `hat_id`: consumes its pending trigger
    /// events and returns them, in arrival order, for the command.
    ///
    /// This is the script-hat counterpart of `build_prompt`: it records the
    /// hat's activation, makes it the active hat for scope enforcement and
    /// `default_publishes`, and starts the gated iteration. Follow it with
    /// `process_output` and the usual event processing.
    ///
    /// Returns `None`, and starts nothing, when `hat_id` is not a script hat,
    /// has nothing pending, or has hit `max_activations` (its triggers are
    /// then dropped and `<hat>.exhausted` is published once, as for any hat).
    pub fn begin_script_iteration(&mut self, hat_id: &HatId) -> Option<Vec<Event>> {
        if !self.is_script_hat(hat_id)
            || self
                .bus
                .peek_pending(hat_id)
                .is_none_or(|pending| pending.is_empty())
        {
            return None;
        }

        let pending = self.bus.take_pending(hat_id);
        let (drop_pending, exhausted_event) = self.check_hat_exhaustion(hat_id, &pending);
        if drop_pending {
            if let Some(event) = exhausted_event {
                self.bus.publish(event);
            }
            return None;
        }

        self.begin_gated_iteration();
        self.record_hat_activations(std::slice::from_ref(hat_id));
        self.state.last_active_hat_ids = vec![hat_id.clone()];
        self.state.script_iteration = true;
        debug!(
            hat = %hat_id.as_str(),
            triggers = pending.len(),
            "Starting script iteration"
        );
        Some(pending)
    }

    /// Publishes `<hat>.failed` for a script run that failed. The payload is
    /// the failure, then how many emitted events were discarded (if any),
    /// then the last 4 KiB of stderr (if any).
    ///
    /// The event comes from Ralph, not the script, so it is not subject to
    /// the hat's `publishes` scope or to gates. It routes like any event: to
    /// a hat that triggers on it, or to Ralph.
    pub fn publish_script_failure(
        &mut self,
        hat_id: &HatId,
        failure: &ScriptFailure,
        discarded_events: usize,
    ) {
        let mut payload = failure.to_string();
        if discarded_events > 0 {
            payload.push_str(&format!("\n{discarded_events} emitted event(s) discarded"));
        }
        let stderr = tail_on_char_boundary(failure.stderr().trim_end(), SCRIPT_STDERR_TAIL_BYTES);
        if !stderr.is_empty() {
            payload.push_str("\nstderr (last 4 KiB):\n");
            payload.push_str(stderr);
        }

        let topic = format!("{}.failed", hat_id.as_str());
        warn!(hat = %hat_id.as_str(), %failure, "Script hat failed; publishing {topic}");
        let event = Event::new(topic, payload).with_source(hat_id.clone());
        self.state.record_event(&event);
        self.bus.publish(event);
    }

    /// Reads and drops every unread event in the events file, returning how
    /// many agent events were dropped (Ralph's own `iteration.summary` rows
    /// are not counted). Used for a script that failed after emitting: its
    /// partial output is not trusted, so only `<hat>.failed` routes.
    pub fn discard_unread_events(&mut self) -> std::io::Result<usize> {
        let result = self.event_reader.read_new_events()?;
        let dropped = result
            .events
            .iter()
            .filter(|event| event.topic.as_str() != "iteration.summary")
            .count()
            + result.malformed.len();
        if dropped > 0 {
            warn!(dropped, "Discarding events emitted by a failed script hat");
        }
        Ok(dropped)
    }

    /// Script hats that pending events would activate. A model iteration
    /// leaves their triggers on the bus, so a prompt never swallows work a
    /// script hat should run.
    pub(super) fn active_script_hat_ids(&self) -> HashSet<HatId> {
        self.determine_active_hat_ids(&self.peek_pending_regular_events())
            .into_iter()
            .filter(|hat_id| self.is_script_hat(hat_id))
            .collect()
    }
}
