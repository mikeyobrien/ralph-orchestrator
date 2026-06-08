use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{SlackError, SlackResult};
use crate::state::{SlackStateManager, SlackThreadStatus, thread_key};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackMessageEvent {
    pub event_id: Option<String>,
    pub channel_id: String,
    pub user_id: Option<String>,
    pub text: String,
    pub ts: String,
    pub thread_ts: Option<String>,
    pub bot_id: Option<String>,
    pub app_mention: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadCommand {
    Help,
    Repo,
    Status,
    Tail { n: usize },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerAction {
    Ignored,
    Duplicate,
    Appended {
        topic: String,
        loop_id: String,
    },
    Command {
        command: ThreadCommand,
        loop_id: String,
        channel_id: String,
        thread_ts: String,
        user_id: String,
    },
    StartLoop {
        loop_id: String,
        prompt: String,
        channel_id: String,
        thread_ts: String,
    },
}

pub fn handle_message(
    manager: &SlackStateManager,
    workspace_root: &Path,
    allowed_channels: &[String],
    allowed_users: &[String],
    event: SlackMessageEvent,
) -> SlackResult<HandlerAction> {
    handle_message_with_repo(
        manager,
        workspace_root,
        None,
        None,
        allowed_channels,
        allowed_users,
        event,
    )
}

pub fn handle_message_with_repo(
    manager: &SlackStateManager,
    workspace_root: &Path,
    repo_alias: Option<&str>,
    repo_dir: Option<&Path>,
    allowed_channels: &[String],
    allowed_users: &[String],
    event: SlackMessageEvent,
) -> SlackResult<HandlerAction> {
    if event.bot_id.is_some() {
        return Ok(HandlerAction::Ignored);
    }
    let Some(user_id) = event.user_id.as_deref() else {
        return Ok(HandlerAction::Ignored);
    };
    if allowed_channels.is_empty()
        || !allowed_channels
            .iter()
            .any(|channel| channel == &event.channel_id)
    {
        return Ok(HandlerAction::Ignored);
    }
    if allowed_users.is_empty() || !allowed_users.iter().any(|user| user == user_id) {
        return Ok(HandlerAction::Ignored);
    }

    if let Some(event_id) = event.event_id.as_deref() {
        if !manager.mark_event_seen(event_id)? {
            return Ok(HandlerAction::Duplicate);
        }
    }

    let thread_ts = event.thread_ts.as_deref().unwrap_or(&event.ts);
    let state = manager.load_or_default()?;
    if let Some(loop_id) = state
        .thread_to_loop
        .get(&thread_key(&event.channel_id, thread_ts))
        .cloned()
    {
        validate_loop_id(&loop_id)?;
        let Some(binding) = state.threads.get(&loop_id) else {
            return Ok(HandlerAction::Ignored);
        };
        if let Some(command) = parse_thread_command(&event.text) {
            return Ok(HandlerAction::Command {
                command,
                loop_id,
                channel_id: event.channel_id,
                thread_ts: thread_ts.to_string(),
                user_id: user_id.to_string(),
            });
        }
        if binding.status != SlackThreadStatus::Running {
            return Ok(HandlerAction::Ignored);
        }
        let topic = if state.pending_questions.contains_key(&loop_id) {
            "human.response"
        } else {
            "human.guidance"
        };
        append_event(
            &events_path(&binding.workspace_root, &loop_id),
            topic,
            &event.text,
        )?;
        if topic == "human.response" {
            manager.remove_pending_question(&loop_id)?;
        }
        return Ok(HandlerAction::Appended {
            topic: topic.to_string(),
            loop_id,
        });
    }

    if event.thread_ts.is_none() && event.app_mention {
        let loop_id = loop_id_for_slack_thread(&event.channel_id, &event.ts);
        validate_loop_id(&loop_id)?;
        let prompt = strip_app_mention(&event.text);
        manager.bind_thread_with_repo(
            &loop_id,
            &event.channel_id,
            &event.ts,
            user_id,
            workspace_root,
            repo_alias,
            repo_dir,
        )?;
        return Ok(HandlerAction::StartLoop {
            loop_id,
            prompt,
            channel_id: event.channel_id,
            thread_ts: event.ts,
        });
    }

    Ok(HandlerAction::Ignored)
}

pub fn parse_thread_command(text: &str) -> Option<ThreadCommand> {
    let trimmed = text
        .trim()
        .trim_start_matches('/')
        .trim_start_matches('!')
        .to_ascii_lowercase();
    let mut parts = trimmed.split_whitespace();
    match parts.next()? {
        "help" => Some(ThreadCommand::Help),
        "repo" => Some(ThreadCommand::Repo),
        "status" => Some(ThreadCommand::Status),
        "stop" | "cancel" => Some(ThreadCommand::Stop),
        "tail" => {
            let n = parts
                .next()
                .and_then(|part| part.parse::<usize>().ok())
                .unwrap_or(10)
                .clamp(1, 25);
            Some(ThreadCommand::Tail { n })
        }
        _ => None,
    }
}

pub fn loop_id_for_slack_thread(channel_id: &str, thread_ts: &str) -> String {
    format!("slack-{}-{}", channel_id, thread_ts.replace('.', "-"))
}

pub fn validate_loop_id(loop_id: &str) -> SlackResult<()> {
    if loop_id.is_empty()
        || loop_id.len() > 128
        || loop_id
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return Err(SlackError::InvalidLoopId(loop_id.to_string()));
    }
    Ok(())
}

fn strip_app_mention(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("<@") {
        if let Some((_, prompt)) = rest.split_once('>') {
            return prompt.trim().to_string();
        }
    }
    trimmed.to_string()
}

pub fn events_path(workspace_root: &Path, loop_id: &str) -> PathBuf {
    let ralph_dir = if loop_id == "main" {
        workspace_root.join(".ralph")
    } else {
        workspace_root
            .join(".worktrees")
            .join(loop_id)
            .join(".ralph")
    };
    let marker = ralph_dir.join("current-events");
    if let Ok(relative) = std::fs::read_to_string(&marker) {
        let relative = relative.trim();
        if !relative.is_empty() {
            if loop_id == "main" {
                return workspace_root.join(relative);
            }
            return workspace_root
                .join(".worktrees")
                .join(loop_id)
                .join(relative);
        }
    }
    ralph_dir.join("events.jsonl")
}

fn append_event(path: &Path, topic: &str, payload: &str) -> SlackResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let event_json = serde_json::json!({
        "topic": topic,
        "payload": payload,
        "ts": Utc::now().to_rfc3339(),
    });
    writeln!(file, "{}", serde_json::to_string(&event_json)?)?;
    Ok(())
}
