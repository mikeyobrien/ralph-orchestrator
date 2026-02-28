use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
        source: std::io::Error,
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
        source: std::io::Error,
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

        let mut command = Command::new(&resolved_command);
        command.args(request.command.iter().skip(1));
        command.current_dir(&resolved_cwd);
        command.envs(&request.env);

        // Step 3.3 wires JSON stdin payload delivery.
        command.stdin(Stdio::null());

        // Step 3.4 adds stdout/stderr capture + truncation.
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        let mut child = command.spawn().map_err(|source| HookExecutorError::Spawn {
            phase_event: request.phase_event.clone(),
            hook_name: request.hook_name.clone(),
            command: request.command.join(" "),
            cwd: resolved_cwd.display().to_string(),
            source,
        })?;

        let status = child.wait().map_err(|source| HookExecutorError::Wait {
            phase_event: request.phase_event.clone(),
            hook_name: request.hook_name.clone(),
            command: request.command.join(" "),
            source,
        })?;

        let ended_at = Utc::now();

        Ok(HookRunResult {
            started_at,
            ended_at,
            duration_ms: duration_ms(started_at, ended_at),
            exit_code: status.code(),
            timed_out: false,
            stdout: HookStreamOutput::default(),
            stderr: HookStreamOutput::default(),
        })
    }
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
