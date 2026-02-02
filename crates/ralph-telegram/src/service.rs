use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::bot::TelegramBot;
use crate::error::{TelegramError, TelegramResult};
use crate::handler::MessageHandler;
use crate::state::StateManager;

/// Maximum number of retry attempts for sending messages.
pub const MAX_SEND_RETRIES: u32 = 3;

/// Base delay for exponential backoff (1 second).
pub const BASE_RETRY_DELAY: Duration = Duration::from_secs(1);

/// Execute a fallible send operation with exponential backoff retry.
///
/// Retries up to [`MAX_SEND_RETRIES`] times with delays of 1s, 2s, 4s.
/// Returns the result on success, or `TelegramError::Send` after all
/// retries are exhausted.
///
/// The `sleep_fn` parameter allows tests to substitute a no-op sleep.
pub fn retry_with_backoff<F, S>(mut send_fn: F, mut sleep_fn: S) -> TelegramResult<i32>
where
    F: FnMut(u32) -> TelegramResult<i32>,
    S: FnMut(Duration),
{
    let mut last_error = String::new();

    for attempt in 1..=MAX_SEND_RETRIES {
        match send_fn(attempt) {
            Ok(msg_id) => return Ok(msg_id),
            Err(e) => {
                last_error = e.to_string();
                warn!(
                    attempt = attempt,
                    max_retries = MAX_SEND_RETRIES,
                    error = %last_error,
                    "Telegram send failed, {}",
                    if attempt < MAX_SEND_RETRIES {
                        "retrying with backoff"
                    } else {
                        "all retries exhausted"
                    }
                );
                if attempt < MAX_SEND_RETRIES {
                    let delay = BASE_RETRY_DELAY * 2u32.pow(attempt - 1);
                    sleep_fn(delay);
                }
            }
        }
    }

    Err(TelegramError::Send {
        attempts: MAX_SEND_RETRIES,
        reason: last_error,
    })
}

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

/// Coordinates the Telegram bot lifecycle with the Ralph event loop.
///
/// Manages startup, shutdown, message sending, and response waiting.
/// Uses the host tokio runtime (from `#[tokio::main]`) for async operations.
pub struct TelegramService {
    workspace_root: PathBuf,
    bot_token: String,
    timeout_secs: u64,
    loop_id: String,
    state_manager: StateManager,
    handler: MessageHandler,
    bot: TelegramBot,
    shutdown: Arc<AtomicBool>,
}

impl TelegramService {
    /// Create a new TelegramService.
    ///
    /// Resolves the bot token from config or `RALPH_TELEGRAM_BOT_TOKEN` env var.
    pub fn new(
        workspace_root: PathBuf,
        bot_token: Option<String>,
        timeout_secs: u64,
        loop_id: String,
    ) -> TelegramResult<Self> {
        let resolved_token = bot_token
            .or_else(|| std::env::var("RALPH_TELEGRAM_BOT_TOKEN").ok())
            .ok_or(TelegramError::MissingBotToken)?;

        let state_path = workspace_root.join(".ralph/telegram-state.json");
        let state_manager = StateManager::new(&state_path);
        let handler_state_manager = StateManager::new(&state_path);
        let handler = MessageHandler::new(handler_state_manager, &workspace_root);
        let bot = TelegramBot::new(&resolved_token);
        let shutdown = Arc::new(AtomicBool::new(false));

        Ok(Self {
            workspace_root,
            bot_token: resolved_token,
            timeout_secs,
            loop_id,
            state_manager,
            handler,
            bot,
            shutdown,
        })
    }

    /// Get a reference to the workspace root.
    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    /// Get the configured timeout in seconds.
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    /// Get a reference to the bot token (masked for logging).
    pub fn bot_token_masked(&self) -> String {
        if self.bot_token.len() > 8 {
            format!(
                "{}...{}",
                &self.bot_token[..4],
                &self.bot_token[self.bot_token.len() - 4..]
            )
        } else {
            "****".to_string()
        }
    }

    /// Get a reference to the state manager.
    pub fn state_manager(&self) -> &StateManager {
        &self.state_manager
    }

    /// Get a mutable reference to the message handler.
    pub fn handler(&mut self) -> &mut MessageHandler {
        &mut self.handler
    }

    /// Get the loop ID this service is associated with.
    pub fn loop_id(&self) -> &str {
        &self.loop_id
    }

    /// Returns a clone of the shutdown flag.
    ///
    /// Signal handlers can set this flag to interrupt `wait_for_response()`
    /// without waiting for the full timeout.
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    /// Start the Telegram service.
    ///
    /// Spawns a background polling task on the host tokio runtime to receive
    /// incoming messages. Must be called from within a tokio runtime context.
    pub fn start(&self) -> TelegramResult<()> {
        info!(
            bot_token = %self.bot_token_masked(),
            workspace = %self.workspace_root.display(),
            timeout_secs = self.timeout_secs,
            "Telegram service starting"
        );

        // Spawn the polling task on the host tokio runtime
        let handle = tokio::runtime::Handle::try_current().map_err(|_| {
            TelegramError::Startup("no tokio runtime available for polling".to_string())
        })?;

        let raw_bot = teloxide::Bot::new(&self.bot_token);
        let workspace_root = self.workspace_root.clone();
        let state_path = self.workspace_root.join(".ralph/telegram-state.json");
        let shutdown = self.shutdown.clone();
        let loop_id = self.loop_id.clone();

        handle.spawn(async move {
            Self::poll_updates(raw_bot, workspace_root, state_path, shutdown, loop_id).await;
        });

        // Send greeting if we already know the chat ID
        if let Ok(state) = self.state_manager.load_or_default()
            && let Some(chat_id) = state.chat_id
        {
            let greeting = crate::bot::TelegramBot::format_greeting(&self.loop_id);
            match self.send_with_retry(chat_id, &greeting) {
                Ok(_) => info!("Sent greeting to chat {}", chat_id),
                Err(e) => warn!(error = %e, "Failed to send greeting"),
            }
        }

        info!("Telegram service started — polling for incoming messages");
        Ok(())
    }

    /// Background polling task that receives incoming Telegram messages.
    ///
    /// Uses long polling (`getUpdates`) to receive messages, then routes them
    /// through `MessageHandler` to write events to the correct loop's JSONL.
    async fn poll_updates(
        bot: teloxide::Bot,
        workspace_root: PathBuf,
        state_path: PathBuf,
        shutdown: Arc<AtomicBool>,
        loop_id: String,
    ) {
        use teloxide::payloads::{GetUpdatesSetters, SetMessageReactionSetters};
        use teloxide::requests::Requester;

        let state_manager = StateManager::new(&state_path);
        let handler_state_manager = StateManager::new(&state_path);
        let handler = MessageHandler::new(handler_state_manager, &workspace_root);
        let mut offset: i32 = 0;

        if let Ok(state) = state_manager.load_or_default()
            && let Some(last_update_id) = state.last_update_id
        {
            offset = last_update_id + 1;
        }

        // Register bot commands with Telegram API
        Self::register_commands(&bot).await;

        info!(loop_id = %loop_id, "Telegram polling task started");

        while !shutdown.load(Ordering::Relaxed) {
            let request = bot.get_updates().offset(offset).timeout(10);
            match request.await {
                Ok(updates) => {
                    for update in updates {
                        // Next offset = current update ID + 1
                        #[allow(clippy::cast_possible_wrap)]
                        {
                            offset = update.id.0 as i32 + 1;
                        }

                        // Extract message from update kind
                        let msg = match update.kind {
                            teloxide::types::UpdateKind::Message(msg) => msg,
                            _ => continue,
                        };

                        let text = match msg.text() {
                            Some(t) => t,
                            None => continue,
                        };

                        let chat_id = msg.chat.id.0;
                        let reply_to: Option<i32> = msg.reply_to_message().map(|r| r.id.0);

                        info!(
                            chat_id = chat_id,
                            text = %text,
                            "Received Telegram message"
                        );

                        // Handle bot commands before routing to handler
                        if crate::commands::is_command(text)
                            && let Some(response) =
                                crate::commands::handle_command(text, &workspace_root)
                        {
                            use teloxide::payloads::SendMessageSetters;
                            let send_result = bot
                                .send_message(teloxide::types::ChatId(chat_id), &response)
                                .parse_mode(teloxide::types::ParseMode::Html)
                                .await;
                            if let Err(e) = send_result {
                                warn!(error = %e, "Failed to send command response");
                            }
                            continue;
                        }

                        let mut state = match state_manager.load_or_default() {
                            Ok(s) => s,
                            Err(e) => {
                                warn!(error = %e, "Failed to load Telegram state");
                                continue;
                            }
                        };

                        match handler.handle_message(&mut state, text, chat_id, reply_to) {
                            Ok(topic) => {
                                let emoji = if topic == "human.response" {
                                    "👍"
                                } else {
                                    "👀"
                                };
                                let react_result = bot
                                    .set_message_reaction(teloxide::types::ChatId(chat_id), msg.id)
                                    .reaction(vec![teloxide::types::ReactionType::Emoji {
                                        emoji: emoji.to_string(),
                                    }])
                                    .await;
                                if let Err(e) = react_result {
                                    warn!(error = %e, "Failed to react to message");
                                }

                                // For guidance, also send a short text reply
                                if topic == "human.guidance" {
                                    let _ = bot
                                        .send_message(
                                            teloxide::types::ChatId(chat_id),
                                            "📝 <b>Guidance received</b> — will apply next iteration.",
                                        )
                                        .await;
                                }
                            }
                            Err(e) => {
                                warn!(
                                    error = %e,
                                    text = %text,
                                    "Failed to handle incoming Telegram message"
                                );
                            }
                        }

                        state.last_seen = Some(Utc::now());
                        state.last_update_id = Some(offset.saturating_sub(1));
                        if let Err(e) = state_manager.save(&state) {
                            warn!(error = %e, "Failed to persist Telegram state");
                        }
                    }
                }
                Err(e) => {
                    if !shutdown.load(Ordering::Relaxed) {
                        warn!(error = %e, "Telegram polling error — retrying in 5s");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }

        info!(loop_id = %loop_id, "Telegram polling task stopped");
    }

    /// Register bot commands with the Telegram API so they appear in the menu.
    async fn register_commands(bot: &teloxide::Bot) {
        use teloxide::requests::Requester;
        use teloxide::types::BotCommand;

        let commands = vec![
            BotCommand::new("status", "Current loop status"),
            BotCommand::new("tasks", "Open tasks"),
            BotCommand::new("memories", "Recent memories"),
            BotCommand::new("tail", "Last 20 events"),
            BotCommand::new("stop", "Stop the loop"),
            BotCommand::new("help", "List available commands"),
        ];

        match bot.set_my_commands(commands).await {
            Ok(_) => info!("Registered bot commands with Telegram API"),
            Err(e) => warn!(error = %e, "Failed to register bot commands"),
        }
    }

    /// Stop the Telegram service gracefully.
    ///
    /// Signals the background polling task to shut down.
    pub fn stop(self) {
        // Send farewell if we know the chat ID
        if let Ok(state) = self.state_manager.load_or_default()
            && let Some(chat_id) = state.chat_id
        {
            let farewell = crate::bot::TelegramBot::format_farewell(&self.loop_id);
            match self.send_with_retry(chat_id, &farewell) {
                Ok(_) => info!("Sent farewell to chat {}", chat_id),
                Err(e) => warn!(error = %e, "Failed to send farewell"),
            }
        }

        self.shutdown.store(true, Ordering::Relaxed);
        info!(
            workspace = %self.workspace_root.display(),
            "Telegram service stopped"
        );
    }

    /// Send a question to the human via Telegram and store it as a pending question.
    ///
    /// The question payload is extracted from the `human.interact` event. A pending
    /// question is stored in the state manager so that incoming replies can be
    /// routed back to the correct loop.
    ///
    /// On send failure, retries up to 3 times with exponential backoff (1s, 2s, 4s).
    /// Returns the message ID of the sent Telegram message, or 0 if no chat ID
    /// is configured (question is logged but not sent).
    pub fn send_question(&self, payload: &str) -> TelegramResult<i32> {
        let mut state = self.state_manager.load_or_default()?;

        let message_id = if let Some(chat_id) = state.chat_id {
            self.send_with_retry(chat_id, payload)?
        } else {
            warn!(
                loop_id = %self.loop_id,
                "No chat ID configured — human.interact question logged but not sent: {}",
                payload
            );
            0
        };

        self.state_manager
            .add_pending_question(&mut state, &self.loop_id, message_id)?;

        debug!(
            loop_id = %self.loop_id,
            message_id = message_id,
            "Stored pending question"
        );

        Ok(message_id)
    }

    /// Send a periodic check-in message via Telegram.
    ///
    /// Loads the chat ID from state and sends a short status update so the
    /// human knows the loop is still running. Skips silently if no chat ID
    /// is configured. Returns `Ok(0)` when skipped, or the message ID on
    /// success.
    ///
    /// When a [`CheckinContext`] is provided, the message includes richer
    /// details: current hat, task progress, and cumulative cost.
    pub fn send_checkin(
        &self,
        iteration: u32,
        elapsed: Duration,
        context: Option<&CheckinContext>,
    ) -> TelegramResult<i32> {
        let state = self.state_manager.load_or_default()?;
        let Some(chat_id) = state.chat_id else {
            debug!(
                loop_id = %self.loop_id,
                "No chat ID configured — skipping check-in"
            );
            return Ok(0);
        };

        let elapsed_secs = elapsed.as_secs();
        let minutes = elapsed_secs / 60;
        let seconds = elapsed_secs % 60;
        let elapsed_str = if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        };

        let msg = match context {
            Some(ctx) => {
                let mut lines = vec![format!(
                    "Still working — iteration <b>{}</b>, <code>{}</code> elapsed.",
                    iteration, elapsed_str
                )];

                if let Some(hat) = &ctx.current_hat {
                    lines.push(format!(
                        "Hat: <code>{}</code>",
                        crate::bot::escape_html(hat)
                    ));
                }

                if ctx.open_tasks > 0 || ctx.closed_tasks > 0 {
                    lines.push(format!(
                        "Tasks: <b>{}</b> open, {} closed",
                        ctx.open_tasks, ctx.closed_tasks
                    ));
                }

                if ctx.cumulative_cost > 0.0 {
                    lines.push(format!("Cost: <code>${:.4}</code>", ctx.cumulative_cost));
                }

                lines.join("\n")
            }
            None => format!(
                "Still working — iteration <b>{}</b>, <code>{}</code> elapsed.",
                iteration, elapsed_str
            ),
        };
        self.send_with_retry(chat_id, &msg)
    }

    /// Send a document (file) to the human via Telegram.
    ///
    /// Loads the chat ID from state and sends the file at `file_path` with an
    /// optional caption. Returns `Ok(0)` if no chat ID is configured.
    pub fn send_document(&self, file_path: &Path, caption: Option<&str>) -> TelegramResult<i32> {
        let state = self.state_manager.load_or_default()?;
        let Some(chat_id) = state.chat_id else {
            warn!(
                loop_id = %self.loop_id,
                file = %file_path.display(),
                "No chat ID configured — document not sent"
            );
            return Ok(0);
        };

        self.send_document_with_retry(chat_id, file_path, caption)
    }

    /// Send a photo to the human via Telegram.
    ///
    /// Loads the chat ID from state and sends the image at `file_path` with an
    /// optional caption. Returns `Ok(0)` if no chat ID is configured.
    pub fn send_photo(&self, file_path: &Path, caption: Option<&str>) -> TelegramResult<i32> {
        let state = self.state_manager.load_or_default()?;
        let Some(chat_id) = state.chat_id else {
            warn!(
                loop_id = %self.loop_id,
                file = %file_path.display(),
                "No chat ID configured — photo not sent"
            );
            return Ok(0);
        };

        self.send_photo_with_retry(chat_id, file_path, caption)
    }

    /// Attempt to send a message with exponential backoff retries.
    ///
    /// Uses the host tokio runtime via `block_in_place` + `Handle::block_on`
    /// to bridge the sync event loop to the async BotApi.
    fn send_with_retry(&self, chat_id: i64, payload: &str) -> TelegramResult<i32> {
        use crate::bot::BotApi;

        let handle = tokio::runtime::Handle::try_current().map_err(|_| TelegramError::Send {
            attempts: 0,
            reason: "no tokio runtime available for sending".to_string(),
        })?;

        retry_with_backoff(
            |_attempt| {
                tokio::task::block_in_place(|| {
                    handle.block_on(self.bot.send_message(chat_id, payload))
                })
            },
            |delay| std::thread::sleep(delay),
        )
    }

    /// Attempt to send a document with exponential backoff retries.
    fn send_document_with_retry(
        &self,
        chat_id: i64,
        file_path: &Path,
        caption: Option<&str>,
    ) -> TelegramResult<i32> {
        use crate::bot::BotApi;

        let handle = tokio::runtime::Handle::try_current().map_err(|_| TelegramError::Send {
            attempts: 0,
            reason: "no tokio runtime available for sending".to_string(),
        })?;

        retry_with_backoff(
            |_attempt| {
                tokio::task::block_in_place(|| {
                    handle.block_on(self.bot.send_document(chat_id, file_path, caption))
                })
            },
            |delay| std::thread::sleep(delay),
        )
    }

    /// Attempt to send a photo with exponential backoff retries.
    fn send_photo_with_retry(
        &self,
        chat_id: i64,
        file_path: &Path,
        caption: Option<&str>,
    ) -> TelegramResult<i32> {
        use crate::bot::BotApi;

        let handle = tokio::runtime::Handle::try_current().map_err(|_| TelegramError::Send {
            attempts: 0,
            reason: "no tokio runtime available for sending".to_string(),
        })?;

        retry_with_backoff(
            |_attempt| {
                tokio::task::block_in_place(|| {
                    handle.block_on(self.bot.send_photo(chat_id, file_path, caption))
                })
            },
            |delay| std::thread::sleep(delay),
        )
    }

    /// Poll the events file for a `human.response` event, blocking until one
    /// arrives or the configured timeout expires.
    ///
    /// Polls the given `events_path` every second for new lines containing
    /// `"human.response"`. On response, removes the pending question and
    /// returns the response message. On timeout, removes the pending question
    /// and returns `None`.
    pub fn wait_for_response(&self, events_path: &Path) -> TelegramResult<Option<String>> {
        let timeout = Duration::from_secs(self.timeout_secs);
        let poll_interval = Duration::from_millis(250);
        let deadline = Instant::now() + timeout;

        // Track file position to only read new lines
        let initial_pos = if events_path.exists() {
            std::fs::metadata(events_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        let mut file_pos = initial_pos;

        info!(
            loop_id = %self.loop_id,
            timeout_secs = self.timeout_secs,
            events_path = %events_path.display(),
            "Waiting for human.response"
        );

        loop {
            if Instant::now() >= deadline {
                warn!(
                    loop_id = %self.loop_id,
                    timeout_secs = self.timeout_secs,
                    "Timed out waiting for human.response"
                );

                // Remove pending question on timeout
                if let Ok(mut state) = self.state_manager.load_or_default() {
                    let _ = self
                        .state_manager
                        .remove_pending_question(&mut state, &self.loop_id);
                }

                return Ok(None);
            }

            // Check if we've been interrupted (Ctrl+C / SIGTERM / SIGHUP)
            if self.shutdown.load(Ordering::Relaxed) {
                info!(loop_id = %self.loop_id, "Interrupted while waiting for human.response");
                if let Ok(mut state) = self.state_manager.load_or_default() {
                    let _ = self
                        .state_manager
                        .remove_pending_question(&mut state, &self.loop_id);
                }
                return Ok(None);
            }

            // Read new lines from the events file
            if let Some(response) = Self::check_for_response(events_path, &mut file_pos)? {
                info!(
                    loop_id = %self.loop_id,
                    "Received human.response: {}",
                    response
                );

                // Remove pending question on response
                if let Ok(mut state) = self.state_manager.load_or_default() {
                    let _ = self
                        .state_manager
                        .remove_pending_question(&mut state, &self.loop_id);
                }

                return Ok(Some(response));
            }

            std::thread::sleep(poll_interval);
        }
    }

    /// Check the events file for a `human.response` event starting from
    /// `file_pos`. Updates `file_pos` to the new end of file.
    fn check_for_response(
        events_path: &Path,
        file_pos: &mut u64,
    ) -> TelegramResult<Option<String>> {
        use std::io::{BufRead, BufReader, Seek, SeekFrom};

        if !events_path.exists() {
            return Ok(None);
        }

        let mut file = std::fs::File::open(events_path)?;
        file.seek(SeekFrom::Start(*file_pos))?;

        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            let line_bytes = line.len() as u64 + 1; // +1 for newline
            *file_pos += line_bytes;

            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as JSON event
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line)
                && event.get("topic").and_then(|t| t.as_str()) == Some("human.response")
            {
                let message = event
                    .get("payload")
                    .and_then(|p| p.as_str())
                    .unwrap_or("")
                    .to_string();
                return Ok(Some(message));
            }

            // Also check pipe-separated format (written by MessageHandler)
            if line.contains("EVENT: human.response") {
                // Extract message from pipe-separated format:
                // EVENT: human.response | message: "..." | timestamp: "..."
                let message = line
                    .split('|')
                    .find(|part| part.trim().starts_with("message:"))
                    .and_then(|part| {
                        let value = part.trim().strip_prefix("message:")?;
                        let trimmed = value.trim().trim_matches('"');
                        Some(trimmed.to_string())
                    })
                    .unwrap_or_default();
                return Ok(Some(message));
            }
        }

        Ok(None)
    }
}

impl ralph_proto::RobotService for TelegramService {
    fn send_question(&self, payload: &str) -> anyhow::Result<i32> {
        Ok(TelegramService::send_question(self, payload)?)
    }

    fn wait_for_response(&self, events_path: &Path) -> anyhow::Result<Option<String>> {
        Ok(TelegramService::wait_for_response(self, events_path)?)
    }

    fn send_checkin(
        &self,
        iteration: u32,
        elapsed: Duration,
        context: Option<&ralph_proto::CheckinContext>,
    ) -> anyhow::Result<i32> {
        // Convert ralph_proto::CheckinContext to the local CheckinContext
        let local_context = context.map(|ctx| CheckinContext {
            current_hat: ctx.current_hat.clone(),
            open_tasks: ctx.open_tasks,
            closed_tasks: ctx.closed_tasks,
            cumulative_cost: ctx.cumulative_cost,
        });
        Ok(TelegramService::send_checkin(
            self,
            iteration,
            elapsed,
            local_context.as_ref(),
        )?)
    }

    fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    fn stop(self: Box<Self>) {
        TelegramService::stop(*self);
    }
}

impl fmt::Debug for TelegramService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelegramService")
            .field("workspace_root", &self.workspace_root)
            .field("bot_token", &self.bot_token_masked())
            .field("timeout_secs", &self.timeout_secs)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn test_service(dir: &TempDir) -> TelegramService {
        TelegramService::new(
            dir.path().to_path_buf(),
            Some("test-token-12345".to_string()),
            300,
            "main".to_string(),
        )
        .unwrap()
    }

    #[test]
    fn new_with_explicit_token() {
        let dir = TempDir::new().unwrap();
        let service = TelegramService::new(
            dir.path().to_path_buf(),
            Some("test-token-12345".to_string()),
            300,
            "main".to_string(),
        );
        assert!(service.is_ok());
    }

    #[test]
    fn new_without_token_fails() {
        // Only run this test when the env var is not set,
        // to avoid needing unsafe remove_var
        if std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_ok() {
            return;
        }

        let dir = TempDir::new().unwrap();
        let service = TelegramService::new(dir.path().to_path_buf(), None, 300, "main".to_string());
        assert!(service.is_err());
        assert!(matches!(
            service.unwrap_err(),
            TelegramError::MissingBotToken
        ));
    }

    #[test]
    fn bot_token_masked_works() {
        let dir = TempDir::new().unwrap();
        let service = TelegramService::new(
            dir.path().to_path_buf(),
            Some("abcd1234efgh5678".to_string()),
            300,
            "main".to_string(),
        )
        .unwrap();
        let masked = service.bot_token_masked();
        assert_eq!(masked, "abcd...5678");
    }

    #[test]
    fn loop_id_accessor() {
        let dir = TempDir::new().unwrap();
        let service = TelegramService::new(
            dir.path().to_path_buf(),
            Some("token".to_string()),
            60,
            "feature-auth".to_string(),
        )
        .unwrap();
        assert_eq!(service.loop_id(), "feature-auth");
    }

    #[test]
    fn send_question_stores_pending_question() {
        let dir = TempDir::new().unwrap();
        let service = test_service(&dir);

        service.send_question("Which DB to use?").unwrap();

        // Verify pending question is stored
        let state = service.state_manager().load_or_default().unwrap();
        assert!(
            state.pending_questions.contains_key("main"),
            "pending question should be stored for loop_id 'main'"
        );
    }

    #[test]
    fn send_question_returns_message_id() {
        let dir = TempDir::new().unwrap();
        let service = test_service(&dir);

        let msg_id = service.send_question("async or sync?").unwrap();
        // Without a chat_id in state, message_id is 0
        assert_eq!(msg_id, 0);
    }

    #[test]
    fn check_for_response_json_format() {
        let dir = TempDir::new().unwrap();
        let events_path = dir.path().join("events.jsonl");

        // Write a non-response event first
        let mut file = std::fs::File::create(&events_path).unwrap();
        writeln!(
            file,
            r#"{{"topic":"build.done","payload":"tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass","ts":"2026-01-30T00:00:00Z"}}"#
        )
        .unwrap();
        // Write a human.response event
        writeln!(
            file,
            r#"{{"topic":"human.response","payload":"Use async","ts":"2026-01-30T00:01:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut pos = 0;
        let result = TelegramService::check_for_response(&events_path, &mut pos).unwrap();
        assert_eq!(result, Some("Use async".to_string()));
    }

    #[test]
    fn check_for_response_pipe_format() {
        let dir = TempDir::new().unwrap();
        let events_path = dir.path().join("events.jsonl");

        let mut file = std::fs::File::create(&events_path).unwrap();
        writeln!(
            file,
            r#"EVENT: human.response | message: "Use sync" | timestamp: "2026-01-30T00:01:00Z""#
        )
        .unwrap();
        file.flush().unwrap();

        let mut pos = 0;
        let result = TelegramService::check_for_response(&events_path, &mut pos).unwrap();
        assert_eq!(result, Some("Use sync".to_string()));
    }

    #[test]
    fn check_for_response_skips_non_response_events() {
        let dir = TempDir::new().unwrap();
        let events_path = dir.path().join("events.jsonl");

        let mut file = std::fs::File::create(&events_path).unwrap();
        writeln!(
            file,
            r#"{{"topic":"build.done","payload":"done","ts":"2026-01-30T00:00:00Z"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"topic":"human.guidance","payload":"check errors","ts":"2026-01-30T00:01:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut pos = 0;
        let result = TelegramService::check_for_response(&events_path, &mut pos).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn check_for_response_missing_file() {
        let dir = TempDir::new().unwrap();
        let events_path = dir.path().join("does-not-exist.jsonl");

        let mut pos = 0;
        let result = TelegramService::check_for_response(&events_path, &mut pos).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn check_for_response_tracks_position() {
        let dir = TempDir::new().unwrap();
        let events_path = dir.path().join("events.jsonl");

        // Write one event
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&events_path)
            .unwrap();
        writeln!(
            file,
            r#"{{"topic":"build.done","payload":"done","ts":"2026-01-30T00:00:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut pos = 0;
        let result = TelegramService::check_for_response(&events_path, &mut pos).unwrap();
        assert_eq!(result, None);
        assert!(pos > 0, "position should advance after reading");

        let pos_after_first = pos;

        // Append a human.response
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&events_path)
            .unwrap();
        writeln!(
            file,
            r#"{{"topic":"human.response","payload":"yes","ts":"2026-01-30T00:02:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        // Should find the response starting from where we left off
        let result = TelegramService::check_for_response(&events_path, &mut pos).unwrap();
        assert_eq!(result, Some("yes".to_string()));
        assert!(pos > pos_after_first, "position should advance further");
    }

    #[test]
    fn wait_for_response_returns_on_response() {
        let dir = TempDir::new().unwrap();
        let service = TelegramService::new(
            dir.path().to_path_buf(),
            Some("token".to_string()),
            5, // enough time for the writer thread
            "main".to_string(),
        )
        .unwrap();

        let events_path = dir.path().join("events.jsonl");
        // Create an empty events file so wait_for_response records position 0
        std::fs::File::create(&events_path).unwrap();

        // Store a pending question first
        service.send_question("Which plan?").unwrap();

        // Spawn a thread to write the response after a brief delay
        let writer_path = events_path.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&writer_path)
                .unwrap();
            writeln!(
                file,
                r#"{{"topic":"human.response","payload":"Go with plan A","ts":"2026-01-30T00:00:00Z"}}"#
            )
            .unwrap();
            file.flush().unwrap();
        });

        let result = service.wait_for_response(&events_path).unwrap();
        writer.join().unwrap();

        assert_eq!(result, Some("Go with plan A".to_string()));

        // Pending question should be removed
        let state = service.state_manager().load_or_default().unwrap();
        assert!(
            !state.pending_questions.contains_key("main"),
            "pending question should be removed after response"
        );
    }

    #[test]
    fn wait_for_response_returns_none_on_timeout() {
        let dir = TempDir::new().unwrap();
        let service = TelegramService::new(
            dir.path().to_path_buf(),
            Some("token".to_string()),
            1, // 1 second timeout
            "main".to_string(),
        )
        .unwrap();

        let events_path = dir.path().join("events.jsonl");
        // Create an empty events file with no human.response
        std::fs::File::create(&events_path).unwrap();

        // Store a pending question
        service.send_question("Will this timeout?").unwrap();

        let result = service.wait_for_response(&events_path).unwrap();
        assert_eq!(result, None, "should return None on timeout");

        // Pending question should be removed even on timeout
        let state = service.state_manager().load_or_default().unwrap();
        assert!(
            !state.pending_questions.contains_key("main"),
            "pending question should be removed on timeout"
        );
    }

    #[test]
    fn retry_with_backoff_succeeds_on_first_attempt() {
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(
            |attempt| {
                attempts_clone.lock().unwrap().push(attempt);
                Ok(42)
            },
            |_delay| {},
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*attempts.lock().unwrap(), vec![1]);
    }

    #[test]
    fn retry_with_backoff_succeeds_on_second_attempt() {
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let attempts_clone = attempts.clone();
        let delays = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let delays_clone = delays.clone();

        let result = retry_with_backoff(
            |attempt| {
                attempts_clone.lock().unwrap().push(attempt);
                if attempt < 2 {
                    Err(TelegramError::Send {
                        attempts: attempt,
                        reason: "transient failure".to_string(),
                    })
                } else {
                    Ok(99)
                }
            },
            |delay| {
                delays_clone.lock().unwrap().push(delay);
            },
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 99);
        assert_eq!(*attempts.lock().unwrap(), vec![1, 2]);
        // First retry delay: 1s * 2^0 = 1s
        assert_eq!(*delays.lock().unwrap(), vec![Duration::from_secs(1)]);
    }

    #[test]
    fn retry_with_backoff_succeeds_on_third_attempt() {
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let attempts_clone = attempts.clone();
        let delays = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let delays_clone = delays.clone();

        let result = retry_with_backoff(
            |attempt| {
                attempts_clone.lock().unwrap().push(attempt);
                if attempt < 3 {
                    Err(TelegramError::Send {
                        attempts: attempt,
                        reason: "transient failure".to_string(),
                    })
                } else {
                    Ok(7)
                }
            },
            |delay| {
                delays_clone.lock().unwrap().push(delay);
            },
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 7);
        assert_eq!(*attempts.lock().unwrap(), vec![1, 2, 3]);
        // Delays: 1s * 2^0 = 1s, 1s * 2^1 = 2s
        assert_eq!(
            *delays.lock().unwrap(),
            vec![Duration::from_secs(1), Duration::from_secs(2)]
        );
    }

    #[test]
    fn retry_with_backoff_fails_after_all_retries() {
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let attempts_clone = attempts.clone();
        let delays = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let delays_clone = delays.clone();

        let result = retry_with_backoff(
            |attempt| {
                attempts_clone.lock().unwrap().push(attempt);
                Err(TelegramError::Send {
                    attempts: attempt,
                    reason: format!("failure on attempt {}", attempt),
                })
            },
            |delay| {
                delays_clone.lock().unwrap().push(delay);
            },
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            TelegramError::Send {
                attempts: 3,
                reason: _
            }
        ));
        // Should report the last error message
        if let TelegramError::Send { reason, .. } = &err {
            assert!(reason.contains("failure on attempt 3"));
        }
        assert_eq!(*attempts.lock().unwrap(), vec![1, 2, 3]);
        // Delays: 1s, 2s (no delay after final attempt)
        assert_eq!(
            *delays.lock().unwrap(),
            vec![Duration::from_secs(1), Duration::from_secs(2)]
        );
    }

    #[test]
    fn retry_with_backoff_exponential_delays_are_correct() {
        let delays = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let delays_clone = delays.clone();

        let _ = retry_with_backoff(
            |_attempt| {
                Err(TelegramError::Send {
                    attempts: 1,
                    reason: "always fail".to_string(),
                })
            },
            |delay| {
                delays_clone.lock().unwrap().push(delay);
            },
        );

        let recorded = delays.lock().unwrap().clone();
        // Backoff: 1s * 2^0 = 1s, 1s * 2^1 = 2s (no sleep after 3rd attempt)
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], Duration::from_secs(1));
        assert_eq!(recorded[1], Duration::from_secs(2));
    }

    #[test]
    fn checkin_context_default() {
        let ctx = CheckinContext::default();
        assert!(ctx.current_hat.is_none());
        assert_eq!(ctx.open_tasks, 0);
        assert_eq!(ctx.closed_tasks, 0);
        assert!(ctx.cumulative_cost.abs() < f64::EPSILON);
    }

    #[test]
    fn checkin_context_with_hat_and_tasks() {
        let ctx = CheckinContext {
            current_hat: Some("executor".to_string()),
            open_tasks: 3,
            closed_tasks: 5,
            cumulative_cost: 1.2345,
        };
        assert_eq!(ctx.current_hat.as_deref(), Some("executor"));
        assert_eq!(ctx.open_tasks, 3);
        assert_eq!(ctx.closed_tasks, 5);
        assert!((ctx.cumulative_cost - 1.2345).abs() < f64::EPSILON);
    }

    #[test]
    fn wait_for_response_returns_none_on_shutdown() {
        let dir = TempDir::new().unwrap();
        let service = TelegramService::new(
            dir.path().to_path_buf(),
            Some("token".to_string()),
            60, // long timeout — shutdown flag should preempt it
            "main".to_string(),
        )
        .unwrap();

        let events_path = dir.path().join("events.jsonl");
        std::fs::File::create(&events_path).unwrap();

        // Set shutdown flag before calling wait_for_response
        service.shutdown_flag().store(true, Ordering::Relaxed);

        let start = Instant::now();
        let result = service.wait_for_response(&events_path).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(result, None, "should return None when shutdown flag is set");
        assert!(
            elapsed < Duration::from_secs(2),
            "should return quickly, not wait for timeout (elapsed: {:?})",
            elapsed
        );
    }
}
