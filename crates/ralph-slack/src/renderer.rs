use serde_json::{Value, json};

use crate::state::SlackThreadStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct SlackRenderedMessage {
    pub text: String,
    pub blocks: Vec<Value>,
}

pub struct SlackBlocks;

impl SlackBlocks {
    pub fn start_card(
        loop_id: &str,
        prompt: &str,
        repo: Option<&str>,
        branch: Option<&str>,
    ) -> SlackRenderedMessage {
        let prompt = truncate(prompt, 700);
        let text =
            format!("🤖 Ralph loop started\nLoop: {loop_id}\nStatus: running\nPrompt: {prompt}");
        SlackRenderedMessage {
            text,
            blocks: vec![
                header("🤖 Ralph loop started"),
                context(vec![
                    format!("*Status:* Running"),
                    format!("*Loop:* `{}`", escape_mrkdwn(loop_id)),
                    format!(
                        "*Repo:* {}",
                        repo.map(escape_mrkdwn)
                            .unwrap_or_else(|| "unknown".to_string())
                    ),
                    format!(
                        "*Branch:* {}",
                        branch
                            .map(escape_mrkdwn)
                            .unwrap_or_else(|| "unknown".to_string())
                    ),
                ]),
                section(format!("*Prompt*\n{}", escape_mrkdwn(&prompt))),
                actions(
                    loop_id,
                    &[
                        ActionButton::status(),
                        ActionButton::tail(),
                        ActionButton::stop(),
                    ],
                ),
            ],
        }
    }

    pub fn progress_card(
        loop_id: &str,
        iteration: Option<u64>,
        hat: Option<&str>,
        topic: &str,
        last_message: &str,
        elapsed_secs: Option<u64>,
    ) -> SlackRenderedMessage {
        let iteration = iteration
            .map(|iteration| iteration.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let hat = hat.unwrap_or("agent");
        let elapsed = elapsed_secs
            .map(|secs| format!("{}s", secs))
            .unwrap_or_else(|| "unknown".to_string());
        let mut last_message = redact_secrets(last_message);
        if last_message.len() > 1000 {
            last_message.truncate(1000);
            last_message.push('…');
        }
        let text = format!(
            "Ralph update\nLoop: {loop_id}\nIteration: {iteration}\nHat: {hat}\nTopic: {topic}\nLast message:\n```\n{last_message}\n```"
        );
        SlackRenderedMessage {
            text,
            blocks: vec![
                header("⚙️ Ralph update"),
                context(vec![
                    format!("*Loop:* `{}`", escape_mrkdwn(loop_id)),
                    format!("*Iteration:* {}", escape_mrkdwn(&iteration)),
                    format!("*Hat:* {}", escape_mrkdwn(hat)),
                    format!("*Elapsed:* {}", escape_mrkdwn(&elapsed)),
                ]),
                section(format!("*Topic:* {}", escape_mrkdwn(topic))),
                section(format!(
                    "*Last message*\n```\n{}\n```",
                    escape_code_block(&last_message)
                )),
            ],
        }
    }

    pub fn final_card(
        loop_id: &str,
        status: SlackThreadStatus,
        duration_secs: Option<u64>,
        note: Option<&str>,
    ) -> SlackRenderedMessage {
        let status_label = status_label(&status);
        let duration = duration_secs
            .map(|secs| format!("{}s", secs))
            .unwrap_or_else(|| "unknown".to_string());
        let note = note.unwrap_or("Try `tail 10` in this thread for recent events.");
        SlackRenderedMessage {
            text: format!("Ralph loop {status_label}\nLoop: {loop_id}\n{note}"),
            blocks: vec![
                header(format!("Ralph loop {status_label}")),
                context(vec![
                    format!("*Status:* {}", title_case(status_label)),
                    format!("*Loop:* `{}`", escape_mrkdwn(loop_id)),
                    format!("*Duration:* {}", escape_mrkdwn(&duration)),
                ]),
                section(escape_mrkdwn(note)),
                actions(loop_id, &[ActionButton::tail(), ActionButton::status()]),
            ],
        }
    }

    pub fn status_card(
        loop_id: &str,
        status: SlackThreadStatus,
        repo: &str,
        pending_question: bool,
        process_id: Option<u32>,
    ) -> SlackRenderedMessage {
        let status_label = status_label(&status);
        let pending = if pending_question { "yes" } else { "no" };
        let process = process_id
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        SlackRenderedMessage {
            text: format!(
                "Ralph thread status\nloop: {loop_id}\nrepo: {repo}\nthread status: {status_label}\npending question: {pending}\nprocess id: {process}"
            ),
            blocks: vec![
                header("Ralph thread status"),
                section(format!(
                    "*Loop:* `{}`\n*Repo:* {}\n*Thread status:* {}\n*Pending question:* {}\n*Process id:* `{}`",
                    escape_mrkdwn(loop_id),
                    escape_mrkdwn(repo),
                    title_case(status_label),
                    pending,
                    escape_mrkdwn(&process)
                )),
                actions(loop_id, &[ActionButton::tail(), ActionButton::stop()]),
            ],
        }
    }

    pub fn help_card() -> SlackRenderedMessage {
        let text = "Ralph Slack commands: help, status, tail [n], stop/cancel. Plain replies become guidance, or answer the pending human question.".to_string();
        SlackRenderedMessage {
            text: text.clone(),
            blocks: vec![
                header("Ralph Slack help"),
                section(
                    "*Commands*\n• `help` — show this help\n• `status` — show loop status\n• `tail [n]` — show recent events\n• `stop` / `cancel` — stop the loop\n\nPlain replies become guidance, or answer the pending human question.",
                ),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ActionButton {
    text: &'static str,
    action_id: &'static str,
    value: &'static str,
    style: Option<&'static str>,
}

impl ActionButton {
    fn status() -> Self {
        Self {
            text: "Status",
            action_id: "ralph_slack_status",
            value: "status",
            style: None,
        }
    }

    fn tail() -> Self {
        Self {
            text: "Tail",
            action_id: "ralph_slack_tail",
            value: "tail",
            style: None,
        }
    }

    fn stop() -> Self {
        Self {
            text: "Stop",
            action_id: "ralph_slack_stop",
            value: "stop",
            style: Some("danger"),
        }
    }
}

fn header(text: impl Into<String>) -> Value {
    json!({
        "type": "header",
        "text": {"type": "plain_text", "text": truncate(&text.into(), 150), "emoji": true}
    })
}

fn section(text: impl Into<String>) -> Value {
    json!({
        "type": "section",
        "text": {"type": "mrkdwn", "text": truncate(&text.into(), 3000)}
    })
}

fn context(elements: Vec<String>) -> Value {
    json!({
        "type": "context",
        "elements": elements.into_iter().map(|text| json!({"type": "mrkdwn", "text": truncate(&text, 300)})).collect::<Vec<_>>()
    })
}

fn actions(loop_id: &str, buttons: &[ActionButton]) -> Value {
    json!({
        "type": "actions",
        "elements": buttons.iter().map(|button| {
            let mut value = json!({
                "type": "button",
                "text": {"type": "plain_text", "text": button.text, "emoji": true},
                "action_id": button.action_id,
                "value": if button.value == "status" { loop_id.to_string() } else { format!("{}:{}", button.value, loop_id) }
            });
            if let Some(style) = button.style {
                value["style"] = json!(style);
            }
            value
        }).collect::<Vec<_>>()
    })
}

pub(crate) fn redact_secrets(text: &str) -> String {
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

fn status_label(status: &SlackThreadStatus) -> &'static str {
    match status {
        SlackThreadStatus::Running => "running",
        SlackThreadStatus::Completed => "completed",
        SlackThreadStatus::Failed => "failed",
        SlackThreadStatus::Stopped => "stopped",
    }
}

fn title_case(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn escape_mrkdwn(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_code_block(text: &str) -> String {
    text.replace("```", "`\u{200b}`\u{200b}`")
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out = text
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}
