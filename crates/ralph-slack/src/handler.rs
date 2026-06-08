use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{SlackError, SlackResult};
use crate::state::{SlackStateManager, thread_key};

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
    Log { n: usize },
    Handoff,
    Artifacts,
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
        if binding.status.is_terminal() {
            if let Some(prompt) = parse_followup_command(&event.text) {
                let followup_loop_id = loop_id_for_slack_thread(&event.channel_id, &event.ts);
                validate_loop_id(&followup_loop_id)?;
                manager.bind_followup_thread(
                    &followup_loop_id,
                    &event.channel_id,
                    &event.ts,
                    &event.ts,
                    user_id,
                    &binding.workspace_root,
                    &loop_id,
                )?;
                return Ok(HandlerAction::StartLoop {
                    loop_id: followup_loop_id,
                    prompt,
                    channel_id: event.channel_id,
                    thread_ts: event.ts,
                });
            }
            if let Some(command) = parse_thread_command(&event.text) {
                return Ok(HandlerAction::Command {
                    command,
                    loop_id,
                    channel_id: event.channel_id,
                    thread_ts: thread_ts.to_string(),
                    user_id: user_id.to_string(),
                });
            }
            return Ok(HandlerAction::Ignored);
        }
        if let Some(command) = parse_thread_command(&event.text) {
            return Ok(HandlerAction::Command {
                command,
                loop_id,
                channel_id: event.channel_id,
                thread_ts: thread_ts.to_string(),
                user_id: user_id.to_string(),
            });
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
        "handoff" => Some(ThreadCommand::Handoff),
        "artifacts" => Some(ThreadCommand::Artifacts),
        "tail" => {
            let n = parts
                .next()
                .and_then(|part| part.parse::<usize>().ok())
                .unwrap_or(10)
                .clamp(1, 25);
            Some(ThreadCommand::Tail { n })
        }
        "log" => {
            let n = parts
                .next()
                .and_then(|part| part.parse::<usize>().ok())
                .unwrap_or(20)
                .clamp(1, 50);
            Some(ThreadCommand::Log { n })
        }
        _ => None,
    }
}

pub fn parse_followup_command(text: &str) -> Option<String> {
    let trimmed = text.trim().trim_start_matches('/').trim_start_matches('!');
    let lower = trimmed.to_ascii_lowercase();
    for prefix in ["followup ", "follow-up ", "fork ", "new work "] {
        if lower.starts_with(prefix) {
            let prompt = trimmed[prefix.len()..].trim();
            if !prompt.is_empty() {
                return Some(prompt.to_string());
            }
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SlackThreadStatus;

    fn event(text: &str, ts: &str, thread_ts: Option<&str>) -> SlackMessageEvent {
        SlackMessageEvent {
            event_id: Some(format!("evt-{ts}")),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: text.to_string(),
            ts: ts.to_string(),
            thread_ts: thread_ts.map(str::to_string),
            bot_id: None,
            app_mention: false,
        }
    }

    #[test]
    fn completed_thread_plain_reply_does_not_append_old_loop_event() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));
        manager
            .bind_thread("slack-C123-1-0", "C123", "1.0", "U123", dir.path())
            .unwrap();
        manager
            .add_pending_question("slack-C123-1-0", "C123", "1.0", "msg-1")
            .unwrap();
        manager
            .finish_thread("slack-C123-1-0", SlackThreadStatus::Completed)
            .unwrap();

        let action = handle_message(
            &manager,
            dir.path(),
            &["C123".to_string()],
            &["U123".to_string()],
            event("please keep going", "1.1", Some("1.0")),
        )
        .unwrap();

        assert_eq!(action, HandlerAction::Ignored);
        assert!(!events_path(dir.path(), "slack-C123-1-0").exists());
        let state = manager.load_or_default().unwrap();
        assert!(!state.pending_questions.contains_key("slack-C123-1-0"));
        assert_eq!(
            state.threads["slack-C123-1-0"].status,
            SlackThreadStatus::Completed
        );
        assert!(state.threads["slack-C123-1-0"].process_id.is_none());
    }

    #[test]
    fn completed_thread_followup_forks_new_bound_loop() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));
        manager
            .bind_thread("slack-C123-1-0", "C123", "1.0", "U123", dir.path())
            .unwrap();
        manager
            .finish_thread("slack-C123-1-0", SlackThreadStatus::Completed)
            .unwrap();

        let action = handle_message(
            &manager,
            dir.path(),
            &["C123".to_string()],
            &["U123".to_string()],
            event("fork add a regression test", "1.2", Some("1.0")),
        )
        .unwrap();

        let HandlerAction::StartLoop {
            loop_id,
            prompt,
            channel_id,
            thread_ts,
        } = action
        else {
            panic!("expected fork to start a new loop");
        };
        assert_eq!(loop_id, "slack-C123-1-2");
        assert_eq!(prompt, "add a regression test");
        assert_eq!(channel_id, "C123");
        assert_eq!(thread_ts, "1.2");
        let state = manager.load_or_default().unwrap();
        assert_eq!(
            state.thread_to_loop.get("C123:1.0").map(String::as_str),
            Some("slack-C123-1-0")
        );
        assert_eq!(
            state.thread_to_loop.get("C123:1.2").map(String::as_str),
            Some("slack-C123-1-2")
        );
        assert_eq!(
            state.threads["slack-C123-1-2"].parent_loop_id.as_deref(),
            Some("slack-C123-1-0")
        );
        assert_eq!(
            state.threads["slack-C123-1-2"].status,
            SlackThreadStatus::Running
        );
    }

    #[test]
    fn completed_thread_read_only_commands_are_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));
        manager
            .bind_thread("slack-C123-1-0", "C123", "1.0", "U123", dir.path())
            .unwrap();
        manager
            .finish_thread("slack-C123-1-0", SlackThreadStatus::Failed)
            .unwrap();

        let action = handle_message(
            &manager,
            dir.path(),
            &["C123".to_string()],
            &["U123".to_string()],
            event("handoff", "1.3", Some("1.0")),
        )
        .unwrap();

        assert_eq!(
            action,
            HandlerAction::Command {
                command: ThreadCommand::Handoff,
                loop_id: "slack-C123-1-0".to_string(),
                channel_id: "C123".to_string(),
                thread_ts: "1.0".to_string(),
                user_id: "U123".to_string(),
            }
        );
    }
}
