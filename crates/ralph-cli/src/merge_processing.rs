//! Merge-queue draining: spawn `merge-ralph` subprocesses for queued worktree
//! loops.
//!
//! This is engine-agnostic coordination — it depends only on the merge queue,
//! the bundled `merge-loop` preset, and diagnostics logging, never on the
//! orchestration engine. It lives outside `loop_runner` so it survives the
//! in-house engine's deletion (the autoloop engine path drains the queue via
//! [`process_pending_merges_cli`] from `completion_coord`).

use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ralph_core::MergeQueue;
use tracing::{debug, info, warn};

fn merge_ralph_args(loop_id: &str) -> Vec<String> {
    vec![
        "run".to_string(),
        "-c".to_string(),
        ".ralph/merge-loop-config.yml".to_string(),
        "-H".to_string(),
        "builtin:merge-loop".to_string(),
        "--exclusive".to_string(),
        "--no-tui".to_string(),
        "-p".to_string(),
        format!("Merge loop {loop_id} from branch ralph/{loop_id}"),
    ]
}

/// Drain the merge queue by spawning a `merge-ralph` subprocess (`ralph_cmd`)
/// for each queued entry. Errors are logged and leave the entry queued for
/// manual retry; never panics.
fn process_pending_merges_with_command(repo_root: &Path, ralph_cmd: &OsStr) {
    let queue = MergeQueue::new(repo_root);

    // Get all pending merges
    let pending = match queue.list_by_state(ralph_core::merge_queue::MergeState::Queued) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read merge queue: {}", e);
            return;
        }
    };

    if pending.is_empty() {
        debug!("No pending merges in queue");
        return;
    }

    info!(
        count = pending.len(),
        "Processing pending merges from queue"
    );

    // Get the merge-loop preset content
    let preset = match crate::presets::get_preset("merge-loop") {
        Some(p) => p,
        None => {
            warn!("merge-loop preset not found, pending merges will remain queued");
            return;
        }
    };

    // Write a core-only merge config once (shared by all merge loops).
    let mut core_value: serde_yaml::Value = match serde_yaml::from_str(preset.content) {
        Ok(value) => value,
        Err(e) => {
            warn!(
                error = %e,
                "Failed to parse merge-loop preset, pending merges will remain queued"
            );
            return;
        }
    };

    if let Some(mapping) = core_value.as_mapping_mut() {
        let hats_key = serde_yaml::Value::String("hats".to_string());
        let events_key = serde_yaml::Value::String("events".to_string());
        mapping.remove(&hats_key);
        mapping.remove(&events_key);
    }

    let core_yaml = match serde_yaml::to_string(&core_value) {
        Ok(yaml) => yaml,
        Err(e) => {
            warn!(
                error = %e,
                "Failed to serialize core-only merge config, pending merges will remain queued"
            );
            return;
        }
    };

    let config_path = repo_root.join(".ralph/merge-loop-config.yml");
    if let Err(e) = fs::write(&config_path, core_yaml) {
        warn!(
            error = %e,
            "Failed to write merge config, pending merges will remain queued"
        );
        return;
    }

    // Process each pending merge
    for entry in pending {
        let loop_id = &entry.loop_id;

        info!(loop_id = %loop_id, "Spawning merge-ralph process");

        // Redirect subprocess stdio to a log file to prevent TUI corruption.
        // If log file creation fails, fall back to Stdio::null rather than
        // inheriting the parent's terminal (which would corrupt the TUI).
        let (stdout_stdio, stderr_stdio, log_path) =
            match create_merge_subprocess_log_file(repo_root, loop_id) {
                Ok((file, path)) => match file.try_clone() {
                    Ok(file_clone) => (Stdio::from(file_clone), Stdio::from(file), Some(path)),
                    Err(e) => {
                        warn!(
                            loop_id = %loop_id,
                            error = %e,
                            "Failed to clone log file handle, subprocess output will be discarded"
                        );
                        (Stdio::null(), Stdio::null(), None)
                    }
                },
                Err(e) => {
                    warn!(
                        loop_id = %loop_id,
                        error = %e,
                        "Failed to create subprocess log file, output will be discarded"
                    );
                    (Stdio::null(), Stdio::null(), None)
                }
            };

        match Command::new(ralph_cmd)
            .current_dir(repo_root)
            .args(merge_ralph_args(loop_id))
            .env("RALPH_MERGE_LOOP_ID", loop_id)
            .stdout(stdout_stdio)
            .stderr(stderr_stdio)
            .spawn()
        {
            Ok(child) => {
                if let Some(path) = log_path {
                    info!(
                        loop_id = %loop_id,
                        pid = child.id(),
                        log_file = %path.display(),
                        "merge-ralph spawned successfully"
                    );
                } else {
                    info!(
                        loop_id = %loop_id,
                        pid = child.id(),
                        "merge-ralph spawned successfully"
                    );
                }
            }
            Err(e) => {
                warn!(
                    loop_id = %loop_id,
                    error = %e,
                    "Failed to spawn merge-ralph, loop will remain queued for manual retry"
                );
            }
        }
    }
}

/// Creates a timestamped log file for a merge subprocess under `.ralph/diagnostics/logs/`.
///
/// Uses the loop_id in the filename for easier identification when debugging.
/// Participates in the existing log rotation scheme.
fn create_merge_subprocess_log_file(
    repo_root: &Path,
    loop_id: &str,
) -> std::io::Result<(File, PathBuf)> {
    use chrono::Local;

    let logs_dir = repo_root.join(".ralph").join("diagnostics").join("logs");
    fs::create_dir_all(&logs_dir)?;

    let _ = ralph_core::diagnostics::rotate_logs(&logs_dir, 10);

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
    let log_path = logs_dir.join(format!("ralph-merge-{}-{}.log", loop_id, timestamp));
    let file = File::create(&log_path)?;

    Ok((file, log_path))
}

/// Drain the merge queue using the `ralph` binary on `PATH`.
pub(crate) fn process_pending_merges(repo_root: &Path) {
    process_pending_merges_with_command(repo_root, OsStr::new("ralph"));
}

/// Public wrapper for CLI invocation of [`process_pending_merges`].
///
/// Called by `ralph loops process` and by the completion coordinator on
/// primary-loop completion.
pub fn process_pending_merges_cli(repo_root: &Path) {
    process_pending_merges(repo_root);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn write_fake_executable(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        let script = format!("#!/bin/sh\n{}\n", body);
        std::fs::write(&path, script).expect("write script");
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
        path
    }

    #[test]
    fn merge_ralph_args_include_non_empty_merge_prompt() {
        let args = merge_ralph_args("loop-1234");
        let prompt_flag = args.iter().position(|arg| arg == "-p").expect("-p flag");

        assert_eq!(
            args.get(prompt_flag + 1).map(String::as_str),
            Some("Merge loop loop-1234 from branch ralph/loop-1234")
        );
    }

    #[test]
    fn test_process_pending_merges_handles_missing_preset() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".ralph/merge-queue")).expect("queue dir");

        process_pending_merges(repo_root);
    }

    #[cfg(unix)]
    #[test]
    fn test_process_pending_merges_spawns_for_queue_entry() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".ralph/merge-queue")).expect("queue dir");

        let queue_file = repo_root.join(".ralph/merge-queue/loop-1234.json");
        std::fs::write(
            &queue_file,
            r#"{"loop_id":"1234","state":"queued","created_at":"2026-01-01T00:00:00Z"}"#,
        )
        .expect("queue file");

        let bin_dir = repo_root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let ralph_path = write_fake_executable(&bin_dir, "ralph", "exit 0");

        process_pending_merges_with_command(repo_root, ralph_path.as_os_str());
    }

    #[test]
    fn test_process_pending_merges_missing_command_keeps_queue() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        let queue = ralph_core::merge_queue::MergeQueue::new(repo_root);
        queue.enqueue("loop-9999", "merge prompt").expect("enqueue");

        process_pending_merges_with_command(repo_root, OsStr::new("ralph-command-missing-12345"));

        let config_path = repo_root.join(".ralph/merge-loop-config.yml");
        assert!(config_path.exists());
        let entries = queue
            .list_by_state(ralph_core::merge_queue::MergeState::Queued)
            .expect("list queued");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].loop_id, "loop-9999");
    }

    #[test]
    fn test_process_pending_merges_with_empty_queue_no_config_written() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        std::fs::create_dir_all(repo_root.join(".ralph/merge-queue")).expect("queue dir");

        let config_path = repo_root.join(".ralph/merge-loop-config.yml");
        assert!(!config_path.exists());

        process_pending_merges_with_command(repo_root, OsStr::new("ralph"));

        assert!(!config_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_process_pending_merges_redirects_subprocess_output_to_log_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();

        // Enqueue a merge entry using the proper API
        let queue = ralph_core::merge_queue::MergeQueue::new(repo_root);
        queue.enqueue("test-loop", "merge prompt").expect("enqueue");

        // Create a fake ralph that writes to both stdout and stderr
        let bin_dir = repo_root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let ralph_path = write_fake_executable(
            &bin_dir,
            "ralph",
            "echo 'stdout output' && echo 'stderr output' >&2 && sleep 0.1",
        );

        process_pending_merges_with_command(repo_root, ralph_path.as_os_str());

        // Wait for subprocess to finish writing
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Verify a log file was created under .ralph/diagnostics/logs/
        let logs_dir = repo_root.join(".ralph/diagnostics/logs");
        assert!(logs_dir.exists(), "diagnostics logs directory should exist");

        let log_files: Vec<_> = std::fs::read_dir(&logs_dir)
            .expect("read logs dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("ralph-merge-"))
            .collect();
        assert!(
            !log_files.is_empty(),
            "should have at least one merge subprocess log file"
        );

        // Verify the log file contains the subprocess output
        let log_content = std::fs::read_to_string(log_files[0].path()).expect("read log file");
        assert!(
            log_content.contains("stdout output"),
            "log file should contain stdout, got: {log_content}"
        );
        assert!(
            log_content.contains("stderr output"),
            "log file should contain stderr, got: {log_content}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_process_pending_merges_falls_back_to_null_on_log_creation_failure() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();

        // Block log file creation by placing a regular file where the logs directory would be
        let diagnostics_dir = repo_root.join(".ralph/diagnostics");
        std::fs::create_dir_all(&diagnostics_dir).expect("diagnostics dir");
        std::fs::write(diagnostics_dir.join("logs"), "not a directory").expect("block logs dir");

        // Enqueue a merge entry using the proper API
        let queue = ralph_core::merge_queue::MergeQueue::new(repo_root);
        queue.enqueue("test-loop", "merge prompt").expect("enqueue");

        // Create a fake ralph
        let bin_dir = repo_root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let ralph_path = write_fake_executable(&bin_dir, "ralph", "exit 0");

        // Should not panic even though log file creation fails
        process_pending_merges_with_command(repo_root, ralph_path.as_os_str());
    }
}
