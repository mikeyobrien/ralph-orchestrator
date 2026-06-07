use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::api::SlackApi;
use crate::error::{SlackError, SlackResult};
use crate::handler::validate_loop_id;
use crate::state::SlackStateManager;

#[derive(Debug, Clone)]
pub struct SlackService {
    workspace_root: PathBuf,
    timeout_secs: u64,
    loop_id: String,
    channel_id: String,
    thread_ts: String,
    api: SlackApi,
    state_manager: SlackStateManager,
    shutdown: Arc<AtomicBool>,
}

impl SlackService {
    pub fn new(
        workspace_root: PathBuf,
        bot_token: Option<String>,
        timeout_secs: u64,
        loop_id: String,
        channel_id: String,
        thread_ts: String,
        api_base_url: Option<String>,
    ) -> SlackResult<Self> {
        Self::new_with_state_path(
            workspace_root,
            bot_token,
            timeout_secs,
            loop_id,
            channel_id,
            thread_ts,
            api_base_url,
            None,
        )
    }

    pub fn new_with_state_path(
        workspace_root: PathBuf,
        bot_token: Option<String>,
        timeout_secs: u64,
        loop_id: String,
        channel_id: String,
        thread_ts: String,
        api_base_url: Option<String>,
        state_path: Option<PathBuf>,
    ) -> SlackResult<Self> {
        validate_loop_id(&loop_id)?;
        let resolved_token = bot_token
            .or_else(|| std::env::var("RALPH_SLACK_BOT_TOKEN").ok())
            .ok_or(SlackError::MissingBotToken)?;
        let state_path =
            state_path.unwrap_or_else(|| workspace_root.join(".ralph/slack-state.json"));
        Ok(Self {
            workspace_root,
            timeout_secs,
            loop_id,
            channel_id,
            thread_ts,
            api: SlackApi::new(resolved_token, api_base_url),
            state_manager: SlackStateManager::new(state_path),
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn state_manager(&self) -> &SlackStateManager {
        &self.state_manager
    }

    pub fn send_question(&self, payload: &str) -> SlackResult<i32> {
        let ts = self.post_thread_message(payload)?;
        self.state_manager.add_pending_question(
            &self.loop_id,
            &self.channel_id,
            &self.thread_ts,
            &ts,
        )?;
        Ok(1)
    }

    pub fn send_checkin(
        &self,
        iteration: u32,
        elapsed: Duration,
        context: Option<&ralph_proto::CheckinContext>,
    ) -> SlackResult<i32> {
        let mut message = format!(
            "Ralph check-in: iteration {} after {}s",
            iteration,
            elapsed.as_secs()
        );
        if let Some(context) = context {
            if let Some(hat) = &context.current_hat {
                message.push_str(&format!("; hat {}", hat));
            }
            message.push_str(&format!(
                "; tasks {}/{}; cost ${:.4}",
                context.closed_tasks, context.open_tasks, context.cumulative_cost
            ));
        }
        self.post_thread_message(&message)?;
        Ok(1)
    }

    pub fn send_file(&self, file_path: &Path, caption: Option<&str>) -> SlackResult<i32> {
        let (canonical_path, filename, length) = self.validate_upload_path(file_path)?;
        let handle = tokio::runtime::Handle::try_current().map_err(|_| {
            SlackError::Api("no tokio runtime available for Slack API call".to_string())
        })?;
        tokio::task::block_in_place(|| {
            handle.block_on(self.api.upload_file_external(
                &self.channel_id,
                &self.thread_ts,
                &canonical_path,
                &filename,
                length,
                caption,
            ))
        })?;
        Ok(1)
    }

    pub fn wait_for_response(&self, events_path: &Path) -> SlackResult<Option<String>> {
        let deadline = Instant::now() + Duration::from_secs(self.timeout_secs);
        let mut file_pos = if events_path.exists() {
            std::fs::metadata(events_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0)
        } else {
            0
        };

        loop {
            if self.shutdown.load(Ordering::Relaxed) || Instant::now() >= deadline {
                let _ = self.state_manager.remove_pending_question(&self.loop_id);
                return Ok(None);
            }
            if let Some(response) = check_for_response(events_path, &mut file_pos)? {
                let _ = self.state_manager.remove_pending_question(&self.loop_id);
                return Ok(Some(response));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    pub fn stop(self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    fn validate_upload_path(&self, file_path: &Path) -> SlackResult<(PathBuf, String, u64)> {
        let canonical_root = self.workspace_root.canonicalize()?;
        let canonical_path = file_path.canonicalize()?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(SlackError::FilePath(format!(
                "{} is outside workspace {}",
                canonical_path.display(),
                canonical_root.display()
            )));
        }
        let metadata = std::fs::metadata(&canonical_path)?;
        if !metadata.is_file() {
            return Err(SlackError::FilePath(format!(
                "{} is not a regular file",
                canonical_path.display()
            )));
        }
        let filename = canonical_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| SlackError::FilePath("file name is not valid UTF-8".to_string()))?
            .to_string();
        Ok((canonical_path, filename, metadata.len()))
    }

    fn post_thread_message(&self, text: &str) -> SlackResult<String> {
        let handle = tokio::runtime::Handle::try_current().map_err(|_| {
            SlackError::Api("no tokio runtime available for Slack API call".to_string())
        })?;
        tokio::task::block_in_place(|| {
            handle.block_on(
                self.api
                    .post_message(&self.channel_id, Some(&self.thread_ts), text),
            )
        })
    }
}

impl ralph_proto::RobotService for SlackService {
    fn send_question(&self, payload: &str) -> anyhow::Result<i32> {
        Ok(SlackService::send_question(self, payload)?)
    }

    fn wait_for_response(&self, events_path: &Path) -> anyhow::Result<Option<String>> {
        Ok(SlackService::wait_for_response(self, events_path)?)
    }

    fn send_checkin(
        &self,
        iteration: u32,
        elapsed: Duration,
        context: Option<&ralph_proto::CheckinContext>,
    ) -> anyhow::Result<i32> {
        Ok(SlackService::send_checkin(
            self, iteration, elapsed, context,
        )?)
    }

    fn send_file(&self, file_path: &Path, caption: Option<&str>) -> anyhow::Result<i32> {
        Ok(SlackService::send_file(self, file_path, caption)?)
    }

    fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    fn shutdown_flag(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    fn stop(self: Box<Self>) {
        SlackService::stop(*self);
    }
}

fn check_for_response(events_path: &Path, file_pos: &mut u64) -> SlackResult<Option<String>> {
    use std::io::{BufRead, BufReader, Seek, SeekFrom};

    if !events_path.exists() {
        return Ok(None);
    }
    let mut file = std::fs::File::open(events_path)?;
    file.seek(SeekFrom::Start(*file_pos))?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        *file_pos += line.len() as u64 + 1;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
            if event.get("topic").and_then(|topic| topic.as_str()) == Some("human.response") {
                return Ok(Some(
                    event
                        .get("payload")
                        .and_then(|payload| payload.as_str())
                        .unwrap_or_default()
                        .to_string(),
                ));
            }
        }
    }
    Ok(None)
}
