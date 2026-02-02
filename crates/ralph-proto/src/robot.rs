//! Human-in-the-loop robot service abstractions.
//!
//! Defines the [`RobotService`] trait that communication backends (Telegram,
//! Slack, etc.) implement to provide human-in-the-loop interaction during
//! orchestration loops. The core event loop uses this trait to send questions,
//! receive responses, and send periodic check-ins — without knowing which
//! communication platform is being used.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Additional context for enhanced check-in messages.
///
/// Provides richer information than the basic iteration + elapsed time,
/// including current hat, task progress, and cost tracking.
#[derive(Debug, Default)]
pub struct CheckinContext {
    /// The currently active hat name (e.g., "executor", "reviewer").
    pub current_hat: Option<String>,
    /// Number of open (non-terminal) tasks.
    pub open_tasks: usize,
    /// Number of closed tasks.
    pub closed_tasks: usize,
    /// Cumulative cost in USD.
    pub cumulative_cost: f64,
}

/// A communication service for human-in-the-loop interaction.
///
/// Implementors handle platform-specific concerns: sending messages,
/// waiting for responses, and periodic check-ins. The event loop holds
/// an `Option<Box<dyn RobotService>>` and calls these methods when
/// `human.interact` events are detected.
pub trait RobotService: Send + Sync {
    /// Send a question to the human and store it as pending.
    ///
    /// Returns the platform-specific message ID on success, or 0 if
    /// no recipient is configured (question is logged but not sent).
    fn send_question(&self, payload: &str) -> anyhow::Result<i32>;

    /// Poll the events file for a `human.response` event.
    ///
    /// Blocks until a response arrives or the configured timeout expires.
    /// Returns `Ok(Some(response))` on response, `Ok(None)` on timeout.
    fn wait_for_response(&self, events_path: &Path) -> anyhow::Result<Option<String>>;

    /// Send a periodic check-in message.
    ///
    /// Returns `Ok(0)` if no recipient is configured (skipped silently),
    /// or the message ID on success.
    fn send_checkin(
        &self,
        iteration: u32,
        elapsed: Duration,
        context: Option<&CheckinContext>,
    ) -> anyhow::Result<i32>;

    /// Get the configured response timeout in seconds.
    fn timeout_secs(&self) -> u64;

    /// Get a clone of the shutdown flag for cooperative interruption.
    ///
    /// Signal handlers can set this flag to interrupt `wait_for_response()`
    /// without waiting for the full timeout.
    fn shutdown_flag(&self) -> Arc<AtomicBool>;

    /// Stop the service gracefully.
    ///
    /// Called during loop termination to cleanly shut down the backend.
    fn stop(self: Box<Self>);
}
