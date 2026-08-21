//! Loop termination reasons — engine-agnostic.
//!
//! Both the in-house event loop and the autoloop engine path produce a
//! [`TerminationReason`]; the coordination layer (summary, history, merge queue,
//! exit codes) consumes it. Defined outside the engine module so it survives the
//! in-house engine's deletion.

/// Reason the event loop terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationReason {
    /// Completion promise was detected in output.
    CompletionPromise,
    /// Maximum iterations reached.
    MaxIterations,
    /// Maximum runtime exceeded.
    MaxRuntime,
    /// Maximum cost exceeded.
    MaxCost,
    /// Too many consecutive failures.
    ConsecutiveFailures,
    /// Loop thrashing detected (repeated blocked events).
    LoopThrashing,
    /// Stale loop detected (same topic emitted 3+ times consecutively).
    LoopStale,
    /// Too many consecutive malformed JSONL lines in events file.
    ValidationFailure,
    /// The engine reported an unrecoverable error.
    EngineError { detail: Option<String> },
    /// Manually stopped.
    Stopped,
    /// Execution was suspended by the engine without completing or failing.
    Suspended,
    /// Completion was deliberately held by the engine.
    CompletionHeld,
    /// Interrupted by signal (SIGINT/SIGTERM).
    Interrupted,
    /// Restart requested via Telegram `/restart` command.
    RestartRequested,
    /// Workspace directory (worktree) was removed externally.
    WorkspaceGone,
    /// Loop was cancelled gracefully via loop.cancel event (human rejection, timeout).
    Cancelled,
}

impl TerminationReason {
    /// Returns the exit code for this termination reason per spec.
    ///
    /// Per spec "Loop Termination" section:
    /// - 0: Completion promise detected or manually stopped (clean exit)
    /// - 1: Consecutive failures or unrecoverable error (failure)
    /// - 2: Max iterations, max runtime, or max cost exceeded (limit)
    /// - 130: User interrupt (SIGINT = 128 + 2)
    pub fn exit_code(&self) -> i32 {
        match self {
            TerminationReason::CompletionPromise => 0,
            TerminationReason::ConsecutiveFailures
            | TerminationReason::LoopThrashing
            | TerminationReason::LoopStale
            | TerminationReason::ValidationFailure
            | TerminationReason::EngineError { .. }
            | TerminationReason::WorkspaceGone => 1,
            TerminationReason::Stopped
            | TerminationReason::Suspended
            | TerminationReason::CompletionHeld => 0,
            TerminationReason::MaxIterations
            | TerminationReason::MaxRuntime
            | TerminationReason::MaxCost => 2,
            TerminationReason::Interrupted => 130,
            // Restart uses exit code 3 to signal the caller to exec-replace
            TerminationReason::RestartRequested => 3,
            // Cancelled is a clean exit (0) — the loop stopped intentionally
            TerminationReason::Cancelled => 0,
        }
    }

    /// Returns the reason string for use in loop.terminate event payload.
    ///
    /// Per spec event payload format:
    /// `completed | max_iterations | max_runtime | consecutive_failures | interrupted | error`
    pub fn as_str(&self) -> &'static str {
        match self {
            TerminationReason::CompletionPromise => "completed",
            TerminationReason::MaxIterations => "max_iterations",
            TerminationReason::MaxRuntime => "max_runtime",
            TerminationReason::MaxCost => "max_cost",
            TerminationReason::ConsecutiveFailures => "consecutive_failures",
            TerminationReason::LoopThrashing => "loop_thrashing",
            TerminationReason::LoopStale => "loop_stale",
            TerminationReason::ValidationFailure => "validation_failure",
            TerminationReason::EngineError { .. } => "error",
            TerminationReason::Stopped => "stopped",
            TerminationReason::Suspended => "suspended",
            TerminationReason::CompletionHeld => "completion_held",
            TerminationReason::Interrupted => "interrupted",
            TerminationReason::RestartRequested => "restart_requested",
            TerminationReason::WorkspaceGone => "workspace_gone",
            TerminationReason::Cancelled => "cancelled",
        }
    }

    /// Returns true if this is a successful completion (not an error or limit).
    pub fn is_success(&self) -> bool {
        matches!(self, TerminationReason::CompletionPromise)
    }

    /// Snake-case label for loop history records.
    ///
    /// Differs from [`as_str`](Self::as_str) only for `CompletionPromise`
    /// (`"completion_promise"` vs `"completed"`) to preserve the existing
    /// JSONL history format.
    pub fn history_label(&self) -> &'static str {
        match self {
            TerminationReason::CompletionPromise => "completion_promise",
            other => other.as_str(),
        }
    }

    /// Short human-readable description for merge-queue review entries.
    pub fn review_description(&self) -> &'static str {
        match self {
            TerminationReason::CompletionPromise => "completed",
            TerminationReason::MaxIterations => "max iterations reached",
            TerminationReason::MaxRuntime => "max runtime exceeded",
            TerminationReason::MaxCost => "max cost exceeded",
            TerminationReason::ConsecutiveFailures => "consecutive failures",
            TerminationReason::LoopThrashing => "loop thrashing detected",
            TerminationReason::LoopStale => "stale loop detected",
            TerminationReason::ValidationFailure => "validation failure",
            TerminationReason::EngineError { .. } => "engine error",
            TerminationReason::Stopped => "manually stopped",
            TerminationReason::Suspended => "suspended by engine",
            TerminationReason::CompletionHeld => "completion held by engine",
            TerminationReason::Interrupted => "interrupted by signal",
            TerminationReason::RestartRequested => "restart requested",
            TerminationReason::WorkspaceGone => "workspace directory removed",
            TerminationReason::Cancelled => "cancelled by human",
        }
    }
}

impl std::fmt::Display for TerminationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.review_description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_review_description() {
        let reason = TerminationReason::MaxIterations;
        assert_eq!(format!("{reason}"), reason.review_description());
    }

    #[test]
    fn display_completion_promise() {
        assert_eq!(
            format!("{}", TerminationReason::CompletionPromise),
            "completed"
        );
    }

    #[test]
    fn manually_stopped_is_a_clean_exit() {
        assert_eq!(TerminationReason::Stopped.exit_code(), 0);
    }

    #[test]
    fn intentional_non_completion_states_exit_cleanly_but_are_not_success() {
        for reason in [
            TerminationReason::Suspended,
            TerminationReason::CompletionHeld,
        ] {
            assert_eq!(reason.exit_code(), 0);
            assert!(!reason.is_success());
        }
        assert_eq!(TerminationReason::Suspended.as_str(), "suspended");
        assert_eq!(
            TerminationReason::CompletionHeld.as_str(),
            "completion_held"
        );
    }

    #[test]
    fn engine_error_has_failure_contract() {
        let reason = TerminationReason::EngineError {
            detail: Some("boom".to_string()),
        };

        assert_eq!(reason.exit_code(), 1);
        assert_eq!(reason.as_str(), "error");
        assert_eq!(reason.history_label(), "error");
        assert_eq!(reason.review_description(), "engine error");
        assert!(!reason.is_success());
    }
}
