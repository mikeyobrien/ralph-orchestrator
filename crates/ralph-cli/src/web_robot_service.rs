use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use ralph_proto::{CheckinContext, RobotService};
use serde_json::{Map, Value, json};
use tracing::{debug, info};

/// File-backed RObot service used by `RObot.mode: web`.
///
/// This is intentionally loop-side only: another process can observe and write
/// files under `.ralph/api/`, but this module does not expose HTTP routes.
pub(crate) struct WebRobotService {
    workspace_root: PathBuf,
    api_dir: PathBuf,
    timeout_secs: u64,
    loop_id: String,
    shutdown: Arc<AtomicBool>,
    next_question_id: AtomicI32,
    active_question: Mutex<Option<ActiveQuestion>>,
}

#[derive(Clone, Debug)]
struct ActiveQuestion {
    id: i32,
    response_token: String,
}

enum ResponseRead {
    Ready(String),
    Incomplete,
    Stale,
    Invalid(anyhow::Error),
}

impl WebRobotService {
    const QUESTION_FILE: &'static str = "robot-question.json";
    const RESPONSE_FILE: &'static str = "robot-response.json";
    const CHECKIN_FILE: &'static str = "robot-checkin.json";
    const POLL_INTERVAL: Duration = Duration::from_millis(250);

    pub(crate) fn new(workspace_root: PathBuf, timeout_secs: u64, loop_id: String) -> Self {
        let api_dir = workspace_root.join(".ralph/api");
        Self {
            workspace_root,
            api_dir,
            timeout_secs,
            loop_id,
            shutdown: Arc::new(AtomicBool::new(false)),
            next_question_id: AtomicI32::new(1),
            active_question: Mutex::new(None),
        }
    }

    pub(crate) fn start(&self) -> Result<()> {
        fs::create_dir_all(&self.api_dir)
            .with_context(|| format!("failed to create {}", self.api_dir.display()))?;
        info!(
            workspace = %self.workspace_root.display(),
            api_dir = %self.api_dir.display(),
            timeout_secs = self.timeout_secs,
            loop_id = %self.loop_id,
            "Web robot service active"
        );
        Ok(())
    }

    pub(crate) fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    fn get_active_question(&self) -> Result<Option<ActiveQuestion>> {
        if let Some(active_question) = self
            .active_question
            .lock()
            .map_err(|err| anyhow!("web robot active question lock poisoned: {err}"))?
            .clone()
        {
            return Ok(Some(active_question));
        }

        let Some(active_question) = self.hydrate_active_question_from_file()? else {
            return Ok(None);
        };
        self.next_question_id
            .fetch_max(active_question.id.saturating_add(1), Ordering::Relaxed);
        *self
            .active_question
            .lock()
            .map_err(|err| anyhow!("web robot active question lock poisoned: {err}"))? =
            Some(active_question.clone());
        Ok(Some(active_question))
    }

    fn hydrate_active_question_from_file(&self) -> Result<Option<ActiveQuestion>> {
        let question_path = self.question_path();
        if !question_path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&question_path)
            .with_context(|| format!("failed to read {}", question_path.display()))?;
        let value: Value = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", question_path.display()))?;
        let Value::Object(map) = value else {
            return Ok(None);
        };
        if Self::response_str(&map, &["loop_id"]) != Some(self.loop_id.as_str()) {
            return Ok(None);
        }
        let Some(id) = Self::response_i64(&map, &["id"]) else {
            return Ok(None);
        };
        let Ok(id) = i32::try_from(id) else {
            return Ok(None);
        };
        let Some(response_token) = Self::response_str(&map, &["response_token", "token"]) else {
            return Ok(None);
        };

        Ok(Some(ActiveQuestion {
            id,
            response_token: response_token.to_string(),
        }))
    }

    fn question_path(&self) -> PathBuf {
        self.api_dir.join(Self::QUESTION_FILE)
    }

    fn response_path(&self) -> PathBuf {
        self.api_dir.join(Self::RESPONSE_FILE)
    }

    fn checkin_path(&self) -> PathBuf {
        self.api_dir.join(Self::CHECKIN_FILE)
    }

    fn send_question(&self, payload: &str) -> Result<i32> {
        fs::create_dir_all(&self.api_dir)
            .with_context(|| format!("failed to create {}", self.api_dir.display()))?;

        if let Some(active_question) = self.get_active_question()? {
            info!(
                question_id = active_question.id,
                loop_id = %self.loop_id,
                "Reusing existing web robot question"
            );
            return Ok(active_question.id);
        }

        let question_id = self.next_question_id.fetch_add(1, Ordering::Relaxed);
        let response_token = format!(
            "{}-{}-{}",
            self.loop_id,
            question_id,
            Utc::now().timestamp_micros()
        );
        let payload_json = serde_json::from_str::<Value>(payload).ok();
        let hat = payload_json
            .as_ref()
            .and_then(|value| {
                Self::first_json_path(
                    value,
                    &[
                        &["hat"],
                        &["current_hat"],
                        &["context", "hat"],
                        &["context", "current_hat"],
                        &["metadata", "hat"],
                        &["metadata", "current_hat"],
                    ],
                )
            })
            .cloned()
            .unwrap_or(Value::Null);
        let iteration = payload_json
            .as_ref()
            .and_then(|value| {
                Self::first_json_path(
                    value,
                    &[
                        &["iteration"],
                        &["context", "iteration"],
                        &["metadata", "iteration"],
                    ],
                )
            })
            .cloned()
            .unwrap_or(Value::Null);

        let mut question = Map::new();
        question.insert("id".to_string(), Value::from(question_id));
        question.insert(
            "response_token".to_string(),
            Value::String(response_token.clone()),
        );
        question.insert("payload".to_string(), Value::String(payload.to_string()));
        question.insert(
            "payload_json".to_string(),
            payload_json.unwrap_or(Value::Null),
        );
        question.insert("hat".to_string(), hat);
        question.insert("iteration".to_string(), iteration);
        question.insert("loop_id".to_string(), Value::String(self.loop_id.clone()));
        question.insert(
            "timestamp".to_string(),
            Value::String(Utc::now().to_rfc3339()),
        );
        question.insert(
            "timeout_seconds".to_string(),
            Value::from(self.timeout_secs),
        );

        Self::write_json_atomic(&self.question_path(), &Value::Object(question))?;
        *self
            .active_question
            .lock()
            .map_err(|err| anyhow!("web robot active question lock poisoned: {err}"))? =
            Some(ActiveQuestion {
                id: question_id,
                response_token,
            });

        info!(
            question_id,
            loop_id = %self.loop_id,
            "Wrote web robot question"
        );
        Ok(question_id)
    }

    fn first_json_path<'a>(value: &'a Value, paths: &[&[&str]]) -> Option<&'a Value> {
        paths.iter().find_map(|path| {
            path.iter()
                .try_fold(value, |current, key| current.get(*key))
        })
    }

    fn wait_for_response(&self) -> Result<Option<String>> {
        let deadline = if self.timeout_secs == 0 {
            None
        } else {
            Some(Instant::now() + Duration::from_secs(self.timeout_secs))
        };
        let active_question = self
            .get_active_question()?
            .ok_or_else(|| anyhow!("no active web robot question"))?;

        info!(
            loop_id = %self.loop_id,
            timeout_secs = self.timeout_secs,
            response_path = %self.response_path().display(),
            "Waiting for web robot response"
        );

        loop {
            if let Some(response) = self.read_response_file(&active_question)? {
                self.cleanup_question_and_response()?;
                info!(loop_id = %self.loop_id, "Received web robot response");
                return Ok(Some(response));
            }

            if self.shutdown.load(Ordering::Relaxed) {
                debug!(loop_id = %self.loop_id, "Web robot wait interrupted");
                self.cleanup_question_and_response()?;
                return Ok(None);
            }

            if let Some(deadline) = deadline
                && Instant::now() >= deadline
            {
                self.cleanup_question_and_response()?;
                return Ok(None);
            }

            std::thread::sleep(Self::POLL_INTERVAL);
        }
    }

    fn send_checkin(
        &self,
        iteration: u32,
        elapsed: Duration,
        context: Option<&CheckinContext>,
    ) -> Result<i32> {
        fs::create_dir_all(&self.api_dir)
            .with_context(|| format!("failed to create {}", self.api_dir.display()))?;

        let context_json = context.map_or(Value::Null, |ctx| {
            json!({
                "current_hat": ctx.current_hat,
                "open_tasks": ctx.open_tasks,
                "closed_tasks": ctx.closed_tasks,
                "cumulative_cost": ctx.cumulative_cost,
            })
        });

        let checkin = json!({
            "iteration": iteration,
            "elapsed_seconds": elapsed.as_secs(),
            "elapsed_millis": elapsed.as_millis(),
            "loop_id": self.loop_id,
            "timestamp": Utc::now().to_rfc3339(),
            "context": context_json,
        });

        Self::write_json_atomic(&self.checkin_path(), &checkin)?;
        debug!(iteration, loop_id = %self.loop_id, "Wrote web robot check-in");
        Ok(0)
    }

    fn read_response_file(&self, active_question: &ActiveQuestion) -> Result<Option<String>> {
        let response_path = self.response_path();
        if !response_path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&response_path)
            .with_context(|| format!("failed to read {}", response_path.display()))?;
        let value: Value = match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(err) => {
                debug!(
                    error = %err,
                    response_path = %response_path.display(),
                    "Ignoring incomplete web robot response file"
                );
                return Ok(None);
            }
        };
        match Self::extract_correlated_response(value, active_question, &self.loop_id) {
            ResponseRead::Ready(response) => Ok(Some(response)),
            ResponseRead::Incomplete => {
                debug!(
                    response_path = %response_path.display(),
                    question_id = active_question.id,
                    "Ignoring incomplete web robot response file"
                );
                Ok(None)
            }
            ResponseRead::Stale => {
                debug!(
                    response_path = %response_path.display(),
                    question_id = active_question.id,
                    "Removing stale web robot response file"
                );
                self.remove_response_file()?;
                Ok(None)
            }
            ResponseRead::Invalid(err) => {
                debug!(
                    error = %err,
                    response_path = %response_path.display(),
                    "Removing invalid web robot response file"
                );
                self.remove_response_file()?;
                Ok(None)
            }
        }
    }

    fn extract_correlated_response(
        value: Value,
        active_question: &ActiveQuestion,
        loop_id: &str,
    ) -> ResponseRead {
        let Value::Object(map) = value else {
            return ResponseRead::Stale;
        };

        match Self::response_i64(&map, &["id", "question_id"]) {
            Some(id) if id == i64::from(active_question.id) => {}
            Some(_) => return ResponseRead::Stale,
            None => return ResponseRead::Incomplete,
        }
        match Self::response_str(&map, &["loop_id"]) {
            Some(response_loop_id) if response_loop_id == loop_id => {}
            Some(_) => return ResponseRead::Stale,
            None => return ResponseRead::Incomplete,
        }
        match Self::response_str(&map, &["response_token", "token"]) {
            Some(token) if token == active_question.response_token.as_str() => {}
            Some(_) => return ResponseRead::Stale,
            None => return ResponseRead::Incomplete,
        }

        if !["response", "message", "text", "payload"]
            .iter()
            .any(|key| map.get(*key).is_some_and(|value| !value.is_null()))
        {
            return ResponseRead::Incomplete;
        }

        match Self::extract_response(Value::Object(map)) {
            Ok(response) => ResponseRead::Ready(response),
            Err(err) => ResponseRead::Invalid(err),
        }
    }

    fn response_i64(map: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
        keys.iter().find_map(|key| map.get(*key)?.as_i64())
    }

    fn response_str<'a>(map: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
        keys.iter().find_map(|key| map.get(*key)?.as_str())
    }

    fn extract_response(value: Value) -> Result<String> {
        match value {
            Value::String(response) => Ok(response),
            Value::Object(map) => {
                for key in ["response", "message", "text", "payload"] {
                    if let Some(value) = map.get(key) {
                        return match value {
                            Value::String(response) => Ok(response.clone()),
                            Value::Object(_) | Value::Array(_) => Ok(value.to_string()),
                            Value::Null => Err(anyhow!("response field '{key}' is null")),
                            other => Ok(other.to_string()),
                        };
                    }
                }
                Err(anyhow!(
                    "robot response must contain one of: response, message, text, payload"
                ))
            }
            other => Ok(other.to_string()),
        }
    }

    fn cleanup_question_and_response(&self) -> Result<()> {
        self.remove_question_file()?;
        self.remove_response_file()?;
        *self
            .active_question
            .lock()
            .map_err(|err| anyhow!("web robot active question lock poisoned: {err}"))? = None;
        Ok(())
    }

    fn remove_question_file(&self) -> Result<()> {
        Self::remove_file_if_exists(&self.question_path())
    }

    fn remove_response_file(&self) -> Result<()> {
        Self::remove_file_if_exists(&self.response_path())
    }

    fn remove_file_if_exists(path: &Path) -> Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
        }
    }

    fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;

        let tmp_path = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(value)?;
        fs::write(&tmp_path, bytes)
            .with_context(|| format!("failed to write {}", tmp_path.display()))?;
        fs::rename(&tmp_path, path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    }

    fn stop(self) {
        self.shutdown.store(true, Ordering::Relaxed);
        debug!(loop_id = %self.loop_id, "Web robot service stopped");
    }
}

impl RobotService for WebRobotService {
    fn send_question(&self, payload: &str) -> Result<i32> {
        WebRobotService::send_question(self, payload)
    }

    fn wait_for_response(&self, events_path: &Path) -> Result<Option<String>> {
        let _ = events_path;
        WebRobotService::wait_for_response(self)
    }

    fn response_events_are_durable(&self) -> bool {
        false
    }

    fn send_checkin(
        &self,
        iteration: u32,
        elapsed: Duration,
        context: Option<&CheckinContext>,
    ) -> Result<i32> {
        WebRobotService::send_checkin(self, iteration, elapsed, context)
    }

    fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    fn stop(self: Box<Self>) {
        WebRobotService::stop(*self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_proto::RobotService;
    use tempfile::TempDir;

    fn service(dir: &TempDir, timeout_secs: u64) -> WebRobotService {
        let service =
            WebRobotService::new(dir.path().to_path_buf(), timeout_secs, "loop-1".to_string());
        service.start().expect("start service");
        service
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).expect("read json")).expect("parse json")
    }

    fn response_path(dir: &TempDir) -> PathBuf {
        dir.path().join(".ralph/api/robot-response.json")
    }

    fn events_path(dir: &TempDir) -> PathBuf {
        dir.path().join(".ralph/events.jsonl")
    }

    fn correlated_response(dir: &TempDir, key: &str, value: &str) -> Value {
        let question = read_json(&dir.path().join(".ralph/api/robot-question.json"));
        json!({
            "id": question["id"],
            "loop_id": question["loop_id"],
            "response_token": question["response_token"],
            key: value,
        })
    }

    fn write_correlated_response(dir: &TempDir, key: &str, value: &str) {
        let response = correlated_response(dir, key, value);
        fs::write(
            response_path(dir),
            serde_json::to_string(&response).unwrap(),
        )
        .expect("write response");
    }

    #[test]
    fn send_question_writes_question_file_and_preserves_uncorrelated_response() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 300);
        let response_path = response_path(&dir);
        fs::write(&response_path, r#"{"response":"already answered"}"#)
            .expect("write uncorrelated response");

        let id = service
            .send_question(r#"{"question":"Proceed?","hat":"planner","iteration":7}"#)
            .expect("send question");

        assert_eq!(id, 1);
        assert!(
            response_path.exists(),
            "preexisting response must survive send_question"
        );
        let question = read_json(&dir.path().join(".ralph/api/robot-question.json"));
        assert_eq!(question["id"], json!(1));
        assert_eq!(question["payload_json"]["question"], json!("Proceed?"));
        assert_eq!(question["hat"], json!("planner"));
        assert_eq!(question["iteration"], json!(7));
        assert_eq!(question["loop_id"], json!("loop-1"));
        assert!(question["response_token"].as_str().is_some());
        assert_eq!(question["timeout_seconds"], json!(300));
        assert!(question["timestamp"].as_str().is_some());
    }

    #[test]
    fn wait_for_response_consumes_correlated_preexisting_response() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 300);
        service
            .send_question("Need approval?")
            .expect("send question");
        write_correlated_response(&dir, "response", "approved");
        let response_path = response_path(&dir);

        let response = RobotService::wait_for_response(&service, Path::new("unused-events.jsonl"))
            .expect("wait for response");

        assert_eq!(response, Some("approved".to_string()));
        assert!(
            !dir.path().join(".ralph/api/robot-question.json").exists(),
            "question file should be removed"
        );
        assert!(!response_path.exists(), "response file should be removed");
    }

    #[test]
    fn restart_reuses_existing_question_and_accepts_preexisting_response() {
        let dir = TempDir::new().expect("temp dir");
        let initial_service = service(&dir, 300);
        let original_id = initial_service
            .send_question("Need approval?")
            .expect("send question");
        let original_question = read_json(&dir.path().join(".ralph/api/robot-question.json"));
        write_correlated_response(&dir, "response", "approved while down");
        drop(initial_service);

        let restarted = service(&dir, 300);
        let reused_id = restarted
            .send_question("Need approval after restart?")
            .expect("reuse question");
        let reused_question = read_json(&dir.path().join(".ralph/api/robot-question.json"));
        let response =
            RobotService::wait_for_response(&restarted, Path::new("unused-events.jsonl"))
                .expect("wait for response");

        assert_eq!(reused_id, original_id);
        assert_eq!(
            reused_question["response_token"],
            original_question["response_token"]
        );
        assert_eq!(response, Some("approved while down".to_string()));
        assert!(
            !response_path(&dir).exists(),
            "response file should be removed"
        );
    }

    #[test]
    fn send_question_extracts_nested_human_interact_context() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 300);

        service
            .send_question(
                r#"{"question":"Proceed?","context":{"current_hat":"planner","iteration":7}}"#,
            )
            .expect("send question");

        let question = read_json(&dir.path().join(".ralph/api/robot-question.json"));
        assert_eq!(question["payload_json"]["question"], json!("Proceed?"));
        assert_eq!(question["hat"], json!("planner"));
        assert_eq!(question["iteration"], json!(7));
    }

    #[test]
    fn send_checkin_writes_checkin_file() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 300);
        let context = CheckinContext {
            current_hat: Some("executor".to_string()),
            open_tasks: 2,
            closed_tasks: 3,
            cumulative_cost: 1.25,
        };

        service
            .send_checkin(4, Duration::from_millis(1_234), Some(&context))
            .expect("send checkin");

        let checkin = read_json(&dir.path().join(".ralph/api/robot-checkin.json"));
        assert_eq!(checkin["iteration"], json!(4));
        assert_eq!(checkin["elapsed_seconds"], json!(1));
        assert_eq!(checkin["elapsed_millis"], json!(1234));
        assert_eq!(checkin["loop_id"], json!("loop-1"));
        assert_eq!(checkin["context"]["current_hat"], json!("executor"));
        assert_eq!(checkin["context"]["open_tasks"], json!(2));
        assert_eq!(checkin["context"]["closed_tasks"], json!(3));
        assert_eq!(checkin["context"]["cumulative_cost"], json!(1.25));
    }

    #[test]
    fn wait_for_response_removes_stale_uncorrelated_response() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 300);
        service
            .send_question("Need approval?")
            .expect("send question");
        let response_path = response_path(&dir);
        fs::write(
            &response_path,
            r#"{"id":999,"loop_id":"loop-1","response_token":"stale","response":"old"}"#,
        )
        .expect("write stale response");
        let active_question = service.active_question.lock().unwrap().clone().unwrap();

        let response = service
            .read_response_file(&active_question)
            .expect("read stale response");

        assert_eq!(response, None);
        assert!(
            !response_path.exists(),
            "stale response file should be removed"
        );
    }

    #[test]
    fn wait_for_response_times_out_and_cleans_question() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 1);
        service
            .send_question("Need approval?")
            .expect("send question");

        let response = RobotService::wait_for_response(&service, Path::new("unused-events.jsonl"))
            .expect("wait for response");

        assert_eq!(response, None);
        assert!(
            !dir.path().join(".ralph/api/robot-question.json").exists(),
            "question file should be removed on timeout"
        );
    }

    #[test]
    fn timeout_zero_waits_until_response_without_timing_out() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 0);
        service
            .send_question("Need approval?")
            .expect("send question");
        let response_path = response_path(&dir);
        let response = correlated_response(&dir, "message", "continue");

        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            fs::write(&response_path, serde_json::to_string(&response).unwrap())
                .expect("write response");
        });

        let start = Instant::now();
        let response = RobotService::wait_for_response(&service, &events_path(&dir))
            .expect("wait for response");
        writer.join().expect("join writer");

        assert_eq!(response, Some("continue".to_string()));
        assert!(
            start.elapsed() >= Duration::from_millis(250),
            "timeout_seconds=0 should not return immediately"
        );
    }

    #[test]
    fn timeout_zero_still_honors_shutdown() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 0);
        service
            .send_question("Need approval?")
            .expect("send question");
        service.shutdown_flag().store(true, Ordering::Relaxed);

        let start = Instant::now();
        let response = RobotService::wait_for_response(&service, Path::new("unused-events.jsonl"))
            .expect("wait for response");

        assert_eq!(response, None);
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn wait_for_response_retries_partial_json_until_correlated_response_arrives() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 1);
        service
            .send_question("Need approval?")
            .expect("send question");
        let response_path = response_path(&dir);
        let response = correlated_response(&dir, "response", "ready");
        fs::write(&response_path, r#"{"response":"par"#).expect("write partial response");

        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            fs::write(&response_path, serde_json::to_string(&response).unwrap())
                .expect("write complete response");
        });

        let response = RobotService::wait_for_response(&service, &events_path(&dir))
            .expect("wait for response");
        writer.join().expect("join writer");

        assert_eq!(response, Some("ready".to_string()));
    }

    #[test]
    fn wait_for_response_retries_valid_incomplete_json_until_response_arrives() {
        let dir = TempDir::new().expect("temp dir");
        let service = service(&dir, 1);
        service
            .send_question("Need approval?")
            .expect("send question");
        let response_path = response_path(&dir);
        let question = read_json(&dir.path().join(".ralph/api/robot-question.json"));
        let response = correlated_response(&dir, "response", "complete");
        fs::write(
            &response_path,
            serde_json::to_string(&json!({
                "id": question["id"],
                "loop_id": question["loop_id"],
            }))
            .unwrap(),
        )
        .expect("write incomplete response");

        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            fs::write(&response_path, serde_json::to_string(&response).unwrap())
                .expect("write complete response");
        });

        let response = RobotService::wait_for_response(&service, &events_path(&dir))
            .expect("wait for response");
        writer.join().expect("join writer");

        assert_eq!(response, Some("complete".to_string()));
    }
}
