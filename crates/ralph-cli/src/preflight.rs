//! Preflight command for validating configuration and environment.

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use ralph_core::{CheckResult, CheckStatus, PreflightReport, PreflightRunner, RalphConfig};
use serde_yaml::{Mapping, Value};
use tracing::{info, warn};

use crate::{ConfigSource, HatsSource, presets};

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
    hats_source: Option<&HatsSource>,
    args: PreflightArgs,
    use_colors: bool,
) -> Result<()> {
    let source_label = config_source_label(config_sources, hats_source);
    let config = load_config_for_preflight(config_sources, hats_source).await?;

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
    hats_source: Option<&HatsSource>,
) -> Result<RalphConfig> {
    let (mut core_value, overrides, core_label) = load_core_value(config_sources).await?;

    validate_core_config_shape(&core_value, &core_label)?;

    if let Some(source) = hats_source {
        if let Some(mapping) = core_value.as_mapping()
            && (mapping_get(mapping, "hats").is_some() || mapping_get(mapping, "events").is_some())
        {
            warn!(
                "Core config '{}' contains hats/events and hats source '{}' was provided; hats source takes precedence for hats/events",
                core_label,
                source.label()
            );
        }

        let hats_value = load_hats_value(source).await?;
        validate_hats_config_shape(&hats_value, &source.label())?;
        core_value = merge_hats_overlay(core_value, hats_value)?;
    }

    let mut config: RalphConfig = serde_yaml::from_value(core_value)
        .with_context(|| format!("Failed to parse merged core config from {}", core_label))?;

    config.normalize();
    config.core.workspace_root =
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    crate::apply_config_overrides(&mut config, &overrides)?;

    Ok(config)
}

pub(crate) fn config_source_label(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
) -> String {
    let primary = config_sources
        .iter()
        .find(|source| !matches!(source, ConfigSource::Override { .. }));

    let core_label = match primary {
        Some(ConfigSource::File(path)) => path.display().to_string(),
        Some(ConfigSource::Builtin(name)) => format!("builtin:{}", name),
        Some(ConfigSource::Remote(url)) => url.clone(),
        Some(ConfigSource::Override { .. }) | None => {
            crate::default_config_path().to_string_lossy().to_string()
        }
    };

    if let Some(source) = hats_source {
        format!("{} + hats:{}", core_label, source.label())
    } else {
        core_label
    }
}

async fn load_core_value(
    config_sources: &[ConfigSource],
) -> Result<(Value, Vec<ConfigSource>, String)> {
    let (primary_sources, overrides): (Vec<_>, Vec<_>) = config_sources
        .iter()
        .partition(|source| !matches!(source, ConfigSource::Override { .. }));
    let overrides: Vec<ConfigSource> = overrides.into_iter().cloned().collect();

    if primary_sources.len() > 1 {
        warn!("Multiple config sources specified, using first one. Others ignored.");
    }

    if let Some(source) = primary_sources.first() {
        match source {
            ConfigSource::File(path) => {
                if path.exists() {
                    let content = std::fs::read_to_string(path)
                        .with_context(|| format!("Failed to load config from {:?}", path))?;
                    let value = parse_yaml_value(&content, &path.display().to_string())?;
                    Ok((value, overrides, path.display().to_string()))
                } else {
                    warn!("Config file {:?} not found, using defaults", path);
                    Ok((default_core_value()?, overrides, path.display().to_string()))
                }
            }
            ConfigSource::Builtin(name) => {
                anyhow::bail!(
                    "`-c builtin:{name}` is no longer supported.\n\nBuiltin presets are now hat collections.\nUse:\n  ralph run -c ralph.yml -H builtin:{name}\n\nOr for preflight:\n  ralph preflight -c ralph.yml -H builtin:{name}"
                );
            }
            ConfigSource::Remote(url) => {
                info!("Fetching core config from {}", url);
                let response = reqwest::get(url)
                    .await
                    .with_context(|| format!("Failed to fetch core config from {}", url))?;

                if !response.status().is_success() {
                    anyhow::bail!(
                        "Failed to fetch core config from {}: HTTP {}",
                        url,
                        response.status()
                    );
                }

                let content = response
                    .text()
                    .await
                    .with_context(|| format!("Failed to read core config content from {}", url))?;

                let value = parse_yaml_value(&content, url)?;
                Ok((value, overrides, url.clone()))
            }
            ConfigSource::Override { .. } => unreachable!("Partitioned out overrides"),
        }
    } else {
        let default_path = crate::default_config_path();
        if default_path.exists() {
            let content = std::fs::read_to_string(&default_path).with_context(|| {
                format!("Failed to load config from {}", default_path.display())
            })?;
            let value = parse_yaml_value(&content, &default_path.display().to_string())?;
            Ok((value, overrides, default_path.display().to_string()))
        } else {
            warn!(
                "Config file {} not found, using defaults",
                default_path.display()
            );
            Ok((
                default_core_value()?,
                overrides,
                default_path.display().to_string(),
            ))
        }
    }
}

async fn load_hats_value(source: &HatsSource) -> Result<Value> {
    match source {
        HatsSource::File(path) => {
            if !path.exists() {
                anyhow::bail!("Hats file not found: {}", path.display());
            }
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to load hats from {:?}", path))?;
            let value = parse_yaml_value(&content, &path.display().to_string())?;
            normalize_hats_source_value(value, &path.display().to_string())
        }
        HatsSource::Remote(url) => {
            info!("Fetching hats config from {}", url);
            let response = reqwest::get(url)
                .await
                .with_context(|| format!("Failed to fetch hats config from {}", url))?;

            if !response.status().is_success() {
                anyhow::bail!(
                    "Failed to fetch hats config from {}: HTTP {}",
                    url,
                    response.status()
                );
            }

            let content = response
                .text()
                .await
                .with_context(|| format!("Failed to read hats config content from {}", url))?;

            let value = parse_yaml_value(&content, url)?;
            normalize_hats_source_value(value, url)
        }
        HatsSource::Builtin(name) => {
            let preset = presets::get_preset(name).ok_or_else(|| {
                let available = presets::preset_names().join(", ");
                anyhow::anyhow!(
                    "Unknown hat collection '{}'. Available builtins: {}",
                    name,
                    available
                )
            })?;

            let preset_value = parse_yaml_value(preset.content, &format!("builtin:{}", name))?;
            extract_hat_overlay_from_preset(preset_value)
        }
    }
}

fn parse_yaml_value(content: &str, label: &str) -> Result<Value> {
    serde_yaml::from_str(content).with_context(|| format!("Failed to parse YAML from {}", label))
}

fn normalize_hats_source_value(value: Value, label: &str) -> Result<Value> {
    let (disallowed, has_hat_keys) = {
        let mapping = value
            .as_mapping()
            .ok_or_else(|| anyhow::anyhow!("Hats config '{}' must be a YAML mapping", label))?;
        (
            hats_disallowed_keys(mapping),
            mapping_get(mapping, "hats").is_some() || mapping_get(mapping, "events").is_some(),
        )
    };

    if disallowed.is_empty() {
        return Ok(value);
    }

    if has_hat_keys {
        warn!(
            "Hats source '{}' contains core/runtime keys [{}]; ignoring them and using hats/events/event_loop only",
            label,
            disallowed.join(", ")
        );
        return extract_hat_overlay_from_preset(value);
    }

    anyhow::bail!(
        "Hats config '{}' contains non-hats keys: {}",
        label,
        disallowed.join(", ")
    )
}

fn default_core_value() -> Result<Value> {
    let mut value = serde_yaml::to_value(RalphConfig::default())
        .context("Failed to build default core config")?;

    if let Some(mapping) = value.as_mapping_mut() {
        let hats_key = Value::String("hats".to_string());
        let events_key = Value::String("events".to_string());
        mapping.remove(&hats_key);
        mapping.remove(&events_key);
    }

    Ok(value)
}

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    let key_value = Value::String(key.to_string());
    mapping.get(&key_value)
}

fn mapping_insert(mapping: &mut Mapping, key: &str, value: Value) {
    mapping.insert(Value::String(key.to_string()), value);
}

fn validate_core_config_shape(value: &Value, label: &str) -> Result<()> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Core config '{}' must be a YAML mapping", label))?;

    if mapping_get(mapping, "project").is_some() {
        anyhow::bail!(ralph_core::ConfigError::DeprecatedProjectKey);
    }

    Ok(())
}

const ALLOWED_HATS_TOP_LEVEL: &[&str] = &["hats", "events", "event_loop", "name", "description"];

fn hats_disallowed_keys(mapping: &Mapping) -> Vec<String> {
    let mut disallowed = Vec::new();
    for key in mapping.keys() {
        if let Some(k) = key.as_str()
            && !ALLOWED_HATS_TOP_LEVEL.contains(&k)
        {
            disallowed.push(k.to_string());
        }
    }
    disallowed
}

fn validate_hats_config_shape(value: &Value, label: &str) -> Result<()> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Hats config '{}' must be a YAML mapping", label))?;

    let disallowed = hats_disallowed_keys(mapping);
    if !disallowed.is_empty() {
        anyhow::bail!(
            "Hats config '{}' contains non-hats keys: {}\n\nA hats file may only contain: {}\nCore/backend/runtime settings belong in -c/--config.",
            label,
            disallowed.join(", "),
            ALLOWED_HATS_TOP_LEVEL.join(", ")
        );
    }

    Ok(())
}

fn extract_hat_overlay_from_preset(preset_value: Value) -> Result<Value> {
    let mapping = preset_value
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Builtin hat collection must be a YAML mapping"))?;

    let mut overlay = Mapping::new();
    for key in ["name", "description", "event_loop", "events", "hats"] {
        if let Some(value) = mapping_get(mapping, key) {
            mapping_insert(&mut overlay, key, value.clone());
        }
    }

    Ok(Value::Mapping(overlay))
}

fn merge_hats_overlay(mut core: Value, hats: Value) -> Result<Value> {
    let core_mapping = core
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("Core config must be a YAML mapping"))?;
    let hats_mapping = hats
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Hats config must be a YAML mapping"))?;

    if let Some(hats_value) = mapping_get(hats_mapping, "hats") {
        mapping_insert(core_mapping, "hats", hats_value.clone());
    }

    if let Some(events_value) = mapping_get(hats_mapping, "events") {
        mapping_insert(core_mapping, "events", events_value.clone());
    }

    if let Some(event_loop_overlay) = mapping_get(hats_mapping, "event_loop") {
        let overlay_mapping = event_loop_overlay
            .as_mapping()
            .ok_or_else(|| anyhow::anyhow!("hats.event_loop must be a mapping when provided"))?;

        let event_loop_value = mapping_get(core_mapping, "event_loop")
            .cloned()
            .unwrap_or_else(|| Value::Mapping(Mapping::new()));

        let mut event_loop_mapping = event_loop_value
            .as_mapping()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("core.event_loop must be a mapping when provided"))?;

        for (key, value) in overlay_mapping {
            event_loop_mapping.insert(key.clone(), value.clone());
        }

        mapping_insert(
            core_mapping,
            "event_loop",
            Value::Mapping(event_loop_mapping),
        );
    }

    Ok(core)
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
        let file_label = config_source_label(
            &[ConfigSource::File(std::path::PathBuf::from(
                "/tmp/ralph.yml",
            ))],
            None,
        );
        assert_eq!(file_label, "/tmp/ralph.yml");

        let builtin_label =
            config_source_label(&[ConfigSource::Builtin("starter".to_string())], None);
        assert_eq!(builtin_label, "builtin:starter");

        let remote_label = config_source_label(
            &[ConfigSource::Remote(
                "https://example.com/ralph.yml".to_string(),
            )],
            None,
        );
        assert_eq!(remote_label, "https://example.com/ralph.yml");

        let override_label = config_source_label(
            &[ConfigSource::Override {
                key: "core.scratchpad".to_string(),
                value: "x".to_string(),
            }],
            None,
        );
        assert_eq!(override_label, "ralph.yml");

        let with_hats_label = config_source_label(
            &[ConfigSource::File(std::path::PathBuf::from("ralph.yml"))],
            Some(&HatsSource::Builtin("feature".to_string())),
        );
        assert_eq!(with_hats_label, "ralph.yml + hats:builtin:feature");
    }

    #[test]
    fn validate_core_config_shape_rejects_project() {
        let core: Value = serde_yaml::from_str(
            r"
project:
  specs_dir: my_specs
",
        )
        .unwrap();

        let err = validate_core_config_shape(&core, "core.yml").unwrap_err();
        assert!(err.to_string().contains("Invalid config key 'project'"));
    }

    #[test]
    fn validate_core_config_shape_allows_single_file_combined_config() {
        let core: Value = serde_yaml::from_str(
            r"
cli:
  backend: claude
hats:
  builder:
    name: Builder
",
        )
        .unwrap();

        assert!(validate_core_config_shape(&core, "core.yml").is_ok());
    }

    #[test]
    fn validate_hats_config_shape_rejects_core_keys() {
        let hats: Value = serde_yaml::from_str(
            r"
cli:
  backend: claude
hats:
  builder:
    name: Builder
",
        )
        .unwrap();

        let err = validate_hats_config_shape(&hats, "hats.yml").unwrap_err();
        assert!(err.to_string().contains("contains non-hats keys"));
    }

    #[test]
    fn merge_hats_overlay_replaces_hats_and_merges_event_loop() {
        let core: Value = serde_yaml::from_str(
            r"
cli:
  backend: claude
event_loop:
  max_iterations: 100
  completion_promise: LOOP_COMPLETE
hats:
  builder:
    name: Builder
",
        )
        .unwrap();

        let hats: Value = serde_yaml::from_str(
            r"
event_loop:
  completion_promise: REVIEW_COMPLETE
hats:
  reviewer:
    name: Reviewer
",
        )
        .unwrap();

        let merged = merge_hats_overlay(core, hats).unwrap();
        let config: RalphConfig = serde_yaml::from_value(merged).unwrap();

        assert_eq!(config.event_loop.max_iterations, 100);
        assert_eq!(config.event_loop.completion_promise, "REVIEW_COMPLETE");
        assert!(config.hats.contains_key("reviewer"));
        assert!(!config.hats.contains_key("builder"));
    }

    #[tokio::test]
    async fn load_config_for_preflight_hats_source_takes_precedence_over_core_hats() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core_path = temp_dir.path().join("ralph.yml");
        let hats_path = temp_dir.path().join("hats.yml");

        std::fs::write(
            &core_path,
            r"
cli:
  backend: claude
event_loop:
  max_iterations: 50
  completion_promise: LOOP_COMPLETE
hats:
  builder:
    name: Builder
    description: Core builder
",
        )
        .unwrap();

        std::fs::write(
            &hats_path,
            r"
event_loop:
  completion_promise: REVIEW_COMPLETE
hats:
  reviewer:
    name: Reviewer
    description: Hats reviewer
",
        )
        .unwrap();

        let config_sources = vec![ConfigSource::File(core_path)];
        let hats_source = HatsSource::File(hats_path);

        let config = load_config_for_preflight(&config_sources, Some(&hats_source))
            .await
            .unwrap();

        assert_eq!(config.event_loop.max_iterations, 50);
        assert_eq!(config.event_loop.completion_promise, "REVIEW_COMPLETE");
        assert!(config.hats.contains_key("reviewer"));
        assert!(!config.hats.contains_key("builder"));
    }

    #[test]
    fn normalize_hats_source_value_extracts_legacy_mixed_preset() {
        let legacy: Value = serde_yaml::from_str(
            r"
cli:
  backend: claude
core:
  specs_dir: ./specs/
event_loop:
  completion_promise: LOOP_COMPLETE
hats:
  builder:
    name: Builder
",
        )
        .unwrap();

        let normalized = normalize_hats_source_value(legacy, "legacy.yml").unwrap();
        let mapping = normalized.as_mapping().unwrap();

        assert!(mapping_get(mapping, "hats").is_some());
        assert!(mapping_get(mapping, "event_loop").is_some());
        assert!(mapping_get(mapping, "cli").is_none());
        assert!(mapping_get(mapping, "core").is_none());
    }
}
