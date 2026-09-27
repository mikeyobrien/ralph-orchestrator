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

/// Merges one config layer over another.
///
/// Like [`merge_yaml_values`], except that `cli.args` belong to the backend
/// they were written for: when the overlay selects a different `cli.backend`
/// without its own `cli.args`, the base layer's args are dropped instead of
/// leaking onto the new backend. A user config that pairs `backend: pi` with
/// pi flags must not hand those flags to a project's `backend: claude`.
pub(crate) fn merge_config_layers(mut base: Value, overlay: Value) -> Result<Value> {
    let cli = Value::String("cli".to_string());
    let backend = Value::String("backend".to_string());
    let args = Value::String("args".to_string());
    let overlay_cli = overlay.get(&cli).and_then(Value::as_mapping);
    let overlay_backend = overlay_cli.and_then(|cli| cli.get(&backend));
    let overlay_has_args = overlay_cli.is_some_and(|cli| cli.contains_key(&args));
    if let Some(overlay_backend) = overlay_backend
        && !overlay_has_args
        && let Some(base_cli) = base.get_mut(&cli).and_then(Value::as_mapping_mut)
        && base_cli.get(&backend) != Some(overlay_backend)
        && base_cli.remove(&args).is_some()
    {
        tracing::debug!(
            "cli.args from a lower config layer were dropped because a higher layer selected a different cli.backend"
        );
    }
    merge_yaml_values(base, overlay)
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

pub(crate) fn home_dir_from_env() -> Option<PathBuf> {
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

    fn yaml(text: &str) -> Value {
        serde_yaml::from_str(text).unwrap()
    }

    #[test]
    fn a_higher_layer_backend_does_not_inherit_another_backends_args() {
        let user = yaml("cli:\n  backend: pi\n  args: [--provider, spark]\n");
        let project = yaml("cli:\n  backend: claude\n");
        let merged = merge_config_layers(user, project).unwrap();
        assert_eq!(merged["cli"]["backend"], yaml("claude"));
        assert!(merged["cli"].get("args").is_none(), "{merged:?}");
    }

    #[test]
    fn args_are_kept_when_the_backend_is_unchanged_or_overridden_together() {
        let user = yaml("cli:\n  backend: pi\n  args: [--provider, spark]\n");
        let same = merge_config_layers(user.clone(), yaml("cli:\n  backend: pi\n")).unwrap();
        assert_eq!(same["cli"]["args"], yaml("[--provider, spark]"));

        let paired = merge_config_layers(
            user.clone(),
            yaml("cli:\n  backend: claude\n  args: [--verbose]\n"),
        )
        .unwrap();
        assert_eq!(paired["cli"]["args"], yaml("[--verbose]"));

        let args_only = merge_config_layers(user, yaml("cli:\n  args: [--model, x]\n")).unwrap();
        assert_eq!(args_only["cli"]["backend"], yaml("pi"));
        assert_eq!(args_only["cli"]["args"], yaml("[--model, x]"));
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
