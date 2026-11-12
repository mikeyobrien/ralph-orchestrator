use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Process groups of running commands started with `own_process_group`.
///
/// Such a command leads its own process group, so a timeout can kill the
/// whole tree (a shell's children inherit the output pipes and would
/// otherwise keep the run waiting). The flip side is that a terminal Ctrl+C
/// no longer reaches them, so Ralph's signal handlers forward to these groups.
static ACTIVE_PROCESS_GROUPS: LazyLock<Mutex<HashSet<u32>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Unregisters a hook's process group when its run ends, however it ends.
struct ActiveProcessGroup(u32);

impl ActiveProcessGroup {
    fn register(pid: u32) -> Self {
        if let Ok(mut groups) = ACTIVE_PROCESS_GROUPS.lock() {
            groups.insert(pid);
        }
        Self(pid)
    }
}

impl Drop for ActiveProcessGroup {
    fn drop(&mut self) {
        if let Ok(mut groups) = ACTIVE_PROCESS_GROUPS.lock() {
            groups.remove(&self.0);
        }
    }
}

/// Sends `signal` to the process group of every running command that was
/// started with `own_process_group` (script hats, gate commands).
///
/// Call this when Ralph is interrupted: those commands are not in Ralph's
/// process group, so neither a terminal Ctrl+C nor signalling Ralph's own
/// group reaches them.
#[cfg(unix)]
pub fn signal_active_hook_process_groups(signal: nix::sys::signal::Signal) {
    let groups: Vec<u32> = ACTIVE_PROCESS_GROUPS
        .lock()
        .map(|groups| groups.iter().copied().collect())
        .unwrap_or_default();
    for pid in groups {
        if let Ok(pid) = i32::try_from(pid) {
            let _ = nix::sys::signal::killpg(nix::unistd::Pid::from_raw(pid), signal);
        }
    }
}

/// Input contract for executing a single lifecycle hook command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRunRequest {
    /// Canonical lifecycle phase-event key (for example `pre.loop.start`).
    pub phase_event: String,

    /// Stable hook identifier from config (`hooks.events.<phase>[].name`).
    pub hook_name: String,

    /// Command argv (`command[0]` executable + args).
    pub command: Vec<String>,

    /// Project workspace root used as the base for relative cwd resolution.
    pub workspace_root: PathBuf,

    /// Optional per-hook working directory override.
    pub cwd: Option<PathBuf>,

    /// Optional per-hook environment variable overrides.
    pub env: HashMap<String, String>,

    /// Hook timeout guardrail in seconds.
    pub timeout_seconds: u64,

    /// Max captured bytes per output stream.
    pub max_output_bytes: u64,

    /// JSON lifecycle payload that will be written to stdin.
    pub stdin_payload: serde_json::Value,

    /// Run the command in its own process group, so a timeout kills its
    /// descendants too. Only for commands that start after Ralph's signal
    /// handlers are installed (they forward interrupts to these groups via
    /// `signal_active_hook_process_groups`). Lifecycle hooks leave it off so
    /// a terminal Ctrl+C keeps reaching them directly, including the
    /// `pre.loop.start` hooks that run before those handlers exist.
    #[serde(default)]
    pub own_process_group: bool,
}

/// Captured hook stream output with truncation metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookStreamOutput {
    /// Captured UTF-8 output text.
    pub content: String,

    /// Whether the captured output was truncated.
    pub truncated: bool,
}

/// Structured outcome for one hook invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRunResult {
    /// Hook execution start time.
    pub started_at: DateTime<Utc>,

    /// Hook execution end time.
    pub ended_at: DateTime<Utc>,

    /// Total wall-clock duration in milliseconds.
    pub duration_ms: u64,

    /// Process exit code (None when terminated by signal/timeout without code).
    pub exit_code: Option<i32>,

    /// Whether execution hit timeout enforcement.
    pub timed_out: bool,

    /// Captured/truncated stdout.
    pub stdout: HookStreamOutput,

    /// Captured/truncated stderr.
    pub stderr: HookStreamOutput,
}

/// Hook executor errors.
#[derive(Debug, thiserror::Error)]
pub enum HookExecutorError {
    /// Command argv is missing an executable entry.
    #[error("hook '{hook_name}' for phase-event '{phase_event}' has an empty command argv")]
    EmptyCommand {
        phase_event: String,
        hook_name: String,
    },

    /// Command argv executable could not be resolved to a launchable binary path.
    #[error(
        "hook '{hook_name}' for phase-event '{phase_event}' command '{command}' could not be resolved: {reason}"
    )]
    CommandResolution {
        phase_event: String,
        hook_name: String,
        command: String,
        reason: String,
    },

    /// Process spawn failed after command/cwd/env resolution.
    #[error(
        "failed to spawn hook '{hook_name}' for phase-event '{phase_event}' with command '{command}' (cwd: {cwd}): {source}"
    )]
    Spawn {
        phase_event: String,
        hook_name: String,
        command: String,
        cwd: String,
        #[source]
        source: io::Error,
    },

    /// Serializing the JSON stdin payload failed.
    #[error(
        "failed to serialize stdin payload for hook '{hook_name}' phase-event '{phase_event}' with command '{command}': {source}"
    )]
    StdinSerialize {
        phase_event: String,
        hook_name: String,
        command: String,
        #[source]
        source: serde_json::Error,
    },

    /// Writing stdin payload bytes to the child process failed.
    #[error(
        "failed to write stdin payload for hook '{hook_name}' phase-event '{phase_event}' with command '{command}': {source}"
    )]
    StdinWrite {
        phase_event: String,
        hook_name: String,
        command: String,
        #[source]
        source: io::Error,
    },

    /// Timeout enforcement attempted to terminate the process but kill failed.
    #[error(
        "hook '{hook_name}' for phase-event '{phase_event}' exceeded timeout ({timeout_seconds}s) and could not be terminated (command: '{command}'): {source}"
    )]
    TimeoutTerminate {
        phase_event: String,
        hook_name: String,
        command: String,
        timeout_seconds: u64,
        #[source]
        source: io::Error,
    },

    /// Reading captured stdout/stderr bytes failed.
    #[error(
        "failed to capture {stream} for hook '{hook_name}' phase-event '{phase_event}' with command '{command}': {source}"
    )]
    OutputRead {
        phase_event: String,
        hook_name: String,
        command: String,
        stream: &'static str,
        #[source]
        source: io::Error,
    },

    /// Output collector thread panicked while reading stdout/stderr.
    #[error(
        "hook '{hook_name}' phase-event '{phase_event}' output collector for {stream} panicked (command: '{command}')"
    )]
    OutputCollectorJoin {
        phase_event: String,
        hook_name: String,
        command: String,
        stream: &'static str,
    },

    /// Waiting for spawned process completion failed.
    #[error(
        "failed while waiting for hook '{hook_name}' for phase-event '{phase_event}' with command '{command}': {source}"
    )]
    Wait {
        phase_event: String,
        hook_name: String,
        command: String,
        #[source]
        source: io::Error,
    },
}

/// Contract for executing one hook run request.
pub trait HookExecutorContract {
    /// Executes a hook command invocation.
    fn run(&self, request: HookRunRequest) -> Result<HookRunResult, HookExecutorError>;
}

/// Default hook executor implementation.
#[derive(Debug, Clone, Default)]
pub struct HookExecutor;

impl HookExecutor {
    /// Creates a new hook executor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl HookExecutorContract for HookExecutor {
    fn run(&self, request: HookRunRequest) -> Result<HookRunResult, HookExecutorError> {
        let started_at = Utc::now();
        let resolved_cwd = resolve_hook_cwd(&request.workspace_root, request.cwd.as_deref());

        let executable = request
            .command
            .first()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| HookExecutorError::EmptyCommand {
                phase_event: request.phase_event.clone(),
                hook_name: request.hook_name.clone(),
            })?;

        let resolved_command =
            resolve_hook_command(executable, &resolved_cwd, hook_path_override(&request.env))
                .map_err(|reason| HookExecutorError::CommandResolution {
                    phase_event: request.phase_event.clone(),
                    hook_name: request.hook_name.clone(),
                    command: executable.to_string(),
                    reason,
                })?;

        let command_display = request.command.join(" ");

        let mut command = Command::new(&resolved_command);
        command.args(request.command.iter().skip(1));
        command.current_dir(&resolved_cwd);
        command.envs(&request.env);
        #[cfg(unix)]
        if request.own_process_group {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        // Step 3.3 wires JSON stdin payload delivery.
        command.stdin(Stdio::piped());

        // Step 3.4 captures stdout/stderr with deterministic truncation.
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        // Retry on ETXTBSY: the kernel defers fput() from close(), so exec()
        // can briefly see a stale write-count on the inode. A single retry
        // after yielding the thread is sufficient for the task_work to drain.
        let mut child = None;
        for attempt in 0..3 {
            match command.spawn() {
                Ok(c) => {
                    child = Some(c);
                    break;
                }
                Err(e) if e.raw_os_error() == Some(26 /* ETXTBSY */) && attempt < 2 => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(source) => {
                    return Err(HookExecutorError::Spawn {
                        phase_event: request.phase_event.clone(),
                        hook_name: request.hook_name.clone(),
                        command: command_display,
                        cwd: resolved_cwd.display().to_string(),
                        source,
                    });
                }
            }
        }
        let mut child = child.expect("spawn loop must break or return");
        let _active_group = request
            .own_process_group
            .then(|| ActiveProcessGroup::register(child.id()));

        write_stdin_payload(
            &mut child,
            &request.stdin_payload,
            &request.phase_event,
            &request.hook_name,
            &command_display,
        )?;

        let stdout_collector =
            spawn_stream_collector(child.stdout.take(), request.max_output_bytes);
        let stderr_collector =
            spawn_stream_collector(child.stderr.take(), request.max_output_bytes);

        let (status, timed_out) = wait_for_completion(
            &mut child,
            request.own_process_group,
            request.timeout_seconds,
            &request.phase_event,
            &request.hook_name,
            &command_display,
        )?;

        let stdout = collect_stream_output(
            stdout_collector,
            "stdout",
            &request.phase_event,
            &request.hook_name,
            &command_display,
        )?;
        let stderr = collect_stream_output(
            stderr_collector,
            "stderr",
            &request.phase_event,
            &request.hook_name,
            &command_display,
        )?;

        let ended_at = Utc::now();

        Ok(HookRunResult {
            started_at,
            ended_at,
            duration_ms: duration_ms(started_at, ended_at),
            exit_code: status.code(),
            timed_out,
            stdout,
            stderr,
        })
    }
}

const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const STREAM_READ_BUFFER_BYTES: usize = 4096;

type StreamCollector = thread::JoinHandle<io::Result<HookStreamOutput>>;

fn write_stdin_payload(
    child: &mut Child,
    stdin_payload: &serde_json::Value,
    phase_event: &str,
    hook_name: &str,
    command: &str,
) -> Result<(), HookExecutorError> {
    let Some(mut stdin) = child.stdin.take() else {
        return Ok(());
    };

    let payload =
        serde_json::to_vec(stdin_payload).map_err(|source| HookExecutorError::StdinSerialize {
            phase_event: phase_event.to_string(),
            hook_name: hook_name.to_string(),
            command: command.to_string(),
            source,
        })?;

    if let Err(source) = stdin.write_all(&payload)
        && source.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(HookExecutorError::StdinWrite {
            phase_event: phase_event.to_string(),
            hook_name: hook_name.to_string(),
            command: command.to_string(),
            source,
        });
    }

    if let Err(source) = stdin.flush()
        && source.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(HookExecutorError::StdinWrite {
            phase_event: phase_event.to_string(),
            hook_name: hook_name.to_string(),
            command: command.to_string(),
            source,
        });
    }

    Ok(())
}

fn wait_for_completion(
    child: &mut Child,
    own_process_group: bool,
    timeout_seconds: u64,
    phase_event: &str,
    hook_name: &str,
    command: &str,
) -> Result<(ExitStatus, bool), HookExecutorError> {
    let timeout = Duration::from_secs(timeout_seconds);
    let wait_started_at = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status, false)),
            Ok(None) => {
                if wait_started_at.elapsed() >= timeout {
                    let status = terminate_for_timeout(
                        child,
                        own_process_group,
                        timeout_seconds,
                        phase_event,
                        hook_name,
                        command,
                    )?;
                    return Ok((status, true));
                }

                let elapsed = wait_started_at.elapsed();
                let remaining = timeout.saturating_sub(elapsed);
                thread::sleep(remaining.min(WAIT_POLL_INTERVAL));
            }
            Err(source) => {
                return Err(HookExecutorError::Wait {
                    phase_event: phase_event.to_string(),
                    hook_name: hook_name.to_string(),
                    command: command.to_string(),
                    source,
                });
            }
        }
    }
}

fn terminate_for_timeout(
    child: &mut Child,
    own_process_group: bool,
    timeout_seconds: u64,
    phase_event: &str,
    hook_name: &str,
    command: &str,
) -> Result<ExitStatus, HookExecutorError> {
    // A child leading its own process group: kill the group so descendants
    // holding the output pipes die too. `child.kill()` below still covers
    // the leader if the group signal fails.
    #[cfg(unix)]
    if own_process_group && let Ok(pid) = i32::try_from(child.id()) {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }

    if let Err(source) = child.kill() {
        if let Ok(Some(status)) = child.try_wait() {
            return Ok(status);
        }

        return Err(HookExecutorError::TimeoutTerminate {
            phase_event: phase_event.to_string(),
            hook_name: hook_name.to_string(),
            command: command.to_string(),
            timeout_seconds,
            source,
        });
    }

    child.wait().map_err(|source| HookExecutorError::Wait {
        phase_event: phase_event.to_string(),
        hook_name: hook_name.to_string(),
        command: command.to_string(),
        source,
    })
}

fn spawn_stream_collector<R>(stream: Option<R>, max_output_bytes: u64) -> StreamCollector
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let Some(reader) = stream else {
            return Ok(HookStreamOutput::default());
        };

        capture_stream_output(reader, max_output_bytes)
    })
}

fn collect_stream_output(
    collector: StreamCollector,
    stream: &'static str,
    phase_event: &str,
    hook_name: &str,
    command: &str,
) -> Result<HookStreamOutput, HookExecutorError> {
    let captured = collector
        .join()
        .map_err(|_| HookExecutorError::OutputCollectorJoin {
            phase_event: phase_event.to_string(),
            hook_name: hook_name.to_string(),
            command: command.to_string(),
            stream,
        })?;

    captured.map_err(|source| HookExecutorError::OutputRead {
        phase_event: phase_event.to_string(),
        hook_name: hook_name.to_string(),
        command: command.to_string(),
        stream,
        source,
    })
}

fn capture_stream_output<R: Read>(
    mut reader: R,
    max_output_bytes: u64,
) -> io::Result<HookStreamOutput> {
    let capture_limit = usize::try_from(max_output_bytes).unwrap_or(usize::MAX);
    let mut captured = Vec::with_capacity(capture_limit.min(STREAM_READ_BUFFER_BYTES));
    let mut truncated = false;
    let mut buffer = [0_u8; STREAM_READ_BUFFER_BYTES];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        if captured.len() < capture_limit {
            let remaining = capture_limit - captured.len();
            let to_copy = remaining.min(bytes_read);
            captured.extend_from_slice(&buffer[..to_copy]);

            if to_copy < bytes_read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    if let Err(error) = std::str::from_utf8(&captured)
        && error.error_len().is_none()
    {
        captured.truncate(error.valid_up_to());
    }

    Ok(HookStreamOutput {
        content: String::from_utf8_lossy(&captured).into_owned(),
        truncated,
    })
}

fn resolve_hook_cwd(workspace_root: &Path, hook_cwd: Option<&Path>) -> PathBuf {
    match hook_cwd {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => workspace_root.join(path),
        None => workspace_root.to_path_buf(),
    }
}

fn hook_path_override(env_map: &HashMap<String, String>) -> Option<&str> {
    env_map
        .get("PATH")
        .or_else(|| env_map.get("Path"))
        .map(String::as_str)
}

fn resolve_hook_command(
    command: &str,
    cwd: &Path,
    path_override: Option<&str>,
) -> Result<PathBuf, String> {
    let command_path = Path::new(command);
    if command_path.is_absolute() || command_path.components().count() > 1 {
        let resolved = if command_path.is_absolute() {
            command_path.to_path_buf()
        } else {
            cwd.join(command_path)
        };

        if !resolved.exists() {
            return Err(format!(
                "command '{command}' resolves to '{}' but the file does not exist",
                resolved.display()
            ));
        }

        if !is_executable_file(&resolved) {
            return Err(format!(
                "command '{command}' resolves to '{}' but it is not executable",
                resolved.display()
            ));
        }

        return Ok(resolved);
    }

    let path_value = path_override
        .map(OsString::from)
        .or_else(|| env::var_os("PATH"))
        .ok_or_else(|| {
            format!(
                "PATH is not set while resolving command '{command}'; set PATH or provide an absolute/relative path"
            )
        })?;

    let mut visited = HashSet::new();
    let extensions = executable_extensions();

    for dir in env::split_paths(&path_value) {
        if !visited.insert(dir.clone()) {
            continue;
        }

        for extension in &extensions {
            let candidate = if extension.is_empty() {
                dir.join(command)
            } else {
                dir.join(format!("{command}{}", extension.to_string_lossy()))
            };

            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }

    Err(format!("command '{command}' was not found in PATH"))
}

fn executable_extensions() -> Vec<OsString> {
    if cfg!(windows) {
        let exts = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        exts.split(';')
            .filter(|ext| !ext.trim().is_empty())
            .map(|ext| OsString::from(ext.trim().to_string()))
            .collect()
    } else {
        vec![OsString::new()]
    }
}

fn is_executable_file(path: &Path) -> bool {
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

fn duration_ms(started_at: DateTime<Utc>, ended_at: DateTime<Utc>) -> u64 {
    let milliseconds = ended_at
        .signed_duration_since(started_at)
        .num_milliseconds();
    if milliseconds <= 0 {
        return 0;
    }

    u64::try_from(milliseconds).unwrap_or(u64::MAX)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::{TempDir, tempdir};

    fn write_executable_script(temp_dir: &TempDir, file_name: &str, body: &str) -> PathBuf {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let script_path = temp_dir.path().join(file_name);
        let script = format!("#!/bin/sh\nset -eu\n{body}\n");

        // Set mode at creation time and sync before close to avoid the
        // ETXTBSY race (deferred fput between close and exec).
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o755)
                .open(&script_path)
                .expect("create script file");
            file.write_all(script.as_bytes())
                .expect("write script file");
            file.sync_all().expect("sync script file");
        }

        // Force the kernel to process the deferred fput from close() above
        // by issuing another syscall that touches the same inode.
        let _ = fs::metadata(&script_path).expect("stat script");

        script_path
    }

    fn request_with_command(workspace_root: &Path, command: Vec<String>) -> HookRunRequest {
        HookRunRequest {
            phase_event: "pre.loop.start".to_string(),
            hook_name: "test-hook".to_string(),
            command,
            workspace_root: workspace_root.to_path_buf(),
            cwd: None,
            env: HashMap::new(),
            timeout_seconds: 2,
            max_output_bytes: 1024,
            stdin_payload: json!({"schema_version": 1, "phase_event": "pre.loop.start"}),
            own_process_group: false,
        }
    }

    #[test]
    fn run_reports_successful_exit_and_stream_content() {
        let temp_dir = tempdir().expect("tempdir");
        let script_path = write_executable_script(
            &temp_dir,
            "success.sh",
            "printf 'ok-stdout'\nprintf 'ok-stderr' >&2",
        );

        let request = request_with_command(
            temp_dir.path(),
            vec![script_path.to_string_lossy().into_owned()],
        );

        let result = HookExecutor::new().run(request).expect("hook run succeeds");

        assert_eq!(result.exit_code, Some(0));
        assert!(!result.timed_out);
        assert_eq!(result.stdout.content, "ok-stdout");
        assert!(!result.stdout.truncated);
        assert_eq!(result.stderr.content, "ok-stderr");
        assert!(!result.stderr.truncated);
        assert!(result.ended_at >= result.started_at);
    }

    #[test]
    fn run_preserves_non_zero_exit_code_without_timeout() {
        let temp_dir = tempdir().expect("tempdir");
        let script_path = write_executable_script(
            &temp_dir,
            "nonzero.sh",
            "printf 'failing-hook' >&2\nexit 17",
        );

        let request = request_with_command(
            temp_dir.path(),
            vec![script_path.to_string_lossy().into_owned()],
        );

        let result = HookExecutor::new()
            .run(request)
            .expect("hook run completes");

        assert_eq!(result.exit_code, Some(17));
        assert!(!result.timed_out);
        assert_eq!(result.stderr.content, "failing-hook");
        assert!(!result.stderr.truncated);
    }

    #[test]
    fn run_marks_timed_out_when_command_exceeds_timeout() {
        let temp_dir = tempdir().expect("tempdir");
        let script_path = write_executable_script(&temp_dir, "timeout.sh", "while :; do :; done");

        let mut request = request_with_command(
            temp_dir.path(),
            vec![script_path.to_string_lossy().into_owned()],
        );
        request.timeout_seconds = 1;

        let result = HookExecutor::new()
            .run(request)
            .expect("hook run completes");

        assert!(result.timed_out);
        assert_ne!(result.exit_code, Some(0));
    }

    #[test]
    fn hooks_stay_in_ralphs_process_group_unless_asked() {
        let temp_dir = tempdir().expect("tempdir");
        let ralph_group = nix::unistd::getpgrp().to_string();
        for (own_group, same_group) in [(false, true), (true, false)] {
            let mut request = request_with_command(
                temp_dir.path(),
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    // Builtins only on Linux (field 5 of /proc/<pid>/stat is
                    // the pgid): spawning `ps` under a loaded parallel suite
                    // could outlast the timeout and print nothing.
                    "if [ -r /proc/$$/stat ]; then read -r stat < /proc/$$/stat; \
                     set -- $stat; echo \"$5\"; else ps -o pgid= -p $$; fi"
                        .to_string(),
                ],
            );
            // Not a timeout test: leave a loaded machine plenty of room.
            request.timeout_seconds = 30;
            request.own_process_group = own_group;
            let result = HookExecutor::new().run(request).expect("hook runs");
            assert!(
                result.exit_code == Some(0) && !result.timed_out,
                "own_process_group={own_group}: exit {:?}, timed_out {}, stderr {:?}",
                result.exit_code,
                result.timed_out,
                result.stderr.content
            );
            assert_eq!(
                result.stdout.content.trim() == ralph_group,
                same_group,
                "own_process_group={own_group}: child pgid {} vs ralph {ralph_group}",
                result.stdout.content.trim()
            );
        }
    }

    #[test]
    fn timeout_kills_the_whole_process_group() {
        // `sleep` is a grandchild that inherits the output pipes. Killing only
        // the shell leaves it holding them, so the run would block until it
        // exits on its own.
        let temp_dir = tempdir().expect("tempdir");
        let mut request = request_with_command(
            temp_dir.path(),
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "echo started; sleep 30; echo done".to_string(),
            ],
        );
        // 3s lets a loaded machine start the shell and print before the
        // kill; 20s is still well short of the grandchild's 30s sleep.
        request.timeout_seconds = 3;
        request.own_process_group = true;

        let started = Instant::now();
        let result = HookExecutor::new()
            .run(request)
            .expect("hook run completes");

        assert!(result.timed_out);
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "timed-out run took {:?}",
            started.elapsed()
        );
        assert_eq!(result.stdout.content, "started\n");
    }

    #[test]
    fn run_truncates_stdout_and_stderr_at_max_output_bytes() {
        let temp_dir = tempdir().expect("tempdir");
        let script_path = write_executable_script(
            &temp_dir,
            "truncate.sh",
            "printf '1234567890'\nprintf 'abcdefghij' >&2",
        );

        let mut request = request_with_command(
            temp_dir.path(),
            vec![script_path.to_string_lossy().into_owned()],
        );
        request.max_output_bytes = 8;

        let result = HookExecutor::new().run(request).expect("hook run succeeds");

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.content, "12345678");
        assert!(result.stdout.truncated);
        assert_eq!(result.stderr.content, "abcdefgh");
        assert!(result.stderr.truncated);
    }

    #[test]
    fn run_writes_json_payload_to_hook_stdin() {
        let temp_dir = tempdir().expect("tempdir");
        let script_path = write_executable_script(&temp_dir, "stdin.sh", "cat > \"$1\"");
        let captured_path = temp_dir.path().join("stdin-captured.json");

        let mut request = request_with_command(
            temp_dir.path(),
            vec![
                script_path.to_string_lossy().into_owned(),
                captured_path.to_string_lossy().into_owned(),
            ],
        );
        let payload = json!({
            "schema_version": 1,
            "phase": "pre",
            "event": "loop.start",
            "loop": {"id": "loop-test", "is_primary": true}
        });
        request.stdin_payload = payload.clone();

        let result = HookExecutor::new().run(request).expect("hook run succeeds");

        assert_eq!(result.exit_code, Some(0));
        assert!(!result.timed_out);

        let written_payload = fs::read_to_string(captured_path).expect("read captured stdin");
        let parsed_payload: serde_json::Value =
            serde_json::from_str(&written_payload).expect("parse captured stdin json");

        assert_eq!(parsed_payload, payload);
    }
}
