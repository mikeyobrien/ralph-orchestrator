//! Daemon mode abstractions.
//!
//! Defines the [`DaemonAdapter`] trait that communication adapters (Telegram,
//! Slack, etc.) implement to support `ralph bot daemon`. The CLI layer creates
//! the adapter and passes a [`StartLoopFn`] callback — the adapter calls it
//! when a user sends a message that should start an orchestration loop.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use async_trait::async_trait;

/// Callback the adapter calls to start an orchestration loop.
///
/// Accepts a prompt string, returns `Ok(description)` on success (e.g.,
/// `"CompletionPromise"`) or `Err` on failure. The adapter doesn't need
/// to know about `TerminationReason` — it just reports the result.
pub type StartLoopFn = Box<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>> + Send + Sync,
>;

/// A communication adapter that can run in daemon mode.
///
/// The daemon is a persistent process that listens for messages on a
/// communication platform and starts orchestration loops on demand.
///
/// Implementors handle all platform-specific concerns: authentication,
/// message polling, greeting/farewell, and idle-mode commands. When a
/// loop is running, the adapter hands off interaction to the loop's own
/// communication service (e.g., `TelegramService`) and simply awaits
/// completion.
#[async_trait]
pub trait DaemonAdapter: Send + Sync {
    /// Run the daemon loop. Blocks until shutdown (Ctrl+C / SIGTERM).
    ///
    /// - `workspace_root` — the project directory where `ralph.yml` lives.
    /// - `start_loop` — callback to start an orchestration loop with a prompt.
    async fn run_daemon(
        &self,
        workspace_root: PathBuf,
        start_loop: StartLoopFn,
    ) -> anyhow::Result<()>;
}
