//! Iterations API endpoints.
//!
//! GET /api/sessions/{id}/iterations - Get iteration history for a session

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use super::sessions::ErrorResponse;

/// Response format for iteration list.
#[derive(Debug, Serialize)]
pub struct IterationsResponse {
    pub iterations: Vec<IterationItem>,
    pub total: usize,
}

/// Single iteration item.
#[derive(Debug, Serialize)]
pub struct IterationItem {
    pub number: u32,
    pub hat: Option<String>,
    pub started_at: String, // ISO 8601 format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
}

/// Event from events.jsonl.
#[derive(Debug, Deserialize)]
struct Event {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    iteration: Option<u32>,
    #[serde(default)]
    hat: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
}

/// GET /api/sessions/{id}/iterations - Get iteration history.
///
/// Parses events.jsonl for iteration.started and iteration.completed events
/// to build a timeline of iterations with durations.
pub async fn get_iterations(path: web::Path<String>) -> impl Responder {
    let session_id = path.into_inner();

    // Find the events file for this session
    let events_path = match find_events_file(&session_id) {
        Some(path) => path,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: format!("session_not_found: {}", session_id),
            });
        }
    };

    // Parse events file
    let iterations = match parse_iterations(&events_path) {
        Ok(iterations) => iterations,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("failed_to_parse_events: {}", e),
            });
        }
    };

    let total = iterations.len();

    HttpResponse::Ok().json(IterationsResponse { iterations, total })
}

/// Find the events file for a session.
///
/// Searches in:
/// 1. .ralph/current-events (pointer to active events file)
/// 2. .ralph/events-*.jsonl (most recent file matching session ID)
/// 3. ~/.ralph/agent/events-*.jsonl (fallback for home-based sessions)
fn find_events_file(session_id: &str) -> Option<PathBuf> {
    // Check current directory .ralph/current-events
    let current_events = PathBuf::from(".ralph/current-events");
    if current_events.exists() {
        if let Ok(relative_path) = std::fs::read_to_string(&current_events) {
            let events_file = PathBuf::from(relative_path.trim());
            if events_file.exists() {
                return Some(events_file);
            }
        }
    }

    // Check for events files in .ralph directory
    if let Ok(entries) = std::fs::read_dir(".ralph") {
        let mut matching_files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name() {
                        if name.to_string_lossy().starts_with("events-")
                            && name.to_string_lossy().ends_with(".jsonl")
                        {
                            return Some(path);
                        }
                    }
                }
                None
            })
            .collect();

        // Sort by modified time (newest first)
        matching_files.sort_by(|a, b| {
            let a_time = std::fs::metadata(a).and_then(|m| m.modified()).ok();
            let b_time = std::fs::metadata(b).and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        if let Some(newest) = matching_files.first() {
            return Some(newest.clone());
        }
    }

    // Fallback to home directory
    if let Some(home) = std::env::var_os("HOME") {
        let home_events = PathBuf::from(home)
            .join(".ralph")
            .join("agent")
            .join(format!("events-{}.jsonl", session_id));
        if home_events.exists() {
            return Some(home_events);
        }
    }

    None
}

/// Parse iterations from events file.
///
/// Looks for:
/// - iteration.started: marks beginning of iteration with hat
/// - iteration.completed: marks end of iteration (calculates duration)
fn parse_iterations(events_path: &PathBuf) -> Result<Vec<IterationItem>, std::io::Error> {
    let file = File::open(events_path)?;
    let reader = BufReader::new(file);

    let mut iterations: Vec<IterationItem> = Vec::new();
    let mut current_iteration: Option<(u32, String, Option<String>)> = None; // (number, started_at, hat)

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: Event = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed lines
        };

        match event.event_type.as_str() {
            "iteration.started" => {
                if let (Some(number), Some(timestamp)) = (event.iteration, event.timestamp) {
                    // Close previous iteration if any
                    if let Some((prev_num, prev_start, prev_hat)) = current_iteration.take() {
                        iterations.push(IterationItem {
                            number: prev_num,
                            hat: prev_hat,
                            started_at: prev_start,
                            duration_secs: None, // No end event seen
                        });
                    }

                    // Start new iteration
                    current_iteration = Some((number, timestamp, event.hat.clone()));
                }
            }
            "iteration.completed" => {
                if let (Some(number), Some(timestamp)) = (event.iteration, event.timestamp) {
                    if let Some((iter_num, started_at, hat)) = current_iteration.take() {
                        if iter_num == number {
                            // Calculate duration
                            let duration_secs = calculate_duration(&started_at, &timestamp);
                            iterations.push(IterationItem {
                                number,
                                hat,
                                started_at,
                                duration_secs,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Close final iteration if still open
    if let Some((number, started_at, hat)) = current_iteration {
        iterations.push(IterationItem {
            number,
            hat,
            started_at,
            duration_secs: None,
        });
    }

    Ok(iterations)
}

/// Calculate duration between two ISO 8601 timestamps in seconds.
fn calculate_duration(start: &str, end: &str) -> Option<u64> {
    use chrono::{DateTime, Utc};

    let start_time: DateTime<Utc> = start.parse().ok()?;
    let end_time: DateTime<Utc> = end.parse().ok()?;

    let duration = end_time.signed_duration_since(start_time);
    Some(duration.num_seconds().max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_iterations_single() {
        let tmp = TempDir::new().unwrap();
        let events_file = tmp.path().join("events.jsonl");

        let events_data = r#"{"type":"iteration.started","iteration":1,"hat":"planner","timestamp":"2024-01-01T00:00:00Z"}
{"type":"iteration.completed","iteration":1,"timestamp":"2024-01-01T00:01:00Z"}
"#;

        std::fs::write(&events_file, events_data).unwrap();

        let iterations = parse_iterations(&events_file).unwrap();

        assert_eq!(iterations.len(), 1);
        assert_eq!(iterations[0].number, 1);
        assert_eq!(iterations[0].hat, Some("planner".to_string()));
        assert_eq!(iterations[0].duration_secs, Some(60));
    }

    #[test]
    fn test_parse_iterations_multiple() {
        let tmp = TempDir::new().unwrap();
        let events_file = tmp.path().join("events.jsonl");

        let events_data = r#"{"type":"iteration.started","iteration":1,"hat":"planner","timestamp":"2024-01-01T00:00:00Z"}
{"type":"iteration.completed","iteration":1,"timestamp":"2024-01-01T00:00:45Z"}
{"type":"iteration.started","iteration":2,"hat":"builder","timestamp":"2024-01-01T00:01:00Z"}
{"type":"iteration.completed","iteration":2,"timestamp":"2024-01-01T00:03:00Z"}
"#;

        std::fs::write(&events_file, events_data).unwrap();

        let iterations = parse_iterations(&events_file).unwrap();

        assert_eq!(iterations.len(), 2);

        // First iteration
        assert_eq!(iterations[0].number, 1);
        assert_eq!(iterations[0].hat, Some("planner".to_string()));
        assert_eq!(iterations[0].duration_secs, Some(45));

        // Second iteration
        assert_eq!(iterations[1].number, 2);
        assert_eq!(iterations[1].hat, Some("builder".to_string()));
        assert_eq!(iterations[1].duration_secs, Some(120));
    }

    #[test]
    fn test_parse_iterations_incomplete() {
        let tmp = TempDir::new().unwrap();
        let events_file = tmp.path().join("events.jsonl");

        let events_data = r#"{"type":"iteration.started","iteration":1,"hat":"planner","timestamp":"2024-01-01T00:00:00Z"}
{"type":"iteration.started","iteration":2,"hat":"builder","timestamp":"2024-01-01T00:01:00Z"}
"#;

        std::fs::write(&events_file, events_data).unwrap();

        let iterations = parse_iterations(&events_file).unwrap();

        assert_eq!(iterations.len(), 2);

        // First iteration closed when second started (no duration)
        assert_eq!(iterations[0].number, 1);
        assert_eq!(iterations[0].duration_secs, None);

        // Second iteration still open
        assert_eq!(iterations[1].number, 2);
        assert_eq!(iterations[1].duration_secs, None);
    }

    #[test]
    fn test_calculate_duration() {
        let start = "2024-01-01T00:00:00Z";
        let end = "2024-01-01T00:01:30Z";

        let duration = calculate_duration(start, end);
        assert_eq!(duration, Some(90));
    }

    #[test]
    fn test_calculate_duration_invalid() {
        let duration = calculate_duration("invalid", "2024-01-01T00:00:00Z");
        assert_eq!(duration, None);
    }

    #[test]
    fn test_parse_iterations_with_noise() {
        let tmp = TempDir::new().unwrap();
        let events_file = tmp.path().join("events.jsonl");

        let events_data = r#"{"type":"iteration.started","iteration":1,"hat":"planner","timestamp":"2024-01-01T00:00:00Z"}
{"type":"other.event","data":"noise"}
invalid json line
{"type":"iteration.completed","iteration":1,"timestamp":"2024-01-01T00:00:30Z"}
"#;

        std::fs::write(&events_file, events_data).unwrap();

        let iterations = parse_iterations(&events_file).unwrap();

        assert_eq!(iterations.len(), 1);
        assert_eq!(iterations[0].number, 1);
        assert_eq!(iterations[0].duration_secs, Some(30));
    }

    #[test]
    fn test_iteration_item_serialization() {
        let item = IterationItem {
            number: 1,
            hat: Some("planner".to_string()),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            duration_secs: Some(60),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"number\":1"));
        assert!(json.contains("\"hat\":\"planner\""));
        assert!(json.contains("\"duration_secs\":60"));
    }

    #[test]
    fn test_iteration_item_serialization_no_duration() {
        let item = IterationItem {
            number: 1,
            hat: Some("planner".to_string()),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            duration_secs: None,
        };

        let json = serde_json::to_string(&item).unwrap();
        // duration_secs should be omitted when None
        assert!(!json.contains("duration_secs"));
    }
}
