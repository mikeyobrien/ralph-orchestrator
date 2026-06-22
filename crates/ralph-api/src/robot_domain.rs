use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::errors::ApiError;
use crate::loop_support::now_ts;

#[derive(Clone)]
pub struct RobotDomain {
    workspace_root: PathBuf,
}

pub struct RobotResponseWrite {
    pub question_id: i64,
    pub response: String,
}

pub struct RobotGuidanceWrite {
    pub text: String,
}

impl RobotDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    pub fn question(&self) -> Result<Option<Value>, ApiError> {
        self.read_optional_json(&self.api_dir().join("robot-question.json"))
    }

    pub fn checkin(&self) -> Result<Option<Value>, ApiError> {
        self.read_optional_json(&self.api_dir().join("robot-checkin.json"))
    }

    pub fn respond(&self, params: &Value) -> Result<RobotResponseWrite, ApiError> {
        let map = object_params(params, "robot.respond")?;
        let question_id = required_i64(map, &["questionId", "question_id", "id"])?;
        let loop_id = required_string(map, &["loopId", "loop_id"])?;
        let response_token = required_string(map, &["responseToken", "response_token", "token"])?;
        let response = required_response(map)?;

        let payload = json!({
            "id": question_id,
            "loop_id": loop_id,
            "response_token": response_token,
            "response": response,
            "timestamp": now_ts(),
        });
        self.write_json_atomic(&self.api_dir().join("robot-response.json"), &payload)?;

        Ok(RobotResponseWrite {
            question_id,
            response,
        })
    }

    pub fn guidance(&self, params: &Value) -> Result<RobotGuidanceWrite, ApiError> {
        let map = object_params(params, "robot.guidance")?;
        let text = required_string(map, &["text", "message", "payload"])?;
        let events_path = self.active_events_path()?;
        append_simple_event(&events_path, "human.guidance", &text)?;

        Ok(RobotGuidanceWrite { text })
    }

    fn api_dir(&self) -> PathBuf {
        self.workspace_root.join(".ralph/api")
    }

    fn active_events_path(&self) -> Result<PathBuf, ApiError> {
        let marker_path = self.workspace_root.join(".ralph/current-events");
        let marker = fs::read_to_string(&marker_path).map_err(|err| {
            ApiError::not_found(format!(
                "failed to read active events marker {}: {err}",
                marker_path.display()
            ))
        })?;
        let marker = marker.trim();
        if marker.is_empty() {
            return Err(ApiError::invalid_params("active events marker is empty"));
        }

        let marker_path = Path::new(marker);
        if marker_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(ApiError::invalid_params(
                "active events marker must not contain parent directory components",
            ));
        }

        let resolved = if marker_path.is_absolute() {
            marker_path.to_path_buf()
        } else {
            self.workspace_root.join(marker_path)
        };

        if !resolved.starts_with(&self.workspace_root) {
            return Err(ApiError::invalid_params(
                "active events marker resolves outside workspace",
            ));
        }

        Ok(resolved)
    }

    fn read_optional_json(&self, path: &Path) -> Result<Option<Value>, ApiError> {
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(path).map_err(|err| {
            ApiError::internal(format!("failed to read {}: {err}", path.display()))
        })?;
        serde_json::from_str(&raw)
            .map(Some)
            .map_err(|err| ApiError::internal(format!("failed to parse {}: {err}", path.display())))
    }

    fn write_json_atomic(&self, path: &Path, value: &Value) -> Result<(), ApiError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                ApiError::internal(format!("failed to create {}: {err}", parent.display()))
            })?;
        }

        let tmp_path = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|err| ApiError::internal(format!("failed to serialize robot JSON: {err}")))?;
        fs::write(&tmp_path, bytes).map_err(|err| {
            ApiError::internal(format!("failed to write {}: {err}", tmp_path.display()))
        })?;
        fs::rename(&tmp_path, path).map_err(|err| {
            ApiError::internal(format!(
                "failed to move {} to {}: {err}",
                tmp_path.display(),
                path.display()
            ))
        })
    }
}

fn object_params<'a>(params: &'a Value, method: &str) -> Result<&'a Map<String, Value>, ApiError> {
    params
        .as_object()
        .ok_or_else(|| ApiError::invalid_params(format!("{method} params must be an object")))
}

fn required_i64(map: &Map<String, Value>, keys: &[&str]) -> Result<i64, ApiError> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(Value::as_i64))
        .ok_or_else(|| {
            ApiError::invalid_params(format!(
                "missing or invalid integer field '{}'",
                keys.join("'/'")
            ))
        })
}

fn required_string(map: &Map<String, Value>, keys: &[&str]) -> Result<String, ApiError> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            ApiError::invalid_params(format!(
                "missing or invalid string field '{}'",
                keys.join("'/'")
            ))
        })
}

fn required_response(map: &Map<String, Value>) -> Result<String, ApiError> {
    let value = ["response", "message", "text", "payload"]
        .iter()
        .find_map(|key| map.get(*key))
        .ok_or_else(|| {
            ApiError::invalid_params("robot.respond requires response, message, text, or payload")
        })?;

    match value {
        Value::String(response) if !response.is_empty() => Ok(response.clone()),
        Value::String(_) => Err(ApiError::invalid_params("robot response cannot be empty")),
        Value::Null => Err(ApiError::invalid_params("robot response cannot be null")),
        Value::Object(_) | Value::Array(_) => Ok(value.to_string()),
        other => Ok(other.to_string()),
    }
}

fn append_simple_event(path: &Path, topic: &str, payload: &str) -> Result<(), ApiError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            ApiError::internal(format!("failed to create {}: {err}", parent.display()))
        })?;
    }

    let event = json!({
        "topic": topic,
        "payload": payload,
        "ts": now_ts(),
    });
    let line = serde_json::to_string(&event)
        .map_err(|err| ApiError::internal(format!("failed to serialize event: {err}")))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| ApiError::internal(format!("failed to open {}: {err}", path.display())))?;
    writeln!(file, "{line}")
        .map_err(|err| ApiError::internal(format!("failed to append {}: {err}", path.display())))
}
