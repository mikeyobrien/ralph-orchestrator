use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use async_trait::async_trait;

use crate::api::SlackApi;
use crate::error::{SlackError, SlackResult};
use crate::handler::{
    HandlerAction, SlackMessageEvent, ThreadCommand, events_path, handle_message_with_repo,
};
use crate::renderer::{SlackBlocks, SlackRenderedMessage, redact_secrets};
use crate::state::{SlackStateManager, SlackThreadBinding, SlackThreadStatus};

#[derive(Debug, Clone)]
pub struct SlackDaemonConfig {
    pub workspace_root: PathBuf,
    pub allowed_channels: Vec<String>,
    pub allowed_users: Vec<String>,
    pub repo_aliases: BTreeMap<String, PathBuf>,
    pub channel_repos: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartLoopRequest {
    pub loop_id: String,
    pub prompt: String,
    pub channel_id: String,
    pub thread_ts: String,
    pub workspace_root: PathBuf,
    pub state_path: PathBuf,
    pub repo_alias: Option<String>,
    pub repo_dir: Option<PathBuf>,
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

    async fn update_thread_blocks(
        &self,
        channel_id: &str,
        message_ts: &str,
        message: &SlackRenderedMessage,
    ) -> SlackResult<()>;
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
        let workdir = request
            .repo_dir
            .as_deref()
            .map(|repo_dir| worktree_path.join(repo_dir))
            .unwrap_or_else(|| worktree_path.clone());
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
        configure_slack_loop_command(
            &mut command,
            &workdir,
            &request,
            self.config_path.as_deref(),
        );
        command
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
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

fn configure_slack_loop_command(
    command: &mut Command,
    worktree_path: &Path,
    request: &StartLoopRequest,
    config_path: Option<&Path>,
) {
    command
        .current_dir(worktree_path)
        .arg("run")
        .arg("--autonomous")
        .arg("-p")
        .arg(&request.prompt)
        .envs(&request.env);
    if let Some(config_path) = config_path {
        command.arg("-c").arg(config_path);
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

    async fn update_thread_blocks(
        &self,
        channel_id: &str,
        message_ts: &str,
        message: &SlackRenderedMessage,
    ) -> SlackResult<()> {
        self.api
            .update_blocks(channel_id, message_ts, message)
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

    pub async fn reconcile_stale_threads(&self) -> SlackResult<usize> {
        let state = self.state_manager.load_or_default()?;
        let stale_bindings = state
            .threads
            .values()
            .filter(|binding| binding.status == SlackThreadStatus::Running)
            .filter(|binding| {
                binding
                    .process_id
                    .map(|process_id| !process_is_alive(process_id))
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();
        drop(state);

        for binding in &stale_bindings {
            finish_stale_thread(&self.state_manager, self.notifier.clone(), binding).await?;
        }

        Ok(stale_bindings.len())
    }

    pub async fn handle_event(&self, event: SlackMessageEvent) -> SlackResult<HandlerAction> {
        let mut event = event;
        let target_for_start = if event.thread_ts.is_none() && event.app_mention {
            match self.resolve_start_event(&mut event).await? {
                Some(target) => target,
                None => return Ok(HandlerAction::Ignored),
            }
        } else {
            RepoTarget::default_root(self.config.workspace_root.clone())
        };

        let action = handle_message_with_repo(
            &self.state_manager,
            &target_for_start.root,
            target_for_start.alias.as_deref(),
            target_for_start.dir.as_deref(),
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
                let target_for_action = self
                    .state_manager
                    .load_or_default()?
                    .threads
                    .get(loop_id)
                    .map(|binding| RepoTarget {
                        alias: binding.repo_alias.clone(),
                        root: binding.workspace_root.clone(),
                        dir: binding.repo_dir.clone(),
                    })
                    .unwrap_or_else(|| target_for_start.clone());
                let start_message = SlackBlocks::start_card(
                    loop_id,
                    prompt,
                    Some(&target_for_action.summary()),
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
                        workspace_root: target_for_action.root.clone(),
                        state_path: self.state_manager.path().to_path_buf(),
                        repo_alias: target_for_action.alias.clone(),
                        repo_dir: target_for_action.dir.clone(),
                        env: slack_loop_env(
                            loop_id,
                            channel_id,
                            thread_ts,
                            self.state_manager.path(),
                            target_for_action.alias.as_deref(),
                            target_for_action.dir.as_deref(),
                        ),
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
                        target_for_action.root.clone(),
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

    async fn resolve_start_event(
        &self,
        event: &mut SlackMessageEvent,
    ) -> SlackResult<Option<RepoTarget>> {
        if event.bot_id.is_some() {
            return Ok(None);
        }
        let Some(user_id) = event.user_id.as_deref() else {
            return Ok(None);
        };
        if self.config.allowed_channels.is_empty()
            || !self
                .config
                .allowed_channels
                .iter()
                .any(|channel| channel == &event.channel_id)
        {
            return Ok(None);
        }
        if self.config.allowed_users.is_empty()
            || !self.config.allowed_users.iter().any(|user| user == user_id)
        {
            return Ok(None);
        }

        let prompt = strip_app_mention_for_start(&event.text);
        let parsed = match parse_start_repo_directives(&prompt) {
            Ok(parsed) => parsed,
            Err(message) => {
                if !self.mark_start_event_seen(event)? {
                    return Ok(None);
                }
                self.notifier
                    .post_thread_message(&event.channel_id, &event.ts, &message)
                    .await?;
                return Ok(None);
            }
        };
        let target = match resolve_repo_target(
            &self.config.repo_aliases,
            self.config
                .channel_repos
                .get(&event.channel_id)
                .map(String::as_str),
            parsed.repo_alias.as_deref(),
            parsed.dir.as_deref(),
        ) {
            Ok(Some(target)) => target,
            Ok(None) => {
                if !self.mark_start_event_seen(event)? {
                    return Ok(None);
                }
                self.post_repo_clarification(event).await?;
                return Ok(None);
            }
            Err(message) => {
                if !self.mark_start_event_seen(event)? {
                    return Ok(None);
                }
                self.notifier
                    .post_thread_message(&event.channel_id, &event.ts, &message)
                    .await?;
                return Ok(None);
            }
        };
        event.text = rewrite_start_text_with_prompt(&event.text, &parsed.prompt);
        Ok(Some(target))
    }

    fn mark_start_event_seen(&self, event: &SlackMessageEvent) -> SlackResult<bool> {
        match event.event_id.as_deref() {
            Some(event_id) => self.state_manager.mark_event_seen(event_id),
            None => Ok(true),
        }
    }

    async fn post_repo_clarification(&self, event: &SlackMessageEvent) -> SlackResult<()> {
        let aliases = configured_alias_list(&self.config.repo_aliases);
        let message = if aliases.is_empty() {
            "human.interact: Which repo should Ralph use? No safe repo aliases are configured; ask an operator to set RObot.slack.repo_aliases and channel_repos.".to_string()
        } else {
            format!(
                "human.interact: Which repo should Ralph use? Reply with a new mention like `@Ralph repo=<alias> ...`. Configured aliases: {aliases}"
            )
        };
        self.notifier
            .post_thread_message(&event.channel_id, &event.ts, &message)
            .await?;
        Ok(())
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
            ThreadCommand::Repo => {
                self.notifier
                    .post_thread_message(channel_id, thread_ts, &repo_command_text(binding))
                    .await?;
            }
            ThreadCommand::Obs => {
                self.notifier
                    .post_thread_message(
                        channel_id,
                        thread_ts,
                        &obs_text(binding, state.pending_questions.contains_key(loop_id)),
                    )
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
            ThreadCommand::Log { n } => {
                self.notifier
                    .post_thread_message(
                        channel_id,
                        thread_ts,
                        &plain_tail_text(
                            &slack_loop_log_path(&binding.workspace_root, loop_id),
                            *n,
                        ),
                    )
                    .await?;
            }
            ThreadCommand::Handoff => {
                self.notifier
                    .post_thread_message(channel_id, thread_ts, &handoff_text(binding))
                    .await?;
            }
            ThreadCommand::Artifacts => {
                self.notifier
                    .post_thread_message(channel_id, thread_ts, &artifacts_text(binding))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoTarget {
    pub alias: Option<String>,
    pub root: PathBuf,
    pub dir: Option<PathBuf>,
}

impl RepoTarget {
    fn default_root(root: PathBuf) -> Self {
        Self {
            alias: None,
            root,
            dir: None,
        }
    }

    fn summary(&self) -> String {
        match (&self.alias, &self.dir) {
            (Some(alias), Some(dir)) => {
                format!(
                    "{alias}:{} ({})",
                    dir.display(),
                    self.root.join(dir).display()
                )
            }
            (Some(alias), None) => format!("{alias} ({})", self.root.display()),
            (None, Some(dir)) => format!("{} (dir {})", self.root.display(), dir.display()),
            (None, None) => self.root.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStartRepoDirectives {
    pub repo_alias: Option<String>,
    pub dir: Option<PathBuf>,
    pub prompt: String,
}

pub fn parse_start_repo_directives(prompt: &str) -> Result<ParsedStartRepoDirectives, String> {
    let mut remaining = prompt.trim();
    let mut repo_alias = None;
    let mut dir = None;

    if let Some(after_in) = remaining.strip_prefix("in ") {
        let Some((candidate, after_colon)) = after_in.split_once(':') else {
            return Err("Slack repo selector `in <alias>:` must include `:`.".to_string());
        };
        let candidate = candidate.trim();
        if is_valid_repo_alias(candidate) {
            repo_alias = Some(candidate.to_string());
            remaining = after_colon.trim_start();
        } else {
            return Err(invalid_repo_alias_message(candidate));
        }
    }

    let mut prompt_parts = Vec::new();
    let mut parsing_directives = true;
    for part in remaining.split_whitespace() {
        if parsing_directives {
            if let Some(value) = part.strip_prefix("repo=") {
                if is_valid_repo_alias(value) {
                    repo_alias = Some(value.to_string());
                    continue;
                }
                return Err(invalid_repo_alias_message(value));
            }
            if let Some(value) = part.strip_prefix("dir=") {
                if !value.is_empty() {
                    dir = Some(PathBuf::from(value));
                    continue;
                }
                return Err("Slack repo dir cannot be empty.".to_string());
            }
            parsing_directives = false;
        }
        prompt_parts.push(part);
    }

    Ok(ParsedStartRepoDirectives {
        repo_alias,
        dir,
        prompt: prompt_parts.join(" "),
    })
}

pub fn resolve_repo_target(
    repo_aliases: &BTreeMap<String, PathBuf>,
    channel_default_alias: Option<&str>,
    explicit_alias: Option<&str>,
    dir: Option<&Path>,
) -> Result<Option<RepoTarget>, String> {
    let alias = explicit_alias.or(channel_default_alias);
    let Some(alias) = alias else {
        return Ok(None);
    };
    let Some(root) = repo_aliases.get(alias) else {
        return Err(format!(
            "Unknown repo alias `{alias}`. Configured aliases: {}",
            configured_alias_list(repo_aliases)
        ));
    };
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Repo alias `{alias}` is not usable: {error}"))?;
    let safe_dir = match dir {
        Some(dir) => Some(validate_repo_subdir(&canonical_root, dir)?),
        None => None,
    };
    Ok(Some(RepoTarget {
        alias: Some(alias.to_string()),
        root: canonical_root,
        dir: safe_dir,
    }))
}

pub fn validate_repo_subdir(repo_root: &Path, dir: &Path) -> Result<PathBuf, String> {
    if dir.as_os_str().is_empty() || dir == Path::new(".") {
        return Ok(PathBuf::new());
    }
    if dir.is_absolute() {
        return Err("Slack repo dir must be relative to the repo root.".to_string());
    }
    if dir.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("Slack repo dir cannot contain `..` or absolute path components.".to_string());
    }
    let full = repo_root.join(dir);
    let canonical = full
        .canonicalize()
        .map_err(|error| format!("Slack repo dir `{}` is not usable: {error}", dir.display()))?;
    if !canonical.starts_with(repo_root) {
        return Err("Slack repo dir must stay inside the configured repo root.".to_string());
    }
    canonical
        .strip_prefix(repo_root)
        .map(Path::to_path_buf)
        .map_err(|_| "Slack repo dir must stay inside the configured repo root.".to_string())
}

fn repo_command_text(binding: &SlackThreadBinding) -> String {
    let alias = binding.repo_alias.as_deref().unwrap_or("unbound");
    let dir = binding
        .repo_dir
        .as_deref()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.display().to_string())
        .unwrap_or_else(|| ".".to_string());
    let worktree = binding
        .workspace_root
        .join(".worktrees")
        .join(&binding.loop_id);
    let branch = format!("ralph-slack-{}", binding.loop_id);
    format!(
        "Repo\nalias: `{alias}`\nroot: `{}`\ndir: `{dir}`\nloop: `{}`\nthread status: {:?}\nparent loop: `{}`\nworktree: `{}`\nbranch: `{branch}`",
        binding.workspace_root.display(),
        binding.loop_id,
        binding.status,
        binding.parent_loop_id.as_deref().unwrap_or("none"),
        worktree.display()
    )
}

fn obs_text(binding: &SlackThreadBinding, pending_question: bool) -> String {
    let alias = binding.repo_alias.as_deref().unwrap_or("unbound");
    let dir = binding
        .repo_dir
        .as_deref()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.display().to_string())
        .unwrap_or_else(|| ".".to_string());
    let process_id = binding
        .process_id
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "none".to_string());
    let pending = if pending_question { "yes" } else { "no" };
    let worktree = binding
        .workspace_root
        .join(".worktrees")
        .join(&binding.loop_id);
    let branch = format!("ralph-slack-{}", binding.loop_id);
    let event_path = events_path(&binding.workspace_root, &binding.loop_id);
    let log_path = binding
        .log_path
        .clone()
        .unwrap_or_else(|| slack_loop_log_path(&binding.workspace_root, &binding.loop_id));
    let cards = format!(
        "start=`{}` progress=`{}` stream=`{}` final=`{}`",
        binding.start_card_ts.as_deref().unwrap_or("none"),
        binding.progress_message_ts.as_deref().unwrap_or("none"),
        binding.stream_ts.as_deref().unwrap_or("none"),
        binding.final_card_ts.as_deref().unwrap_or("none")
    );
    format!(
        "Ralph observable\nloop: `{}`\nstatus: `{}`\npending question: `{pending}`\nprocess id: `{process_id}`\nrepo alias: `{alias}`\nrepo root: `{}`\nrepo dir: `{dir}`\nworktree: `{}`\nbranch: `{branch}`\ncards: {cards}\n{}\n{}",
        binding.loop_id,
        status_observable_label(&binding.status),
        binding.workspace_root.display(),
        worktree.display(),
        latest_event_observation(&event_path),
        latest_log_observation(&log_path)
    )
}

fn latest_event_observation(event_path: &Path) -> String {
    match std::fs::read_to_string(event_path) {
        Ok(contents) => {
            let line_count = contents
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
            let Some(update) = latest_progress_update(&contents, 0) else {
                return format!(
                    "events: `{}` ({line_count} line(s))\nlatest event: none parseable",
                    event_path.display()
                );
            };
            let iteration = update
                .iteration
                .map(|iteration| iteration.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let hat = update.hat.as_deref().unwrap_or("agent");
            let topic = update.topic;
            let payload =
                collapse_tail_whitespace(&truncate_tail_payload(&redact_secrets(&update.payload)));
            if payload.is_empty() {
                format!(
                    "events: `{}` ({line_count} line(s))\nlatest event: iter `{iteration}` · hat `{hat}` · topic `{topic}`",
                    event_path.display()
                )
            } else {
                format!(
                    "events: `{}` ({line_count} line(s))\nlatest event: iter `{iteration}` · hat `{hat}` · topic `{topic}`\nlatest message: {payload}",
                    event_path.display()
                )
            }
        }
        Err(error) => format!(
            "events: `{}` (missing: {error})\nlatest event: none",
            event_path.display()
        ),
    }
}

fn latest_log_observation(log_path: &Path) -> String {
    match std::fs::read_to_string(log_path) {
        Ok(contents) => {
            let line_count = contents
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
            let latest = contents
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(|line| collapse_tail_whitespace(&truncate_tail_payload(&redact_secrets(line))))
                .unwrap_or_else(|| "none".to_string());
            format!(
                "log: `{}` ({line_count} line(s))\nlatest log: {latest}",
                log_path.display()
            )
        }
        Err(error) => format!(
            "log: `{}` (missing: {error})\nlatest log: none",
            log_path.display()
        ),
    }
}

fn status_observable_label(status: &SlackThreadStatus) -> &'static str {
    match status {
        SlackThreadStatus::Running => "running",
        SlackThreadStatus::Completed => "completed",
        SlackThreadStatus::Failed => "failed",
        SlackThreadStatus::Stopped => "stopped",
    }
}

fn configured_alias_list(repo_aliases: &BTreeMap<String, PathBuf>) -> String {
    if repo_aliases.is_empty() {
        return "none".to_string();
    }
    repo_aliases
        .keys()
        .map(|alias| format!("`{alias}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_valid_repo_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn invalid_repo_alias_message(alias: &str) -> String {
    if alias.is_empty() {
        return "Slack repo alias cannot be empty.".to_string();
    }
    format!(
        "Slack repo alias `{alias}` is invalid. Use only letters, numbers, hyphen, or underscore."
    )
}

fn strip_app_mention_for_start(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("<@") {
        if let Some((_, prompt)) = rest.split_once('>') {
            return prompt.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn rewrite_start_text_with_prompt(original: &str, prompt: &str) -> String {
    let trimmed = original.trim();
    if let Some(rest) = trimmed.strip_prefix("<@") {
        if let Some((mention, _)) = rest.split_once('>') {
            return format!("<@{mention}> {}", prompt.trim()).trim().to_string();
        }
    }
    prompt.trim().to_string()
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
        let started_at = std::time::Instant::now();
        let mut checkpoint = ProgressCheckpoint::default();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let event_path = events_path(&workspace_root, &loop_id);
            if let Ok(contents) = std::fs::read_to_string(&event_path) {
                if let Err(error) = sync_loop_progress_once(
                    &state_manager,
                    notifier.clone(),
                    &loop_id,
                    &channel_id,
                    &thread_ts,
                    &contents,
                    started_at.elapsed(),
                    &mut checkpoint,
                )
                .await
                {
                    tracing::warn!(%loop_id, ?error, "failed to sync Slack loop progress update");
                }
            }
            if !process_is_alive(process_id) {
                break;
            }
        }

        let status = derive_final_status(&workspace_root, &loop_id);
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

async fn finish_stale_thread<N>(
    state_manager: &SlackStateManager,
    notifier: N,
    binding: &SlackThreadBinding,
) -> SlackResult<()>
where
    N: ThreadNotifier,
{
    let status = derive_final_status(&binding.workspace_root, &binding.loop_id);
    state_manager.finish_thread(&binding.loop_id, status.clone())?;

    let note = "Try `tail 10` in this thread for recent events.";
    let message = SlackBlocks::final_card(&binding.loop_id, status, None, Some(note));
    if let Some(final_card_ts) = binding.final_card_ts.as_deref() {
        if let Err(error) = notifier
            .update_thread_blocks(&binding.channel_id, final_card_ts, &message)
            .await
        {
            tracing::warn!(loop_id = %binding.loop_id, ?error, "failed to update Slack stale loop final card");
        }
    } else {
        match notifier
            .post_thread_blocks(&binding.channel_id, &binding.thread_ts, &message)
            .await
        {
            Ok(final_card_ts) => {
                state_manager.set_thread_message_timestamps(
                    &binding.loop_id,
                    None,
                    None,
                    None,
                    Some(&final_card_ts),
                )?;
            }
            Err(error) => {
                tracing::warn!(loop_id = %binding.loop_id, ?error, "failed to post Slack stale loop final card");
            }
        }
    }

    Ok(())
}

fn derive_final_status(workspace_root: &Path, loop_id: &str) -> SlackThreadStatus {
    let event_contents =
        std::fs::read_to_string(events_path(workspace_root, loop_id)).unwrap_or_default();
    let log_contents =
        std::fs::read_to_string(slack_loop_log_path(workspace_root, loop_id)).unwrap_or_default();

    if indicates_completed(&event_contents) || indicates_completed(&log_contents) {
        SlackThreadStatus::Completed
    } else {
        SlackThreadStatus::Failed
    }
}

fn indicates_completed(contents: &str) -> bool {
    contents.contains("\"topic\":\"LOOP_COMPLETE\"")
        || contents.contains("LOOP_COMPLETE")
        || contents.contains("## Reason\\ncompleted")
        || contents.contains("## Reason\ncompleted")
        || contents.contains("\"payload\":\"## Reason\\ncompleted")
}

fn slack_loop_log_path(workspace_root: &Path, loop_id: &str) -> PathBuf {
    workspace_root
        .join(".ralph/slack-loop-logs")
        .join(format!("{loop_id}.log"))
}

const MIN_PROGRESS_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProgressCheckpoint {
    last_reported_line_count: usize,
    last_sent_at: Option<std::time::Duration>,
}

pub async fn sync_loop_progress_once<N>(
    state_manager: &SlackStateManager,
    notifier: N,
    loop_id: &str,
    channel_id: &str,
    thread_ts: &str,
    contents: &str,
    elapsed: std::time::Duration,
    checkpoint: &mut ProgressCheckpoint,
) -> SlackResult<bool>
where
    N: ThreadNotifier,
{
    let Some(update) = latest_progress_update(contents, checkpoint.last_reported_line_count) else {
        return Ok(false);
    };
    if let Some(last_sent_at) = checkpoint.last_sent_at {
        if elapsed.saturating_sub(last_sent_at) < MIN_PROGRESS_UPDATE_INTERVAL {
            return Ok(false);
        }
    }

    let message = SlackBlocks::progress_card(
        loop_id,
        update.iteration,
        update.hat.as_deref(),
        &update.topic,
        &update.payload,
        Some(elapsed.as_secs()),
    );
    let progress_message_ts = state_manager
        .load_or_default()?
        .threads
        .get(loop_id)
        .and_then(|binding| binding.progress_message_ts.clone());

    if let Some(progress_message_ts) = progress_message_ts {
        notifier
            .update_thread_blocks(channel_id, &progress_message_ts, &message)
            .await?;
    } else {
        let progress_message_ts = notifier
            .post_thread_blocks(channel_id, thread_ts, &message)
            .await?;
        state_manager.set_thread_message_timestamps(
            loop_id,
            None,
            Some(&progress_message_ts),
            None,
            None,
        )?;
    }
    checkpoint.last_reported_line_count = update.line_count;
    checkpoint.last_sent_at = Some(elapsed);
    Ok(true)
}

fn process_is_alive(process_id: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(process_id.to_string())
        .stderr(Stdio::null())
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

fn handoff_text(binding: &SlackThreadBinding) -> String {
    let handoff_path = binding.handoff_path.clone().unwrap_or_else(|| {
        binding
            .workspace_root
            .join(".worktrees")
            .join(&binding.loop_id)
            .join(".ralph/agent/summary.md")
    });
    match std::fs::read_to_string(&handoff_path) {
        Ok(contents) if !contents.trim().is_empty() => {
            let mut text = redact_secrets(contents.trim());
            if text.len() > 3000 {
                text.truncate(3000);
                text.push_str("…");
            }
            format!(
                "Handoff for {}:
```
{}
```",
                binding.loop_id, text
            )
        }
        _ => format!(
            "No handoff summary found for {} yet. Expected: {}",
            binding.loop_id,
            handoff_path.display()
        ),
    }
}

fn artifacts_text(binding: &SlackThreadBinding) -> String {
    let artifacts_path = binding.artifacts_path.clone().unwrap_or_else(|| {
        binding
            .workspace_root
            .join(".worktrees")
            .join(&binding.loop_id)
            .join(".ralph")
    });
    if !artifacts_path.exists() {
        return format!(
            "No local artifacts found for {} at {}",
            binding.loop_id,
            artifacts_path.display()
        );
    }
    let mut entries = std::fs::read_dir(&artifacts_path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("events.jsonl"))
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    entries.sort();
    if entries.is_empty() {
        return format!("No local artifacts found for {}.", binding.loop_id);
    }
    entries.truncate(20);
    format!(
        "Local artifacts for {}:
{}",
        binding.loop_id,
        entries.join("\n")
    )
}

fn plain_tail_text(path: &Path, n: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let lines = contents.lines().rev().take(n).collect::<Vec<_>>();
            if lines.is_empty() {
                return "No log lines yet.".to_string();
            }
            let mut formatted = lines
                .into_iter()
                .rev()
                .map(redact_secrets)
                .collect::<Vec<_>>()
                .join("\n");
            if formatted.len() > 3000 {
                formatted.truncate(3000);
                formatted.push_str("…");
            }
            format!(
                "Latest Ralph log lines (last {n}):
```
{formatted}
```"
            )
        }
        Err(_) => "No loop log found for this loop yet.".to_string(),
    }
}

fn tail_text(path: &Path, n: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let lines: Vec<_> = contents.lines().rev().take(n).collect();
            if lines.is_empty() {
                return "No events yet.".to_string();
            }
            let mut formatted = lines
                .into_iter()
                .rev()
                .map(format_tail_event)
                .collect::<Vec<_>>()
                .join("\n");
            if formatted.len() > 3000 {
                formatted.truncate(3000);
                formatted.push_str("…");
            }
            format!("Latest Ralph events (last {n}):\n{formatted}")
        }
        Err(_) => "No event file found for this loop yet.".to_string(),
    }
}

fn format_tail_event(line: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        let raw = truncate_tail_payload(&redact_secrets(line.trim()));
        return format!("• raw event\n  {raw}");
    };
    let topic = value
        .get("topic")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let actor = tail_actor(&value, topic);
    let ts = tail_ts(value.get("ts").and_then(|value| value.as_str()));
    let iteration = value
        .get("iteration")
        .and_then(|value| value.as_u64())
        .map(|iteration| format!(" · iter {iteration}"))
        .unwrap_or_default();
    let payload = truncate_tail_payload(&redact_secrets(&event_payload_text(value.get("payload"))));
    let payload = collapse_tail_whitespace(&payload);
    if payload.is_empty() {
        format!("• `{ts}`{iteration} · {actor} · `{topic}`")
    } else {
        format!("• `{ts}`{iteration} · {actor} · `{topic}`\n  {payload}")
    }
}

fn tail_actor(value: &serde_json::Value, topic: &str) -> String {
    if topic == "human.response" {
        return "operator".to_string();
    }
    if topic.starts_with("human.") {
        return "human-loop".to_string();
    }
    value
        .get("hat")
        .or_else(|| value.get("triggered"))
        .and_then(|value| value.as_str())
        .unwrap_or("agent")
        .to_string()
}

fn tail_ts(ts: Option<&str>) -> String {
    let Some(ts) = ts else {
        return "time unknown".to_string();
    };
    if let Some((_, time)) = ts.split_once('T') {
        let short = time
            .chars()
            .take_while(|ch| *ch != '.' && *ch != '+' && *ch != 'Z')
            .collect::<String>();
        if !short.is_empty() {
            return short;
        }
    }
    ts.to_string()
}

fn truncate_tail_payload(payload: &str) -> String {
    let mut payload = payload.to_string();
    if payload.len() > 700 {
        payload.truncate(700);
        payload.push('…');
    }
    payload
}

fn collapse_tail_whitespace(payload: &str) -> String {
    payload.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn slack_loop_env(
    loop_id: &str,
    channel_id: &str,
    thread_ts: &str,
    state_path: &Path,
    repo_alias: Option<&str>,
    repo_dir: Option<&Path>,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::from([
        ("RALPH_LOOP_ID".to_string(), loop_id.to_string()),
        ("RALPH_SLACK_CHANNEL_ID".to_string(), channel_id.to_string()),
        ("RALPH_SLACK_THREAD_TS".to_string(), thread_ts.to_string()),
        (
            "RALPH_SLACK_STATE_PATH".to_string(),
            state_path.to_string_lossy().to_string(),
        ),
    ]);
    if let Some(repo_alias) = repo_alias {
        env.insert("RALPH_SLACK_REPO_ALIAS".to_string(), repo_alias.to_string());
    }
    if let Some(repo_dir) = repo_dir {
        env.insert(
            "RALPH_SLACK_REPO_DIR".to_string(),
            repo_dir.to_string_lossy().to_string(),
        );
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_text_formats_jsonl_as_operator_summary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"ts":"2026-06-07T21:59:34.126921+00:00","iteration":0,"hat":"loop","topic":"plan.start","triggered":"planner","payload":"live smoke: create .ralph/live-slack-smoke.txt containing ok from slack and ask for approval."}"#,
                "\n",
                r#"{"payload":"Approval requested: created artifact .ralph/live-slack-smoke.txt with token-secret-token-abc. Options: approve or request changes.","topic":"human.interact","ts":"2026-06-07T22:00:35.327605+00:00"}"#,
                "\n",
                r#"{"payload":"Approve","topic":"human.response","ts":"2026-06-07T22:01:16.584587+00:00"}"#,
                "\n"
            ),
        )
        .unwrap();

        let text = tail_text(&path, 10);

        assert!(text.contains("Latest Ralph events (last 10):"));
        assert!(text.contains("`21:59:34` · iter 0 · loop · `plan.start`"));
        assert!(text.contains("human-loop · `human.interact`"));
        assert!(text.contains("operator · `human.response`"));
        assert!(text.contains("Approve"));
        assert!(text.contains("[redacted]"));
        assert!(!text.contains(r#"{"ts""#));
        assert!(!text.contains("secret-token-abc"));
    }

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
    fn slack_loop_env_shares_root_state_without_forcing_workspace_root() {
        let env = slack_loop_env(
            "loop-1",
            "C123",
            "1780.1",
            Path::new("/daemon/.ralph/slack-state.json"),
            Some("ralph"),
            Some(Path::new("crates/ralph-slack")),
        );
        assert_eq!(env.get("RALPH_LOOP_ID").unwrap(), "loop-1");
        assert_eq!(env.get("RALPH_SLACK_CHANNEL_ID").unwrap(), "C123");
        assert_eq!(env.get("RALPH_SLACK_THREAD_TS").unwrap(), "1780.1");
        assert_eq!(env.get("RALPH_SLACK_REPO_ALIAS").unwrap(), "ralph");
        assert_eq!(
            env.get("RALPH_SLACK_REPO_DIR").unwrap(),
            "crates/ralph-slack"
        );
        assert_eq!(
            env.get("RALPH_SLACK_STATE_PATH").unwrap(),
            "/daemon/.ralph/slack-state.json"
        );
        assert!(!env.contains_key("RALPH_WORKSPACE_ROOT"));
    }

    #[test]
    fn slack_loop_command_uses_config_backend_without_hardcoded_claude() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path().join("worktree");
        let config = dir.path().join("ralph.slack.yml");
        let request = StartLoopRequest {
            loop_id: "slack-C123-1780-1".to_string(),
            prompt: "do the thing".to_string(),
            channel_id: "C123".to_string(),
            thread_ts: "1780.1".to_string(),
            workspace_root: dir.path().to_path_buf(),
            state_path: dir.path().join(".ralph/slack-state.json"),
            repo_alias: None,
            repo_dir: None,
            env: BTreeMap::new(),
        };
        let mut command = Command::new("ralph");

        configure_slack_loop_command(&mut command, &worktree, &request, Some(&config));

        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "run",
                "--autonomous",
                "-p",
                "do the thing",
                "-c",
                config.to_str().unwrap(),
            ]
        );
        assert!(!args.iter().any(|arg| arg == "-b" || arg == "claude"));
        assert_eq!(command.get_current_dir(), Some(worktree.as_path()));
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
