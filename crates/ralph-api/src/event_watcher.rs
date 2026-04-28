//! Polls the active `.ralph/events-*.jsonl` file for new event records and
//! publishes them to the stream domain as `loop.orchestration` events.
//!
//! The watcher runs as an independent tokio task spawned at server startup.
//! It has no interaction with the RPC mutex or any other shared state beyond
//! the [`StreamDomain`] publish interface.

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use ralph_core::EventRecord;
use serde_json::json;
use tracing::{debug, trace, warn};

use crate::stream_domain::StreamDomain;

/// How often the watcher checks for new lines.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Spawns the event-file watcher as a background tokio task.
///
/// The task runs until the provided `shutdown` future resolves (typically
/// wired to the server's graceful-shutdown signal).
pub fn spawn_watcher(
    workspace_root: PathBuf,
    streams: StreamDomain,
    shutdown: tokio::sync::watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        run_watcher(workspace_root, streams, shutdown).await;
    })
}

async fn run_watcher(
    workspace_root: PathBuf,
    streams: StreamDomain,
    mut shutdown: tokio::sync::watch::Receiver<()>,
) {
    let marker_path = workspace_root.join(".ralph/current-events");
    let mut current_file: Option<PathBuf> = None;
    let mut offset: u64 = 0;

    debug!(
        "event watcher started, polling every {}ms",
        POLL_INTERVAL.as_millis()
    );

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                debug!("event watcher shutting down");
                return;
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {
                poll_once(&marker_path, &workspace_root, &streams, &mut current_file, &mut offset);
            }
        }
    }
}

fn poll_once(
    marker_path: &Path,
    workspace_root: &Path,
    streams: &StreamDomain,
    current_file: &mut Option<PathBuf>,
    offset: &mut u64,
) {
    // Resolve the active events file from the marker.
    let active_path = match resolve_active_events_path(marker_path, workspace_root) {
        Some(path) => path,
        None => {
            trace!("no active events file (marker missing or unreadable)");
            return;
        }
    };

    // Detect file switch (new loop started).
    if current_file.as_ref() != Some(&active_path) {
        debug!(path = %active_path.display(), "switching to new events file");
        *current_file = Some(active_path.clone());
        *offset = 0;
    }

    // Read new lines from the current offset.
    let new_lines = match read_new_lines(&active_path, offset) {
        Ok(lines) => lines,
        Err(err) => {
            if err.kind() != io::ErrorKind::NotFound {
                warn!(error = %err, path = %active_path.display(), "failed reading events file");
            }
            return;
        }
    };

    for line in new_lines {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<EventRecord>(&line) {
            Ok(record) => publish_record(streams, &record),
            Err(err) => {
                trace!(error = %err, "skipping unparseable event line");
            }
        }
    }
}

fn resolve_active_events_path(marker_path: &Path, workspace_root: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(marker_path).ok()?;
    let relative = content.trim();
    if relative.is_empty() {
        return None;
    }
    Some(workspace_root.join(relative))
}

/// Reads all complete lines from `offset` to EOF, advancing `offset`.
fn read_new_lines(path: &Path, offset: &mut u64) -> io::Result<Vec<String>> {
    let metadata = fs::metadata(path)?;
    let file_len = metadata.len();

    // File was recreated or truncated (shouldn't happen with per-run files,
    // but handle defensively).
    if file_len < *offset {
        *offset = 0;
    }

    if file_len == *offset {
        return Ok(Vec::new());
    }

    // Read the new bytes.
    let content = fs::read_to_string(path)?;
    let new_content = if (*offset as usize) < content.len() {
        &content[(*offset as usize)..]
    } else {
        ""
    };

    let mut lines = Vec::new();
    for line in new_content.lines() {
        lines.push(line.to_string());
    }

    *offset = file_len;
    Ok(lines)
}

fn publish_record(streams: &StreamDomain, record: &EventRecord) {
    // Skip internal/meta events that aren't useful for the UI.
    if record.topic.is_empty() {
        return;
    }

    streams.publish(
        "loop.orchestration",
        "loop",
        &record.hat,
        json!({
            "iteration": record.iteration,
            "hat": record.hat,
            "topic": record.topic,
            "triggered": record.triggered,
            "ts": record.ts,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_marker(dir: &Path, events_file: &str) {
        let marker = dir.join(".ralph/current-events");
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, events_file).unwrap();
    }

    fn write_event_line(dir: &Path, file: &str, record: &EventRecord) {
        let path = dir.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        let mut json = serde_json::to_string(record).unwrap();
        json.push('\n');
        f.write_all(json.as_bytes()).unwrap();
    }

    fn sample_record(iteration: u32, hat: &str, topic: &str) -> EventRecord {
        EventRecord {
            ts: "2026-04-23T12:00:00Z".to_string(),
            iteration,
            hat: hat.to_string(),
            topic: topic.to_string(),
            triggered: Some("reviewer".to_string()),
            payload: String::new(),
            blocked_count: None,
            wave_id: None,
            wave_index: None,
            wave_total: None,
        }
    }

    #[test]
    fn read_new_lines_returns_appended_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");
        fs::write(&path, "line1\nline2\n").unwrap();

        let mut offset = 0u64;
        let lines = read_new_lines(&path, &mut offset).unwrap();
        assert_eq!(lines, vec!["line1", "line2"]);
        assert_eq!(offset, 12); // "line1\nline2\n" = 12 bytes

        // Append more
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"line3\n").unwrap();

        let lines = read_new_lines(&path, &mut offset).unwrap();
        assert_eq!(lines, vec!["line3"]);
    }

    #[test]
    fn read_new_lines_handles_missing_file() {
        let mut offset = 0u64;
        let result = read_new_lines(Path::new("/nonexistent/events.jsonl"), &mut offset);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn resolve_active_events_path_reads_marker() {
        let tmp = TempDir::new().unwrap();
        write_marker(tmp.path(), ".ralph/events-20260423.jsonl");

        let result =
            resolve_active_events_path(&tmp.path().join(".ralph/current-events"), tmp.path());
        assert_eq!(
            result,
            Some(tmp.path().join(".ralph/events-20260423.jsonl"))
        );
    }

    #[test]
    fn resolve_active_events_path_returns_none_when_missing() {
        let result = resolve_active_events_path(
            Path::new("/nonexistent/.ralph/current-events"),
            Path::new("/nonexistent"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn poll_once_detects_file_switch() {
        let tmp = TempDir::new().unwrap();
        let marker_path = tmp.path().join(".ralph/current-events");

        // First file
        write_marker(tmp.path(), ".ralph/events-a.jsonl");
        write_event_line(
            tmp.path(),
            ".ralph/events-a.jsonl",
            &sample_record(1, "planner", "build.task"),
        );

        let streams = StreamDomain::new();
        let mut current_file: Option<PathBuf> = None;
        let mut offset = 0u64;

        poll_once(
            &marker_path,
            tmp.path(),
            &streams,
            &mut current_file,
            &mut offset,
        );
        assert_eq!(current_file, Some(tmp.path().join(".ralph/events-a.jsonl")));
        assert!(offset > 0);

        // Switch to second file
        write_marker(tmp.path(), ".ralph/events-b.jsonl");
        write_event_line(
            tmp.path(),
            ".ralph/events-b.jsonl",
            &sample_record(1, "builder", "build.done"),
        );

        poll_once(
            &marker_path,
            tmp.path(),
            &streams,
            &mut current_file,
            &mut offset,
        );
        assert_eq!(current_file, Some(tmp.path().join(".ralph/events-b.jsonl")));
    }
}
