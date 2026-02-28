use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

        // Step 3.3 wires JSON stdin payload delivery.
        command.stdin(Stdio::piped());

        // Step 3.4 captures stdout/stderr with deterministic truncation.
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|source| HookExecutorError::Spawn {
            phase_event: request.phase_event.clone(),
            hook_name: request.hook_name.clone(),
            command: command_display.clone(),
            cwd: resolved_cwd.display().to_string(),
            source,
        })?;

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
    timeout_seconds: u64,
    phase_event: &str,
    hook_name: &str,
    command: &str,
) -> Result<ExitStatus, HookExecutorError> {
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
