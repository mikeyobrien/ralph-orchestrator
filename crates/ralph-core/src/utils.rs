//! Utility functions for common operations.
//!
//! This module provides shared utilities used across the Ralph orchestrator.

use serde::{Deserialize, Deserializer};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Deserialize)]
#[serde(untagged)]
enum FlexiblePayload {
    String(String),
    Object(serde_json::Value),
}

fn flexible_payload_to_string(flex: FlexiblePayload) -> String {
    match flex {
        FlexiblePayload::String(s) => s,
        FlexiblePayload::Object(serde_json::Value::Null) => String::new(),
        FlexiblePayload::Object(obj) => {
            serde_json::to_string(&obj).unwrap_or_else(|_| obj.to_string())
        }
    }
}

/// Deserializes a payload that may be a string or JSON object into a required `String`.
/// Null or missing values become an empty string.
pub fn deserialize_flexible_payload_required<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<FlexiblePayload>::deserialize(deserializer)?;
    Ok(opt.map(flexible_payload_to_string).unwrap_or_default())
}

/// Deserializes a payload that may be a string or JSON object into an `Option<String>`.
/// Null or missing values become `None`.
pub fn deserialize_flexible_payload_optional<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<FlexiblePayload>::deserialize(deserializer)?;
    Ok(opt.map(flexible_payload_to_string))
}

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

/// Formats a duration as a human-readable string (e.g., "45s", "2m 5s", "1h 2m 5s").
///
/// Useful for summary files, status messages, and user-facing output where
/// a natural-language duration is preferred over MM:SS format.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use ralph_core::utils::format_duration;
///
/// assert_eq!(format_duration(Duration::from_secs(45)), "45s");
/// assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
/// assert_eq!(format_duration(Duration::from_secs(3725)), "1h 2m 5s");
/// ```
pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Returns the list of executable file extensions for the current platform.
///
/// On Windows, reads `PATHEXT` (falling back to `.COM;.EXE;.BAT;.CMD`).
/// On Unix, returns a single empty extension (executables have no required extension).
pub fn executable_extensions() -> Vec<OsString> {
    if cfg!(windows) {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
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

/// Converts a flock errno into an `io::Error`.
#[cfg(unix)]
pub fn flock_io_error(errno: nix::errno::Errno) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("flock failed: {}", errno))
}

/// Returns `true` if the errno indicates the lock is held by another process.
#[cfg(unix)]
pub fn is_lock_contention(errno: nix::errno::Errno) -> bool {
    errno == nix::errno::Errno::EWOULDBLOCK || errno == nix::errno::Errno::EAGAIN
}

/// Clones a usable `File` handle from a `nix::fcntl::Flock`.
///
/// `Flock` doesn't expose the inner `File` directly, so we duplicate the
/// file descriptor to obtain an independent handle that can be seeked and
/// read/written while the lock is held.
#[cfg(unix)]
pub fn clone_file_from_flock(
    flock: &nix::fcntl::Flock<std::fs::File>,
) -> io::Result<std::fs::File> {
    use std::os::fd::AsFd;
    let owned_fd = flock
        .as_fd()
        .try_clone_to_owned()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(owned_fd.into())
}

/// Returns the current UTC time as an RFC 3339 string.
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Returns today's UTC date as a `YYYY-MM-DD` string.
pub fn today_ymd() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Creates parent directories for the given path if they don't exist.
pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Serializes a value as JSON and writes it as a single JSONL line.
pub fn write_jsonl_line(
    writer: &mut impl std::io::Write,
    value: &impl serde::Serialize,
) -> io::Result<()> {
    let json =
        serde_json::to_string(value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    writeln!(writer, "{}", json)
}

/// Parses JSONL content leniently: skips empty lines and malformed JSON, collecting only valid records.
pub fn parse_jsonl_lenient<T: serde::de::DeserializeOwned>(content: &str) -> Vec<T> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Reads a JSONL file leniently, skipping empty and malformed lines.
///
/// Returns an empty `Vec` if the file does not exist.
pub fn read_jsonl_lenient<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(parse_jsonl_lenient(&content))
}

/// Opens a file in create + append mode.
pub fn open_append(path: impl AsRef<Path>) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.as_ref())
}

/// Opens (or creates) a file for read + write without truncating.
pub fn open_read_write(path: impl AsRef<Path>) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.as_ref())
}

/// Searches PATH for a command and returns its resolved path, or `None`.
///
/// If `command` contains a path separator (e.g. `./script.sh`), it's checked
/// directly. Otherwise, each directory on `PATH` is probed with platform
/// extensions (`.exe`/`.cmd` on Windows, bare name on Unix).
pub fn find_executable(command: &str) -> Option<std::path::PathBuf> {
    use std::path::Path;

    let path = Path::new(command);
    if path.components().count() > 1 {
        return if path.is_file() {
            Some(path.to_path_buf())
        } else {
            None
        };
    }

    let path_var = std::env::var_os("PATH")?;
    let extensions = executable_extensions();

    for dir in std::env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = if ext.is_empty() {
                dir.join(command)
            } else {
                dir.join(format!("{}{}", command, ext.to_string_lossy()))
            };

            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Strips ANSI escape sequences from raw bytes.
///
/// Handles CSI sequences (`\x1b[...`), OSC sequences (`\x1b]...BEL/ST`),
/// and simple escape sequences (`\x1b` + single char).
pub fn strip_ansi_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i >= bytes.len() {
                break;
            }

            match bytes[i] {
                b'[' => {
                    // CSI sequence: ESC [ ... (final byte in 0x40-0x7E range)
                    i += 1;
                    while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
                b']' => {
                    // OSC sequence: ESC ] ... (terminated by BEL or ST)
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    result
}

/// Strips ANSI escape sequences from raw bytes, returning a `String`.
pub fn strip_ansi_from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&strip_ansi_bytes(bytes)).into_owned()
}

/// Strips ANSI escape sequences from a string.
pub fn strip_ansi(s: &str) -> String {
    strip_ansi_from_bytes(s.as_bytes())
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
    fn format_duration_seconds_only() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
    }

    #[test]
    fn format_duration_hours_minutes_seconds() {
        assert_eq!(format_duration(Duration::from_secs(3725)), "1h 2m 5s");
    }

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
    }

    #[test]
    fn strip_ansi_bytes_removes_csi() {
        let input = b"Hello, \x1b[32mWorld\x1b[0m!";
        assert_eq!(strip_ansi_bytes(input), b"Hello, World!");
    }

    #[test]
    fn strip_ansi_bytes_removes_multiple_csi() {
        let input = b"\x1b[1m\x1b[32mBold Green\x1b[0m Normal";
        assert_eq!(strip_ansi_bytes(input), b"Bold Green Normal");
    }

    #[test]
    fn strip_ansi_str_wrapper() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
    }

    #[test]
    fn parse_jsonl_lenient_basic() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Item {
            id: u32,
        }

        let content = "{\"id\":1}\n{\"id\":2}\n";
        let items: Vec<Item> = parse_jsonl_lenient(content);
        assert_eq!(items, vec![Item { id: 1 }, Item { id: 2 }]);
    }

    #[test]
    fn parse_jsonl_lenient_skips_bad_lines() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Item {
            id: u32,
        }

        let content = "{\"id\":1}\nnot json\n\n{\"id\":3}\n";
        let items: Vec<Item> = parse_jsonl_lenient(content);
        assert_eq!(items, vec![Item { id: 1 }, Item { id: 3 }]);
    }

    #[test]
    fn parse_jsonl_lenient_empty_input() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Item {
            id: u32,
        }

        let items: Vec<Item> = parse_jsonl_lenient("");
        assert!(items.is_empty());
    }

    #[test]
    fn read_jsonl_lenient_missing_file() {
        #[derive(serde::Deserialize)]
        struct Item {
            _id: u32,
        }

        let items: Vec<Item> = read_jsonl_lenient(Path::new("/nonexistent/file.jsonl")).unwrap();
        assert!(items.is_empty());
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
