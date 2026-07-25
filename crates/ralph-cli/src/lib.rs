use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

mod colors {
    pub const DIM: &str = "\x1b[2m";
    pub const RESET: &str = "\x1b[0m";
    pub const CYAN: &str = "\x1b[36m";
    pub const GREEN: &str = "\x1b[32m";
}

/// Clean diagnostic logs from .ralph/diagnostics directory
pub fn clean_diagnostics(workspace_root: &Path, use_colors: bool, dry_run: bool) -> Result<()> {
    let diagnostics_dir = workspace_root.join(".ralph/diagnostics");

    // Check if directory exists
    if !diagnostics_dir.exists() {
        if use_colors {
            println!(
                "{}Nothing to clean:{} Directory '{}' does not exist",
                colors::DIM,
                colors::RESET,
                diagnostics_dir.display()
            );
        } else {
            println!(
                "Nothing to clean: Directory '{}' does not exist",
                diagnostics_dir.display()
            );
        }
        return Ok(());
    }

    // Dry run mode - list what would be deleted
    if dry_run {
        if use_colors {
            println!(
                "{}Dry run mode:{} Would delete directory and all contents:",
                colors::CYAN,
                colors::RESET
            );
        } else {
            println!("Dry run mode: Would delete directory and all contents:");
        }
        println!("  {}", diagnostics_dir.display());

        // List directory contents (simplified for lib - just show count)
        if let Ok(entries) = fs::read_dir(&diagnostics_dir) {
            let count = entries.count();
            println!("  ({} session directories)", count);
        }

        return Ok(());
    }

    // Perform actual deletion
    fs::remove_dir_all(&diagnostics_dir).with_context(|| {
        format!(
            "Failed to delete directory '{}'. Check permissions and try again.",
            diagnostics_dir.display()
        )
    })?;

    // Success message
    if use_colors {
        println!(
            "{}✓{} Cleaned: Deleted '{}' and all contents",
            colors::GREEN,
            colors::RESET,
            diagnostics_dir.display()
        );
    } else {
        println!(
            "Cleaned: Deleted '{}' and all contents",
            diagnostics_dir.display()
        );
    }

    Ok(())
}

/// Collect event run-history artifacts under `.ralph/`: `events.jsonl`,
/// `events-*.jsonl`, and the `current-events` marker.
fn event_artifacts(workspace_root: &Path) -> Vec<std::path::PathBuf> {
    let ralph_dir = workspace_root.join(".ralph");
    let mut found = Vec::new();

    let marker = ralph_dir.join("current-events");
    if marker.exists() {
        found.push(marker);
    }

    if let Ok(entries) = fs::read_dir(&ralph_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".jsonl") && (name == "events.jsonl" || name.starts_with("events-")) {
                found.push(entry.path());
            }
        }
    }

    found.sort();
    found
}

/// Clean event run history (`events*.jsonl` + `current-events` marker) from `.ralph/`
pub fn clean_events(workspace_root: &Path, use_colors: bool, dry_run: bool) -> Result<()> {
    let targets = event_artifacts(workspace_root);

    if targets.is_empty() {
        if use_colors {
            println!(
                "{}Nothing to clean:{} No event files found in '{}'",
                colors::DIM,
                colors::RESET,
                workspace_root.join(".ralph").display()
            );
        } else {
            println!(
                "Nothing to clean: No event files found in '{}'",
                workspace_root.join(".ralph").display()
            );
        }
        return Ok(());
    }

    if dry_run {
        if use_colors {
            println!(
                "{}Dry run mode:{} Would delete event files:",
                colors::CYAN,
                colors::RESET
            );
        } else {
            println!("Dry run mode: Would delete event files:");
        }
        for path in &targets {
            println!("  {}", path.display());
        }
        return Ok(());
    }

    for path in &targets {
        fs::remove_file(path).with_context(|| {
            format!(
                "Failed to delete '{}'. Check permissions and try again.",
                path.display()
            )
        })?;
    }

    if use_colors {
        println!(
            "{}✓{} Cleaned: Deleted {} event file(s)",
            colors::GREEN,
            colors::RESET,
            targets.len()
        );
    } else {
        println!("Cleaned: Deleted {} event file(s)", targets.len());
    }
    for path in &targets {
        println!("  {}", path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_diagnostics_no_dir_is_ok() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let result = clean_diagnostics(temp_dir.path(), false, false);
        assert!(result.is_ok());
        assert!(!temp_dir.path().join(".ralph/diagnostics").exists());
    }

    #[test]
    fn clean_diagnostics_dry_run_keeps_dir() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let diagnostics_dir = temp_dir.path().join(".ralph/diagnostics");
        std::fs::create_dir_all(&diagnostics_dir).expect("create diagnostics");
        std::fs::write(diagnostics_dir.join("session.log"), "data").expect("write log");

        clean_diagnostics(temp_dir.path(), false, true).expect("dry run");
        assert!(diagnostics_dir.exists());
    }

    #[test]
    fn clean_diagnostics_deletes_dir() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let diagnostics_dir = temp_dir.path().join(".ralph/diagnostics");
        std::fs::create_dir_all(&diagnostics_dir).expect("create diagnostics");
        std::fs::write(diagnostics_dir.join("session.log"), "data").expect("write log");

        clean_diagnostics(temp_dir.path(), false, false).expect("clean diagnostics");
        assert!(!diagnostics_dir.exists());
    }

    /// Creates a `.ralph/` with event artifacts plus non-event files that must survive.
    fn events_fixture() -> tempfile::TempDir {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let ralph_dir = temp_dir.path().join(".ralph");
        std::fs::create_dir_all(ralph_dir.join("specs")).expect("create .ralph");
        for name in [
            "events.jsonl",
            "events-20260101-000000.jsonl",
            "current-events",
            "loops.json",
            "merge-queue.jsonl",
            "events-notes.md",
        ] {
            std::fs::write(ralph_dir.join(name), "x").expect("write fixture");
        }
        std::fs::write(ralph_dir.join("specs/plan.md"), "x").expect("write spec");
        temp_dir
    }

    #[test]
    fn clean_events_no_files_is_ok() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        assert!(clean_events(temp_dir.path(), false, false).is_ok());
    }

    #[test]
    fn clean_events_dry_run_keeps_files() {
        let temp_dir = events_fixture();
        clean_events(temp_dir.path(), false, true).expect("dry run");
        assert!(temp_dir.path().join(".ralph/events.jsonl").exists());
        assert!(temp_dir.path().join(".ralph/current-events").exists());
    }

    #[test]
    fn clean_events_deletes_only_event_artifacts() {
        let temp_dir = events_fixture();
        clean_events(temp_dir.path(), false, false).expect("clean events");

        let ralph_dir = temp_dir.path().join(".ralph");
        for gone in [
            "events.jsonl",
            "events-20260101-000000.jsonl",
            "current-events",
        ] {
            assert!(!ralph_dir.join(gone).exists(), "{gone} should be deleted");
        }
        for kept in ["loops.json", "merge-queue.jsonl", "events-notes.md"] {
            assert!(ralph_dir.join(kept).exists(), "{kept} should survive");
        }
        assert!(ralph_dir.join("specs/plan.md").exists());
    }
}
