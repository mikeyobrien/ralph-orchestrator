use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

use crate::errors::ApiError;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateParams {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGetResult {
    pub raw: String,
    pub parsed: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateResult {
    pub success: bool,
    pub parsed: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct ConfigDomain {
    config_path: PathBuf,
}

impl ConfigDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            config_path: workspace_root.as_ref().join("ralph.yml"),
        }
    }

    pub fn get(&self) -> Result<ConfigGetResult, ApiError> {
        if !self.config_path.exists() {
            return Err(ApiError::not_found(
                "configuration file not found at ralph.yml",
            ));
        }

        let raw = fs::read_to_string(&self.config_path).map_err(|error| {
            ApiError::internal(format!(
                "failed reading config file '{}': {error}",
                self.config_path.display()
            ))
        })?;

        let parsed = parse_yaml_to_json_object(&raw).unwrap_or_else(|error| {
            warn!(
                path = %self.config_path.display(),
                %error,
                "failed parsing config yaml in config.get; returning empty object"
            );
            serde_json::Map::new()
        });

        Ok(ConfigGetResult { raw, parsed })
    }

    pub fn update(&self, params: ConfigUpdateParams) -> Result<ConfigUpdateResult, ApiError> {
        let parsed = parse_yaml_to_json_object(&params.content)
            .map_err(|error| ApiError::config_invalid(format!("invalid YAML syntax: {error}")))?;

        safe_write(&self.config_path, &params.content)?;

        Ok(ConfigUpdateResult {
            success: true,
            parsed,
        })
    }
}

fn parse_yaml_to_json_object(content: &str) -> Result<serde_json::Map<String, Value>, String> {
    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|error| error.to_string())?;
    let json_value = serde_json::to_value(yaml_value).map_err(|error| error.to_string())?;

    match json_value {
        Value::Object(map) => Ok(map),
        _ => Err("configuration root must be a YAML mapping/object".to_string()),
    }
}

fn safe_write(path: &Path, content: &str) -> Result<(), ApiError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ApiError::internal(format!(
                "failed creating config directory '{}': {error}",
                parent.display()
            ))
        })?;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    let temp_path = path.with_extension(format!("tmp-{}-{nanos}", std::process::id()));

    fs::write(&temp_path, content).map_err(|error| {
        ApiError::internal(format!(
            "failed writing temporary config '{}': {error}",
            temp_path.display()
        ))
    })?;

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(ApiError::internal(format!(
            "failed replacing config file '{}': {error}",
            path.display()
        )));
    }

    Ok(())
}
