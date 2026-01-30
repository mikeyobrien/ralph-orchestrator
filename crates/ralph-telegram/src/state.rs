use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::TelegramResult;

/// Persistent state for the Telegram bot, stored at `.ralph/telegram-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramState {
    /// The chat ID for the human operator (auto-detected from first message).
    pub chat_id: Option<i64>,

    /// Timestamp of the last message seen.
    pub last_seen: Option<DateTime<Utc>>,

    /// Pending questions keyed by loop ID, tracking which message awaits a reply.
    #[serde(default)]
    pub pending_questions: HashMap<String, PendingQuestion>,
}

/// A question sent to the human that is awaiting a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingQuestion {
    /// When the question was sent.
    pub asked_at: DateTime<Utc>,

    /// The Telegram message ID, used to match reply-to routing.
    pub message_id: i32,
}

/// Manages persistence of Telegram bot state to disk.
pub struct StateManager {
    path: PathBuf,
}

impl StateManager {
    /// Create a new StateManager that reads/writes to the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Load state from disk. Returns `None` if the file doesn't exist.
    pub fn load(&self) -> TelegramResult<Option<TelegramState>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&self.path)?;
        let state: TelegramState = serde_json::from_str(&contents)?;
        Ok(Some(state))
    }

    /// Save state to disk using atomic write (temp file + rename).
    pub fn save(&self, state: &TelegramState) -> TelegramResult<()> {
        let json = serde_json::to_string_pretty(state)?;
        let tmp_path = self.path.with_extension("json.tmp");

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    /// Load existing state or create a fresh empty state.
    pub fn load_or_default(&self) -> TelegramResult<TelegramState> {
        Ok(self.load()?.unwrap_or_else(|| TelegramState {
            chat_id: None,
            last_seen: None,
            pending_questions: HashMap::new(),
        }))
    }

    /// Add a pending question for a given loop.
    pub fn add_pending_question(
        &self,
        state: &mut TelegramState,
        loop_id: &str,
        message_id: i32,
    ) -> TelegramResult<()> {
        state.pending_questions.insert(
            loop_id.to_string(),
            PendingQuestion {
                asked_at: Utc::now(),
                message_id,
            },
        );
        self.save(state)
    }

    /// Remove a pending question for a given loop.
    pub fn remove_pending_question(
        &self,
        state: &mut TelegramState,
        loop_id: &str,
    ) -> TelegramResult<()> {
        state.pending_questions.remove(loop_id);
        self.save(state)
    }

    /// Given a reply_to_message_id, find which loop it belongs to.
    pub fn get_loop_for_reply(
        &self,
        state: &TelegramState,
        reply_message_id: i32,
    ) -> Option<String> {
        state
            .pending_questions
            .iter()
            .find(|(_, q)| q.message_id == reply_message_id)
            .map(|(loop_id, _)| loop_id.clone())
    }

    /// Return the path to the state file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_manager() -> (StateManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("telegram-state.json");
        (StateManager::new(path), dir)
    }

    #[test]
    fn load_missing_file_returns_none() {
        let (mgr, _dir) = test_manager();
        assert!(mgr.load().unwrap().is_none());
    }

    #[test]
    fn save_and_load_round_trip() {
        let (mgr, _dir) = test_manager();
        let state = TelegramState {
            chat_id: Some(123_456),
            last_seen: Some(Utc::now()),
            pending_questions: HashMap::new(),
        };
        mgr.save(&state).unwrap();

        let loaded = mgr.load().unwrap().unwrap();
        assert_eq!(loaded.chat_id, Some(123_456));
    }

    #[test]
    fn corrupted_json_returns_error() {
        let (mgr, _dir) = test_manager();
        std::fs::write(mgr.path(), "not json").unwrap();
        assert!(mgr.load().is_err());
    }

    #[test]
    fn pending_question_tracking() {
        let (mgr, _dir) = test_manager();
        let mut state = mgr.load_or_default().unwrap();

        mgr.add_pending_question(&mut state, "main", 42).unwrap();
        assert!(state.pending_questions.contains_key("main"));
        assert_eq!(state.pending_questions["main"].message_id, 42);

        mgr.remove_pending_question(&mut state, "main").unwrap();
        assert!(!state.pending_questions.contains_key("main"));
    }

    #[test]
    fn reply_routing_lookup() {
        let (mgr, _dir) = test_manager();
        let mut state = mgr.load_or_default().unwrap();

        mgr.add_pending_question(&mut state, "main", 10).unwrap();
        mgr.add_pending_question(&mut state, "feature-auth", 20)
            .unwrap();

        assert_eq!(mgr.get_loop_for_reply(&state, 10), Some("main".to_string()));
        assert_eq!(
            mgr.get_loop_for_reply(&state, 20),
            Some("feature-auth".to_string())
        );
        assert_eq!(mgr.get_loop_for_reply(&state, 99), None);
    }
}
