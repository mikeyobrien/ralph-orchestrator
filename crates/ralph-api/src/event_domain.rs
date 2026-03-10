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

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_core::EventRecord;
    use std::io::Write;

    fn write_records(dir: &Path, records: &[EventRecord]) {
        let events_dir = dir.join(".ralph");
        std::fs::create_dir_all(&events_dir).unwrap();
        let mut f = File::create(events_dir.join("events.jsonl")).unwrap();
        for r in records {
            writeln!(f, "{}", serde_json::to_string(r).unwrap()).unwrap();
        }
    }

    fn record(topic: &str, hat: &str, payload: &str, ts: &str, iteration: u32) -> EventRecord {
        EventRecord {
            ts: ts.into(),
            iteration,
            hat: hat.into(),
            topic: topic.into(),
            triggered: None,
            payload: payload.into(),
            blocked_count: None,
        }
    }

    #[test]
    fn list_returns_events_from_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let records = vec![
            record("build.done", "Builder", "ok", "2026-01-01T00:00:00Z", 1),
            record(
                "review.done",
                "Reviewer",
                "approved",
                "2026-01-02T00:00:00Z",
                2,
            ),
            record(
                "test.pass",
                "Tester",
                "all green",
                "2026-01-03T00:00:00Z",
                3,
            ),
        ];
        write_records(tmp.path(), &records);

        let domain = EventDomain::new(tmp.path());
        let result = domain.list(EventListParams::default()).unwrap();
        let events = result["events"].as_array().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(result["total"], 3);
        assert_eq!(events[0]["topic"], "build.done");
        assert_eq!(events[0]["sourceHat"], "Builder");
        assert_eq!(events[0]["iteration"], 1);
        assert_eq!(events[1]["payload"], "approved");
        assert_eq!(events[2]["timestamp"], "2026-01-03T00:00:00Z");
    }

    #[test]
    fn list_filters_by_topic_exact() {
        let tmp = tempfile::tempdir().unwrap();
        write_records(
            tmp.path(),
            &[
                record("build.done", "B", "", "2026-01-01T00:00:00Z", 1),
                record("review.done", "R", "", "2026-01-02T00:00:00Z", 2),
                record("build.failed", "B", "", "2026-01-03T00:00:00Z", 3),
            ],
        );
        let domain = EventDomain::new(tmp.path());
        let result = domain
            .list(EventListParams {
                topic: Some("build.done".into()),
                ..Default::default()
            })
            .unwrap();
        let events = result["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["topic"], "build.done");
    }

    #[test]
    fn list_filters_by_topic_wildcard() {
        let tmp = tempfile::tempdir().unwrap();
        write_records(
            tmp.path(),
            &[
                record("build.done", "B", "", "2026-01-01T00:00:00Z", 1),
                record("review.done", "R", "", "2026-01-02T00:00:00Z", 2),
                record("build.failed", "B", "", "2026-01-03T00:00:00Z", 3),
            ],
        );
        let domain = EventDomain::new(tmp.path());
        let result = domain
            .list(EventListParams {
                topic: Some("build.*".into()),
                ..Default::default()
            })
            .unwrap();
        let events = result["events"].as_array().unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn list_filters_by_task_id_in_payload() {
        let tmp = tempfile::tempdir().unwrap();
        write_records(
            tmp.path(),
            &[
                record("a", "B", "working on task-123", "2026-01-01T00:00:00Z", 1),
                record("b", "B", "unrelated work", "2026-01-02T00:00:00Z", 2),
            ],
        );
        let domain = EventDomain::new(tmp.path());
        let result = domain
            .list(EventListParams {
                task_id: Some("task-123".into()),
                ..Default::default()
            })
            .unwrap();
        let events = result["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["topic"], "a");
    }

    #[test]
    fn list_respects_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let records: Vec<_> = (0..5)
            .map(|i| record("t", "B", "", &format!("2026-01-0{}T00:00:00Z", i + 1), i))
            .collect();
        write_records(tmp.path(), &records);

        let domain = EventDomain::new(tmp.path());
        let result = domain
            .list(EventListParams {
                limit: Some(2),
                ..Default::default()
            })
            .unwrap();
        let events = result["events"].as_array().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(result["total"], 5);
    }

    #[test]
    fn list_filters_by_after_timestamp() {
        let tmp = tempfile::tempdir().unwrap();
        write_records(
            tmp.path(),
            &[
                record("a", "B", "", "2026-01-01T00:00:00Z", 1),
                record("b", "B", "", "2026-01-02T00:00:00Z", 2),
                record("c", "B", "", "2026-01-03T00:00:00Z", 3),
            ],
        );
        let domain = EventDomain::new(tmp.path());
        let result = domain
            .list(EventListParams {
                after: Some("2026-01-01T00:00:00Z".into()),
                ..Default::default()
            })
            .unwrap();
        let events = result["events"].as_array().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["topic"], "b");
        assert_eq!(events[1]["topic"], "c");
    }

    #[test]
    fn list_empty_file_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let events_dir = tmp.path().join(".ralph");
        std::fs::create_dir_all(&events_dir).unwrap();
        File::create(events_dir.join("events.jsonl")).unwrap();

        let domain = EventDomain::new(tmp.path());
        let result = domain.list(EventListParams::default()).unwrap();
        assert_eq!(result["events"].as_array().unwrap().len(), 0);
        assert_eq!(result["total"], 0);
    }

    #[test]
    fn list_skips_malformed_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let events_dir = tmp.path().join(".ralph");
        std::fs::create_dir_all(&events_dir).unwrap();
        let mut f = File::create(events_dir.join("events.jsonl")).unwrap();
        let r1 = record("first", "B", "", "2026-01-01T00:00:00Z", 1);
        writeln!(f, "{}", serde_json::to_string(&r1).unwrap()).unwrap();
        writeln!(f, "not json at all").unwrap();
        let r2 = record("third", "B", "", "2026-01-03T00:00:00Z", 3);
        writeln!(f, "{}", serde_json::to_string(&r2).unwrap()).unwrap();

        let domain = EventDomain::new(tmp.path());
        let result = domain.list(EventListParams::default()).unwrap();
        let events = result["events"].as_array().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["topic"], "first");
        assert_eq!(events[1]["topic"], "third");
    }
}
