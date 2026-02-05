//! Telegram daemon adapter.
//!
//! Implements [`DaemonAdapter`] for Telegram, providing a persistent process
//! that listens for messages and starts orchestration loops on demand.
//!
//! Uses a **turn-taking model**: the daemon polls Telegram while idle, but
//! stops polling when a loop starts — the loop's own [`TelegramService`]
//! takes over for the full Telegram feature set (commands, guidance,
//! responses, check-ins). When the loop finishes, the daemon resumes.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tracing::{error, info, warn};

use hats_proto::daemon::{DaemonAdapter, StartLoopFn};

use crate::bot::{BotApi, TelegramBot, escape_html};
use crate::loop_lock::{LockState, lock_path, lock_state};
use crate::state::StateManager;

async fn wait_for_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

/// A Telegram-based daemon adapter.
///
/// Polls Telegram for messages while idle and delegates loop execution
/// to the provided [`StartLoopFn`] callback. Supports `/status` commands
/// and graceful shutdown via `SIGINT`/`SIGTERM`.
pub struct TelegramDaemon {
    bot_token: String,
    chat_id: i64,
}

impl TelegramDaemon {
    /// Create a new Telegram daemon.
    ///
    /// `bot_token` — Telegram Bot API token.
    /// `chat_id` — The Telegram chat to communicate with.
    pub fn new(bot_token: String, chat_id: i64) -> Self {
        Self { bot_token, chat_id }
    }
}

#[async_trait]
impl DaemonAdapter for TelegramDaemon {
    async fn run_daemon(
        &self,
        workspace_root: PathBuf,
        start_loop: StartLoopFn,
    ) -> anyhow::Result<()> {
        let bot = TelegramBot::new(&self.bot_token);
        let chat_id = self.chat_id;

        let state_manager = StateManager::new(workspace_root.join(".hats/telegram-state.json"));

        // Send greeting
        let _ = bot.send_message(chat_id, "Hats daemon online 🤖").await;

        // Install signal handlers for graceful shutdown
        let shutdown = Arc::new(AtomicBool::new(false));
        {
            let flag = shutdown.clone();
            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                flag.store(true, Ordering::Relaxed);
            });
        }
        #[cfg(unix)]
        {
            let flag = shutdown.clone();
            tokio::spawn(async move {
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(mut sigterm) => {
                        sigterm.recv().await;
                        flag.store(true, Ordering::Relaxed);
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to register SIGTERM handler");
                        flag.store(true, Ordering::Relaxed);
                    }
                }
            });
        }

        let mut offset: i32 = 0;

        // Main daemon loop
        'daemon: while !shutdown.load(Ordering::Relaxed) {
            // ── Idle: poll Telegram for messages ──
            let updates = match tokio::select! {
                _ = wait_for_shutdown(shutdown.clone()) => {
                    break 'daemon;
                }
                updates = poll_updates(&self.bot_token, 30, offset) => updates,
            } {
                Ok(u) => u,
                Err(e) => {
                    warn!(error = %e, "Telegram poll failed, retrying");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            for update in updates {
                offset = update.update_id + 1;

                let text = match update.text.as_deref() {
                    Some(t) => t,
                    None => continue,
                };

                if let Ok(mut state) = state_manager.load_or_default() {
                    if state.chat_id.is_none() {
                        state.chat_id = Some(chat_id);
                    }
                    state.last_seen = Some(chrono::Utc::now());
                    state.last_update_id = Some(update.update_id);
                    if let Err(e) = state_manager.save(&state) {
                        warn!(error = %e, "Failed to persist Telegram state");
                    }
                } else {
                    warn!("Failed to load Telegram state");
                }

                info!(text = %text, "Daemon received message");

                // Handle daemon-only commands
                if text.starts_with('/') {
                    match text.split_whitespace().next().unwrap_or("") {
                        "/status" => {
                            let msg = match lock_state(&workspace_root) {
                                Ok(LockState::Active) => "A loop is running.".to_string(),
                                Ok(LockState::Stale) => {
                                    "No active loop (stale lock file found).".to_string()
                                }
                                Ok(LockState::Inactive) => {
                                    "Idle — waiting for messages.".to_string()
                                }
                                Err(e) => format!("Failed to check lock state: {}", e),
                            };
                            let _ = bot.send_message(chat_id, &msg).await;
                        }
                        _ => {
                            let _ = bot
                                .send_message(
                                    chat_id,
                                    "Unknown command. I only handle /status while idle.",
                                )
                                .await;
                        }
                    }
                    continue;
                }

                // Regular message → check lock state
                let lock_path = lock_path(&workspace_root);
                let state = match lock_state(&workspace_root) {
                    Ok(state) => state,
                    Err(e) => {
                        warn!(error = %e, "Failed to check loop lock state");
                        let _ = bot
                            .send_message(
                                chat_id,
                                "Failed to check loop state; try again in a moment.",
                            )
                            .await;
                        continue;
                    }
                };
                if state == LockState::Active {
                    let _ = bot
                        .send_message(
                            chat_id,
                            "A loop is already running — it will receive your messages directly.",
                        )
                        .await;
                    continue;
                }

                if state == LockState::Stale {
                    warn!(
                        lock_path = %lock_path.display(),
                        "Found stale loop lock; starting new loop"
                    );
                }

                // No loop running — start one with this message as prompt
                let escaped = escape_html(text);
                let ack = format!("Starting loop: <i>{}</i>", escaped);
                let _ = bot.send_message(chat_id, &ack).await;

                // ── Loop Running: hand off Telegram to the loop ──
                // The loop's TelegramService polls getUpdates, handles commands,
                // guidance, responses, check-ins. We just await completion.
                let prompt = text.to_string();
                let mut loop_handle = tokio::spawn(start_loop(prompt));
                let result = tokio::select! {
                    _ = wait_for_shutdown(shutdown.clone()) => {
                        loop_handle.abort();
                        let _ = loop_handle.await;
                        break 'daemon;
                    }
                    result = &mut loop_handle => result,
                };

                // Loop finished — daemon resumes polling.
                match result {
                    Ok(Ok(description)) => {
                        let notification =
                            format!("Loop complete ({}).", escape_html(&description));
                        let _ = bot.send_message(chat_id, &notification).await;
                    }
                    Ok(Err(e)) => {
                        let notification = format!("Loop failed: {}", escape_html(&e.to_string()));
                        let _ = bot.send_message(chat_id, &notification).await;
                    }
                    Err(e) => {
                        let notification = format!("Loop failed: {}", escape_html(&e.to_string()));
                        let _ = bot.send_message(chat_id, &notification).await;
                    }
                }
            }
        }

        // Farewell
        let _ = bot.send_message(chat_id, "Hats daemon offline 👋").await;

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Lightweight Telegram polling (teloxide Bot client)
// ─────────────────────────────────────────────────────────────────────────────

/// A minimal parsed update for daemon idle polling.
struct DaemonUpdate {
    update_id: i32,
    text: Option<String>,
}

/// Long-poll `getUpdates` using the teloxide Bot client.
///
/// Uses teloxide's built-in HTTP client rather than raw `reqwest`
/// since `hats-telegram` already depends on teloxide.
async fn poll_updates(
    token: &str,
    timeout_secs: u64,
    offset: i32,
) -> anyhow::Result<Vec<DaemonUpdate>> {
    use teloxide::payloads::GetUpdatesSetters;
    use teloxide::requests::Requester;

    let bot = teloxide::Bot::new(token);
    let request = bot
        .get_updates()
        .offset(offset)
        .timeout(timeout_secs as u32);

    let updates = request
        .await
        .map_err(|e| anyhow::anyhow!("Telegram getUpdates failed: {}", e))?;

    let mut results = Vec::new();
    for update in updates {
        #[allow(clippy::cast_possible_wrap)]
        let id = update.id.0 as i32;

        let text = match update.kind {
            teloxide::types::UpdateKind::Message(ref msg) => msg.text().map(String::from),
            _ => None,
        };

        results.push(DaemonUpdate {
            update_id: id,
            text,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_daemon_creation() {
        let daemon = TelegramDaemon::new("test-token".to_string(), 12345);
        assert_eq!(daemon.bot_token, "test-token");
        assert_eq!(daemon.chat_id, 12345);
    }
}
