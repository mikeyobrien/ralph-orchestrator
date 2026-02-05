//! PTY handle abstraction for TUI integration.
//!
//! Provides a channel-based interface for bidirectional communication with a PTY.
//! The TUI can send input and control commands while receiving output asynchronously.

use tokio::sync::{mpsc, watch};

/// Handle for communicating with a PTY process.
pub struct PtyHandle {
    /// Receives output from the PTY.
    pub output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    /// Sends input to the PTY.
    pub input_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Sends control commands to the PTY.
    pub control_tx: mpsc::UnboundedSender<ControlCommand>,
    /// Signals when the PTY process has terminated.
    /// TUI should exit when this becomes `true`.
    pub terminated_rx: watch::Receiver<bool>,
}

/// Control commands for PTY management.
#[derive(Debug, Clone)]
pub enum ControlCommand {
    /// Resize the PTY to the given (cols, rows).
    Resize(u16, u16),
    /// Terminate the PTY process.
    Kill,
    /// Skip current iteration.
    Skip,
    /// Abort the loop.
    Abort,
}
