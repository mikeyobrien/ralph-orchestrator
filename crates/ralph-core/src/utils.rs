//! Utility functions for common operations.
//!
//! This module provides shared utilities used across the Ralph orchestrator.

use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Generates a unique ID with the given prefix: `{prefix}-{unix_secs}-{4_hex_chars}`.
pub fn generate_prefixed_id(prefix: &str) -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let timestamp = duration.as_secs();
    let hex_suffix = format!("{:04x}", duration.subsec_micros() % 0x10000);
    format!("{prefix}-{timestamp}-{hex_suffix}")
}

/// Checks whether a process with the given PID is still running.
///
/// Uses `kill(pid, 0)` on Unix (signal 0 probes without delivering a signal).
/// Returns `false` on non-Unix platforms.
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid as i32), None).is_ok()
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Formats a duration as MM:SS (minutes:seconds).
///
/// Useful for displaying elapsed time in TUI headers, status bars, and logs.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use ralph_core::utils::format_elapsed;
///
/// assert_eq!(format_elapsed(Duration::from_secs(0)), "00:00");
/// assert_eq!(format_elapsed(Duration::from_secs(65)), "01:05");
/// assert_eq!(format_elapsed(Duration::from_secs(3661)), "61:01"); // Handles >60 mins
/// ```
pub fn format_elapsed(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins:02}:{secs:02}")
}

/// Returns the list of executable file extensions for the current platform.
///
/// On Windows, reads `PATHEXT` (falling back to `.COM;.EXE;.BAT;.CMD`).
/// On Unix, returns a single empty extension (executables have no required extension).
pub fn executable_extensions() -> Vec<OsString> {
    if cfg!(windows) {
        let exts =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        exts.split(';')
            .filter(|ext| !ext.trim().is_empty())
            .map(|ext| OsString::from(ext.trim().to_string()))
            .collect()
    } else {
        vec![OsString::new()]
    }
}

/// Checks whether a path points to a file with executable permissions.
///
/// On Unix, verifies the execute bit is set. On other platforms, any file is considered executable.
pub fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_elapsed_zero() {
        assert_eq!(format_elapsed(Duration::from_secs(0)), "00:00");
    }

    #[test]
    fn format_elapsed_seconds_only() {
        assert_eq!(format_elapsed(Duration::from_secs(45)), "00:45");
    }

    #[test]
    fn format_elapsed_one_minute() {
        assert_eq!(format_elapsed(Duration::from_mins(1)), "01:00");
    }

    #[test]
    fn format_elapsed_mixed() {
        assert_eq!(format_elapsed(Duration::from_secs(272)), "04:32");
    }

    #[test]
    fn format_elapsed_large_value() {
        // 61 minutes and 1 second
        assert_eq!(format_elapsed(Duration::from_secs(3661)), "61:01");
    }

    #[test]
    fn format_elapsed_pads_single_digits() {
        // Ensure single-digit values are zero-padded
        assert_eq!(format_elapsed(Duration::from_secs(5)), "00:05");
        assert_eq!(format_elapsed(Duration::from_secs(65)), "01:05");
    }

    #[test]
    fn format_elapsed_ignores_subsecond() {
        // Milliseconds should be truncated, not rounded
        assert_eq!(format_elapsed(Duration::from_millis(999)), "00:00");
        assert_eq!(format_elapsed(Duration::from_millis(1500)), "00:01");
    }

    #[cfg(unix)]
    #[test]
    fn is_process_alive_self() {
        assert!(is_process_alive(std::process::id()));
    }

    #[cfg(unix)]
    #[test]
    fn is_process_alive_bogus_pid() {
        assert!(!is_process_alive(u32::MAX - 1));
    }

    #[test]
    fn generate_prefixed_id_format() {
        let id = generate_prefixed_id("test");
        assert!(id.starts_with("test-"));
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2].len(), 4);
    }
}
