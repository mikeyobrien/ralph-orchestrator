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
}
