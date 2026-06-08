use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::SlackResult;

const MAX_SEEN_EVENT_IDS: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlackThreadStatus {
    Running,
    Completed,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackThreadBinding {
    pub loop_id: String,
    pub channel_id: String,
    pub thread_ts: String,
    pub root_ts: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub workspace_root: PathBuf,
    #[serde(default)]
    pub repo_alias: Option<String>,
    #[serde(default)]
    pub repo_dir: Option<PathBuf>,
    pub status: SlackThreadStatus,
    #[serde(default)]
    pub process_id: Option<u32>,
    #[serde(default)]
    pub start_card_ts: Option<String>,
    #[serde(default)]
    pub progress_message_ts: Option<String>,
    #[serde(default)]
    pub stream_ts: Option<String>,
    #[serde(default)]
    pub final_card_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSlackQuestion {
    pub channel_id: String,
    pub thread_ts: String,
    pub message_ts: String,
    pub asked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackState {
    pub team_id: Option<String>,
    pub last_socket_envelope_id: Option<String>,
    pub threads: HashMap<String, SlackThreadBinding>,
    pub thread_to_loop: HashMap<String, String>,
    pub pending_questions: HashMap<String, PendingSlackQuestion>,
    pub seen_event_ids: VecDeque<String>,
}

impl Default for SlackState {
    fn default() -> Self {
        Self {
            team_id: None,
            last_socket_envelope_id: None,
            threads: HashMap::new(),
            thread_to_loop: HashMap::new(),
            pending_questions: HashMap::new(),
            seen_event_ids: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlackStateManager {
    path: PathBuf,
}

impl SlackStateManager {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> SlackResult<Option<SlackState>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&self.path)?;
        let state = serde_json::from_str(&contents)?;
        Ok(Some(state))
    }

    pub fn load_or_default(&self) -> SlackResult<SlackState> {
        Ok(self.load()?.unwrap_or_default())
    }

    pub fn save(&self, state: &SlackState) -> SlackResult<()> {
        let json = serde_json::to_string_pretty(state)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = self.path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    pub fn bind_thread(
        &self,
        loop_id: &str,
        channel_id: &str,
        thread_ts: &str,
        created_by: &str,
        workspace_root: impl AsRef<Path>,
    ) -> SlackResult<()> {
        self.bind_thread_with_repo(
            loop_id,
            channel_id,
            thread_ts,
            created_by,
            workspace_root,
            None,
            None::<&Path>,
        )
    }

    pub fn bind_thread_with_repo(
        &self,
        loop_id: &str,
        channel_id: &str,
        thread_ts: &str,
        created_by: &str,
        workspace_root: impl AsRef<Path>,
        repo_alias: Option<&str>,
        repo_dir: Option<impl AsRef<Path>>,
    ) -> SlackResult<()> {
        let mut state = self.load_or_default()?;
        state.threads.insert(
            loop_id.to_string(),
            SlackThreadBinding {
                loop_id: loop_id.to_string(),
                channel_id: channel_id.to_string(),
                thread_ts: thread_ts.to_string(),
                root_ts: thread_ts.to_string(),
                created_by: created_by.to_string(),
                created_at: Utc::now(),
                workspace_root: workspace_root.as_ref().to_path_buf(),
                repo_alias: repo_alias.map(ToOwned::to_owned),
                repo_dir: repo_dir.map(|dir| dir.as_ref().to_path_buf()),
                status: SlackThreadStatus::Running,
                process_id: None,
                start_card_ts: None,
                progress_message_ts: None,
                stream_ts: None,
                final_card_ts: None,
            },
        );
        state
            .thread_to_loop
            .insert(thread_key(channel_id, thread_ts), loop_id.to_string());
        self.save(&state)
    }

    pub fn loop_for_thread(
        &self,
        channel_id: &str,
        thread_ts: &str,
    ) -> SlackResult<Option<String>> {
        let state = self.load_or_default()?;
        Ok(state
            .thread_to_loop
            .get(&thread_key(channel_id, thread_ts))
            .cloned())
    }

    pub fn add_pending_question(
        &self,
        loop_id: &str,
        channel_id: &str,
        thread_ts: &str,
        message_ts: &str,
    ) -> SlackResult<()> {
        let mut state = self.load_or_default()?;
        state.pending_questions.insert(
            loop_id.to_string(),
            PendingSlackQuestion {
                channel_id: channel_id.to_string(),
                thread_ts: thread_ts.to_string(),
                message_ts: message_ts.to_string(),
                asked_at: Utc::now(),
            },
        );
        self.save(&state)
    }

    pub fn remove_pending_question(&self, loop_id: &str) -> SlackResult<()> {
        let mut state = self.load_or_default()?;
        state.pending_questions.remove(loop_id);
        self.save(&state)
    }

    pub fn has_pending_question(&self, loop_id: &str) -> SlackResult<bool> {
        Ok(self
            .load_or_default()?
            .pending_questions
            .contains_key(loop_id))
    }

    pub fn set_thread_process_id(&self, loop_id: &str, process_id: Option<u32>) -> SlackResult<()> {
        let mut state = self.load_or_default()?;
        if let Some(binding) = state.threads.get_mut(loop_id) {
            binding.process_id = process_id;
        }
        self.save(&state)
    }

    pub fn set_thread_message_timestamps(
        &self,
        loop_id: &str,
        start_card_ts: Option<&str>,
        progress_message_ts: Option<&str>,
        stream_ts: Option<&str>,
        final_card_ts: Option<&str>,
    ) -> SlackResult<()> {
        let mut state = self.load_or_default()?;
        if let Some(binding) = state.threads.get_mut(loop_id) {
            if let Some(ts) = start_card_ts {
                binding.start_card_ts = Some(ts.to_string());
            }
            if let Some(ts) = progress_message_ts {
                binding.progress_message_ts = Some(ts.to_string());
            }
            if let Some(ts) = stream_ts {
                binding.stream_ts = Some(ts.to_string());
            }
            if let Some(ts) = final_card_ts {
                binding.final_card_ts = Some(ts.to_string());
            }
        }
        self.save(&state)
    }

    pub fn set_thread_status(&self, loop_id: &str, status: SlackThreadStatus) -> SlackResult<()> {
        let mut state = self.load_or_default()?;
        if let Some(binding) = state.threads.get_mut(loop_id) {
            binding.status = status;
        }
        state.pending_questions.remove(loop_id);
        self.save(&state)
    }

    pub fn finish_thread(&self, loop_id: &str, status: SlackThreadStatus) -> SlackResult<()> {
        let mut state = self.load_or_default()?;
        if let Some(binding) = state.threads.get_mut(loop_id) {
            binding.status = status;
            binding.process_id = None;
        }
        state.pending_questions.remove(loop_id);
        self.save(&state)
    }

    pub fn mark_event_seen(&self, event_id: &str) -> SlackResult<bool> {
        let mut state = self.load_or_default()?;
        if state.seen_event_ids.iter().any(|seen| seen == event_id) {
            return Ok(false);
        }
        state.seen_event_ids.push_back(event_id.to_string());
        while state.seen_event_ids.len() > MAX_SEEN_EVENT_IDS {
            state.seen_event_ids.pop_front();
        }
        self.save(&state)?;
        Ok(true)
    }
}

pub fn thread_key(channel_id: &str, thread_ts: &str) -> String {
    format!("{}:{}", channel_id, thread_ts)
}
