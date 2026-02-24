use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::errors::ApiError;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningStartParams {
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningRespondParams {
    pub session_id: String,
    pub prompt_id: String,
    pub response: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningGetArtifactParams {
    pub session_id: String,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningSessionSummary {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: u64,
    pub iterations: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningSessionDetail {
    pub id: String,
    pub prompt: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub conversation: Vec<FrontendConversationEntry>,
    pub artifacts: Vec<String>,
    pub message_count: u64,
    pub iterations: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningSessionRecord {
    pub id: String,
    pub prompt: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub iterations: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRecord {
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionMetadata {
    id: String,
    prompt: String,
    status: String,
    created_at: String,
    updated_at: String,
    iterations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationEntry {
    #[serde(rename = "type")]
    entry_type: String,
    id: String,
    text: String,
    ts: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendConversationEntry {
    #[serde(rename = "type")]
    entry_type: String,
    id: String,
    content: String,
    timestamp: String,
}

pub struct PlanningDomain {
    sessions_dir: PathBuf,
}

const MAX_SESSION_ID_LEN: usize = 120;

impl PlanningDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            sessions_dir: workspace_root.as_ref().join(".ralph/planning-sessions"),
        }
    }

    pub fn list(&mut self) -> Result<Vec<PlanningSessionSummary>, ApiError> {
        self.ensure_sessions_dir()?;

        let entries = fs::read_dir(&self.sessions_dir).map_err(|error| {
            ApiError::internal(format!(
                "failed reading planning sessions directory '{}': {error}",
                self.sessions_dir.display()
            ))
        })?;

        let mut sessions = Vec::new();

        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let Some(session_id) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            let Ok(metadata) = self.read_metadata(session_id) else {
                warn!(session_id, "skipping malformed planning session metadata");
                continue;
            };

            let message_count = self.count_messages(session_id);
            sessions.push(PlanningSessionSummary {
                id: metadata.id.clone(),
                title: generate_title(&metadata.prompt),
                prompt: metadata.prompt.clone(),
                status: to_frontend_status(&metadata.status),
                created_at: metadata.created_at.clone(),
                updated_at: metadata.updated_at.clone(),
                message_count,
                iterations: metadata.iterations,
            });
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
        Ok(sessions)
    }

    pub fn get(&self, session_id: &str) -> Result<PlanningSessionDetail, ApiError> {
        validate_session_id(session_id)?;

        let metadata = self.read_metadata(session_id)?;
        let conversation = self.read_conversation(session_id);
        let artifacts = self.read_artifacts(session_id);

        let completed_at = (metadata.status == "completed").then_some(metadata.updated_at.clone());

        Ok(PlanningSessionDetail {
            id: metadata.id,
            prompt: metadata.prompt.clone(),
            title: generate_title(&metadata.prompt),
            status: to_frontend_status(&metadata.status),
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
            completed_at,
            conversation: conversation.clone(),
            artifacts,
            message_count: u64::try_from(conversation.len()).unwrap_or(u64::MAX),
            iterations: metadata.iterations,
        })
    }

    pub fn start(
        &mut self,
        params: PlanningStartParams,
    ) -> Result<PlanningSessionRecord, ApiError> {
        self.ensure_sessions_dir()?;

        let (session_id, session_dir) = self.create_unique_session_dir()?;

        fs::create_dir_all(session_dir.join("artifacts")).map_err(|error| {
            ApiError::internal(format!(
                "failed creating planning session directory '{}': {error}",
                session_dir.display()
            ))
        })?;

        let now = now_ts();
        let metadata = SessionMetadata {
            id: session_id.clone(),
            prompt: params.prompt,
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now,
            iterations: 0,
        };

        self.write_metadata(&metadata)?;
        self.write_empty_conversation(&session_id)?;

        Ok(PlanningSessionRecord {
            id: metadata.id,
            prompt: metadata.prompt,
            status: metadata.status,
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
            iterations: metadata.iterations,
        })
    }

    pub fn respond(&mut self, params: PlanningRespondParams) -> Result<(), ApiError> {
        validate_session_id(&params.session_id)?;

        let mut metadata = self.read_metadata(&params.session_id)?;

        let entry = ConversationEntry {
            entry_type: "user_response".to_string(),
            id: params.prompt_id,
            text: params.response,
            ts: now_ts(),
        };
        self.append_conversation(&params.session_id, &entry)?;

        metadata.status = "active".to_string();
        metadata.updated_at = now_ts();
        self.write_metadata(&metadata)
    }

    pub fn resume(&mut self, session_id: &str) -> Result<(), ApiError> {
        validate_session_id(session_id)?;

        let mut metadata = self.read_metadata(session_id)?;
        metadata.status = "active".to_string();
        metadata.updated_at = now_ts();
        self.write_metadata(&metadata)
    }

    pub fn delete(&mut self, session_id: &str) -> Result<(), ApiError> {
        validate_session_id(session_id)?;

        let session_dir = self.session_dir(session_id);
        if !session_dir.exists() {
            return Err(planning_session_not_found_error(session_id));
        }

        fs::remove_dir_all(&session_dir).map_err(|error| {
            ApiError::internal(format!(
                "failed deleting planning session '{}': {error}",
                session_dir.display()
            ))
        })
    }

    pub fn get_artifact(
        &self,
        params: PlanningGetArtifactParams,
    ) -> Result<ArtifactRecord, ApiError> {
        validate_session_id(&params.session_id)?;

        if is_invalid_filename(&params.filename) {
            return Err(ApiError::invalid_params(
                "planning.get_artifact filename must be a plain file name",
            ));
        }

        // Keep get/list contract consistent: if a filename would not appear in
        // `planning.get` artifact listings, reject direct access as not found.
        if !is_listed_artifact_name(&params.filename) {
            return Err(ApiError::not_found(format!(
                "artifact '{}' not found for planning session '{}'",
                params.filename, params.session_id
            )));
        }

        let session_dir = self.session_dir(&params.session_id);
        if !session_dir.exists() {
            return Err(planning_session_not_found_error(&params.session_id));
        }

        let artifact_path = session_dir.join("artifacts").join(&params.filename);

        // Use symlink_metadata so we inspect the path entry itself, not any
        // target it may point to.  A symlink (or directory, device node, …)
        // must be treated the same as "not found" so the API leaks nothing.
        let fmeta = fs::symlink_metadata(&artifact_path).map_err(|_| {
            ApiError::not_found(format!(
                "artifact '{}' not found for planning session '{}'",
                params.filename, params.session_id
            ))
        })?;
        if !fmeta.is_file() {
            return Err(ApiError::not_found(format!(
                "artifact '{}' not found for planning session '{}'",
                params.filename, params.session_id
            )));
        }

        let content = fs::read_to_string(&artifact_path).map_err(|error| {
            ApiError::not_found(format!(
                "artifact '{}' not found for planning session '{}': {error}",
                params.filename, params.session_id
            ))
        })?;

        Ok(ArtifactRecord {
            filename: params.filename,
            content,
        })
    }

    fn next_session_id(&self) -> String {
        format!(
            "{}-{}",
            Utc::now().format("%Y%m%dT%H%M%S"),
            uuid::Uuid::new_v4().simple()
        )
    }

    fn create_unique_session_dir(&self) -> Result<(String, PathBuf), ApiError> {
        for _ in 0..8 {
            let session_id = self.next_session_id();
            let session_dir = self.session_dir(&session_id);

            match fs::create_dir(&session_dir) {
                Ok(()) => return Ok((session_id, session_dir)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(ApiError::internal(format!(
                        "failed creating planning session directory '{}': {error}",
                        session_dir.display()
                    )));
                }
            }
        }

        Err(ApiError::internal(
            "failed allocating unique planning session id after multiple attempts",
        ))
    }

    fn ensure_sessions_dir(&self) -> Result<(), ApiError> {
        fs::create_dir_all(&self.sessions_dir).map_err(|error| {
            ApiError::internal(format!(
                "failed creating planning sessions directory '{}': {error}",
                self.sessions_dir.display()
            ))
        })
    }

    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(session_id)
    }

    fn metadata_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("session.json")
    }

    fn conversation_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("conversation.jsonl")
    }

    fn read_metadata(&self, session_id: &str) -> Result<SessionMetadata, ApiError> {
        validate_session_id(session_id)?;

        let path = self.metadata_path(session_id);

        let content =
            fs::read_to_string(&path).map_err(|_| planning_session_not_found_error(session_id))?;

        serde_json::from_str::<SessionMetadata>(&content).map_err(|error| {
            ApiError::internal(format!(
                "failed parsing planning metadata '{}': {error}",
                path.display()
            ))
        })
    }

    fn write_metadata(&self, metadata: &SessionMetadata) -> Result<(), ApiError> {
        let path = self.metadata_path(&metadata.id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed creating planning metadata directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let payload = serde_json::to_string_pretty(metadata).map_err(|error| {
            ApiError::internal(format!("failed serializing planning metadata: {error}"))
        })?;

        fs::write(&path, payload).map_err(|error| {
            ApiError::internal(format!(
                "failed writing planning metadata '{}': {error}",
                path.display()
            ))
        })
    }

    fn write_empty_conversation(&self, session_id: &str) -> Result<(), ApiError> {
        let path = self.conversation_path(session_id);
        fs::write(&path, "").map_err(|error| {
            ApiError::internal(format!(
                "failed creating planning conversation '{}': {error}",
                path.display()
            ))
        })
    }

    fn append_conversation(
        &self,
        session_id: &str,
        entry: &ConversationEntry,
    ) -> Result<(), ApiError> {
        let path = self.conversation_path(session_id);
        let mut payload = serde_json::to_string(entry).map_err(|error| {
            ApiError::internal(format!("failed serializing conversation entry: {error}"))
        })?;
        payload.push('\n');

        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| {
                ApiError::internal(format!(
                    "failed opening planning conversation '{}': {error}",
                    path.display()
                ))
            })?;

        file.write_all(payload.as_bytes()).map_err(|error| {
            ApiError::internal(format!(
                "failed appending planning conversation '{}': {error}",
                path.display()
            ))
        })
    }

    fn read_conversation(&self, session_id: &str) -> Vec<FrontendConversationEntry> {
        let path = self.conversation_path(session_id);
        let Ok(content) = fs::read_to_string(path) else {
            return Vec::new();
        };

        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<ConversationEntry>(line).ok())
            .map(|entry| FrontendConversationEntry {
                entry_type: if entry.entry_type == "user_prompt" {
                    "prompt".to_string()
                } else {
                    "response".to_string()
                },
                id: entry.id,
                content: entry.text,
                timestamp: entry.ts,
            })
            .collect()
    }

    fn count_messages(&self, session_id: &str) -> u64 {
        let path = self.conversation_path(session_id);
        let Ok(content) = fs::read_to_string(path) else {
            return 0;
        };

        u64::try_from(
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
        )
        .unwrap_or(u64::MAX)
    }

    fn read_artifacts(&self, session_id: &str) -> Vec<String> {
        let artifacts_dir = self.session_dir(session_id).join("artifacts");
        let Ok(entries) = fs::read_dir(artifacts_dir) else {
            return Vec::new();
        };

        let mut artifacts: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                // file_type() does NOT follow symlinks, so symlinks return
                // is_symlink()=true / is_file()=false and are excluded here.
                let ftype = entry.file_type().ok()?;
                if !ftype.is_file() {
                    return None;
                }
                entry
                    .file_name()
                    .to_str()
                    .map(std::string::ToString::to_string)
            })
            .filter(|name| is_listed_artifact_name(name))
            .collect();
        artifacts.sort();
        artifacts
    }
}

fn validate_session_id(session_id: &str) -> Result<(), ApiError> {
    if session_id.is_empty() || session_id.len() > MAX_SESSION_ID_LEN {
        return Err(ApiError::invalid_params(format!(
            "planning session id must be 1..={MAX_SESSION_ID_LEN} characters"
        ))
        .with_details(serde_json::json!({ "sessionId": session_id })));
    }

    if !session_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(ApiError::invalid_params(
            "planning session id may only contain ASCII letters, digits, '-' or '_'",
        )
        .with_details(serde_json::json!({ "sessionId": session_id })));
    }

    Ok(())
}

fn is_invalid_filename(filename: &str) -> bool {
    let mut components = Path::new(filename).components();

    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) => name.to_string_lossy().is_empty(),
        _ => true,
    }
}

fn is_listed_artifact_name(filename: &str) -> bool {
    !filename.starts_with('.')
        && filename.len() <= 255
        && !filename.is_empty()
        && filename
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        && !is_invalid_filename(filename)
}

fn planning_session_not_found_error(session_id: &str) -> ApiError {
    ApiError::planning_session_not_found(format!("Planning session '{session_id}' not found"))
        .with_details(serde_json::json!({ "sessionId": session_id }))
}

fn to_frontend_status(status: &str) -> String {
    if status == "waiting_for_input" {
        return "paused".to_string();
    }

    status.to_string()
}

fn generate_title(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.chars().count() <= 60 {
        return trimmed.to_string();
    }

    let mut shortened: String = trimmed.chars().take(57).collect();
    shortened.push_str("...");
    shortened
}

fn now_ts() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}
