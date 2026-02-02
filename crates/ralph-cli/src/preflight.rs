//! Preflight command for validating configuration and environment.

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use ralph_core::{CheckResult, CheckStatus, PreflightReport, PreflightRunner, RalphConfig};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::{ConfigSource, presets};

#[derive(Parser, Debug)]
pub struct PreflightArgs {
    /// Output format (human or json)
    #[arg(long, value_enum, default_value_t = PreflightFormat::Human)]
    pub format: PreflightFormat,

    /// Treat warnings as failures
    #[arg(long)]
    pub strict: bool,

    /// Run only specific check(s)
    #[arg(long, value_name = "NAME", action = ArgAction::Append)]
    pub check: Vec<String>,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum PreflightFormat {
    Human,
    Json,
}

pub async fn execute(
    config_sources: &[ConfigSource],
    args: PreflightArgs,
    use_colors: bool,
) -> Result<()> {
    let source_label = config_source_label(config_sources);
    let config = load_config_for_preflight(config_sources).await?;

    let runner = PreflightRunner::default_checks();
    let requested = normalize_checks(&args.check);
    validate_checks(&runner, &requested)?;

    let mut report = if requested.is_empty() {
        runner.run_all(&config).await
    } else {
        runner.run_selected(&config, &requested).await
    };

    let effective_passed = if args.strict {
        report.failures == 0 && report.warnings == 0
    } else {
        report.failures == 0
    };
    report.passed = effective_passed;

    match args.format {
        PreflightFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        PreflightFormat::Human => {
            print_human_report(&report, &source_label, use_colors, args.strict);
        }
    }

    if !effective_passed {
        std::process::exit(1);
    }

    Ok(())
}

fn normalize_checks(checks: &[String]) -> Vec<String> {
    checks.iter().map(|check| check.to_lowercase()).collect()
}

fn validate_checks(runner: &PreflightRunner, checks: &[String]) -> Result<()> {
    if checks.is_empty() {
        return Ok(());
    }

    let available = runner.check_names();
    let unknown: Vec<&String> = checks
        .iter()
        .filter(|check| {
            !available
                .iter()
                .any(|name| name.eq_ignore_ascii_case(check))
        })
        .collect();

    if !unknown.is_empty() {
        let available_list = available.join(", ");
        let unknown_list = unknown
            .iter()
            .map(|check| check.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("Unknown check(s): {unknown_list}. Available checks: {available_list}");
    }

    Ok(())
}

fn print_human_report(report: &PreflightReport, source: &str, use_colors: bool, strict: bool) {
    use crate::display::colors;

    println!("Preflight checks for {}", source);
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

    if strict && report.warnings > 0 {
        println!("Note: strict mode treats warnings as failures.");
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

pub(crate) async fn load_config_for_preflight(
    config_sources: &[ConfigSource],
) -> Result<RalphConfig> {
    let (primary_sources, overrides): (Vec<_>, Vec<_>) = config_sources
        .iter()
        .partition(|source| !matches!(source, ConfigSource::Override { .. }));

    if primary_sources.len() > 1 {
        warn!("Multiple config sources specified, using first one. Others ignored.");
    }

    let mut config = if let Some(source) = primary_sources.first() {
        match source {
            ConfigSource::File(path) => {
                if path.exists() {
                    RalphConfig::from_file(path)
                        .with_context(|| format!("Failed to load config from {:?}", path))?
                } else {
                    warn!("Config file {:?} not found, using defaults", path);
                    RalphConfig::default()
                }
            }
            ConfigSource::Builtin(name) => {
                let preset = presets::get_preset(name).ok_or_else(|| {
                    let available = presets::preset_names().join(", ");
                    anyhow::anyhow!(
                        "Unknown preset '{}'. Run `ralph run --list-presets` to see available presets.\n\nAvailable: {}",
                        name,
                        available
                    )
                })?;
                RalphConfig::parse_yaml(preset.content)
                    .with_context(|| format!("Failed to parse builtin preset '{}'.", name))?
            }
            ConfigSource::Remote(url) => {
                info!("Fetching config from {}", url);
                let response = reqwest::get(url)
                    .await
                    .with_context(|| format!("Failed to fetch config from {}", url))?;

                if !response.status().is_success() {
                    anyhow::bail!(
                        "Failed to fetch config from {}: HTTP {}",
                        url,
                        response.status()
                    );
                }

                let content = response
                    .text()
                    .await
                    .with_context(|| format!("Failed to read config content from {}", url))?;

                RalphConfig::parse_yaml(&content)
                    .with_context(|| format!("Failed to parse config from {}", url))?
            }
            ConfigSource::Override { .. } => unreachable!("Partitioned out overrides"),
        }
    } else {
        let default_path = PathBuf::from("ralph.yml");
        if default_path.exists() {
            RalphConfig::from_file(&default_path)
                .with_context(|| "Failed to load config from ralph.yml")?
        } else {
            warn!("Config file ralph.yml not found, using defaults");
            RalphConfig::default()
        }
    };

    config.normalize();
    config.core.workspace_root =
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let override_sources: Vec<_> = overrides.into_iter().cloned().collect();
    crate::apply_config_overrides(&mut config, &override_sources)?;

    Ok(config)
}

pub(crate) fn config_source_label(config_sources: &[ConfigSource]) -> String {
    let primary = config_sources
        .iter()
        .find(|source| !matches!(source, ConfigSource::Override { .. }));

    match primary {
        Some(ConfigSource::File(path)) => path.display().to_string(),
        Some(ConfigSource::Builtin(name)) => format!("builtin:{}", name),
        Some(ConfigSource::Remote(url)) => url.clone(),
        Some(ConfigSource::Override { .. }) | None => "ralph.yml".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_checks_lowercases() {
        let checks = vec!["Config".to_string(), "BaCkEnD".to_string()];
        let normalized = normalize_checks(&checks);
        assert_eq!(normalized, vec!["config", "backend"]);
    }

    #[test]
    fn validate_checks_accepts_known() {
        let runner = PreflightRunner::default_checks();
        let checks = vec!["config".to_string(), "backend".to_string()];
        assert!(validate_checks(&runner, &checks).is_ok());
    }

    #[test]
    fn validate_checks_rejects_unknown() {
        let runner = PreflightRunner::default_checks();
        let checks = vec!["nope".to_string()];
        let err = validate_checks(&runner, &checks).unwrap_err();
        assert!(err.to_string().contains("Unknown check(s)"));
    }

    #[test]
    fn config_source_label_handles_sources() {
        let file_label =
            config_source_label(&[ConfigSource::File(PathBuf::from("/tmp/ralph.yml"))]);
        assert_eq!(file_label, "/tmp/ralph.yml");

        let builtin_label = config_source_label(&[ConfigSource::Builtin("starter".to_string())]);
        assert_eq!(builtin_label, "builtin:starter");

        let remote_label = config_source_label(&[ConfigSource::Remote(
            "https://example.com/ralph.yml".to_string(),
        )]);
        assert_eq!(remote_label, "https://example.com/ralph.yml");

        let override_label = config_source_label(&[ConfigSource::Override {
            key: "core.scratchpad".to_string(),
            value: "x".to_string(),
        }]);
        assert_eq!(override_label, "ralph.yml");
    }
}
