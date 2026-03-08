//! Preflight command for validating configuration and environment.

use std::path::Path;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use ralph_core::{CheckResult, CheckStatus, PreflightReport, PreflightRunner, RalphConfig};
use serde_yaml::{Mapping, Value};
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

    // Resolve hat imports in core config (if loaded from a file)
    if let Some(base_dir) = core_file_base_dir(config_sources) {
        resolve_hat_imports_in_value(&mut core_value, "hats", &base_dir, &core_label)?;
    }

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

        let mut hats_value = load_hats_value(source).await?;
        validate_hats_config_shape(&hats_value, &source.label())?;

        // Resolve hat imports in hats source
        match source {
            HatsSource::File(path) => {
                let base_dir = path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf();
                resolve_hat_imports_in_value(&mut hats_value, "hats", &base_dir, &source.label())?;
            }
            HatsSource::Builtin(name) => {
                // Resolve imports from embedded shared hat library
                let key = Value::String("hats".to_string());
                if let Some(mapping) = hats_value.as_mapping_mut()
                    && let Some(Value::Mapping(hats_mapping)) = mapping.get_mut(&key)
                {
                    resolve_builtin_hat_imports(hats_mapping, &format!("builtin:{name}"))?;
                }
            }
            HatsSource::Remote(_) => {
                // Remote hats can't resolve local file imports — skip
            }
        }

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

    let user_layer = config_resolution::load_optional_user_config_value()?;

    let (primary_value, primary_label, primary_uses_defaults) = if let Some(source) =
        primary_sources.first()
    {
        match source {
            ConfigSource::File(path) => {
                if path.exists() {
                    let label = path.display().to_string();
                    let content = std::fs::read_to_string(path)
                        .with_context(|| format!("Failed to load config from {}", label))?;
                    let value = config_resolution::parse_yaml_value(&content, &label)?;
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
            let value = config_resolution::parse_yaml_value(&content, &label)?;
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

async fn load_hats_value(source: &HatsSource) -> Result<Value> {
    match source {
        HatsSource::File(path) => {
            if !path.exists() {
                anyhow::bail!("Hats file not found: {}", path.display());
            }
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to load hats from {:?}", path))?;
            let value = config_resolution::parse_yaml_value(&content, &path.display().to_string())?;
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

            let value = config_resolution::parse_yaml_value(&content, url)?;
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

            let preset_value =
                config_resolution::parse_yaml_value(preset.content, &format!("builtin:{}", name))?;
            extract_hat_overlay_from_preset(preset_value)
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
const ALLOWED_HATS_EVENT_LOOP_OVERLAY_KEYS: &[&str] = &["completion_promise", "starting_event"];

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

/// Merges an imported hat definition with local overrides.
///
/// The imported hat provides base values. Any field present in `local_overrides`
/// replaces the corresponding field from `imported`. The `import:` key is
/// removed from the result.
fn merge_imported_hat(mut imported: Mapping, local_overrides: &Mapping) -> Mapping {
    let import_key = Value::String("import".to_string());
    for (key, value) in local_overrides {
        if key == &import_key {
            continue;
        }
        imported.insert(key.clone(), value.clone());
    }
    imported.remove(&import_key);
    imported
}

/// Validates that an import path is safe: must be relative and cannot escape
/// the base directory via `..` components.
fn validate_import_path(import_path_str: &str, source_label: &str, hat_id_str: &str) -> Result<()> {
    let path = std::path::Path::new(import_path_str);
    if path.is_absolute() {
        anyhow::bail!(
            "failed to resolve hat import\n  \
             --> {source_label}, hat '{hat_id_str}'\n\n  \
             cause: import paths must be relative, got absolute path '{import_path_str}'"
        );
    }
    if path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        anyhow::bail!(
            "failed to resolve hat import\n  \
             --> {source_label}, hat '{hat_id_str}'\n\n  \
             cause: import paths cannot use '..' to escape the base directory"
        );
    }
    Ok(())
}

/// Resolves `import:` keys in a hats mapping.
///
/// For each hat that contains an `import:` key:
/// 1. Validates the path (must be relative, no `..` traversal)
/// 2. Loads and parses the referenced YAML via the provided `load_content` closure
/// 3. Validates it contains no `import:` key (no transitive imports)
/// 4. Validates it contains no `events:` key (not allowed in hat files)
/// 5. Uses the imported fields as a base, overlays local fields on top
/// 6. Removes the `import:` key from the result
fn resolve_hat_imports_with<F>(
    hats: &mut Mapping,
    source_label: &str,
    load_content: F,
) -> Result<()>
where
    F: Fn(&str, &str) -> Result<String>, // (import_path, hat_id_str) -> content
{
    let import_key = Value::String("import".to_string());

    // Collect hat IDs that have imports (can't mutate while iterating)
    let hat_ids_with_imports: Vec<Value> = hats
        .iter()
        .filter_map(|(id, def)| {
            def.as_mapping()
                .and_then(|m| m.get(&import_key))
                .map(|_| id.clone())
        })
        .collect();

    for hat_id in hat_ids_with_imports {
        let hat_id_str = hat_id.as_str().unwrap_or("<non-string>");
        // Safe: filter_map above guarantees these exist
        let hat_def = hats.get(&hat_id).unwrap().as_mapping().unwrap();
        let import_value = hat_def.get(&import_key).unwrap();

        let import_path_str = import_value.as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "failed to resolve hat import\n  \
                 --> {source_label}, hat '{hat_id_str}'\n\n  \
                 cause: 'import' must be a string file path"
            )
        })?;

        validate_import_path(import_path_str, source_label, hat_id_str)?;

        let content = load_content(import_path_str, hat_id_str)?;

        let imported_value: Value = serde_yaml::from_str(&content).map_err(|e| {
            anyhow::anyhow!(
                "failed to resolve hat import\n  \
                 --> {source_label}, hat '{hat_id_str}'\n  \
                 --> imports {import_path_str}\n\n  \
                 cause: {e}"
            )
        })?;

        let imported_mapping = imported_value.as_mapping().ok_or_else(|| {
            anyhow::anyhow!(
                "failed to resolve hat import\n  \
                 --> {source_label}, hat '{hat_id_str}'\n  \
                 --> imports {import_path_str}\n\n  \
                 cause: imported hat file must be a YAML mapping"
            )
        })?;

        if imported_mapping.get(&import_key).is_some() {
            anyhow::bail!(
                "failed to resolve hat import\n  \
                 --> {source_label}, hat '{hat_id_str}'\n  \
                 --> imports {import_path_str}\n\n  \
                 cause: imported hat files cannot contain 'import:' directives \
                 (transitive imports are not supported)"
            );
        }

        let events_key = Value::String("events".to_string());
        if imported_mapping.get(&events_key).is_some() {
            anyhow::bail!(
                "failed to resolve hat import\n  \
                 --> {source_label}, hat '{hat_id_str}'\n  \
                 --> imports {import_path_str}\n\n  \
                 cause: imported hat files cannot contain 'events:' \
                 — event metadata belongs in the consuming preset"
            );
        }

        let local_overrides = hat_def.clone();
        let merged = merge_imported_hat(imported_mapping.clone(), &local_overrides);
        hats.insert(hat_id, Value::Mapping(merged));
    }

    Ok(())
}

/// Resolves `import:` keys by reading hat files from the filesystem.
fn resolve_hat_imports(hats: &mut Mapping, base_dir: &Path, source_label: &str) -> Result<()> {
    let base_dir = base_dir.to_path_buf();
    let label = source_label.to_string();
    resolve_hat_imports_with(hats, source_label, |import_path_str, hat_id_str| {
        let resolved_path = base_dir.join(import_path_str);
        std::fs::read_to_string(&resolved_path).map_err(|e| {
            anyhow::anyhow!(
                "failed to resolve hat import\n  \
                 --> {label}, hat '{hat_id_str}'\n  \
                 --> imports {}\n\n  \
                 cause: {e}",
                resolved_path.display()
            )
        })
    })
}

/// Resolves `import:` keys by reading hat files from the embedded shared hat library.
fn resolve_builtin_hat_imports(hats: &mut Mapping, source_label: &str) -> Result<()> {
    resolve_hat_imports_with(hats, source_label, |import_path_str, hat_id_str| {
        crate::presets::get_shared_hat(import_path_str)
            .map(|s| s.to_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "failed to resolve hat import\n  \
                     --> {source_label}, hat '{hat_id_str}'\n  \
                     --> imports {import_path_str}\n\n  \
                     cause: shared hat not found in embedded library\n  \
                     hint: embedded presets can only import from presets/shared-hats/"
                )
            })
    })
}

/// Extracts the base directory from the primary file-based config source.
fn core_file_base_dir(config_sources: &[ConfigSource]) -> Option<std::path::PathBuf> {
    config_sources.iter().find_map(|s| match s {
        ConfigSource::File(path) => Some(
            path.parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf(),
        ),
        _ => None,
    })
}

/// Resolves hat imports within a Value that contains a nested hats mapping
/// at the given `hats_key` (e.g., "hats").
fn resolve_hat_imports_in_value(
    value: &mut Value,
    hats_key: &str,
    base_dir: &std::path::Path,
    source_label: &str,
) -> Result<()> {
    let key = Value::String(hats_key.to_string());
    if let Some(mapping) = value.as_mapping_mut()
        && let Some(Value::Mapping(hats_mapping)) = mapping.get_mut(&key)
    {
        resolve_hat_imports(hats_mapping, base_dir, source_label)?;
    }
    Ok(())
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
            Some(&HatsSource::Builtin("feature".to_string())),
        );
        let expected_core =
            crate::config_resolution::compose_core_label(user_label.as_deref(), "ralph.yml", false);
        assert_eq!(
            with_hats_label,
            format!("{expected_core} + hats:builtin:feature")
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
    fn merge_hats_overlay_ignores_runtime_limits_from_hats_event_loop() {
        let core: Value = serde_yaml::from_str(
            r"
event_loop:
  max_iterations: 100
  max_runtime_seconds: 28800
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

    // ---- Hat import tests ----

    #[test]
    fn merge_imported_hat_uses_imported_fields_as_base() {
        let imported: Mapping = serde_yaml::from_str(
            r#"
name: "Builder"
description: "TDD builder"
max_activations: 5
"#,
        )
        .unwrap();
        let local: Mapping = Mapping::new();

        let result = merge_imported_hat(imported, &local);
        assert_eq!(
            result.get(Value::String("name".into())),
            Some(&Value::String("Builder".into()))
        );
        assert_eq!(
            result.get(Value::String("max_activations".into())),
            Some(&Value::Number(5.into()))
        );
    }

    #[test]
    fn merge_imported_hat_local_override_replaces_field() {
        let imported: Mapping = serde_yaml::from_str(
            r#"
name: "Builder"
max_activations: 5
"#,
        )
        .unwrap();
        let local: Mapping = serde_yaml::from_str(
            r#"
import: "./builder.yml"
max_activations: 3
"#,
        )
        .unwrap();

        let result = merge_imported_hat(imported, &local);
        assert_eq!(
            result.get(Value::String("max_activations".into())),
            Some(&Value::Number(3.into()))
        );
        // import key should be removed
        assert!(result.get(Value::String("import".into())).is_none());
    }

    #[test]
    fn merge_imported_hat_list_override_replaces_not_merges() {
        let imported: Mapping = serde_yaml::from_str(
            r"
publishes:
  - build.done
  - build.blocked
",
        )
        .unwrap();
        let local: Mapping = serde_yaml::from_str(
            r#"
import: "./builder.yml"
publishes:
  - build.done
"#,
        )
        .unwrap();

        let result = merge_imported_hat(imported, &local);
        let publishes = result
            .get(Value::String("publishes".into()))
            .unwrap()
            .as_sequence()
            .unwrap();
        assert_eq!(publishes.len(), 1);
        assert_eq!(publishes[0], Value::String("build.done".into()));
    }

    #[test]
    fn resolve_hat_imports_basic_import() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hat_file = temp_dir.path().join("builder.yml");
        std::fs::write(
            &hat_file,
            r#"
name: "Builder"
description: "TDD builder"
max_activations: 5
"#,
        )
        .unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./builder.yml
",
        )
        .unwrap();

        resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap();

        let builder = hats
            .get(Value::String("builder".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            builder.get(Value::String("name".into())),
            Some(&Value::String("Builder".into()))
        );
        assert_eq!(
            builder.get(Value::String("max_activations".into())),
            Some(&Value::Number(5.into()))
        );
        // import key removed
        assert!(builder.get(Value::String("import".into())).is_none());
    }

    #[test]
    fn resolve_hat_imports_with_local_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hat_file = temp_dir.path().join("builder.yml");
        std::fs::write(
            &hat_file,
            r#"
name: "Builder"
max_activations: 5
"#,
        )
        .unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./builder.yml
  max_activations: 3
",
        )
        .unwrap();

        resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap();

        let builder = hats
            .get(Value::String("builder".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            builder.get(Value::String("max_activations".into())),
            Some(&Value::Number(3.into()))
        );
    }

    #[test]
    fn resolve_hat_imports_no_op_for_hats_without_import() {
        let mut hats: Mapping = serde_yaml::from_str(
            r#"
builder:
  name: "Builder"
  max_activations: 5
"#,
        )
        .unwrap();

        let original = hats.clone();
        resolve_hat_imports(&mut hats, Path::new("/tmp"), "test.yml").unwrap();
        assert_eq!(hats, original);
    }

    #[test]
    fn resolve_hat_imports_rejects_transitive_import() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hat_file = temp_dir.path().join("builder.yml");
        std::fs::write(
            &hat_file,
            r#"
name: "Builder"
import: ./other.yml
"#,
        )
        .unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./builder.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap_err();
        assert!(
            err.to_string()
                .contains("imported hat files cannot contain 'import:' directives")
        );
    }

    #[test]
    fn resolve_hat_imports_rejects_events_in_imported() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hat_file = temp_dir.path().join("builder.yml");
        std::fs::write(
            &hat_file,
            r#"
name: "Builder"
events:
  build.start:
    triggers: ["builder"]
"#,
        )
        .unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./builder.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap_err();
        assert!(
            err.to_string()
                .contains("imported hat files cannot contain 'events:'")
        );
    }

    #[test]
    fn resolve_hat_imports_file_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./nonexistent.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("failed to resolve hat import"));
        assert!(msg.contains("hat 'builder'"));
        assert!(msg.contains("nonexistent.yml"));
    }

    #[test]
    fn resolve_hat_imports_invalid_yaml_in_imported_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("bad.yml"), "{{: not valid yaml [").unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./bad.yml
",
        )
        .unwrap();

        let err = resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("failed to resolve hat import"));
        assert!(msg.contains("hat 'builder'"));
        assert!(msg.contains("bad.yml"));
    }

    #[test]
    fn resolve_hat_imports_non_string_import_path() {
        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: 42
",
        )
        .unwrap();

        let err = resolve_hat_imports(&mut hats, Path::new("/tmp"), "test.yml").unwrap_err();
        assert!(
            err.to_string()
                .contains("'import' must be a string file path")
        );
    }

    #[test]
    fn resolve_hat_imports_multiple_hats() {
        let temp_dir = tempfile::tempdir().unwrap();

        std::fs::write(
            temp_dir.path().join("builder.yml"),
            r#"
name: "Builder"
max_activations: 5
"#,
        )
        .unwrap();

        std::fs::write(
            temp_dir.path().join("reviewer.yml"),
            r#"
name: "Reviewer"
max_activations: 3
"#,
        )
        .unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./builder.yml
reviewer:
  import: ./reviewer.yml
",
        )
        .unwrap();

        resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap();

        let builder = hats
            .get(Value::String("builder".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            builder.get(Value::String("name".into())),
            Some(&Value::String("Builder".into()))
        );

        let reviewer = hats
            .get(Value::String("reviewer".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            reviewer.get(Value::String("name".into())),
            Some(&Value::String("Reviewer".into()))
        );
    }

    #[test]
    fn resolve_hat_imports_rejects_absolute_path() {
        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: /etc/passwd
",
        )
        .unwrap();

        let err = resolve_hat_imports(&mut hats, Path::new("/tmp"), "test.yml").unwrap_err();
        assert!(err.to_string().contains("must be relative"));
    }

    #[test]
    fn resolve_hat_imports_rejects_parent_traversal() {
        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ../../../etc/passwd
",
        )
        .unwrap();

        let err = resolve_hat_imports(&mut hats, Path::new("/tmp"), "test.yml").unwrap_err();
        assert!(err.to_string().contains("cannot use '..'"));
    }

    #[test]
    fn resolve_hat_imports_name_from_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("template.yml"),
            r#"
description: "A template hat"
max_activations: 5
"#,
        )
        .unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r#"
builder:
  import: ./template.yml
  name: "My Builder"
"#,
        )
        .unwrap();

        resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap();

        let builder = hats
            .get(Value::String("builder".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            builder.get(Value::String("name".into())),
            Some(&Value::String("My Builder".into()))
        );
        assert_eq!(
            builder.get(Value::String("description".into())),
            Some(&Value::String("A template hat".into()))
        );
    }

    #[test]
    fn resolve_hat_imports_mixed_inline_and_imported() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("builder.yml"),
            r#"
name: "Imported Builder"
max_activations: 5
"#,
        )
        .unwrap();

        let mut hats: Mapping = serde_yaml::from_str(
            r#"
builder:
  import: ./builder.yml
reviewer:
  name: "Inline Reviewer"
  max_activations: 3
"#,
        )
        .unwrap();

        resolve_hat_imports(&mut hats, temp_dir.path(), "test.yml").unwrap();

        let builder = hats
            .get(Value::String("builder".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            builder.get(Value::String("name".into())),
            Some(&Value::String("Imported Builder".into()))
        );

        let reviewer = hats
            .get(Value::String("reviewer".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            reviewer.get(Value::String("name".into())),
            Some(&Value::String("Inline Reviewer".into()))
        );
    }

    #[test]
    fn resolve_builtin_hat_imports_resolves_shared_hat() {
        let mut hats: Mapping = serde_yaml::from_str(
            r"
committer:
  import: ./shared-hats/committer.yml
",
        )
        .unwrap();

        resolve_builtin_hat_imports(&mut hats, "builtin:test").unwrap();

        let committer = hats
            .get(Value::String("committer".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            committer.get(Value::String("name".into())),
            Some(&Value::String("Committer".into()))
        );
        // import key should be removed
        assert!(committer.get(Value::String("import".into())).is_none());
    }

    #[test]
    fn resolve_builtin_hat_imports_with_overrides() {
        let mut hats: Mapping = serde_yaml::from_str(
            r#"
committer:
  import: ./shared-hats/committer.yml
  name: "Custom Committer"
"#,
        )
        .unwrap();

        resolve_builtin_hat_imports(&mut hats, "builtin:test").unwrap();

        let committer = hats
            .get(Value::String("committer".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        // Local override wins
        assert_eq!(
            committer.get(Value::String("name".into())),
            Some(&Value::String("Custom Committer".into()))
        );
        // Imported field preserved
        assert!(committer.get(Value::String("description".into())).is_some());
    }

    #[test]
    fn resolve_builtin_hat_imports_rejects_unknown_shared_hat() {
        let mut hats: Mapping = serde_yaml::from_str(
            r"
builder:
  import: ./shared-hats/nonexistent.yml
",
        )
        .unwrap();

        let err = resolve_builtin_hat_imports(&mut hats, "builtin:test").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("shared hat not found in embedded library"));
        assert!(msg.contains("'builder'"));
    }

    #[test]
    fn resolve_builtin_hat_imports_no_op_without_imports() {
        let mut hats: Mapping = serde_yaml::from_str(
            r#"
builder:
  name: "Builder"
  max_activations: 5
"#,
        )
        .unwrap();

        assert!(resolve_builtin_hat_imports(&mut hats, "builtin:test").is_ok());
    }

    #[tokio::test]
    async fn load_config_for_preflight_resolves_core_hat_imports() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create shared hat file
        let shared_dir = temp_dir.path().join("shared");
        std::fs::create_dir(&shared_dir).unwrap();
        std::fs::write(
            shared_dir.join("builder.yml"),
            r#"
name: "Imported Builder"
description: "A shared builder hat"
"#,
        )
        .unwrap();

        // Create core config with hat import
        let core_path = temp_dir.path().join("ralph.yml");
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
    import: ./shared/builder.yml
    max_activations: 3
",
        )
        .unwrap();

        let config = load_config_for_preflight(&[ConfigSource::File(core_path)], None)
            .await
            .unwrap();

        let builder = config.hats.get("builder").unwrap();
        assert_eq!(builder.name, "Imported Builder");
        assert_eq!(
            builder.description,
            Some("A shared builder hat".to_string())
        );
        assert_eq!(builder.max_activations, Some(3));
    }

    #[tokio::test]
    async fn load_config_for_preflight_resolves_hats_source_imports() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create shared hat file
        let shared_dir = temp_dir.path().join("shared");
        std::fs::create_dir(&shared_dir).unwrap();
        std::fs::write(
            shared_dir.join("reviewer.yml"),
            r#"
name: "Imported Reviewer"
description: "A shared reviewer hat"
"#,
        )
        .unwrap();

        // Create core config
        let core_path = temp_dir.path().join("ralph.yml");
        std::fs::write(
            &core_path,
            r"
cli:
  backend: claude
event_loop:
  max_iterations: 50
  completion_promise: LOOP_COMPLETE
",
        )
        .unwrap();

        // Create hats file with import
        let hats_path = temp_dir.path().join("hats.yml");
        std::fs::write(
            &hats_path,
            r"
hats:
  reviewer:
    import: ./shared/reviewer.yml
    max_activations: 2
",
        )
        .unwrap();

        let config = load_config_for_preflight(
            &[ConfigSource::File(core_path)],
            Some(&HatsSource::File(hats_path)),
        )
        .await
        .unwrap();

        let reviewer = config.hats.get("reviewer").unwrap();
        assert_eq!(reviewer.name, "Imported Reviewer");
        assert_eq!(
            reviewer.description,
            Some("A shared reviewer hat".to_string())
        );
        assert_eq!(reviewer.max_activations, Some(2));
    }

    #[tokio::test]
    async fn load_config_for_preflight_split_config_resolves_independently() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create two separate shared hat directories
        let core_shared = temp_dir.path().join("core-shared");
        std::fs::create_dir(&core_shared).unwrap();
        std::fs::write(
            core_shared.join("builder.yml"),
            r#"
name: "Core Builder"
description: "From core shared"
"#,
        )
        .unwrap();

        let hats_shared = temp_dir.path().join("hats-shared");
        std::fs::create_dir(&hats_shared).unwrap();
        std::fs::write(
            hats_shared.join("reviewer.yml"),
            r#"
name: "Hats Reviewer"
description: "From hats shared"
"#,
        )
        .unwrap();

        // Core config with import (its builder will be overridden by hats source)
        let core_path = temp_dir.path().join("ralph.yml");
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
    import: ./core-shared/builder.yml
",
        )
        .unwrap();

        // Hats file with its own import
        let hats_path = temp_dir.path().join("hats.yml");
        std::fs::write(
            &hats_path,
            r"
hats:
  reviewer:
    import: ./hats-shared/reviewer.yml
",
        )
        .unwrap();

        let config = load_config_for_preflight(
            &[ConfigSource::File(core_path)],
            Some(&HatsSource::File(hats_path)),
        )
        .await
        .unwrap();

        // Hats source overrides core hats
        assert!(!config.hats.contains_key("builder"));
        let reviewer = config.hats.get("reviewer").unwrap();
        assert_eq!(reviewer.name, "Hats Reviewer");
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

    // --- Integration tests: verify builtin presets with imports resolve correctly ---

    #[tokio::test]
    async fn builtin_feature_resolves_shared_builder() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core_path = temp_dir.path().join("ralph.yml");
        std::fs::write(&core_path, "cli:\n  backend: claude\n").unwrap();

        let config = load_config_for_preflight(
            &[ConfigSource::File(core_path)],
            Some(&HatsSource::Builtin("feature".to_string())),
        )
        .await
        .unwrap();

        let builder = config.hats.get("builder").expect("builder hat missing");
        assert_eq!(builder.name, "Builder");
        assert!(
            builder
                .description
                .as_deref()
                .unwrap()
                .contains("quality gates")
        );
        assert!(builder.triggers.contains(&"build.task".to_string()));
        assert!(builder.publishes.contains(&"build.done".to_string()));
        assert!(builder.instructions.contains("BUILDER MODE"));

        // reviewer should also be present (inline, not imported)
        assert!(config.hats.contains_key("reviewer"));
    }

    #[tokio::test]
    async fn builtin_code_assist_resolves_shared_builder_and_committer() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core_path = temp_dir.path().join("ralph.yml");
        std::fs::write(&core_path, "cli:\n  backend: claude\n").unwrap();

        let config = load_config_for_preflight(
            &[ConfigSource::File(core_path)],
            Some(&HatsSource::Builtin("code-assist".to_string())),
        )
        .await
        .unwrap();

        // Builder: imported from builder-tdd.yml with name override
        let builder = config.hats.get("builder").expect("builder hat missing");
        assert_eq!(builder.name, "⚙️ Builder", "name override should apply");
        assert!(builder.triggers.contains(&"tasks.ready".to_string()));
        assert!(builder.triggers.contains(&"validation.failed".to_string()));
        assert!(builder.instructions.contains("TDD"));

        // Committer: imported from committer.yml with name override
        let committer = config.hats.get("committer").expect("committer hat missing");
        assert_eq!(committer.name, "📦 Committer", "name override should apply");
        assert!(
            committer
                .triggers
                .contains(&"validation.passed".to_string())
        );
        assert!(committer.instructions.contains("Conventional Commit"));

        // Validator and planner should be present (inline)
        assert!(config.hats.contains_key("validator"));
        assert!(config.hats.contains_key("planner"));
    }

    #[tokio::test]
    async fn builtin_pdd_to_code_assist_resolves_with_overrides() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core_path = temp_dir.path().join("ralph.yml");
        std::fs::write(&core_path, "cli:\n  backend: claude\n").unwrap();

        let config = load_config_for_preflight(
            &[ConfigSource::File(core_path)],
            Some(&HatsSource::Builtin("pdd-to-code-assist".to_string())),
        )
        .await
        .unwrap();

        // Builder: imported with description + instructions overrides
        let builder = config.hats.get("builder").expect("builder hat missing");
        assert_eq!(builder.name, "⚙️ Builder");
        assert!(
            builder
                .description
                .as_deref()
                .unwrap()
                .contains("one code task"),
            "pdd description override should apply"
        );
        assert!(
            builder.instructions.contains("Storage Layout"),
            "pdd instructions override should include Storage Layout"
        );
        assert!(
            builder.instructions.contains("Convention Alignment"),
            "pdd instructions override should include Convention Alignment"
        );
        // Should still have the same triggers from the import
        assert!(builder.triggers.contains(&"tasks.ready".to_string()));

        // Committer: imported with instructions override
        let committer = config.hats.get("committer").expect("committer hat missing");
        assert_eq!(committer.name, "📦 Committer");
        assert!(
            committer.instructions.contains("pdd-to-code-assist preset"),
            "pdd committer instructions override should apply"
        );
        // Triggers/publishes should come from the import
        assert!(
            committer
                .triggers
                .contains(&"validation.passed".to_string())
        );
        assert!(committer.publishes.contains(&"commit.complete".to_string()));

        // All 9 hats should be present
        assert_eq!(
            config.hats.len(),
            9,
            "pdd-to-code-assist should have 9 hats"
        );
    }
}
