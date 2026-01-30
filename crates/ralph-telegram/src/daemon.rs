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
use tracing::{info, warn};

use ralph_proto::daemon::{DaemonAdapter, StartLoopFn};

use crate::bot::{BotApi, TelegramBot, escape_html};

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

        // Send greeting
        let _ = bot.send_message(chat_id, "Ralph daemon online 🤖").await;

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
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("Failed to register SIGTERM handler");
                sigterm.recv().await;
                flag.store(true, Ordering::Relaxed);
            });
        }

        let mut offset: i32 = 0;

        // Main daemon loop
        while !shutdown.load(Ordering::Relaxed) {
            // ── Idle: poll Telegram for messages ──
            let updates = match poll_updates(&self.bot_token, 30, offset).await {
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

                info!(text = %text, "Daemon received message");

                // Handle daemon-only commands
                if text.starts_with('/') {
                    match text.split_whitespace().next().unwrap_or("") {
                        "/status" => {
                            let lock_path = workspace_root.join(".ralph/loop.lock");
                            let msg = if lock_path.exists() {
                                "A loop is running."
                            } else {
                                "Idle — waiting for messages."
                            };
                            let _ = bot.send_message(chat_id, msg).await;
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
                let lock_path = workspace_root.join(".ralph/loop.lock");
                if lock_path.exists() {
                    let _ = bot
                        .send_message(
                            chat_id,
                            "A loop is already running — it will receive your messages directly.",
                        )
                        .await;
                    continue;
                }

                // No loop running — start one with this message as prompt
                let escaped = escape_html(text);
                let ack = format!("Starting loop: <i>{}</i>", escaped);
                let _ = bot.send_message(chat_id, &ack).await;

                // ── Loop Running: hand off Telegram to the loop ──
                // The loop's TelegramService polls getUpdates, handles commands,
                // guidance, responses, check-ins. We just await completion.
                let prompt = text.to_string();
                let result = start_loop(prompt).await;

                // Loop finished — daemon resumes polling.
                let notification = match result {
                    Ok(description) => {
                        format!("Loop complete ({}).", escape_html(&description))
                    }
                    Err(e) => format!("Loop failed: {}", escape_html(&e.to_string())),
                };
                let _ = bot.send_message(chat_id, &notification).await;
            }
        }

        // Farewell
        let _ = bot.send_message(chat_id, "Ralph daemon offline 👋").await;

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
/// since `ralph-telegram` already depends on teloxide.
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
