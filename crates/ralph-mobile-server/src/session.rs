//! Session discovery for Ralph Mobile Server.
//!
//! Discovers active Ralph sessions by scanning for `.agent/` directories
//! and parsing scratchpad frontmatter for metadata.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Represents a discovered Ralph session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Deterministic ID from path hash
    pub id: String,
    /// Path to the .agent/ directory
    pub path: PathBuf,
    /// Task name from scratchpad frontmatter
    pub task_name: Option<String>,
    /// Current iteration number
    pub iteration: u32,
    /// Current hat (e.g., "builder", "architect")
    pub hat: Option<String>,
    /// When the session was discovered
    pub started_at: DateTime<Utc>,
    /// Timestamp of last event
    pub last_event_at: Option<DateTime<Utc>>,
}

/// Generate a deterministic session ID from a path.
pub fn session_id_from_path(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Parse YAML frontmatter from scratchpad.md content.
fn parse_frontmatter(content: &str) -> Option<FrontmatterData> {
    let lines: Vec<&str> = content.lines().collect();

    // Must start with ---
    if lines.first() != Some(&"---") {
        return None;
    }

    // Find closing ---
    let end_idx = lines.iter().skip(1).position(|&l| l == "---")?;
    let yaml_content: String = lines[1..=end_idx].join("\n");

    // Parse relevant fields
    let mut task_name = None;
    let mut current_hat = None;

    for line in yaml_content.lines() {
        if let Some(value) = line.strip_prefix("task_name:") {
            task_name = Some(value.trim().to_string());
        }
        if let Some(value) = line.strip_prefix("current_hat:") {
            current_hat = Some(value.trim().to_string());
        }
    }

    Some(FrontmatterData {
        task_name,
        current_hat,
    })
}

#[derive(Debug)]
struct FrontmatterData {
    task_name: Option<String>,
    current_hat: Option<String>,
}

/// Discover Ralph sessions by scanning for .agent/ and .ralph/ directories.
///
/// Searches the given root directory (non-recursively for direct children,
/// then checks each for .agent/ or .ralph/ subdirectory).
pub fn discover_sessions(root: &Path) -> Vec<Session> {
    let mut sessions = Vec::new();

    // Check if root itself has .agent/ or .ralph/
    let agent_dir = root.join(".agent");
    if let Some(session) = check_agent_dir(&agent_dir) {
        sessions.push(session);
    }

    let ralph_dir = root.join(".ralph");
    if let Some(session) = check_ralph_dir(&ralph_dir) {
        sessions.push(session);
    }

    // Check immediate subdirectories for .agent/ or .ralph/
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                let agent_dir = path.join(".agent");
                if let Some(session) = check_agent_dir(&agent_dir) {
                    sessions.push(session);
                }

                let ralph_dir = path.join(".ralph");
                if let Some(session) = check_ralph_dir(&ralph_dir) {
                    sessions.push(session);
                }
            }
        }
    }

    debug!("Discovered {} sessions in {:?}", sessions.len(), root);
    sessions
}

/// Check if a directory is a valid .agent/ directory and create a Session if so.
fn check_agent_dir(agent_dir: &Path) -> Option<Session> {
    if agent_dir.is_dir() && agent_dir.join("events.jsonl").exists() {
        session_from_agent_dir(agent_dir)
    } else {
        None
    }
}

/// Create a Session from an .agent/ directory.
fn session_from_agent_dir(agent_dir: &Path) -> Option<Session> {
    let scratchpad_path = agent_dir.join("scratchpad.md");

    let (task_name, hat) = if scratchpad_path.exists() {
        match fs::read_to_string(&scratchpad_path) {
            Ok(content) => {
                if let Some(fm) = parse_frontmatter(&content) {
                    (fm.task_name, fm.current_hat)
                } else {
                    (None, None)
                }
            }
            Err(e) => {
                warn!("Failed to read scratchpad at {:?}: {}", scratchpad_path, e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    Some(Session {
        id: session_id_from_path(agent_dir),
        path: agent_dir.to_path_buf(),
        task_name,
        iteration: 0, // Will be updated from events
        hat,
        started_at: Utc::now(),
        last_event_at: None,
    })
}

/// Check if a directory is a valid .ralph/ directory and create a Session if so.
fn check_ralph_dir(ralph_dir: &Path) -> Option<Session> {
    if !ralph_dir.is_dir() {
        return None;
    }

    // Check for events-*.jsonl files
    let has_events = fs::read_dir(ralph_dir)
        .ok()?
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("events-")
                && entry.file_name().to_string_lossy().ends_with(".jsonl")
        });

    if !has_events {
        return None;
    }

    // Try to read scratchpad from .ralph/agent/scratchpad.md
    let scratchpad_path = ralph_dir.join("agent").join("scratchpad.md");
    let (task_name, hat) = if scratchpad_path.exists() {
        match fs::read_to_string(&scratchpad_path) {
            Ok(content) => {
                if let Some(fm) = parse_frontmatter(&content) {
                    (fm.task_name, fm.current_hat)
                } else {
                    (None, None)
                }
            }
            Err(e) => {
                warn!("Failed to read scratchpad at {:?}: {}", scratchpad_path, e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    Some(Session {
        id: session_id_from_path(ralph_dir),
        path: ralph_dir.to_path_buf(),
        task_name,
        iteration: 0, // Will be updated from events
        hat,
        started_at: Utc::now(),
        last_event_at: None,
    })
}

/// Resolve the actual events file path for a session.
///
/// This function follows the `current-events` pointer if it exists,
/// otherwise falls back to static paths.
///
/// Resolution order:
/// 1. Read `current-events` pointer and resolve relative path
/// 2. Fall back to `events.jsonl` in session path
/// 3. Fall back to latest `events-*.jsonl` by filename
pub fn resolve_events_path(session_path: &Path) -> Option<PathBuf> {
    // Try current-events pointer
    let pointer_path = session_path.join("current-events");
    if pointer_path.exists() {
        if let Ok(relative) = fs::read_to_string(&pointer_path) {
            let relative = relative.trim();
            // Resolve from project root (parent of session_path)
            let project_root = session_path.parent().unwrap_or(session_path);
            let full_path = project_root.join(relative);
            if full_path.exists() {
                debug!("Resolved events path via pointer: {:?}", full_path);
                return Some(full_path);
            }
        }
    }

    // Fall back to static events.jsonl
    let static_path = session_path.join("events.jsonl");
    if static_path.exists() {
        debug!("Using static events.jsonl: {:?}", static_path);
        return Some(static_path);
    }

    // Fall back to latest events-*.jsonl
    if let Ok(entries) = fs::read_dir(session_path) {
        let mut event_files: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.starts_with("events-") && name.ends_with(".jsonl")
            })
            .collect();

        // Sort by filename (lexicographic = chronological for timestamp format)
        event_files.sort_by_key(|e| e.file_name());

        if let Some(latest) = event_files.last() {
            let path = latest.path();
            debug!("Using latest events file: {:?}", path);
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    fn create_agent_dir(parent: &Path) -> PathBuf {
        let agent_dir = parent.join(".agent");
        fs::create_dir_all(&agent_dir).unwrap();
        // Create events.jsonl (required for discovery)
        File::create(agent_dir.join("events.jsonl")).unwrap();
        agent_dir
    }

    fn create_scratchpad(agent_dir: &Path, content: &str) {
        let mut file = File::create(agent_dir.join("scratchpad.md")).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_discover_sessions_finds_agent_dirs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create two projects with .agent/ directories
        let proj1 = root.join("project1");
        let proj2 = root.join("project2");
        fs::create_dir_all(&proj1).unwrap();
        fs::create_dir_all(&proj2).unwrap();

        create_agent_dir(&proj1);
        create_agent_dir(&proj2);

        let sessions = discover_sessions(root);
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_parse_scratchpad_frontmatter() {
        let temp = TempDir::new().unwrap();
        let agent_dir = create_agent_dir(temp.path());

        create_scratchpad(
            &agent_dir,
            r#"---
task_name: my-task
current_hat: builder
status: building
---
# My Task
Some content here
"#,
        );

        let sessions = discover_sessions(temp.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].task_name, Some("my-task".to_string()));
        assert_eq!(sessions[0].hat, Some("builder".to_string()));
    }

    #[test]
    fn test_session_id_from_path_deterministic() {
        let path = Path::new("/some/path/.agent");

        let id1 = session_id_from_path(path);
        let id2 = session_id_from_path(path);

        assert_eq!(id1, id2);
        assert!(!id1.is_empty());
    }

    #[test]
    fn test_no_sessions_when_no_agent_dirs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create directories without .agent/
        fs::create_dir_all(root.join("project1")).unwrap();
        fs::create_dir_all(root.join("project2")).unwrap();

        let sessions = discover_sessions(root);
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_handles_missing_scratchpad() {
        let temp = TempDir::new().unwrap();
        let _agent_dir = create_agent_dir(temp.path());

        // No scratchpad.md created

        let sessions = discover_sessions(temp.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].task_name, None);
        assert_eq!(sessions[0].hat, None);
    }

    #[test]
    fn test_handles_malformed_frontmatter() {
        let temp = TempDir::new().unwrap();
        let agent_dir = create_agent_dir(temp.path());

        // Scratchpad without valid frontmatter
        create_scratchpad(&agent_dir, "# Just a header\nNo frontmatter here");

        let sessions = discover_sessions(temp.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].task_name, None);
    }
}
