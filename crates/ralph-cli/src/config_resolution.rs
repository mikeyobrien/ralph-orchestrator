use anyhow::{Context, Result};
use ralph_core::RalphConfig;
use serde_yaml::Value;
use std::path::{Path, PathBuf};

use crate::ConfigSource;

pub(crate) fn default_user_config_path() -> Option<PathBuf> {
    user_config_path_from_home(home_dir_from_env().as_deref())
}

pub(crate) fn user_config_label_if_exists() -> Option<String> {
    let path = default_user_config_path()?;
    path.exists().then(|| path.display().to_string())
}

pub(crate) fn load_optional_user_config_value() -> Result<Option<(Value, String)>> {
    let path = default_user_config_path();
    load_optional_user_config_value_from(path.as_deref())
}

pub(crate) fn load_optional_user_config_value_from(
    path: Option<&Path>,
) -> Result<Option<(Value, String)>> {
    let Some(path) = path else {
        return Ok(None);
    };

    if !path.exists() {
        return Ok(None);
    }

    let label = path.display().to_string();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to load config from {}", label))?;
    let value = parse_yaml_value(&content, &label)?;
    Ok(Some((value, label)))
}

pub(crate) fn parse_yaml_value(content: &str, label: &str) -> Result<Value> {
    serde_yaml::from_str(content).with_context(|| format!("Failed to parse YAML from {}", label))
}

pub(crate) fn default_core_value() -> Result<Value> {
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

pub(crate) fn merge_yaml_values(base: Value, overlay: Value) -> Result<Value> {
    match (base, overlay) {
        (Value::Mapping(mut base_map), Value::Mapping(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                let merged_value = if let Some(base_value) = base_map.remove(&key) {
                    merge_yaml_values(base_value, overlay_value)?
                } else {
                    overlay_value
                };
                base_map.insert(key, merged_value);
            }
            Ok(Value::Mapping(base_map))
        }
        (_, overlay) => Ok(overlay),
    }
}

pub(crate) fn compose_core_label(
    user_label: Option<&str>,
    primary_label: &str,
    primary_uses_defaults: bool,
) -> String {
    match user_label {
        Some(user) if primary_uses_defaults => format!("{user} + defaults"),
        Some(user) => format!("{user} + {primary_label}"),
        None => primary_label.to_string(),
    }
}

pub(crate) fn split_config_sources(
    config_sources: &[ConfigSource],
) -> (Vec<ConfigSource>, Vec<ConfigSource>) {
    config_sources
        .iter()
        .cloned()
        .partition(|source| !matches!(source, ConfigSource::Override { .. }))
}

pub(crate) fn find_workspace_config_path(root: &Path) -> Option<PathBuf> {
    ["ralph.yml", "ralph.yaml"]
        .iter()
        .map(|candidate| root.join(candidate))
        .find(|path| path.exists())
}

fn user_config_path_from_home(home: Option<&Path>) -> Option<PathBuf> {
    Some(home?.join(".ralph").join("config.yml"))
}

fn home_dir_from_env() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut joined = PathBuf::from(drive);
            joined.push(path);
            Some(joined)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    #[test]
    fn user_config_path_uses_ralph_home_convention() {
        let path = user_config_path_from_home(Some(Path::new("/tmp/test-home")))
            .expect("path should exist");
        assert_eq!(path, PathBuf::from("/tmp/test-home/.ralph/config.yml"));
    }

    #[test]
    fn merge_yaml_values_recursively_merges_maps_and_replaces_arrays() {
        let base: Value = serde_yaml::from_str(
            r"
hooks:
  events:
    pre.loop.start:
      - name: user-hook
        command: [./user.sh]
event_loop:
  max_iterations: 10
  tags: [one, two]
",
        )
        .unwrap();
        let overlay: Value = serde_yaml::from_str(
            r"
hooks:
  events:
    pre.loop.start:
      - name: local-hook
        command: [./local.sh]
event_loop:
  completion_promise: LOOP_COMPLETE
  tags: [three]
",
        )
        .unwrap();

        let merged = merge_yaml_values(base, overlay).unwrap();
        let hooks = merged["hooks"]["events"]["pre.loop.start"]
            .as_sequence()
            .expect("hook sequence");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["name"].as_str(), Some("local-hook"));
        assert_eq!(merged["event_loop"]["max_iterations"].as_i64(), Some(10));
        assert_eq!(
            merged["event_loop"]["completion_promise"].as_str(),
            Some("LOOP_COMPLETE")
        );
        let tags = merged["event_loop"]["tags"].as_sequence().expect("tags");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].as_str(), Some("three"));
    }

    #[test]
    fn compose_core_label_uses_defaults_suffix_only_for_user_only_resolution() {
        assert_eq!(
            compose_core_label(Some("/home/test/.ralph/config.yml"), "ralph.yml", true,),
            "/home/test/.ralph/config.yml + defaults"
        );
        assert_eq!(
            compose_core_label(
                Some("/home/test/.ralph/config.yml"),
                "repo/ralph.yml",
                false,
            ),
            "/home/test/.ralph/config.yml + repo/ralph.yml"
        );
        assert_eq!(compose_core_label(None, "ralph.yml", true), "ralph.yml");
    }
}
