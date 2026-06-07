use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;

use crate::api::SlackApi;
use crate::error::{SlackError, SlackResult};
use crate::handler::{
    HandlerAction, SlackMessageEvent, ThreadCommand, events_path, handle_message,
};
use crate::state::{SlackStateManager, SlackThreadBinding, SlackThreadStatus};

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
        let mut command = Command::new(executable);
        command
            .current_dir(&request.workspace_root)
            .arg("run")
            .arg("--no-tui")
            .arg("--loop-id")
            .arg(&request.loop_id)
            .arg("-p")
            .arg(&request.prompt)
            .envs(&request.env);
        if let Some(config_path) = &self.config_path {
            command.arg("-c").arg(config_path);
        }
        let child = command.spawn().map_err(SlackError::Io)?;
        Ok(Some(child.id()))
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
                self.notifier
                    .post_thread_message(
                        channel_id,
                        thread_ts,
                        &format!("🤖 Ralph loop started\nLoop: {}\nStatus: running", loop_id),
                    )
                    .await?;
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
                    .post_thread_message(channel_id, thread_ts, help_text())
                    .await?;
            }
            ThreadCommand::Status => {
                self.notifier
                    .post_thread_message(
                        channel_id,
                        thread_ts,
                        &status_text(binding, state.pending_questions.contains_key(loop_id)),
                    )
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

fn help_text() -> &'static str {
    "Ralph Slack commands: help, status, tail [n], stop/cancel. Plain replies become guidance, or answer the pending human question."
}

fn status_text(binding: &SlackThreadBinding, pending_question: bool) -> String {
    format!(
        "Ralph thread status\nloop: {}\nrepo: {}\nthread status: {:?}\npending question: {}\nprocess id: {}",
        binding.loop_id,
        binding.workspace_root.display(),
        binding.status,
        if pending_question { "yes" } else { "no" },
        binding
            .process_id
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "unknown".to_string())
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

fn redact_secrets(text: &str) -> String {
    let mut out = text.to_string();
    for marker in ["secret-token-", "token-", "xoxb-", "xapp-"] {
        while let Some(start) = out.to_ascii_lowercase().find(marker) {
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map(|offset| start + offset)
                .unwrap_or(out.len());
            out.replace_range(start..end, "[redacted]");
        }
    }
    out
}

pub fn slack_loop_env(
    loop_id: &str,
    channel_id: &str,
    thread_ts: &str,
    workspace_root: &Path,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("RALPH_LOOP_ID".to_string(), loop_id.to_string()),
        ("RALPH_SLACK_CHANNEL_ID".to_string(), channel_id.to_string()),
        ("RALPH_SLACK_THREAD_TS".to_string(), thread_ts.to_string()),
        (
            "RALPH_WORKSPACE_ROOT".to_string(),
            workspace_root.to_string_lossy().to_string(),
        ),
    ])
}
