use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::errors::ApiError;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EventListParams {
    pub topic: Option<String>,
    pub limit: Option<usize>,
    pub after: Option<String>,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventResponse {
    pub topic: String,
    pub payload: String,
    pub source_hat: String,
    pub iteration: u32,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggered: Option<String>,
}

#[derive(Clone)]
pub struct EventDomain {
    workspace_root: PathBuf,
}

impl EventDomain {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    pub fn list(&self, params: EventListParams) -> Result<Value, ApiError> {
        let path = self.workspace_root.join(".ralph/events.jsonl");
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return Ok(json!({ "events": [], "total": 0 })),
        };

        let limit = params.limit.unwrap_or(100).min(1000);
        let mut events = Vec::new();

        for line in BufReader::new(file).lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let record: ralph_core::EventRecord = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // topic filter
            if let Some(ref topic) = params.topic {
                if topic.contains('*') {
                    if !glob_match(topic, &record.topic) {
                        continue;
                    }
                } else if record.topic != *topic {
                    continue;
                }
            }

            // after filter (ISO 8601 lexicographic comparison)
            if let Some(ref after) = params.after {
                if record.ts.as_str() <= after.as_str() {
                    continue;
                }
            }

            // task_id filter (payload contains check)
            if let Some(ref task_id) = params.task_id {
                if !record.payload.contains(task_id.as_str()) {
                    continue;
                }
            }

            events.push(EventResponse {
                topic: record.topic,
                payload: record.payload,
                source_hat: record.hat,
                iteration: record.iteration,
                timestamp: record.ts,
                triggered: record.triggered,
            });
        }

        let total = events.len();
        events.truncate(limit);
        Ok(json!({ "events": events, "total": total }))
    }
}

/// Simple glob matching supporting only `*` as wildcard for any sequence of chars.
fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match text[pos..].find(part) {
            Some(idx) => {
                if i == 0 && idx != 0 {
                    return false;
                }
                pos += idx + part.len();
            }
            None => return false,
        }
    }
    if let Some(last) = parts.last() {
        if !last.is_empty() {
            return text.ends_with(last);
        }
    }
    true
}
