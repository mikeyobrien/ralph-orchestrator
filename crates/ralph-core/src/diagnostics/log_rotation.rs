use chrono::Local;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_LOG_FILES: usize = 5;

/// Rotates log files in the given directory, keeping at most `max_files` entries.
///
/// Lists `ralph-*.log` files, sorts lexicographically (which gives timestamp order),
/// and deletes the oldest if count exceeds `max_files - 1` (to make room for a new one).
pub fn rotate_logs(logs_dir: &Path, max_files: usize) -> io::Result<()> {
    if !logs_dir.exists() {
        return Ok(());
    }

    let mut log_files: Vec<PathBuf> = fs::read_dir(logs_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("ralph-")
                && Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
            {
                Some(entry.path())
            } else {
                None
            }
        })
        .collect();

    log_files.sort();

    // Delete oldest files to make room for a new one
    let to_keep = max_files.saturating_sub(1);
    if log_files.len() > to_keep {
        let to_remove = log_files.len() - to_keep;
        for path in &log_files[..to_remove] {
            let _ = fs::remove_file(path);
        }
    }

    Ok(())
}

/// Creates a new timestamped log file in `.ralph/diagnostics/logs/`.
///
/// Creates the directory if needed, rotates old logs, and returns the file handle
/// and path of the newly created log file.
pub fn create_log_file(base_path: &Path) -> io::Result<(fs::File, PathBuf)> {
    let logs_dir = base_path.join(".ralph").join("diagnostics").join("logs");
    fs::create_dir_all(&logs_dir)?;

    rotate_logs(&logs_dir, MAX_LOG_FILES)?;

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
    let log_path = logs_dir.join(format!("ralph-{}.log", timestamp));
    let file = fs::File::create(&log_path)?;

    Ok((file, log_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_rotate_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        rotate_logs(&logs_dir, 5).unwrap();

        let count = fs::read_dir(&logs_dir).unwrap().count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_rotate_under_limit() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        for i in 0..3 {
            fs::write(
                logs_dir.join(format!("ralph-2025-01-0{}T12-00-00.log", i + 1)),
                "test",
            )
            .unwrap();
        }

        rotate_logs(&logs_dir, 5).unwrap();

        let count: Vec<_> = fs::read_dir(&logs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(count.len(), 3);
    }

    #[test]
    fn test_rotate_at_limit() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        // 5 files, max_files=5 means we keep 4 to make room for a new one
        for i in 0..5 {
            fs::write(
                logs_dir.join(format!("ralph-2025-01-0{}T12-00-00.log", i + 1)),
                "test",
            )
            .unwrap();
        }

        rotate_logs(&logs_dir, 5).unwrap();

        let remaining: Vec<String> = fs::read_dir(&logs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(remaining.len(), 4);
        // Oldest file should be removed
        assert!(!remaining.contains(&"ralph-2025-01-01T12-00-00.log".to_string()));
    }

    #[test]
    fn test_rotate_over_limit() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        for i in 0..8 {
            fs::write(
                logs_dir.join(format!("ralph-2025-01-{:02}T12-00-00.log", i + 1)),
                "test",
            )
            .unwrap();
        }

        rotate_logs(&logs_dir, 5).unwrap();

        let mut remaining: Vec<String> = fs::read_dir(&logs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        remaining.sort();
        assert_eq!(remaining.len(), 4);
        // Only the 4 newest should remain
        assert_eq!(remaining[0], "ralph-2025-01-05T12-00-00.log");
        assert_eq!(remaining[3], "ralph-2025-01-08T12-00-00.log");
    }

    #[test]
    fn test_rotate_ignores_non_matching_files() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        // Create 6 ralph log files + some non-matching files
        for i in 0..6 {
            fs::write(
                logs_dir.join(format!("ralph-2025-01-{:02}T12-00-00.log", i + 1)),
                "test",
            )
            .unwrap();
        }
        fs::write(logs_dir.join("other.log"), "keep me").unwrap();
        fs::write(logs_dir.join("ralph.txt"), "keep me too").unwrap();
        fs::write(logs_dir.join("not-ralph-log.log"), "and me").unwrap();

        rotate_logs(&logs_dir, 5).unwrap();

        let remaining: Vec<String> = fs::read_dir(&logs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        // 4 ralph logs + 3 non-matching files
        assert_eq!(remaining.len(), 7);
        assert!(remaining.contains(&"other.log".to_string()));
        assert!(remaining.contains(&"ralph.txt".to_string()));
        assert!(remaining.contains(&"not-ralph-log.log".to_string()));
    }

    #[test]
    fn test_rotate_nonexistent_dir() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("does-not-exist");

        // Should succeed without error
        rotate_logs(&logs_dir, 5).unwrap();
    }

    #[test]
    fn test_create_log_file_creates_directory() {
        let tmp = TempDir::new().unwrap();

        let (_, path) = create_log_file(tmp.path()).unwrap();

        assert!(path.exists());
        assert!(tmp.path().join(".ralph/diagnostics/logs").exists());
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("ralph-"));
        assert!(
            Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
        );
    }

    #[test]
    fn test_create_log_file_rotates() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join(".ralph/diagnostics/logs");
        fs::create_dir_all(&logs_dir).unwrap();

        // Pre-populate with 5 files
        for i in 0..5 {
            fs::write(
                logs_dir.join(format!("ralph-2025-01-{:02}T12-00-00.log", i + 1)),
                "old",
            )
            .unwrap();
        }

        let (_, _) = create_log_file(tmp.path()).unwrap();

        let count = fs::read_dir(&logs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("ralph-")
                    && Path::new(&name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
            })
            .count();
        // 4 old (after rotation) + 1 new = 5
        assert!(count <= 5);
    }
}
