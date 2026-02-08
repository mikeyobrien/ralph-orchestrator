//! Merge Queue API endpoints.
//!
//! GET /api/merge-queue - Get merge queue status

use actix_web::{HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use super::sessions::ErrorResponse;

/// Response format for merge queue.
#[derive(Debug, Serialize)]
pub struct MergeQueueResponse {
    pub pending: Vec<MergeQueueItem>,
    pub completed: Vec<MergeQueueItem>,
}

/// Single merge queue item.
#[derive(Debug, Serialize)]
pub struct MergeQueueItem {
    pub id: String,
    pub status: String, // "pending" | "completed" | "failed"
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    pub queued_at: String, // ISO 8601 format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
}

/// Event from merge-queue.jsonl.
#[derive(Debug, Deserialize)]
struct MergeEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    worktree_path: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// GET /api/merge-queue - Get merge queue status.
///
/// Reads .ralph/merge-queue.jsonl and returns pending and completed items.
pub async fn get_merge_queue() -> impl Responder {
    let merge_queue_path = find_merge_queue_file();

    if !merge_queue_path.exists() {
        // Return empty queue if file doesn't exist
        return HttpResponse::Ok().json(MergeQueueResponse {
            pending: vec![],
            completed: vec![],
        });
    }

    match parse_merge_queue(&merge_queue_path) {
        Ok((pending, completed)) => HttpResponse::Ok().json(MergeQueueResponse {
            pending,
            completed,
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("failed_to_parse_merge_queue: {}", e),
        }),
    }
}

/// Find the merge queue file.
///
/// Looks in:
/// 1. .ralph/merge-queue.jsonl (current directory)
/// 2. ~/.ralph/merge-queue.jsonl (home directory fallback)
fn find_merge_queue_file() -> PathBuf {
    let local_path = PathBuf::from(".ralph/merge-queue.jsonl");
    if local_path.exists() {
        return local_path;
    }

    // Fallback to home directory
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".ralph/merge-queue.jsonl")
    } else {
        local_path
    }
}

/// Parse merge queue file.
///
/// Event types:
/// - loop.queued: Loop queued for merge
/// - loop.merged: Loop successfully merged
/// - loop.merge_failed: Loop merge failed
fn parse_merge_queue(path: &PathBuf) -> Result<(Vec<MergeQueueItem>, Vec<MergeQueueItem>), std::io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut items: std::collections::HashMap<String, MergeQueueItem> = std::collections::HashMap::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: MergeEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed lines
        };

        let id = match event.id {
            Some(id) => id,
            None => continue,
        };

        match event.event_type.as_str() {
            "loop.queued" => {
                items.insert(
                    id.clone(),
                    MergeQueueItem {
                        id: id.clone(),
                        status: "pending".to_string(),
                        prompt: event.prompt.unwrap_or_default(),
                        worktree_path: event.worktree_path,
                        queued_at: event.timestamp.unwrap_or_default(),
                        merged_at: None,
                    },
                );
            }
            "loop.merged" => {
                if let Some(item) = items.get_mut(&id) {
                    item.status = "completed".to_string();
                    item.merged_at = event.timestamp;
                }
            }
            "loop.merge_failed" => {
                if let Some(item) = items.get_mut(&id) {
                    item.status = "failed".to_string();
                    item.merged_at = event.timestamp;
                }
            }
            _ => {}
        }
    }

    // Split into pending and completed
    let mut pending = Vec::new();
    let mut completed = Vec::new();

    for item in items.into_values() {
        if item.status == "pending" {
            pending.push(item);
        } else {
            completed.push(item);
        }
    }

    // Sort by queued_at (most recent first)
    pending.sort_by(|a, b| b.queued_at.cmp(&a.queued_at));
    completed.sort_by(|a, b| {
        b.merged_at
            .as_ref()
            .unwrap_or(&b.queued_at)
            .cmp(a.merged_at.as_ref().unwrap_or(&a.queued_at))
    });

    Ok((pending, completed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_merge_queue_empty() {
        let tmp = TempDir::new().unwrap();
        let queue_file = tmp.path().join("merge-queue.jsonl");

        std::fs::write(&queue_file, "").unwrap();

        let (pending, completed) = parse_merge_queue(&queue_file).unwrap();

        assert_eq!(pending.len(), 0);
        assert_eq!(completed.len(), 0);
    }

    #[test]
    fn test_parse_merge_queue_pending() {
        let tmp = TempDir::new().unwrap();
        let queue_file = tmp.path().join("merge-queue.jsonl");

        let data = r#"{"type":"loop.queued","id":"loop-1","prompt":"test prompt","worktree_path":"/worktree/loop-1","timestamp":"2024-01-01T00:00:00Z"}
{"type":"loop.queued","id":"loop-2","prompt":"another prompt","timestamp":"2024-01-01T00:01:00Z"}
"#;

        std::fs::write(&queue_file, data).unwrap();

        let (pending, completed) = parse_merge_queue(&queue_file).unwrap();

        assert_eq!(pending.len(), 2);
        assert_eq!(completed.len(), 0);

        // Check first item (most recent first)
        assert_eq!(pending[0].id, "loop-2");
        assert_eq!(pending[0].status, "pending");
        assert_eq!(pending[0].prompt, "another prompt");

        // Check second item
        assert_eq!(pending[1].id, "loop-1");
        assert_eq!(pending[1].worktree_path, Some("/worktree/loop-1".to_string()));
    }

    #[test]
    fn test_parse_merge_queue_completed() {
        let tmp = TempDir::new().unwrap();
        let queue_file = tmp.path().join("merge-queue.jsonl");

        let data = r#"{"type":"loop.queued","id":"loop-1","prompt":"test prompt","timestamp":"2024-01-01T00:00:00Z"}
{"type":"loop.merged","id":"loop-1","timestamp":"2024-01-01T00:05:00Z"}
"#;

        std::fs::write(&queue_file, data).unwrap();

        let (pending, completed) = parse_merge_queue(&queue_file).unwrap();

        assert_eq!(pending.len(), 0);
        assert_eq!(completed.len(), 1);

        assert_eq!(completed[0].id, "loop-1");
        assert_eq!(completed[0].status, "completed");
        assert_eq!(completed[0].merged_at, Some("2024-01-01T00:05:00Z".to_string()));
    }

    #[test]
    fn test_parse_merge_queue_failed() {
        let tmp = TempDir::new().unwrap();
        let queue_file = tmp.path().join("merge-queue.jsonl");

        let data = r#"{"type":"loop.queued","id":"loop-1","prompt":"test prompt","timestamp":"2024-01-01T00:00:00Z"}
{"type":"loop.merge_failed","id":"loop-1","timestamp":"2024-01-01T00:05:00Z"}
"#;

        std::fs::write(&queue_file, data).unwrap();

        let (pending, completed) = parse_merge_queue(&queue_file).unwrap();

        assert_eq!(pending.len(), 0);
        assert_eq!(completed.len(), 1);

        assert_eq!(completed[0].status, "failed");
    }

    #[test]
    fn test_parse_merge_queue_mixed() {
        let tmp = TempDir::new().unwrap();
        let queue_file = tmp.path().join("merge-queue.jsonl");

        let data = r#"{"type":"loop.queued","id":"loop-1","prompt":"prompt 1","timestamp":"2024-01-01T00:00:00Z"}
{"type":"loop.queued","id":"loop-2","prompt":"prompt 2","timestamp":"2024-01-01T00:01:00Z"}
{"type":"loop.merged","id":"loop-1","timestamp":"2024-01-01T00:05:00Z"}
{"type":"loop.queued","id":"loop-3","prompt":"prompt 3","timestamp":"2024-01-01T00:06:00Z"}
"#;

        std::fs::write(&queue_file, data).unwrap();

        let (pending, completed) = parse_merge_queue(&queue_file).unwrap();

        assert_eq!(pending.len(), 2);
        assert_eq!(completed.len(), 1);

        // Pending: loop-2 and loop-3 (sorted by queued_at, most recent first)
        assert_eq!(pending[0].id, "loop-3");
        assert_eq!(pending[1].id, "loop-2");

        // Completed: loop-1
        assert_eq!(completed[0].id, "loop-1");
    }

    #[test]
    fn test_parse_merge_queue_with_noise() {
        let tmp = TempDir::new().unwrap();
        let queue_file = tmp.path().join("merge-queue.jsonl");

        let data = r#"{"type":"loop.queued","id":"loop-1","prompt":"test","timestamp":"2024-01-01T00:00:00Z"}
{"type":"other.event","data":"noise"}
invalid json line
{"type":"loop.merged","id":"loop-1","timestamp":"2024-01-01T00:05:00Z"}
"#;

        std::fs::write(&queue_file, data).unwrap();

        let (pending, completed) = parse_merge_queue(&queue_file).unwrap();

        assert_eq!(pending.len(), 0);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "loop-1");
    }

    #[test]
    fn test_merge_queue_item_serialization() {
        let item = MergeQueueItem {
            id: "loop-1".to_string(),
            status: "pending".to_string(),
            prompt: "test prompt".to_string(),
            worktree_path: Some("/worktree/loop-1".to_string()),
            queued_at: "2024-01-01T00:00:00Z".to_string(),
            merged_at: None,
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"id\":\"loop-1\""));
        assert!(json.contains("\"status\":\"pending\""));
        assert!(json.contains("\"worktree_path\":\"/worktree/loop-1\""));
        // merged_at should be omitted when None
        assert!(!json.contains("merged_at"));
    }

    #[test]
    fn test_merge_queue_response_structure() {
        let response = MergeQueueResponse {
            pending: vec![],
            completed: vec![],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"pending\":[]"));
        assert!(json.contains("\"completed\":[]"));
    }
}
