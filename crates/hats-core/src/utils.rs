//! Utility functions for common operations.
//!
//! This module provides shared utilities used across the Hats orchestrator.

use std::time::Duration;

/// Reads an environment variable with `HATS_` prefix, falling back to the
/// deprecated `RALPH_` prefix. Emits a deprecation warning to stderr on first
/// use of a `RALPH_` variant.
///
/// # Examples
///
/// ```
/// use hats_core::utils::hats_env_var;
///
/// // Checks HATS_VERBOSE first, then RALPH_VERBOSE
/// let val = hats_env_var("VERBOSE");
/// ```
pub fn hats_env_var(suffix: &str) -> Result<String, std::env::VarError> {
    let hats_key = format!("HATS_{suffix}");
    match std::env::var(&hats_key) {
        Ok(val) => Ok(val),
        Err(_) => {
            let ralph_key = format!("RALPH_{suffix}");
            match std::env::var(&ralph_key) {
                Ok(val) => {
                    eprintln!("⚠ {ralph_key} is deprecated. Use {hats_key} instead.");
                    Ok(val)
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// Returns true if the env var (HATS_ or RALPH_ prefixed) is set.
pub fn hats_env_var_is_set(suffix: &str) -> bool {
    hats_env_var(suffix).is_ok()
}

/// Formats a duration as MM:SS (minutes:seconds).
///
/// Useful for displaying elapsed time in TUI headers, status bars, and logs.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use hats_core::utils::format_elapsed;
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
        assert_eq!(format_elapsed(Duration::from_secs(60)), "01:00");
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
}
