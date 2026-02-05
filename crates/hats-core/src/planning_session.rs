//! Planning session management for human-in-the-loop workflows.
//!
//! Planning sessions enable collaborative planning through chat-style interactions.
//! Each session has:
//! - A unique ID (timestamp-based)
//! - A conversation file (JSONL format for prompts/responses)
//! - Session metadata (status, timestamps, etc.)
//! - Artifacts directory (generated design docs, plans)

use crate::loop_context::LoopContext;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Error type for planning session operations.
#[derive(Debug, thiserror::Error)]
pub enum PlanningSessionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Session not found: {0}")]
    NotFound(String),
}

/// Status of a planning session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    /// Session is active and waiting for input or processing
    Active,
    /// Session is waiting for a user response to a specific prompt
    WaitingForInput { prompt_id: String },
    /// Session completed successfully
    Completed,
    /// Session timed out waiting for user input
    TimedOut,
    /// Session failed due to an error
    Failed,
}

/// A single entry in the planning conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    /// Entry type: either a prompt from the agent or response from user
    #[serde(rename = "type")]
    pub entry_type: ConversationType,
    /// Unique identifier for this message
    pub id: String,
    /// The message text
    pub text: String,
    /// ISO 8601 timestamp of the message
    pub ts: String,
}

/// Type of conversation entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConversationType {
    /// A question/prompt from the agent
    UserPrompt,
    /// A response from the user
    UserResponse,
}

/// Metadata for a planning session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier
    pub id: String,
    /// Original user prompt that started the session
    pub prompt: String,
    /// Current session status
    pub status: SessionStatus,
    /// ISO 8601 timestamp when the session was created
    pub created_at: String,
    /// ISO 8601 timestamp of last activity
    pub updated_at: String,
    /// Number of iterations completed
    pub iterations: usize,
    /// Config file used for this session (if any)
    pub config: Option<String>,
}

/// A planning session for human-in-the-loop workflows.
#[derive(Debug)]
pub struct PlanningSession {
    /// Session metadata
    pub metadata: SessionMetadata,
    /// Path to the session directory
    pub session_dir: PathBuf,
    /// Path to the conversation file
    pub conversation_path: PathBuf,
}

impl PlanningSession {
    /// Create a new planning session.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's original prompt/idea
    /// * `context` - The loop context for path resolution
    /// * `config` - Optional config file to use
    pub fn new(
        prompt: &str,
        context: &LoopContext,
        config: Option<String>,
    ) -> Result<Self, PlanningSessionError> {
        let session_id = Self::generate_session_id();
        let session_dir = context.planning_session_dir(&session_id);
        let conversation_path = context.planning_conversation_path(&session_id);

        // Create session directory
        std::fs::create_dir_all(&session_dir)?;

        // Create artifacts directory
        let artifacts_dir = context.planning_artifacts_dir(&session_id);
        std::fs::create_dir_all(&artifacts_dir)?;

        let now = Utc::now().to_rfc3339();
        let metadata = SessionMetadata {
            id: session_id.clone(),
            prompt: prompt.to_string(),
            status: SessionStatus::Active,
            created_at: now.clone(),
            updated_at: now,
            iterations: 0,
            config,
        };

        // Save metadata
        let metadata_path = context.planning_session_metadata_path(&session_id);
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        let mut file = File::create(&metadata_path)?;
        file.write_all(metadata_json.as_bytes())?;

        // Create empty conversation file
        File::create(&conversation_path)?;

        Ok(Self {
            metadata,
            session_dir,
            conversation_path,
        })
    }

    /// Load an existing planning session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    /// * `context` - The loop context for path resolution
    pub fn load(id: &str, context: &LoopContext) -> Result<Self, PlanningSessionError> {
        let session_dir = context.planning_session_dir(id);
        let conversation_path = context.planning_conversation_path(id);
        let metadata_path = context.planning_session_metadata_path(id);

        if !session_dir.exists() {
            return Err(PlanningSessionError::NotFound(id.to_string()));
        }

        // Load metadata
        let metadata_json = std::fs::read_to_string(&metadata_path)?;
        let metadata: SessionMetadata = serde_json::from_str(&metadata_json)?;

        Ok(Self {
            metadata,
            session_dir,
            conversation_path,
        })
    }

    /// Generate a unique session ID based on timestamp.
    fn generate_session_id() -> String {
        let now = Utc::now();
        let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
        // Use nanoseconds for uniqueness (take last 4 hex chars)
        let nano_suffix = format!("{:x}", now.timestamp_subsec_nanos());
        let random_suffix = &nano_suffix[nano_suffix.len().saturating_sub(4)..];
        format!("{}-{}", timestamp, random_suffix)
    }

    /// Get the session ID.
    pub fn id(&self) -> &str {
        &self.metadata.id
    }

    /// Update the session status.
    pub fn set_status(&mut self, status: SessionStatus) -> Result<(), PlanningSessionError> {
        self.metadata.status = status;
        self.metadata.updated_at = Utc::now().to_rfc3339();
        self.save_metadata()
    }

    /// Increment the iteration counter.
    pub fn increment_iterations(&mut self) -> Result<(), PlanningSessionError> {
        self.metadata.iterations += 1;
        self.metadata.updated_at = Utc::now().to_rfc3339();
        self.save_metadata()
    }

    /// Save the session metadata to disk.
    pub fn save_metadata(&self) -> Result<(), PlanningSessionError> {
        let metadata_path = self.session_dir.join("session.json");
        let metadata_json = serde_json::to_string_pretty(&self.metadata)?;
        let mut file = File::create(&metadata_path)?;
        file.write_all(metadata_json.as_bytes())?;
        Ok(())
    }

    /// Append a prompt entry to the conversation.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique prompt identifier (e.g., "q1", "q2")
    /// * `text` - The prompt/question text
    pub fn append_prompt(&self, id: &str, text: &str) -> Result<(), PlanningSessionError> {
        let entry = ConversationEntry {
            entry_type: ConversationType::UserPrompt,
            id: id.to_string(),
            text: text.to_string(),
            ts: Utc::now().to_rfc3339(),
        };
        self.append_entry(&entry)
    }

    /// Append a response entry to the conversation.
    ///
    /// # Arguments
    ///
    /// * `id` - The prompt ID this responds to
    /// * `text` - The user's response text
    pub fn append_response(&mut self, id: &str, text: &str) -> Result<(), PlanningSessionError> {
        let entry = ConversationEntry {
            entry_type: ConversationType::UserResponse,
            id: id.to_string(),
            text: text.to_string(),
            ts: Utc::now().to_rfc3339(),
        };
        self.append_entry(&entry)
    }

    /// Append an entry to the conversation file.
    fn append_entry(&self, entry: &ConversationEntry) -> Result<(), PlanningSessionError> {
        // Open file in append mode
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.conversation_path)?;

        // Write entry as JSONL
        let json = serde_json::to_string(entry)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    /// Find a response to a specific prompt in the conversation.
    ///
    /// # Arguments
    ///
    /// * `prompt_id` - The prompt ID to search for
    ///
    /// Returns the response text if found, None otherwise.
    pub fn find_response(&self, prompt_id: &str) -> Result<Option<String>, PlanningSessionError> {
        if !self.conversation_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&self.conversation_path)?;

        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<ConversationEntry>(line)
                && entry.entry_type == ConversationType::UserResponse
                && entry.id == prompt_id
            {
                return Ok(Some(entry.text));
            }
        }

        Ok(None)
    }

    /// Load all conversation entries.
    pub fn load_conversation(&self) -> Result<Vec<ConversationEntry>, PlanningSessionError> {
        if !self.conversation_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.conversation_path)?;
        let mut entries = Vec::new();

        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<ConversationEntry>(line) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_context() -> (TempDir, LoopContext) {
        let temp = TempDir::new().unwrap();
        let ctx = LoopContext::primary(temp.path().to_path_buf());
        (temp, ctx)
    }

    #[test]
    fn test_generate_session_id() {
        let id1 = PlanningSession::generate_session_id();
        let id2 = PlanningSession::generate_session_id();

        // IDs should be different
        assert_ne!(id1, id2);

        // IDs should have timestamp format
        assert!(id1.len() > 10);
        assert!(id1.contains('-'));
    }

    #[test]
    fn test_create_new_session() {
        let (_temp, ctx) = create_test_context();
        let prompt = "Build a feature for user authentication";

        let session = PlanningSession::new(prompt, &ctx, None).unwrap();

        assert_eq!(session.metadata.prompt, prompt);
        assert_eq!(session.metadata.status, SessionStatus::Active);
        assert_eq!(session.metadata.iterations, 0);
        assert!(session.session_dir.exists());
        assert!(session.conversation_path.exists());
    }

    #[test]
    fn test_load_existing_session() {
        let (_temp, ctx) = create_test_context();
        let prompt = "Build OAuth2 login";

        // Create session
        let session_id = PlanningSession::new(prompt, &ctx, None)
            .unwrap()
            .id()
            .to_string();

        // Load session
        let loaded = PlanningSession::load(&session_id, &ctx).unwrap();

        assert_eq!(loaded.metadata.prompt, prompt);
        assert_eq!(loaded.metadata.id, session_id);
    }

    #[test]
    fn test_load_nonexistent_session() {
        let (_temp, ctx) = create_test_context();

        let result = PlanningSession::load("nonexistent", &ctx);
        assert!(matches!(result, Err(PlanningSessionError::NotFound(_))));
    }

    #[test]
    fn test_append_prompt_and_response() {
        let (_temp, ctx) = create_test_context();
        let mut session = PlanningSession::new("Test prompt", &ctx, None).unwrap();

        // Append a prompt
        session
            .append_prompt("q1", "What is the feature name?")
            .unwrap();

        // Append a response
        session.append_response("q1", "OAuth Login").unwrap();

        // Load conversation
        let entries = session.load_conversation().unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].entry_type, ConversationType::UserPrompt);
        assert_eq!(entries[0].id, "q1");
        assert_eq!(entries[0].text, "What is the feature name?");

        assert_eq!(entries[1].entry_type, ConversationType::UserResponse);
        assert_eq!(entries[1].id, "q1");
        assert_eq!(entries[1].text, "OAuth Login");
    }

    #[test]
    fn test_find_response() {
        let (_temp, ctx) = create_test_context();
        let mut session = PlanningSession::new("Test prompt", &ctx, None).unwrap();

        // No response initially
        assert!(session.find_response("q1").unwrap().is_none());

        // Add prompt and response
        session.append_prompt("q1", "Question?").unwrap();
        session.append_response("q1", "Answer").unwrap();

        // Find the response
        let response = session.find_response("q1").unwrap();
        assert_eq!(response, Some("Answer".to_string()));

        // Non-existent prompt returns None
        assert!(session.find_response("q2").unwrap().is_none());
    }

    #[test]
    fn test_set_status() {
        let (_temp, ctx) = create_test_context();
        let mut session = PlanningSession::new("Test prompt", &ctx, None).unwrap();

        session
            .set_status(SessionStatus::WaitingForInput {
                prompt_id: "q1".to_string(),
            })
            .unwrap();

        assert!(matches!(
            session.metadata.status,
            SessionStatus::WaitingForInput { .. }
        ));

        // Reload and verify status persisted
        let session_id = session.id().to_string();
        let loaded = PlanningSession::load(&session_id, &ctx).unwrap();
        assert!(matches!(
            loaded.metadata.status,
            SessionStatus::WaitingForInput { .. }
        ));
    }

    #[test]
    fn test_increment_iterations() {
        let (_temp, ctx) = create_test_context();
        let mut session = PlanningSession::new("Test prompt", &ctx, None).unwrap();

        assert_eq!(session.metadata.iterations, 0);

        session.increment_iterations().unwrap();
        assert_eq!(session.metadata.iterations, 1);

        session.increment_iterations().unwrap();
        assert_eq!(session.metadata.iterations, 2);
    }

    #[test]
    fn test_artifacts_directory_created() {
        let (_temp, ctx) = create_test_context();
        let session = PlanningSession::new("Test prompt", &ctx, None).unwrap();

        let artifacts_dir = session.session_dir.join("artifacts");
        assert!(artifacts_dir.exists());
        assert!(artifacts_dir.is_dir());
    }
}
