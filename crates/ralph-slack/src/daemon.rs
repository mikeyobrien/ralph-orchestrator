use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use async_trait::async_trait;

use crate::api::SlackApi;
use crate::error::{SlackError, SlackResult};
use crate::handler::{
    HandlerAction, SlackMessageEvent, ThreadCommand, events_path, handle_message,
};
use crate::renderer::{SlackBlocks, SlackRenderedMessage, redact_secrets};
use crate::state::{SlackStateManager, SlackThreadStatus};

#[derive(Debug, Clone)]
pub struct SlackDaemonConfig {
    pub workspace_root: PathBuf,
    pub allowed_channels: Vec<String>,
    pub allowed_users: Vec<String>,
    pub channel_repos: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartLoopRequest {
    pub loop_id: String,
    pub prompt: String,
    pub channel_id: String,
    pub thread_ts: String,
    pub workspace_root: PathBuf,
    pub env: BTreeMap<String, String>,
}

#[async_trait]
pub trait LoopSpawner: Send + Sync + Clone + 'static {
    async fn spawn_loop(&self, request: StartLoopRequest) -> SlackResult<Option<u32>>;

    async fn stop_loop(&self, process_id: u32) -> SlackResult<()>;
}

#[async_trait]
pub trait ThreadNotifier: Send + Sync + Clone + 'static {
    async fn post_thread_message(
        &self,
        channel_id: &str,
        thread_ts: &str,
        text: &str,
    ) -> SlackResult<String>;

    async fn post_thread_blocks(
        &self,
        channel_id: &str,
        thread_ts: &str,
        message: &SlackRenderedMessage,
    ) -> SlackResult<String> {
        self.post_thread_message(channel_id, thread_ts, &message.text)
            .await
    }
}

#[derive(Debug, Clone)]
pub struct CommandLoopSpawner {
    config_path: Option<PathBuf>,
}

impl CommandLoopSpawner {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        Self { config_path }
    }
}

#[async_trait]
impl LoopSpawner for CommandLoopSpawner {
    async fn spawn_loop(&self, request: StartLoopRequest) -> SlackResult<Option<u32>> {
        let executable = std::env::current_exe().map_err(SlackError::Io)?;
        let worktree_path = ensure_slack_loop_worktree(&request.workspace_root, &request.loop_id)?;
        let log_dir = request.workspace_root.join(".ralph/slack-loop-logs");
        std::fs::create_dir_all(&log_dir).map_err(SlackError::Io)?;
        let log_path = log_dir.join(format!("{}.log", request.loop_id));
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(SlackError::Io)?;
        let stderr = stdout.try_clone().map_err(SlackError::Io)?;
        let mut command = Command::new(executable);
        command
            .current_dir(&worktree_path)
            .arg("run")
            .arg("-a")
            .arg("-p")
            .arg(&request.prompt)
            .envs(&request.env)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        if let Some(config_path) = &self.config_path {
            command.arg("-c").arg(config_path);
        }
        let mut child = command.spawn().map_err(SlackError::Io)?;
        let process_id = child.id();
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(Some(process_id))
    }

    async fn stop_loop(&self, process_id: u32) -> SlackResult<()> {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(process_id.to_string())
            .status()
            .map_err(SlackError::Io)?;
        if !status.success() {
            return Err(SlackError::EventWrite(format!(
                "failed to terminate Slack loop process {process_id}"
            )));
        }
        Ok(())
    }
}

fn ensure_slack_loop_worktree(workspace_root: &Path, loop_id: &str) -> SlackResult<PathBuf> {
    let worktree_path = workspace_root.join(".worktrees").join(loop_id);
    if worktree_path.exists() {
        return Ok(worktree_path);
    }
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(SlackError::Io)?;
    }
    let branch = format!("ralph-slack-{loop_id}");
    let status = Command::new("git")
        .current_dir(workspace_root)
        .args([
            "worktree",
            "add",
            "-B",
            &branch,
            &worktree_path.to_string_lossy(),
            "HEAD",
        ])
        .status()
        .map_err(SlackError::Io)?;
    if !status.success() {
        return Err(SlackError::EventWrite(format!(
            "failed to create Slack loop worktree {}",
            worktree_path.display()
        )));
    }
    Ok(worktree_path)
}

#[derive(Debug, Clone)]
pub struct SlackApiNotifier {
    api: SlackApi,
}

impl SlackApiNotifier {
    pub fn new(api: SlackApi) -> Self {
        Self { api }
    }
}

#[async_trait]
impl ThreadNotifier for SlackApiNotifier {
    async fn post_thread_message(
        &self,
        channel_id: &str,
        thread_ts: &str,
        text: &str,
    ) -> SlackResult<String> {
        self.api
            .post_message(channel_id, Some(thread_ts), text)
            .await
    }

    async fn post_thread_blocks(
        &self,
        channel_id: &str,
        thread_ts: &str,
        message: &SlackRenderedMessage,
    ) -> SlackResult<String> {
        self.api
            .post_blocks(channel_id, Some(thread_ts), message)
            .await
    }
}

#[derive(Debug, Clone)]
pub struct SlackDaemon<S, N> {
    config: SlackDaemonConfig,
    state_manager: SlackStateManager,
    spawner: S,
    notifier: N,
}

impl<S, N> SlackDaemon<S, N>
where
    S: LoopSpawner,
    N: ThreadNotifier,
{
    pub fn new(
        config: SlackDaemonConfig,
        state_manager: SlackStateManager,
        spawner: S,
        notifier: N,
    ) -> Self {
        Self {
            config,
            state_manager,
            spawner,
            notifier,
        }
    }

    pub async fn handle_event(&self, event: SlackMessageEvent) -> SlackResult<HandlerAction> {
        if event.thread_ts.is_none()
            && event.app_mention
            && !self.config.channel_repos.contains_key(&event.channel_id)
        {
            return self.handle_unmapped_start_event(event).await;
        }

        let root_for_start = self
            .config
            .channel_repos
            .get(&event.channel_id)
            .cloned()
            .unwrap_or_else(|| self.config.workspace_root.clone());
        let action = handle_message(
            &self.state_manager,
            &root_for_start,
            &self.config.allowed_channels,
            &self.config.allowed_users,
            event,
        )?;

        match &action {
            HandlerAction::StartLoop {
                loop_id,
                prompt,
                channel_id,
                thread_ts,
            } => {
                let start_message = SlackBlocks::start_card(
                    loop_id,
                    prompt,
                    Some(&root_for_start.display().to_string()),
                    None,
                );
                let start_card_ts = self
                    .notifier
                    .post_thread_blocks(channel_id, thread_ts, &start_message)
                    .await?;
                self.state_manager.set_thread_message_timestamps(
                    loop_id,
                    Some(&start_card_ts),
                    None,
                    None,
                    None,
                )?;
                let process_id = self
                    .spawner
                    .spawn_loop(StartLoopRequest {
                        loop_id: loop_id.clone(),
                        prompt: prompt.clone(),
                        channel_id: channel_id.clone(),
                        thread_ts: thread_ts.clone(),
                        workspace_root: root_for_start.clone(),
                        env: slack_loop_env(loop_id, channel_id, thread_ts, &root_for_start),
                    })
                    .await?;
                self.state_manager
                    .set_thread_process_id(loop_id, process_id)?;
                if let Some(process_id) = process_id {
                    monitor_loop_completion(
                        self.state_manager.clone(),
                        self.notifier.clone(),
                        loop_id.clone(),
                        channel_id.clone(),
                        thread_ts.clone(),
                        root_for_start.clone(),
                        process_id,
                    );
                }
            }
            HandlerAction::Command {
                command,
                loop_id,
                channel_id,
                thread_ts,
                user_id,
            } => {
                self.handle_thread_command(command, loop_id, channel_id, thread_ts, user_id)
                    .await?;
            }
            HandlerAction::Ignored => {}
            HandlerAction::Duplicate | HandlerAction::Appended { .. } => {}
        }
        Ok(action)
    }

    async fn handle_unmapped_start_event(
        &self,
        event: SlackMessageEvent,
    ) -> SlackResult<HandlerAction> {
        if event.bot_id.is_some() {
            return Ok(HandlerAction::Ignored);
        }
        let Some(user_id) = event.user_id.as_deref() else {
            return Ok(HandlerAction::Ignored);
        };
        if self.config.allowed_channels.is_empty()
            || !self
                .config
                .allowed_channels
                .iter()
                .any(|channel| channel == &event.channel_id)
        {
            return Ok(HandlerAction::Ignored);
        }
        if self.config.allowed_users.is_empty()
            || !self.config.allowed_users.iter().any(|user| user == user_id)
        {
            return Ok(HandlerAction::Ignored);
        }
        if let Some(event_id) = event.event_id.as_deref() {
            if !self.state_manager.mark_event_seen(event_id)? {
                return Ok(HandlerAction::Duplicate);
            }
        }

        self.notifier
            .post_thread_message(
                &event.channel_id,
                &event.ts,
                "Ralph is not configured for this Slack channel; ask an operator to add RObot.slack.channel_repos for this channel.",
            )
            .await?;
        Ok(HandlerAction::Ignored)
    }

    async fn handle_thread_command(
        &self,
        command: &ThreadCommand,
        loop_id: &str,
        channel_id: &str,
        thread_ts: &str,
        user_id: &str,
    ) -> SlackResult<()> {
        let state = self.state_manager.load_or_default()?;
        let Some(binding) = state.threads.get(loop_id) else {
            return Ok(());
        };
        match command {
            ThreadCommand::Help => {
                self.notifier
                    .post_thread_blocks(channel_id, thread_ts, &SlackBlocks::help_card())
                    .await?;
            }
            ThreadCommand::Status => {
                let message = SlackBlocks::status_card(
                    &binding.loop_id,
                    binding.status.clone(),
                    &binding.workspace_root.display().to_string(),
                    state.pending_questions.contains_key(loop_id),
                    binding.process_id,
                );
                self.notifier
                    .post_thread_blocks(channel_id, thread_ts, &message)
                    .await?;
            }
            ThreadCommand::Tail { n } => {
                self.notifier
                    .post_thread_message(
                        channel_id,
                        thread_ts,
                        &tail_text(&events_path(&binding.workspace_root, loop_id), *n),
                    )
                    .await?;
            }
            ThreadCommand::Stop => {
                if user_id != binding.created_by {
                    self.notifier
                        .post_thread_message(
                            channel_id,
                            thread_ts,
                            "Only the Slack user who started this Ralph thread can stop it.",
                        )
                        .await?;
                    return Ok(());
                }
                if let Some(process_id) = binding.process_id {
                    self.spawner.stop_loop(process_id).await?;
                }
                self.state_manager
                    .set_thread_status(loop_id, SlackThreadStatus::Stopped)?;
                self.notifier
                    .post_thread_message(
                        channel_id,
                        thread_ts,
                        &format!("stopped Ralph loop {loop_id}. Further guidance in this thread is ignored."),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

fn monitor_loop_completion<N>(
    state_manager: SlackStateManager,
    notifier: N,
    loop_id: String,
    channel_id: String,
    thread_ts: String,
    workspace_root: PathBuf,
    process_id: u32,
) where
    N: ThreadNotifier,
{
    tokio::spawn(async move {
        let mut last_reported_line_count = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let event_path = events_path(&workspace_root, &loop_id);
            if let Ok(contents) = std::fs::read_to_string(&event_path) {
                if let Some(update) = latest_progress_update(&contents, last_reported_line_count) {
                    last_reported_line_count = update.line_count;
                    let message = SlackBlocks::progress_card(
                        &loop_id,
                        update.iteration,
                        update.hat.as_deref(),
                        &update.topic,
                        &update.payload,
                        None,
                    );
                    if let Err(error) = notifier
                        .post_thread_blocks(&channel_id, &thread_ts, &message)
                        .await
                    {
                        tracing::warn!(%loop_id, ?error, "failed to post Slack loop progress update");
                    }
                }
            }
            if !process_is_alive(process_id) {
                break;
            }
        }

        let event_path = events_path(&workspace_root, &loop_id);
        let contents = std::fs::read_to_string(&event_path).unwrap_or_default();
        let completed = contents.contains("\"topic\":\"LOOP_COMPLETE\"")
            || contents.contains("## Reason\\ncompleted")
            || contents.contains("\"payload\":\"## Reason\\ncompleted");
        let status = if completed {
            SlackThreadStatus::Completed
        } else {
            SlackThreadStatus::Failed
        };
        if let Err(error) = state_manager.finish_thread(&loop_id, status.clone()) {
            tracing::warn!(%loop_id, ?error, "failed to mark Slack loop finished");
        }
        let note = "Try `tail 10` in this thread for recent events.";
        let message = SlackBlocks::final_card(&loop_id, status, None, Some(note));
        match notifier
            .post_thread_blocks(&channel_id, &thread_ts, &message)
            .await
        {
            Ok(final_card_ts) => {
                if let Err(error) = state_manager.set_thread_message_timestamps(
                    &loop_id,
                    None,
                    None,
                    None,
                    Some(&final_card_ts),
                ) {
                    tracing::warn!(%loop_id, ?error, "failed to save Slack final card timestamp");
                }
            }
            Err(error) => {
                tracing::warn!(%loop_id, ?error, "failed to post Slack loop completion update");
            }
        }
    });
}

fn process_is_alive(process_id: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(process_id.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlackProgressUpdate {
    line_count: usize,
    iteration: Option<u64>,
    hat: Option<String>,
    topic: String,
    payload: String,
}

fn latest_progress_update(
    contents: &str,
    last_reported_line_count: usize,
) -> Option<SlackProgressUpdate> {
    let mut latest = None;
    for (idx, line) in contents.lines().enumerate() {
        let line_count = idx + 1;
        if line_count <= last_reported_line_count || line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let topic = value
            .get("topic")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();
        let payload = event_payload_text(value.get("payload"));
        latest = Some(SlackProgressUpdate {
            line_count,
            iteration: value.get("iteration").and_then(|value| value.as_u64()),
            hat: value
                .get("hat")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            topic,
            payload,
        });
    }
    latest
}

fn event_payload_text(payload: Option<&serde_json::Value>) -> String {
    match payload {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
fn format_progress_update(loop_id: &str, update: &SlackProgressUpdate) -> String {
    let iteration = update
        .iteration
        .map(|iteration| iteration.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let hat = update.hat.as_deref().unwrap_or("agent");
    let mut payload = redact_secrets(&update.payload);
    if payload.len() > 1000 {
        payload.truncate(1000);
        payload.push_str("…");
    }
    format!(
        "Ralph update\nLoop: {loop_id}\nIteration: {iteration}\nHat: {hat}\nTopic: {}\nLast message:\n```\n{}\n```",
        update.topic, payload
    )
}

fn tail_text(path: &Path, n: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let lines: Vec<_> = contents.lines().rev().take(n).collect();
            if lines.is_empty() {
                return "No events yet.".to_string();
            }
            let mut redacted = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
            redacted = redact_secrets(&redacted);
            if redacted.len() > 3000 {
                redacted.truncate(3000);
                redacted.push_str("…");
            }
            format!("Latest Ralph events:\n```\n{}\n```", redacted)
        }
        Err(_) => "No event file found for this loop yet.".to_string(),
    }
}

pub fn slack_loop_env(
    loop_id: &str,
    channel_id: &str,
    thread_ts: &str,
    _workspace_root: &Path,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("RALPH_LOOP_ID".to_string(), loop_id.to_string()),
        ("RALPH_SLACK_CHANNEL_ID".to_string(), channel_id.to_string()),
        ("RALPH_SLACK_THREAD_TS".to_string(), thread_ts.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_update_uses_iteration_hat_topic_and_last_message() {
        let contents = concat!(
            r#"{"ts":"2026-01-01T00:00:00Z","iteration":1,"hat":"planner","topic":"work.start","payload":"first"}"#,
            "\n",
            r#"{"ts":"2026-01-01T00:00:01Z","iteration":2,"hat":"executor","topic":"work.done","payload":"created token-secret-token-abc file"}"#,
            "\n"
        );

        let update = latest_progress_update(contents, 1).expect("latest update");
        assert_eq!(update.line_count, 2);
        assert_eq!(update.iteration, Some(2));
        assert_eq!(update.hat.as_deref(), Some("executor"));
        assert_eq!(update.topic, "work.done");
        assert_eq!(update.payload, "created token-secret-token-abc file");

        let text = format_progress_update("loop-1", &update);
        assert!(text.contains("Loop: loop-1"));
        assert!(text.contains("Iteration: 2"));
        assert!(text.contains("Hat: executor"));
        assert!(text.contains("Topic: work.done"));
        assert!(text.contains("created [redacted] file"));
    }

    #[test]
    fn progress_update_handles_agent_written_events_without_hat() {
        let contents =
            r#"{"topic":"human.guidance","payload":{"note":"steer"},"ts":"2026-01-01T00:00:00Z"}"#;
        let update = latest_progress_update(contents, 0).expect("latest update");
        assert_eq!(update.iteration, None);
        assert_eq!(update.hat, None);
        assert_eq!(update.topic, "human.guidance");
        assert_eq!(update.payload, r#"{"note":"steer"}"#);
        assert!(format_progress_update("loop-2", &update).contains("Hat: agent"));
    }

    #[test]
    fn slack_loop_env_does_not_force_workspace_root() {
        let env = slack_loop_env("loop-1", "C123", "1780.1", Path::new("/repo"));
        assert_eq!(env.get("RALPH_LOOP_ID").unwrap(), "loop-1");
        assert_eq!(env.get("RALPH_SLACK_CHANNEL_ID").unwrap(), "C123");
        assert_eq!(env.get("RALPH_SLACK_THREAD_TS").unwrap(), "1780.1");
        assert!(!env.contains_key("RALPH_WORKSPACE_ROOT"));
    }

    #[test]
    fn ensure_slack_loop_worktree_creates_and_reuses_worktree() {
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        std::fs::write(repo.path().join("README.md"), "smoke").unwrap();
        std::process::Command::new("git")
            .args(["add", "README.md"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo.path())
            .status()
            .unwrap();

        let path = ensure_slack_loop_worktree(repo.path(), "slack-C123-1780-1").unwrap();
        assert!(path.join("README.md").exists());
        assert_eq!(
            ensure_slack_loop_worktree(repo.path(), "slack-C123-1780-1").unwrap(),
            path
        );
    }
}
