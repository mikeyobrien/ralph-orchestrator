//! Preflight command for validating configuration and environment.

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use ralph_core::{
    CheckResult, CheckStatus, HatConfig, PreflightReport, PreflightRunner, RalphConfig,
};
use serde_yaml::{Mapping, Value};
use std::io::ErrorKind;
use std::path::Path;
use tracing::{info, warn};

use crate::{ConfigSource, HatsSource, config_resolution, presets};

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

        // TOML multi-file presets carry loop-level policy (max_iterations,
        // required_events) that the generic hats-overlay allowlist rightly
        // rejects from user-authored YAML hats files. For TOML preset
        // sources we inject those knobs directly into core BEFORE the
        // generic overlay merge, which still filters user-editable
        // event_loop keys defensively.
        if let HatsSource::PresetDir(path) = source {
            apply_toml_preset_core_patch(&mut core_value, path)?;
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

    let (primary_label, primary_uses_defaults) = match primary {
        Some(ConfigSource::File(path)) => (path.display().to_string(), false),
        Some(ConfigSource::Builtin(name)) => (format!("builtin:{}", name), false),
        Some(ConfigSource::Remote(url)) => (url.clone(), false),
        Some(ConfigSource::Override { .. }) => unreachable!("Overrides are filtered out"),
        None => {
            let default_path = crate::default_config_path();
            let uses_defaults = !default_path.exists();
            (default_path.display().to_string(), uses_defaults)
        }
    };

    let core_label = config_resolution::compose_core_label(
        config_resolution::user_config_label_if_exists().as_deref(),
        &primary_label,
        primary_uses_defaults,
    );

    if let Some(source) = hats_source {
        format!("{} + hats:{}", core_label, source.label())
    } else {
        core_label
    }
}

async fn load_core_value(
    config_sources: &[ConfigSource],
) -> Result<(Value, Vec<ConfigSource>, String)> {
    let (primary_sources, overrides) = config_resolution::split_config_sources(config_sources);

    if primary_sources.len() > 1 {
        warn!("Multiple config sources specified, using first one. Others ignored.");
    }

    let mut user_layer = config_resolution::load_optional_user_config_value()?;
    if let (Some((user_value, user_label)), Some(user_path)) = (
        user_layer.as_mut(),
        config_resolution::default_user_config_path(),
    ) {
        resolve_hat_imports_in_config_value(
            user_value,
            source_base_dir(&user_path),
            user_label.as_str(),
        )?;
    }

    let (primary_value, primary_label, primary_uses_defaults) = if let Some(source) =
        primary_sources.first()
    {
        match source {
            ConfigSource::File(path) => {
                if path.exists() {
                    let label = path.display().to_string();
                    let content = std::fs::read_to_string(path)
                        .with_context(|| format!("Failed to load config from {}", label))?;
                    let mut value = config_resolution::parse_yaml_value(&content, &label)?;
                    resolve_hat_imports_in_config_value(&mut value, source_base_dir(path), &label)?;
                    (Some(value), label, false)
                } else {
                    warn!("Config file {:?} not found, using defaults", path);
                    (None, path.display().to_string(), false)
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

                let value = config_resolution::parse_yaml_value(&content, url)?;
                reject_hat_imports_in_config_value(&value, url, UnsupportedImportSource::Remote)?;
                (Some(value), url.clone(), false)
            }
            ConfigSource::Override { .. } => unreachable!("Partitioned out overrides"),
        }
    } else {
        let default_path = crate::default_config_path();
        if default_path.exists() {
            let label = default_path.display().to_string();
            let content = std::fs::read_to_string(&default_path)
                .with_context(|| format!("Failed to load config from {}", label))?;
            let mut value = config_resolution::parse_yaml_value(&content, &label)?;
            resolve_hat_imports_in_config_value(
                &mut value,
                source_base_dir(&default_path),
                &label,
            )?;
            (Some(value), label, false)
        } else {
            warn!(
                "Config file {} not found, using defaults",
                default_path.display()
            );
            (None, default_path.display().to_string(), true)
        }
    };

    let mut merged = config_resolution::default_core_value()?;
    if let Some((user_value, _)) = &user_layer {
        merged = config_resolution::merge_yaml_values(merged, user_value.clone())?;
    }
    if let Some(primary_value) = primary_value {
        merged = config_resolution::merge_yaml_values(merged, primary_value)?;
    }

    let merged_label = config_resolution::compose_core_label(
        user_layer.as_ref().map(|(_, label)| label.as_str()),
        &primary_label,
        primary_uses_defaults,
    );

    Ok((merged, overrides, merged_label))
}

/// Merge loop-level fields an autoloop preset carries (`event_loop.max_iterations`,
/// `event_loop.required_events`) directly into `core_value` before the generic
/// hats overlay merge runs.
///
/// These knobs intentionally bypass [`merge_hats_overlay`]'s allowlist because
/// they're loop-wide policy sourced from an imported preset, not from a
/// user-authored hats YAML file.
fn apply_toml_preset_core_patch(core_value: &mut Value, preset_dir: &Path) -> Result<()> {
    let registry = ralph_core::PresetRegistry::default();
    let overlay = registry.load(preset_dir).with_context(|| {
        format!(
            "Failed to import autoloop preset from {}",
            preset_dir.display()
        )
    })?;

    let Some(overlay_map) = overlay.as_mapping() else {
        return Ok(());
    };
    let Some(overlay_el) = mapping_get(overlay_map, "event_loop").and_then(Value::as_mapping)
    else {
        return Ok(());
    };

    let core_map = core_value
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("Core config must be a YAML mapping"))?;

    let mut core_el_value = mapping_get(core_map, "event_loop")
        .cloned()
        .unwrap_or_else(|| Value::Mapping(Mapping::new()));
    let core_el_map = core_el_value
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("core.event_loop must be a mapping when provided"))?;

    for key in ["max_iterations", "required_events"] {
        if let Some(val) = mapping_get(overlay_el, key) {
            mapping_insert(core_el_map, key, val.clone());
        }
    }

    mapping_insert(core_map, "event_loop", core_el_value);
    Ok(())
}

async fn load_hats_value(source: &HatsSource) -> Result<Value> {
    match source {
        HatsSource::File(path) => {
            if !path.exists() {
                anyhow::bail!("Hats file not found: {}", path.display());
            }
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to load hats from {:?}", path))?;
            let label = path.display().to_string();
            let value = config_resolution::parse_yaml_value(&content, &label)?;
            let mut value = normalize_hats_source_value(value, &label)?;
            resolve_hat_imports_in_hats_source_value(&mut value, source_base_dir(path), &label)?;
            Ok(value)
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

            let value = config_resolution::parse_yaml_value(&content, url)?;
            let value = normalize_hats_source_value(value, url)?;
            reject_hat_imports_in_hats_source_value(&value, url, UnsupportedImportSource::Remote)?;
            Ok(value)
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

            let preset_value =
                config_resolution::parse_yaml_value(preset.content, &format!("builtin:{}", name))?;
            let value = extract_hat_overlay_from_preset(preset_value)?;
            reject_hat_imports_in_hats_source_value(
                &value,
                &format!("builtin:{}", name),
                UnsupportedImportSource::Embedded,
            )?;
            Ok(value)
        }
        HatsSource::PresetDir(path) => {
            if !path.exists() {
                anyhow::bail!(
                    "Preset directory not found: {}. For `-H <name>` lookups, ensure the name resolves under \
                     `./presets/<name>/`, `$XDG_CONFIG_HOME/ralph/presets/<name>/`, `$HOME/.config/ralph/presets/<name>/`, \
                     `$HOME/.config/autoloop/presets/<name>/`, or `$RALPH_PRESETS_DIR/<name>/` \
                     and contains `autoloops.toml` and `topology.toml`. Run `ralph hats list-presets` to see what is discoverable.",
                    path.display()
                );
            }
            let registry = ralph_core::PresetRegistry::default();
            let value = registry
                .load(path)
                .with_context(|| format!("Failed to import preset from {}", path.display()))?;
            let value = extract_hat_overlay_from_preset(value)?;
            reject_hat_imports_in_hats_source_value(
                &value,
                &path.display().to_string(),
                UnsupportedImportSource::PresetDir,
            )?;
            Ok(value)
        }
    }
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

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    let key_value = Value::String(key.to_string());
    mapping.get(&key_value)
}

fn mapping_get_mut<'a>(mapping: &'a mut Mapping, key: &str) -> Option<&'a mut Value> {
    let key_value = Value::String(key.to_string());
    mapping.get_mut(&key_value)
}

fn mapping_insert(mapping: &mut Mapping, key: &str, value: Value) {
    mapping.insert(Value::String(key.to_string()), value);
}

fn source_base_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn resolve_hat_imports_in_config_value(
    value: &mut Value,
    base_dir: &Path,
    source_label: &str,
) -> Result<()> {
    let Some(mapping) = value.as_mapping_mut() else {
        return Ok(());
    };

    let Some(hats_value) = mapping_get_mut(mapping, "hats") else {
        return Ok(());
    };

    let hats = hats_value
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("Config '{}' hats must be a YAML mapping", source_label))?;
    resolve_hat_imports(hats, base_dir, source_label)
}

fn resolve_hat_imports_in_hats_source_value(
    value: &mut Value,
    base_dir: &Path,
    source_label: &str,
) -> Result<()> {
    let Some(mapping) = value.as_mapping_mut() else {
        return Ok(());
    };

    let Some(hats_value) = mapping_get_mut(mapping, "hats") else {
        return Ok(());
    };

    let hats = hats_value.as_mapping_mut().ok_or_else(|| {
        anyhow::anyhow!("Hats config '{}' hats must be a YAML mapping", source_label)
    })?;
    resolve_hat_imports(hats, base_dir, source_label)
}

fn resolve_hat_imports(hats: &mut Mapping, base_dir: &Path, source_label: &str) -> Result<()> {
    for (hat_key, hat_value) in hats.iter_mut() {
        let hat_id = hat_key_label(hat_key);
        let Some(local_hat) = hat_value.as_mapping() else {
            continue;
        };
        let Some(import_value) = mapping_get(local_hat, "import") else {
            continue;
        };

        let import_path = import_value.as_str().ok_or_else(|| {
            hat_import_error(
                source_label,
                &hat_id,
                None,
                "'import' must be a string file path",
            )
        })?;

        let import_path = Path::new(import_path);
        let resolved_path = if import_path.is_absolute() {
            import_path.to_path_buf()
        } else {
            base_dir.join(import_path)
        };

        let content = std::fs::read_to_string(&resolved_path).map_err(|err| {
            let cause = if err.kind() == ErrorKind::NotFound {
                "file not found".to_string()
            } else {
                err.to_string()
            };
            hat_import_error(source_label, &hat_id, Some(&resolved_path), cause)
        })?;

        let imported_value: Value = serde_yaml::from_str(&content).map_err(|err| {
            hat_import_error(source_label, &hat_id, Some(&resolved_path), err.to_string())
        })?;

        let imported_hat = imported_value.as_mapping().ok_or_else(|| {
            hat_import_error(
                source_label,
                &hat_id,
                Some(&resolved_path),
                "imported hat file must be a YAML mapping",
            )
        })?;

        if mapping_get(imported_hat, "import").is_some() {
            return Err(hat_import_error(
                source_label,
                &hat_id,
                Some(&resolved_path),
                "imported hat files cannot contain 'import:' directives (transitive imports are not supported)",
            ));
        }

        if mapping_get(imported_hat, "events").is_some() {
            return Err(hat_import_error(
                source_label,
                &hat_id,
                Some(&resolved_path),
                "imported hat files cannot contain 'events:'; event metadata belongs in the consuming preset",
            ));
        }

        let local_overrides = local_hat.clone();
        let resolved_hat = merge_imported_hat(imported_hat.clone(), &local_overrides);
        serde_yaml::from_value::<HatConfig>(Value::Mapping(resolved_hat.clone())).map_err(
            |err| {
                hat_import_error(
                    source_label,
                    &hat_id,
                    Some(&resolved_path),
                    format!("imported hat schema is invalid: {err}"),
                )
            },
        )?;
        *hat_value = Value::Mapping(resolved_hat);
    }

    Ok(())
}

fn merge_imported_hat(mut imported: Mapping, local_overrides: &Mapping) -> Mapping {
    for (key, value) in local_overrides {
        if key.as_str() == Some("import") {
            continue;
        }
        imported.insert(key.clone(), value.clone());
    }
    imported
}

#[derive(Debug, Clone, Copy)]
enum UnsupportedImportSource {
    Embedded,
    Remote,
    PresetDir,
}

fn reject_hat_imports_in_config_value(
    value: &Value,
    source_label: &str,
    source: UnsupportedImportSource,
) -> Result<()> {
    let Some(mapping) = value.as_mapping() else {
        return Ok(());
    };
    let Some(hats_value) = mapping_get(mapping, "hats") else {
        return Ok(());
    };
    let Some(hats) = hats_value.as_mapping() else {
        return Ok(());
    };
    reject_hat_imports_in_mapping(hats, source_label, source)
}

fn reject_hat_imports_in_hats_source_value(
    value: &Value,
    source_label: &str,
    source: UnsupportedImportSource,
) -> Result<()> {
    let Some(mapping) = value.as_mapping() else {
        return Ok(());
    };
    let Some(hats_value) = mapping_get(mapping, "hats") else {
        return Ok(());
    };
    let Some(hats) = hats_value.as_mapping() else {
        return Ok(());
    };
    reject_hat_imports_in_mapping(hats, source_label, source)
}

fn reject_hat_imports_in_mapping(
    hats: &Mapping,
    source_label: &str,
    source: UnsupportedImportSource,
) -> Result<()> {
    for (hat_key, hat_value) in hats {
        let Some(hat) = hat_value.as_mapping() else {
            continue;
        };
        if mapping_get(hat, "import").is_none() {
            continue;
        }

        let hat_id = hat_key_label(hat_key);
        match source {
            UnsupportedImportSource::Embedded => anyhow::bail!(
                "hat imports are not supported in embedded presets - '{}' contains an 'import:' directive.\n\nhint: use a file-based preset to use hat imports",
                hat_id
            ),
            UnsupportedImportSource::Remote => anyhow::bail!(
                "hat imports are not supported in remote presets - '{}' contains an 'import:' directive.\n\nhint: use a file-based preset to use hat imports",
                hat_id
            ),
            UnsupportedImportSource::PresetDir => anyhow::bail!(
                "hat imports are not supported in preset directories - '{}' contains an 'import:' directive in {}.\n\nhint: use a file-based preset to use hat imports",
                hat_id,
                source_label
            ),
        }
    }

    Ok(())
}

fn hat_key_label(key: &Value) -> String {
    key.as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{key:?}"))
}

fn hat_import_error(
    source_label: &str,
    hat_id: &str,
    import_path: Option<&Path>,
    cause: impl AsRef<str>,
) -> anyhow::Error {
    let import_line = import_path
        .map(|path| format!("\n  --> imports {}", path.display()))
        .unwrap_or_default();

    anyhow::anyhow!(
        "failed to resolve hat import\n  --> {source_label}, hat '{hat_id}'{import_line}\n\n  cause: {}",
        cause.as_ref()
    )
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
const ALLOWED_HATS_EVENT_LOOP_OVERLAY_KEYS: &[&str] = &[
    "completion_promise",
    "starting_event",
    "cancellation_promise",
];

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
            if let Some(key_str) = key.as_str()
                && ALLOWED_HATS_EVENT_LOOP_OVERLAY_KEYS.contains(&key_str)
            {
                event_loop_mapping.insert(key.clone(), value.clone());
            }
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
        let user_label = crate::config_resolution::user_config_label_if_exists();
        let expected_file_label = crate::config_resolution::compose_core_label(
            user_label.as_deref(),
            "/tmp/ralph.yml",
            false,
        );
        assert_eq!(file_label, expected_file_label);

        let builtin_label =
            config_source_label(&[ConfigSource::Builtin("starter".to_string())], None);
        let expected_builtin_label = crate::config_resolution::compose_core_label(
            user_label.as_deref(),
            "builtin:starter",
            false,
        );
        assert_eq!(builtin_label, expected_builtin_label);

        let remote_label = config_source_label(
            &[ConfigSource::Remote(
                "https://example.com/ralph.yml".to_string(),
            )],
            None,
        );
        let expected_remote_label = crate::config_resolution::compose_core_label(
            user_label.as_deref(),
            "https://example.com/ralph.yml",
            false,
        );
        assert_eq!(remote_label, expected_remote_label);

        let override_label = config_source_label(
            &[ConfigSource::Override {
                key: "core.scratchpad".to_string(),
                value: "x".to_string(),
            }],
            None,
        );
        let default_label = crate::default_config_path().to_string_lossy().to_string();
        let expected_override_label = crate::config_resolution::compose_core_label(
            user_label.as_deref(),
            &default_label,
            !crate::default_config_path().exists(),
        );
        assert_eq!(override_label, expected_override_label);

        let with_hats_label = config_source_label(
            &[ConfigSource::File(std::path::PathBuf::from("ralph.yml"))],
            Some(&HatsSource::Builtin("code-assist".to_string())),
        );
        let expected_core =
            crate::config_resolution::compose_core_label(user_label.as_deref(), "ralph.yml", false);
        assert_eq!(
            with_hats_label,
            format!("{expected_core} + hats:builtin:code-assist")
        );
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
    fn resolve_hat_imports_merges_imported_fields() {
        let temp_dir = tempfile::tempdir().unwrap();
        let shared_dir = temp_dir.path().join("shared");
        std::fs::create_dir_all(&shared_dir).unwrap();
        std::fs::write(
            shared_dir.join("builder.yml"),
            r"
name: Imported Builder
description: Imported description
triggers: [build.start]
publishes: [build.done]
instructions: imported instructions
",
        )
        .unwrap();

        let mut config: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: shared/builder.yml
",
        )
        .unwrap();

        resolve_hat_imports_in_config_value(&mut config, temp_dir.path(), "ralph.yml").unwrap();

        let hats = mapping_get(config.as_mapping().unwrap(), "hats")
            .unwrap()
            .as_mapping()
            .unwrap();
        let builder = mapping_get(hats, "builder").unwrap().as_mapping().unwrap();
        assert_eq!(
            mapping_get(builder, "name").and_then(Value::as_str),
            Some("Imported Builder")
        );
        assert_eq!(
            mapping_get(builder, "description").and_then(Value::as_str),
            Some("Imported description")
        );
        assert!(mapping_get(builder, "import").is_none());
    }

    #[test]
    fn merge_imported_hat_uses_field_level_replacement() {
        let imported: Value = serde_yaml::from_str(
            r"
name: Base
publishes: [base.done]
backend:
  type: custom
  command: old-command
",
        )
        .unwrap();
        let overrides: Value = serde_yaml::from_str(
            r"
import: ./base.yml
publishes: [local.done]
backend:
  type: claude
",
        )
        .unwrap();

        let merged = merge_imported_hat(
            imported.as_mapping().unwrap().clone(),
            overrides.as_mapping().unwrap(),
        );

        let publishes = mapping_get(&merged, "publishes")
            .unwrap()
            .as_sequence()
            .unwrap();
        assert_eq!(publishes.len(), 1);
        assert_eq!(publishes[0].as_str(), Some("local.done"));

        let backend = mapping_get(&merged, "backend")
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            mapping_get(backend, "type").and_then(Value::as_str),
            Some("claude")
        );
        assert!(mapping_get(backend, "command").is_none());
        assert!(mapping_get(&merged, "import").is_none());
    }

    #[tokio::test]
    async fn load_config_for_preflight_resolves_core_and_hats_imports_relative_to_each_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core_dir = temp_dir.path().join("core");
        let hats_dir = temp_dir.path().join("hats");
        std::fs::create_dir_all(core_dir.join("shared")).unwrap();
        std::fs::create_dir_all(hats_dir.join("shared")).unwrap();

        std::fs::write(
            core_dir.join("shared/builder.yml"),
            r"
name: Core Builder
description: Imported from the core config directory
triggers: [core.start]
publishes: [core.done]
instructions: core builder
",
        )
        .unwrap();
        std::fs::write(
            hats_dir.join("shared/reviewer.yml"),
            r"
name: Hats Reviewer
description: Imported from the hats source directory
triggers: [review.start]
publishes: [review.done]
instructions: hats reviewer
",
        )
        .unwrap();

        let core_path = core_dir.join("ralph.yml");
        let hats_path = hats_dir.join("workflow.yml");
        std::fs::write(
            &core_path,
            r"
hats:
  builder:
    import: shared/builder.yml
",
        )
        .unwrap();
        std::fs::write(
            &hats_path,
            r"
hats:
  reviewer:
    import: shared/reviewer.yml
",
        )
        .unwrap();

        let config = load_config_for_preflight(
            &[ConfigSource::File(core_path)],
            Some(&HatsSource::File(hats_path)),
        )
        .await
        .unwrap();

        assert!(!config.hats.contains_key("builder"));
        assert_eq!(config.hats["reviewer"].name, "Hats Reviewer");
        assert_eq!(
            config.hats["reviewer"].description.as_deref(),
            Some("Imported from the hats source directory")
        );
    }

    #[test]
    fn resolve_hat_imports_rejects_missing_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: missing.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports_in_config_value(&mut config, temp_dir.path(), "ralph.yml")
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("failed to resolve hat import"));
        assert!(message.contains("file not found"));
    }

    #[test]
    fn resolve_hat_imports_rejects_invalid_yaml() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("broken.yml"), "name: [").unwrap();
        let mut config: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: broken.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports_in_config_value(&mut config, temp_dir.path(), "ralph.yml")
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("failed to resolve hat import"));
        assert!(message.contains("cause:"));
    }

    #[test]
    fn resolve_hat_imports_rejects_invalid_imported_schema_with_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let imported_path = temp_dir.path().join("base.yml");
        std::fs::write(
            &imported_path,
            r"
name: Base
triggers: 42
publishes: [build.done]
",
        )
        .unwrap();
        let mut config: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: base.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports_in_config_value(&mut config, temp_dir.path(), "ralph.yml")
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("failed to resolve hat import"));
        assert!(message.contains(&format!("imports {}", imported_path.display())));
        assert!(message.contains("imported hat schema is invalid"));
        assert!(message.contains("invalid type: integer `42`, expected a sequence"));
    }

    #[test]
    fn resolve_hat_imports_rejects_non_string_import() {
        let mut config: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: 42
",
        )
        .unwrap();

        let err = resolve_hat_imports_in_config_value(&mut config, Path::new("."), "ralph.yml")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("'import' must be a string file path")
        );
    }

    #[test]
    fn resolve_hat_imports_rejects_transitive_import() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("base.yml"),
            r"
import: other.yml
name: Base
",
        )
        .unwrap();
        let mut config: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: base.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports_in_config_value(&mut config, temp_dir.path(), "ralph.yml")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("transitive imports are not supported")
        );
    }

    #[test]
    fn resolve_hat_imports_rejects_imported_events() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("base.yml"),
            r"
name: Base
events:
  build.done:
    description: Done
",
        )
        .unwrap();
        let mut config: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: base.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports_in_config_value(&mut config, temp_dir.path(), "ralph.yml")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("imported hat files cannot contain 'events:'")
        );
    }

    #[test]
    fn reject_hat_imports_in_unsupported_sources() {
        let overlay: Value = serde_yaml::from_str(
            r"
hats:
  builder:
    import: ./builder.yml
",
        )
        .unwrap();

        let embedded_err = reject_hat_imports_in_hats_source_value(
            &overlay,
            "builtin:test",
            UnsupportedImportSource::Embedded,
        )
        .unwrap_err();
        assert!(embedded_err.to_string().contains("embedded presets"));

        let remote_err = reject_hat_imports_in_hats_source_value(
            &overlay,
            "https://example.com/hats.yml",
            UnsupportedImportSource::Remote,
        )
        .unwrap_err();
        assert!(remote_err.to_string().contains("remote presets"));
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

    #[test]
    fn merge_hats_overlay_allows_workflow_promises_from_hats_event_loop() {
        let core: Value = serde_yaml::from_str(
            r"
event_loop:
  max_iterations: 100
  max_runtime_seconds: 28800
  completion_promise: LOOP_COMPLETE
  cancellation_promise: LOOP_CANCELLED
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
  cancellation_promise: BUILD_PARKED
  starting_event: build.start
  max_iterations: 150
  max_runtime_seconds: 14400
hats:
  reviewer:
    name: Reviewer
",
        )
        .unwrap();

        let merged = merge_hats_overlay(core, hats).unwrap();
        let config: RalphConfig = serde_yaml::from_value(merged).unwrap();

        assert_eq!(config.event_loop.max_iterations, 100);
        assert_eq!(config.event_loop.max_runtime_seconds, 28800);
        assert_eq!(config.event_loop.completion_promise, "REVIEW_COMPLETE");
        assert_eq!(config.event_loop.cancellation_promise, "BUILD_PARKED");
        assert_eq!(
            config.event_loop.starting_event.as_deref(),
            Some("build.start")
        );
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
