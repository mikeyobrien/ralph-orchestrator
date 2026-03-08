//! Wave CLI tool for dispatching parallel wave events.
//!
//! Provides `ralph wave emit` for agents to dispatch work items
//! to wave-capable hats that execute in parallel.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Arguments for the wave subcommand.
#[derive(Parser, Debug)]
pub struct WaveArgs {
    #[command(subcommand)]
    pub command: WaveCommands,
}

/// Wave subcommands.
#[derive(Subcommand, Debug)]
pub enum WaveCommands {
    /// Emit multiple events as a wave for parallel execution
    Emit(WaveEmitArgs),
}

/// Arguments for `ralph wave emit`.
#[derive(Parser, Debug)]
pub struct WaveEmitArgs {
    /// Event topic for all wave events (e.g., "review.file")
    pub topic: String,

    /// Payloads for each wave event instance (one per parallel worker)
    #[arg(long, num_args = 1..)]
    pub payloads: Vec<String>,
}

/// Execute a wave command.
pub fn execute(args: WaveArgs, use_colors: bool) -> Result<()> {
    match args.command {
        WaveCommands::Emit(emit_args) => execute_emit(emit_args, use_colors),
    }
}

/// Execute `ralph wave emit` — write N tagged events atomically.
fn execute_emit(args: WaveEmitArgs, use_colors: bool) -> Result<()> {
    // Nested wave prevention: bail if running inside a wave worker
    if std::env::var("RALPH_WAVE_WORKER").is_ok_and(|v| v == "1") {
        bail!(
            "Cannot dispatch waves from inside a wave worker. \
             Wave workers must emit results via `ralph emit`, not `ralph wave emit`."
        );
    }

    let events_file = resolve_events_file();
    let wave_id = write_wave_events(&args.topic, &args.payloads, &events_file)?;

    // Print wave ID to stdout (machine-parseable)
    println!("{}", wave_id);

    // Human-readable confirmation to stderr
    let total = args.payloads.len();
    if use_colors {
        eprintln!(
            "\x1b[32m\u{2713}\x1b[0m Wave dispatched: {} events on topic '{}' (wave {})",
            total, args.topic, wave_id
        );
    } else {
        eprintln!(
            "Wave dispatched: {} events on topic '{}' (wave {})",
            total, args.topic, wave_id
        );
    }

    Ok(())
}

/// Write wave events to a JSONL file. Returns the generated wave ID.
///
/// This is the core logic, separated from CLI concerns for testability.
pub fn write_wave_events(topic: &str, payloads: &[String], events_file: &Path) -> Result<String> {
    if payloads.is_empty() {
        bail!("At least one payload is required");
    }

    let wave_id = generate_wave_id();

    let total = payloads.len() as u32;
    let ts = chrono::Utc::now().to_rfc3339();

    // Ensure parent directory exists
    if let Some(parent) = events_file.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Build all event records
    let mut lines = String::new();
    for (index, payload) in payloads.iter().enumerate() {
        let record = serde_json::json!({
            "topic": topic,
            "payload": payload,
            "ts": ts,
            "wave_id": wave_id,
            "wave_index": index as u32,
            "wave_total": total,
        });
        let json_line = serde_json::to_string(&record)?;
        lines.push_str(&json_line);
        lines.push('\n');
    }

    // Write all events atomically
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_file)
        .with_context(|| format!("Failed to open events file: {}", events_file.display()))?;
    file.write_all(lines.as_bytes())?;

    Ok(wave_id)
}

/// Resolve the events file path from environment and marker files.
///
/// Priority: RALPH_EVENTS_FILE env > .ralph/current-events marker > default .ralph/events.jsonl
pub fn resolve_events_file() -> PathBuf {
    if let Ok(path) = std::env::var("RALPH_EVENTS_FILE") {
        return PathBuf::from(path);
    }
    fs::read_to_string(".ralph/current-events")
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|_| PathBuf::from(".ralph/events.jsonl"))
}

/// Generate a unique wave ID.
///
/// Concatenates nanosecond timestamp, PID, and a process-local atomic counter.
/// Readable and debuggable — each segment is independently meaningful.
fn generate_wave_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);

    format!("w-{nanos:x}-{pid}-{seq}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_wave_events_creates_tagged_events() {
        let tmp = TempDir::new().unwrap();
        let events_path = tmp.path().join("events.jsonl");

        let payloads = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/config.rs".to_string(),
        ];

        let wave_id = write_wave_events("review.file", &payloads, &events_path).unwrap();
        assert!(wave_id.starts_with("w-"));

        let content = fs::read_to_string(&events_path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 3);

        // Parse and verify each event
        for (i, line) in lines.iter().enumerate() {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(event["topic"], "review.file");
            assert_eq!(event["wave_index"], i as u64);
            assert_eq!(event["wave_total"], 3);
            assert_eq!(event["wave_id"], wave_id.as_str());
        }
    }

    #[test]
    fn test_write_wave_events_single_payload() {
        let tmp = TempDir::new().unwrap();
        let events_path = tmp.path().join("events.jsonl");

        let payloads = vec!["only-one".to_string()];
        let wave_id = write_wave_events("test.topic", &payloads, &events_path).unwrap();

        let content = fs::read_to_string(&events_path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 1);

        let event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(event["wave_index"], 0);
        assert_eq!(event["wave_total"], 1);
        assert_eq!(event["wave_id"], wave_id.as_str());
    }

    #[test]
    fn test_write_wave_events_empty_payloads_rejected() {
        let tmp = TempDir::new().unwrap();
        let events_path = tmp.path().join("events.jsonl");

        let result = write_wave_events("test.topic", &[], &events_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_wave_events_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let events_path = tmp.path().join("nested").join("dir").join("events.jsonl");

        let payloads = vec!["payload".to_string()];
        write_wave_events("test.topic", &payloads, &events_path).unwrap();

        assert!(events_path.exists());
    }
}
