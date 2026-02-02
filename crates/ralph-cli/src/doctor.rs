//! CLI command for `ralph doctor`.

use anyhow::Result;
use clap::Parser;
use ralph_adapters::{CliBackend, DEFAULT_PRIORITY};
use ralph_core::{CheckResult, CheckStatus, ConfigError, HatBackend, PreflightReport, RalphConfig};
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use crate::ConfigSource;

/// Run first-run diagnostics and environment validation.
#[derive(Parser, Debug)]
pub struct DoctorArgs {}

pub async fn execute(
    config_sources: &[ConfigSource],
    _args: DoctorArgs,
    use_colors: bool,
) -> Result<()> {
    let source_label = crate::preflight::config_source_label(config_sources);
    let config = crate::preflight::load_config_for_preflight(config_sources).await?;

    let runner = ralph_core::PreflightRunner::default_checks();
    let preflight_report = runner.run_all(&config).await;

    let mut config_check = None;
    let mut other_checks = Vec::new();
    for check in preflight_report.checks {
        match check.name.as_str() {
            "config" => config_check = Some(check),
            "backend" => {}
            _ => other_checks.push(check),
        }
    }

    let mut checks = Vec::new();
    if let Some(check) = config_check {
        checks.push(check);
    }

    checks.push(hat_collection_check(&config));

    let backend_checks = backend_checks(&config, command_version_ok, command_exists);
    checks.extend(backend_checks);

    let auth_backends = auth_backend_names(&config);
    checks.push(auth_hint_check(&auth_backends, |key| env::var(key).ok()));

    checks.extend(other_checks);

    let report = report_from_checks(checks);
    print_human_report(&report, &source_label, use_colors);

    if report.failures > 0 {
        std::process::exit(1);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CommandCheckMode {
    Version,
    PathOnly,
}

fn backend_checks<F, G>(
    _config: &RalphConfig,
    _command_version_ok: F,
    _command_exists: G,
) -> Vec<CheckResult>
where
    F: Fn(&str) -> bool,
    G: Fn(&str) -> bool,
{
    let config = _config;
    let command_version_ok = _command_version_ok;
    let command_exists = _command_exists;

    let mut checks = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    match config.cli.backend.trim() {
        "auto" => {
            for backend in DEFAULT_PRIORITY {
                let command = command_for_backend(backend);
                push_backend_check(
                    &mut checks,
                    &mut seen,
                    backend,
                    &command,
                    false,
                    CommandCheckMode::Version,
                    &command_version_ok,
                    &command_exists,
                    None,
                );
            }

            let any_available = checks.iter().any(|check| check.status == CheckStatus::Pass);

            let summary = if any_available {
                CheckResult::pass("backend:auto", "Auto backend available")
            } else {
                CheckResult::fail(
                    "backend:auto",
                    "No supported backend found",
                    format!("Checked: {}", DEFAULT_PRIORITY.join(", ")),
                )
            };
            checks.push(summary);
        }
        "custom" => {
            let command = config.cli.command.clone().unwrap_or_default();
            if command.trim().is_empty() {
                checks.push(CheckResult::fail(
                    "backend:custom",
                    "Custom backend command missing",
                    "Set cli.command in ralph.yml",
                ));
            } else {
                let backend = canonical_backend_name("custom", Some(&command));
                push_backend_check(
                    &mut checks,
                    &mut seen,
                    &backend,
                    &command,
                    true,
                    CommandCheckMode::PathOnly,
                    &command_version_ok,
                    &command_exists,
                    None,
                );
            }
        }
        backend => {
            let backend = backend.trim().to_lowercase();
            match command_for_named_backend(&backend, config.cli.command.as_deref()) {
                Ok(command) => {
                    push_backend_check(
                        &mut checks,
                        &mut seen,
                        &backend,
                        &command,
                        true,
                        CommandCheckMode::Version,
                        &command_version_ok,
                        &command_exists,
                        None,
                    );
                }
                Err(err) => {
                    checks.push(CheckResult::fail(
                        &format!("backend:{backend}"),
                        "Unknown backend",
                        err,
                    ));
                }
            }
        }
    }

    for (hat_id, hat_config) in &config.hats {
        let Some(hat_backend) = &hat_config.backend else {
            continue;
        };

        let check_mode = match hat_backend {
            HatBackend::Custom { .. } => CommandCheckMode::PathOnly,
            _ => CommandCheckMode::Version,
        };

        match CliBackend::from_hat_backend(hat_backend) {
            Ok(cli_backend) => {
                let backend_name = canonical_backend_name(
                    &hat_backend.to_cli_backend(),
                    Some(cli_backend.command.as_str()),
                );
                push_backend_check(
                    &mut checks,
                    &mut seen,
                    &backend_name,
                    &cli_backend.command,
                    true,
                    check_mode,
                    &command_version_ok,
                    &command_exists,
                    None,
                );
            }
            Err(_) => {
                checks.push(CheckResult::fail(
                    &format!("backend:hat:{hat_id}"),
                    "Unknown hat backend",
                    format!("Hat '{hat_id}' specifies an unknown backend"),
                ));
            }
        }
    }

    checks
}

fn push_backend_check<F, G>(
    checks: &mut Vec<CheckResult>,
    seen: &mut HashSet<String>,
    backend: &str,
    command: &str,
    required: bool,
    check_mode: CommandCheckMode,
    command_version_ok: &F,
    command_exists: &G,
    detail: Option<String>,
) where
    F: Fn(&str) -> bool,
    G: Fn(&str) -> bool,
{
    let name = backend_check_name(backend, command);
    if !seen.insert(name.clone()) {
        return;
    }

    let available = match check_mode {
        CommandCheckMode::Version => command_version_ok(command),
        CommandCheckMode::PathOnly => command_exists(command),
    };

    let status = if available {
        CheckStatus::Pass
    } else if required {
        CheckStatus::Fail
    } else {
        CheckStatus::Warn
    };

    let label = match status {
        CheckStatus::Pass => format!("{backend} CLI available ({command})"),
        CheckStatus::Warn => format!("{backend} CLI missing (optional for auto)"),
        CheckStatus::Fail => format!("{backend} CLI missing"),
    };

    let message = if available {
        None
    } else if let Some(detail) = detail {
        Some(detail)
    } else {
        Some(format!("Command not found or not executable: {command}"))
    };

    checks.push(CheckResult {
        name,
        label,
        status,
        message,
    });
}

fn auth_hint_check<F>(_backends: &[String], _env_lookup: F) -> CheckResult
where
    F: Fn(&str) -> Option<String>,
{
    let env_lookup = _env_lookup;
    let mut missing = Vec::new();

    let mut backends: Vec<String> = _backends
        .iter()
        .map(|backend| backend.trim().to_lowercase())
        .collect();
    backends.sort();
    backends.dedup();

    for backend in backends {
        let Some(envs) = auth_env_vars(&backend) else {
            missing.push(format!("{backend}: authenticate via the CLI"));
            continue;
        };

        if envs.iter().any(|key| env_lookup(key).is_some()) {
            continue;
        }

        missing.push(format!("{backend}: set {}", envs.join(" or ")));
    }

    if missing.is_empty() {
        CheckResult::pass("auth", "Auth hints satisfied")
    } else {
        CheckResult::warn(
            "auth",
            "Authentication not detected for some backends",
            missing.join("\n"),
        )
    }
}

fn hat_collection_check(_config: &RalphConfig) -> CheckResult {
    let config = _config;

    match config.validate() {
        Ok(_) => {
            if config.hats.is_empty() {
                CheckResult::pass("hats", "No custom hats configured (solo mode)")
            } else {
                CheckResult::pass(
                    "hats",
                    format!("Hat collection parsed ({} hat(s))", config.hats.len()),
                )
            }
        }
        Err(err) => match err {
            ConfigError::AmbiguousRouting { .. }
            | ConfigError::ReservedTrigger { .. }
            | ConfigError::MissingDescription { .. } => {
                CheckResult::fail("hats", "Hat collection invalid", err.to_string())
            }
            _ => CheckResult::pass("hats", "Hat collection parsed"),
        },
    }
}

fn auth_backend_names(config: &RalphConfig) -> Vec<String> {
    let mut names = HashSet::new();

    match config.cli.backend.trim() {
        "auto" => {
            for backend in DEFAULT_PRIORITY {
                names.insert((*backend).to_string());
            }
        }
        "custom" => {
            if let Some(command) = config.cli.command.as_deref() {
                names.insert(canonical_backend_name("custom", Some(command)));
            } else {
                names.insert("custom".to_string());
            }
        }
        backend => {
            names.insert(backend.to_lowercase());
        }
    }

    for hat in config.hats.values() {
        let Some(backend) = &hat.backend else {
            continue;
        };

        let name = match backend {
            HatBackend::Named(name) => name.clone(),
            HatBackend::NamedWithArgs { backend_type, .. } => backend_type.clone(),
            HatBackend::KiroAgent { .. } => "kiro".to_string(),
            HatBackend::Custom { command, .. } => canonical_backend_name("custom", Some(command)),
        };

        names.insert(name.to_lowercase());
    }

    names.into_iter().collect()
}

fn auth_env_vars(backend: &str) -> Option<Vec<&'static str>> {
    match backend {
        "claude" => Some(vec!["ANTHROPIC_API_KEY"]),
        "gemini" => Some(vec!["GEMINI_API_KEY"]),
        "codex" => Some(vec!["OPENAI_API_KEY", "CODEX_API_KEY"]),
        "kiro" => Some(vec!["KIRO_API_KEY"]),
        "opencode" => Some(vec![
            "OPENCODE_API_KEY",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
        ]),
        _ => None,
    }
}

fn command_for_backend(backend: &str) -> String {
    CliBackend::from_name(backend)
        .map(|backend| backend.command)
        .unwrap_or_else(|_| backend.to_string())
}

fn command_for_named_backend(
    backend: &str,
    command_override: Option<&str>,
) -> Result<String, String> {
    let backend = backend.trim().to_lowercase();
    if let Some(command) = command_override
        && !command.trim().is_empty()
    {
        return Ok(command.to_string());
    }

    CliBackend::from_name(&backend)
        .map(|backend| backend.command)
        .map_err(|_| format!("Unknown backend: {backend}"))
}

fn canonical_backend_name(backend: &str, command: Option<&str>) -> String {
    if backend != "custom" {
        return backend.to_lowercase();
    }

    let Some(command) = command else {
        return "custom".to_string();
    };

    let basename = Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command);

    let mut normalized = basename.to_string();
    let normalized_lower = normalized.to_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".com"] {
        if normalized_lower.ends_with(ext) {
            let new_len = normalized.len().saturating_sub(ext.len());
            normalized.truncate(new_len);
            break;
        }
    }

    let normalized_lower = normalized.to_lowercase();
    match normalized_lower.as_str() {
        "kiro-cli" => "kiro".to_string(),
        "claude" => "claude".to_string(),
        "gemini" => "gemini".to_string(),
        "codex" => "codex".to_string(),
        "amp" => "amp".to_string(),
        "copilot" => "copilot".to_string(),
        "opencode" => "opencode".to_string(),
        _ => normalized,
    }
}

fn backend_check_name(backend: &str, command: &str) -> String {
    if backend.eq_ignore_ascii_case(command) {
        format!("backend:{backend}")
    } else {
        format!("backend:{backend}@{command}")
    }
}

fn command_version_ok(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn command_exists(command: &str) -> bool {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return path.is_file();
    }

    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };
    let extensions = executable_extensions();

    for dir in env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = if ext.is_empty() {
                dir.join(command)
            } else {
                dir.join(format!("{}{}", command, ext.to_string_lossy()))
            };

            if candidate.is_file() {
                return true;
            }
        }
    }

    false
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

fn report_from_checks(checks: Vec<CheckResult>) -> PreflightReport {
    let warnings = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Warn)
        .count();
    let failures = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Fail)
        .count();

    PreflightReport {
        passed: failures == 0,
        warnings,
        failures,
        checks,
    }
}

fn print_human_report(report: &PreflightReport, source: &str, use_colors: bool) {
    use crate::display::colors;

    println!("Doctor checks for {}", source);
    println!();

    let name_width = report
        .checks
        .iter()
        .map(|check| check.name.len())
        .max()
        .unwrap_or(4)
        .max(4);

    for check in &report.checks {
        print_check_line(check, name_width, use_colors);
    }

    println!();

    let result = if report.passed { "PASS" } else { "FAIL" };
    let mut details = Vec::new();
    if report.failures > 0 {
        details.push(format!("{} failure(s)", report.failures));
    }
    if report.warnings > 0 {
        details.push(format!("{} warning(s)", report.warnings));
    }

    let detail_text = if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join(", "))
    };

    if use_colors {
        let color = if report.passed {
            colors::GREEN
        } else {
            colors::RED
        };
        println!(
            "Result: {color}{result}{reset}{detail}",
            reset = colors::RESET,
            detail = detail_text
        );
    } else {
        println!("Result: {result}{detail}", detail = detail_text);
    }
}

fn print_check_line(check: &CheckResult, name_width: usize, use_colors: bool) {
    use crate::display::colors;

    let (status_text, color) = match check.status {
        CheckStatus::Pass => ("OK", colors::GREEN),
        CheckStatus::Warn => ("WARN", colors::YELLOW),
        CheckStatus::Fail => ("FAIL", colors::RED),
    };

    let status_padded = format!("{status_text:<4}");
    let status_display = if use_colors {
        format!(
            "{color}{status}{reset}",
            status = status_padded,
            reset = colors::RESET
        )
    } else {
        status_padded
    };

    println!(
        "  {status} {name:<width$} {label}",
        status = status_display,
        name = check.name,
        width = name_width,
        label = check.label
    );

    if let Some(message) = &check.message {
        for line in message.lines() {
            println!("      {line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_core::HatConfig;

    fn base_hat(name: &str, backend: Option<HatBackend>) -> HatConfig {
        HatConfig {
            name: name.to_string(),
            description: Some("Test hat".to_string()),
            triggers: vec!["work.start".to_string()],
            publishes: vec![],
            instructions: String::new(),
            extra_instructions: vec![],
            backend,
            default_publishes: None,
            max_activations: None,
        }
    }

    #[test]
    fn backend_checks_include_cli_and_hat_backends() {
        let mut config = RalphConfig::default();
        config.cli.backend = "claude".to_string();
        config.hats.insert(
            "reviewer".to_string(),
            base_hat("Reviewer", Some(HatBackend::Named("gemini".to_string()))),
        );
        let checks = backend_checks(&config, |cmd| cmd == "claude", |_| false);
        let names: HashSet<_> = checks.iter().map(|check| check.name.as_str()).collect();

        assert!(names.contains("backend:claude"));
        assert!(names.contains("backend:gemini"));
    }

    #[test]
    fn backend_checks_map_custom_command_to_known_backend() {
        let mut config = RalphConfig::default();
        config.cli.backend = "custom".to_string();
        config.cli.command = Some("opencode".to_string());
        let checks = backend_checks(&config, |_| false, |cmd| cmd == "opencode");
        let names: Vec<_> = checks.iter().map(|check| check.name.as_str()).collect();

        assert!(names.contains(&"backend:opencode"));
    }

    #[test]
    fn backend_checks_fail_required_missing() {
        let mut config = RalphConfig::default();
        config.cli.backend = "claude".to_string();

        let checks = backend_checks(&config, |_| false, |_| false);
        let claude = checks
            .iter()
            .find(|check| check.name == "backend:claude")
            .expect("expected claude backend check");

        assert_eq!(claude.status, CheckStatus::Fail);
    }

    #[test]
    fn auth_hint_warns_when_env_missing() {
        let backends = vec!["codex".to_string(), "gemini".to_string()];
        let check = auth_hint_check(&backends, |key| match key {
            "OPENAI_API_KEY" => Some("present".to_string()),
            _ => None,
        });

        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.as_deref().unwrap_or("").contains("gemini"));
    }

    #[test]
    fn auth_hint_passes_when_all_env_present() {
        let backends = vec!["codex".to_string(), "gemini".to_string()];
        let check = auth_hint_check(&backends, |key| match key {
            "OPENAI_API_KEY" => Some("present".to_string()),
            "GEMINI_API_KEY" => Some("present".to_string()),
            _ => None,
        });

        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn canonical_backend_name_strips_exe_extension() {
        assert_eq!(
            canonical_backend_name("custom", Some("claude.exe")),
            "claude"
        );
    }

    #[test]
    fn canonical_backend_name_strips_extension_for_unknown_command() {
        assert_eq!(
            canonical_backend_name("custom", Some("my-cli.exe")),
            "my-cli"
        );
    }
}
