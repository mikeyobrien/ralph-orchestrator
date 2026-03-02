//! CLI commands for the `ralph hooks` namespace.
//!
//! This command surface validates hook configuration and command wiring
//! without starting loop execution.

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use ralph_core::RalphConfig;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{ConfigSource, HatsSource, preflight};

/// Manage hook-related commands.
#[derive(Parser, Debug)]
pub struct HooksArgs {
    #[command(subcommand)]
    pub command: HooksCommands,
}

#[derive(Subcommand, Debug)]
pub enum HooksCommands {
    /// Validate hooks configuration and command wiring
    Validate(ValidateArgs),
}

/// Output format for `ralph hooks validate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HooksValidateFormat {
    Human,
    Json,
}

/// Arguments for `ralph hooks validate`.
#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Output format (human or json)
    #[arg(long, value_enum, default_value_t = HooksValidateFormat::Human)]
    pub format: HooksValidateFormat,
}

#[derive(Debug, Serialize)]
struct HooksValidateReport {
    pass: bool,
    source: String,
    hooks_enabled: bool,
    checked_hooks: usize,
    diagnostics: Vec<HooksDiagnostic>,
}

impl HooksValidateReport {
    fn new(source: String) -> Self {
        Self {
            pass: true,
            source,
            hooks_enabled: false,
            checked_hooks: 0,
            diagnostics: Vec::new(),
        }
    }

    fn push_diagnostic(
        &mut self,
        code: &str,
        message: impl Into<String>,
        phase_event: Option<String>,
        hook: Option<String>,
        command: Option<String>,
    ) {
        self.diagnostics.push(HooksDiagnostic {
            code: code.to_string(),
            message: message.into(),
            phase_event,
            hook,
            command,
        });
        self.pass = false;
    }
}

#[derive(Debug, Serialize)]
struct HooksDiagnostic {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hook: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
}

/// Execute a hooks command.
pub async fn execute(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    args: HooksArgs,
    use_colors: bool,
) -> Result<()> {
    match args.command {
        HooksCommands::Validate(validate_args) => {
            execute_validate(config_sources, hats_source, validate_args, use_colors).await
        }
    }
}

async fn execute_validate(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    args: ValidateArgs,
    use_colors: bool,
) -> Result<()> {
    let report = build_report(config_sources, hats_source).await;

    match args.format {
        HooksValidateFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        HooksValidateFormat::Human => {
            print_human_report(&report, use_colors);
        }
    }

    if !report.pass {
        std::process::exit(1);
    }

    Ok(())
}

async fn build_report(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
) -> HooksValidateReport {
    let source_label = preflight::config_source_label(config_sources, hats_source);
    let mut report = HooksValidateReport::new(source_label);

    let config = match preflight::load_config_for_preflight(config_sources, hats_source).await {
        Ok(config) => config,
        Err(error) => {
            report.push_diagnostic("config.load", error.to_string(), None, None, None);
            return report;
        }
    };

    report.hooks_enabled = config.hooks.enabled;
    report.checked_hooks = count_configured_hooks(&config);

    if let Err(error) = config.validate() {
        report.push_diagnostic("hooks.semantic", error.to_string(), None, None, None);
    }

    validate_duplicate_names(&config, &mut report);
    validate_command_resolvability(&config, &mut report);

    report
}

fn count_configured_hooks(config: &RalphConfig) -> usize {
    config.hooks.events.values().map(Vec::len).sum()
}

fn validate_duplicate_names(config: &RalphConfig, report: &mut HooksValidateReport) {
    let mut phase_events: Vec<_> = config.hooks.events.iter().collect();
    phase_events.sort_by_key(|(phase_event, _)| phase_event.as_str());

    for (phase_event, hooks) in phase_events {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for (index, hook) in hooks.iter().enumerate() {
            let name = hook.name.trim();
            if name.is_empty() {
                continue;
            }

            if let Some(first_index) = seen.insert(name, index) {
                report.push_diagnostic(
                    "hooks.duplicate_name",
                    format!(
                        "Duplicate hook name '{name}' in phase-event '{}': indices [{first_index}] and [{index}]. Hook names must be unique per phase-event.",
                        phase_event.as_str()
                    ),
                    Some(phase_event.as_str().to_string()),
                    Some(name.to_string()),
                    hook.command.first().cloned(),
                );
            }
        }
    }
}

fn validate_command_resolvability(config: &RalphConfig, report: &mut HooksValidateReport) {
    let mut phase_events: Vec<_> = config.hooks.events.iter().collect();
    phase_events.sort_by_key(|(phase_event, _)| phase_event.as_str());

    for (phase_event, hooks) in phase_events {
        for hook in hooks {
            let Some(command) = hook
                .command
                .first()
                .map(|entry| entry.trim())
                .filter(|entry| !entry.is_empty())
            else {
                continue;
            };

            let cwd = resolve_hook_cwd(&config.core.workspace_root, hook.cwd.as_deref());
            let path_override = hook_path_override(&hook.env);

            if let Err(message) = resolve_hook_command(command, &cwd, path_override) {
                report.push_diagnostic(
                    "hooks.command_resolvable",
                    format!(
                        "{message}\nFix: ensure command exists and is executable, or invoke the script through an interpreter (for example: ['bash', 'script.sh'])."
                    ),
                    Some(phase_event.as_str().to_string()),
                    non_empty_trimmed(&hook.name),
                    Some(command.to_string()),
                );
            }
        }
    }
}

fn hook_path_override(env_map: &HashMap<String, String>) -> Option<&str> {
    env_map
        .get("PATH")
        .or_else(|| env_map.get("Path"))
        .map(String::as_str)
}

fn resolve_hook_cwd(workspace_root: &Path, hook_cwd: Option<&Path>) -> PathBuf {
    match hook_cwd {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => workspace_root.join(path),
        None => workspace_root.to_path_buf(),
    }
}

fn resolve_hook_command(
    command: &str,
    cwd: &Path,
    path_override: Option<&str>,
) -> std::result::Result<PathBuf, String> {
    let command_path = Path::new(command);
    if command_path.is_absolute() || command_path.components().count() > 1 {
        let resolved = if command_path.is_absolute() {
            command_path.to_path_buf()
        } else {
            cwd.join(command_path)
        };

        if !resolved.exists() {
            return Err(format!(
                "Command '{command}' resolves to '{}' but the file does not exist.",
                resolved.display()
            ));
        }

        if !is_executable_file(&resolved) {
            return Err(format!(
                "Command '{command}' resolves to '{}' but it is not executable.",
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
                "PATH is not set while resolving command '{command}'. Set PATH in the environment or hook env override."
            )
        })?;

    let extensions = executable_extensions();
    let mut seen_paths = HashSet::new();

    for dir in env::split_paths(&path_value) {
        if !seen_paths.insert(dir.clone()) {
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

    let path_source = if path_override.is_some() {
        "hook env PATH"
    } else {
        "process PATH"
    };

    Err(format!(
        "Command '{command}' was not found in {path_source}."
    ))
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

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn print_human_report(report: &HooksValidateReport, use_colors: bool) {
    use crate::display::colors;

    println!("Hooks validation for {}", report.source);
    println!();
    println!("Hooks enabled: {}", report.hooks_enabled);
    println!("Hooks checked: {}", report.checked_hooks);

    if report.diagnostics.is_empty() {
        println!("Diagnostics: none");
    } else {
        println!("Diagnostics:");
        for diagnostic in &report.diagnostics {
            print_human_diagnostic(diagnostic, use_colors);
        }
    }

    println!();

    let result = if report.pass { "PASS" } else { "FAIL" };
    let detail = if report.diagnostics.is_empty() {
        String::new()
    } else {
        format!(" ({} issue(s))", report.diagnostics.len())
    };

    if use_colors {
        let color = if report.pass {
            colors::GREEN
        } else {
            colors::RED
        };
        println!(
            "Result: {color}{result}{reset}{detail}",
            reset = colors::RESET
        );
    } else {
        println!("Result: {result}{detail}");
    }
}

fn print_human_diagnostic(diagnostic: &HooksDiagnostic, use_colors: bool) {
    use crate::display::colors;

    let status = if use_colors {
        format!("{}FAIL{}", colors::RED, colors::RESET)
    } else {
        "FAIL".to_string()
    };

    let mut lines = diagnostic.message.lines();
    let first_line = lines.next().unwrap_or_default();
    println!("  {status} {}: {first_line}", diagnostic.code);

    for line in lines {
        println!("       {line}");
    }

    if let Some(phase_event) = &diagnostic.phase_event {
        println!("       phase_event: {phase_event}");
    }
    if let Some(hook) = &diagnostic.hook {
        println!("       hook: {hook}");
    }
    if let Some(command) = &diagnostic.command {
        println!("       command: {command}");
    }
}
