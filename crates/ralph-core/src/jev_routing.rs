//! Jev workflow routing: where the effective settings come from, and the same
//! validation the engine applies before the first iteration.
//!
//! The engine fails the run on any of these problems; `ralph doctor` uses this
//! module to report them before a run is attempted. The rules mirror autoloop's
//! `readJevRoutingConfig` (autoloop 0.12.0).

use std::path::{Path, PathBuf};

use crate::RalphConfig;

/// Route IDs the engine accepts: `^[a-z][a-z0-9_-]{0,63}$`, never `no_match`.
const NO_MATCH: &str = "no_match";
const MAX_ROUTES: usize = 64;

/// Routing settings as the engine will see them.
#[derive(Debug, Clone, PartialEq)]
pub struct JevRoutingSettings {
    /// Where the settings come from, for messages.
    pub source: String,
    /// Absolute path of the route catalog.
    pub routes_file: PathBuf,
    pub min_confidence: Option<f64>,
    pub timeout_ms: Option<u64>,
}

/// The routing that will be active for `config`, or `None` when it is off.
///
/// An explicit `core.autoloop_preset` is authoritative, so its `[routing.jev]`
/// wins; otherwise `core.routing.jev` is what Ralph emits into its generated
/// preset.
pub fn effective_jev_routing(config: &RalphConfig) -> Result<Option<JevRoutingSettings>, String> {
    let workspace = &config.core.workspace_root;
    if let Some(preset) = config.core.autoloop_preset.as_deref() {
        let preset_path = workspace.join(preset);
        let file = if preset_path.is_dir() {
            preset_path.join("autoloops.toml")
        } else {
            preset_path.clone()
        };
        let Ok(text) = std::fs::read_to_string(&file) else {
            return Ok(None);
        };
        let toml: toml::Value = toml::from_str(&text)
            .map_err(|error| format!("{} is not valid TOML: {error}", file.display()))?;
        let Some(jev) = toml.get("routing").and_then(|routing| routing.get("jev")) else {
            return Ok(None);
        };
        let enabled = match jev.get("enabled") {
            Some(toml::Value::Boolean(value)) => *value,
            Some(toml::Value::String(value)) => value == "true",
            _ => false,
        };
        if !enabled {
            return Ok(None);
        }
        let routes_file = jev
            .get("routes_file")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "{} enables [routing.jev] without routes_file",
                    file.display()
                )
            })?;
        // The engine resolves routes_file against the preset directory.
        let preset_dir = if preset_path.is_dir() {
            preset_path.clone()
        } else {
            preset_path
                .parent()
                .map_or_else(|| workspace.clone(), Path::to_path_buf)
        };
        return Ok(Some(JevRoutingSettings {
            source: file.display().to_string(),
            routes_file: preset_dir.join(routes_file),
            min_confidence: jev.get("min_confidence").and_then(toml_number),
            timeout_ms: jev
                .get("timeout_ms")
                .and_then(toml_number)
                .map(|value| value as u64),
        }));
    }
    Ok(config
        .core
        .routing
        .enabled_jev()
        .map(|jev| JevRoutingSettings {
            source: "core.routing.jev".to_string(),
            routes_file: workspace.join(&jev.routes_file),
            min_confidence: jev.min_confidence,
            timeout_ms: jev.timeout_ms,
        }))
}

fn toml_number(value: &toml::Value) -> Option<f64> {
    match value {
        toml::Value::Integer(value) => Some(*value as f64),
        toml::Value::Float(value) => Some(*value),
        toml::Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

/// Validates `min_confidence` (0..=1) and `timeout_ms` (integer 1..=60000).
pub fn validate_settings(settings: &JevRoutingSettings) -> Result<(), String> {
    if let Some(value) = settings.min_confidence
        && !(0.0..=1.0).contains(&value)
    {
        return Err(format!("min_confidence {value} is outside 0..=1"));
    }
    if let Some(value) = settings.timeout_ms
        && !(1..=60_000).contains(&value)
    {
        return Err(format!("timeout_ms {value} is outside 1..=60000"));
    }
    Ok(())
}

/// Validates a route catalog and returns its route count.
pub fn validate_routes_catalog(path: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
    let routes = value
        .as_array()
        .ok_or_else(|| format!("{} must be a JSON array of routes", path.display()))?;
    if routes.is_empty() || routes.len() > MAX_ROUTES {
        return Err(format!(
            "{} holds {} routes; it must hold 1 to {MAX_ROUTES}",
            path.display(),
            routes.len()
        ));
    }
    let mut ids = std::collections::HashSet::new();
    for (index, route) in routes.iter().enumerate() {
        let field = |name: &str| {
            route
                .get(name)
                .and_then(serde_json::Value::as_str)
                .filter(|text| !text.trim().is_empty())
                .ok_or_else(|| format!("route {index} needs a nonempty `{name}`"))
        };
        let id = field("id")?;
        if !valid_route_id(id) {
            return Err(format!(
                "route {index} id `{id}` must match ^[a-z][a-z0-9_-]{{0,63}}$"
            ));
        }
        if id == NO_MATCH {
            return Err(format!("route {index} uses the reserved id `{NO_MATCH}`"));
        }
        if !ids.insert(id) {
            return Err(format!("route id `{id}` appears more than once"));
        }
        field("description")?;
        field("instructions")?;
    }
    Ok(routes.len())
}

fn valid_route_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some('a'..='z'))
        && id.len() <= 64
        && chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(json: &str) -> Result<usize, String> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("routes.json");
        std::fs::write(&path, json).unwrap();
        validate_routes_catalog(&path)
    }

    #[test]
    fn accepts_a_valid_catalog() {
        assert_eq!(
            catalog(r#"[{"id":"fix-bug","description":"d","instructions":"i"}]"#),
            Ok(1)
        );
    }

    #[test]
    fn rejects_each_catalog_rule_the_engine_enforces() {
        let cases = [
            ("{}", "JSON array"),
            ("[]", "1 to 64"),
            ("not json", "is not JSON"),
            (
                r#"[{"id":"Fix","description":"d","instructions":"i"}]"#,
                "must match",
            ),
            (
                r#"[{"id":"no_match","description":"d","instructions":"i"}]"#,
                "reserved id",
            ),
            (
                r#"[{"id":"a","description":"d","instructions":"i"},{"id":"a","description":"d","instructions":"i"}]"#,
                "more than once",
            ),
            (
                r#"[{"id":"a","description":" ","instructions":"i"}]"#,
                "`description`",
            ),
            (r#"[{"id":"a","description":"d"}]"#, "`instructions`"),
        ];
        for (json, expected) in cases {
            let error = catalog(json).unwrap_err();
            assert!(error.contains(expected), "{json}: {error}");
        }
        let too_many = format!(
            "[{}]",
            (0..65)
                .map(|i| format!(r#"{{"id":"r{i}","description":"d","instructions":"i"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(catalog(&too_many).unwrap_err().contains("1 to 64"));
    }

    #[test]
    fn settings_ranges_match_the_engine() {
        let base = JevRoutingSettings {
            source: "core.routing.jev".to_string(),
            routes_file: PathBuf::from("/r.json"),
            min_confidence: Some(0.8),
            timeout_ms: Some(2000),
        };
        assert_eq!(validate_settings(&base), Ok(()));
        let high = JevRoutingSettings {
            min_confidence: Some(1.5),
            ..base.clone()
        };
        assert!(validate_settings(&high).is_err());
        let slow = JevRoutingSettings {
            timeout_ms: Some(60_001),
            ..base
        };
        assert!(validate_settings(&slow).is_err());
    }

    #[test]
    fn explicit_preset_routing_resolves_against_the_preset_directory() {
        let dir = tempfile::tempdir().unwrap();
        let preset = dir.path().join("preset");
        std::fs::create_dir_all(&preset).unwrap();
        std::fs::write(
            preset.join("autoloops.toml"),
            "[routing.jev]\nenabled = true\nroutes_file = \"routes.json\"\ntimeout_ms = 3000\n",
        )
        .unwrap();
        let mut config = RalphConfig::default();
        config.core.workspace_root = dir.path().to_path_buf();
        config.core.autoloop_preset = Some("preset".to_string());

        let settings = effective_jev_routing(&config).unwrap().expect("enabled");
        assert_eq!(settings.routes_file, preset.join("routes.json"));
        assert_eq!(settings.timeout_ms, Some(3000));
    }

    #[test]
    fn generated_routing_resolves_against_the_workspace() {
        let mut config: RalphConfig = serde_yaml::from_str(
            "core:\n  routing:\n    jev:\n      enabled: true\n      routes_file: routing/routes.json\n",
        )
        .unwrap();
        config.core.workspace_root = PathBuf::from("/work");
        let settings = effective_jev_routing(&config).unwrap().expect("enabled");
        assert_eq!(
            settings.routes_file,
            PathBuf::from("/work/routing/routes.json")
        );
        assert_eq!(settings.source, "core.routing.jev");

        config.core.routing.jev.as_mut().unwrap().enabled = false;
        assert_eq!(effective_jev_routing(&config).unwrap(), None);
    }
}
