use std::fmt;
use std::path::PathBuf;

use crate::error::{TelegramError, TelegramResult};
use crate::handler::MessageHandler;
use crate::state::StateManager;

/// Coordinates the Telegram bot lifecycle with the Ralph event loop.
///
/// Manages startup, shutdown, message sending, and response waiting.
pub struct TelegramService {
    workspace_root: PathBuf,
    bot_token: String,
    timeout_secs: u64,
    _loop_id: String,
    state_manager: StateManager,
    handler: MessageHandler,
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

        Ok(Self {
            workspace_root,
            bot_token: resolved_token,
            timeout_secs,
            _loop_id: loop_id,
            state_manager,
            handler,
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
    use tempfile::TempDir;

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
}
