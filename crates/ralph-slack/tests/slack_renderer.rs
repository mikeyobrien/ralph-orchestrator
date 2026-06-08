use ralph_slack::{SlackBlocks, SlackThreadStatus};

#[test]
fn start_card_has_block_kit_actions_and_plain_text_fallback() {
    let message = SlackBlocks::start_card(
        "slack-C123-1780792150-138669",
        "build a Slack surface with useful progress cards",
        Some("/repo/ralph"),
        Some("feat/slack-thread-surface"),
    );

    assert!(message.text.contains("Ralph loop started"));
    assert!(message.text.contains("slack-C123-1780792150-138669"));
    assert_eq!(message.blocks[0]["type"], "header");
    assert!(
        message
            .blocks
            .iter()
            .any(|block| block["type"] == "actions")
    );
    assert!(message.blocks.iter().any(|block| {
        block["type"] == "context" && block.to_string().contains("feat/slack-thread-surface")
    }));
}

#[test]
fn progress_card_is_concise_and_redacts_secret_shaped_tokens() {
    let message = SlackBlocks::progress_card(
        "slack-C123-1780792150-138669",
        Some(3),
        Some("builder"),
        "work.progress",
        "created xoxb-secret-token-123 in scratch",
        Some(125),
    );

    assert!(message.text.contains("Ralph update"));
    assert!(!message.text.contains("xoxb-secret-token-123"));
    assert!(message.text.contains("[redacted]"));
    assert!(
        message
            .blocks
            .iter()
            .any(|block| block.to_string().contains("builder"))
    );
}

#[test]
fn final_status_and_help_cards_have_fallback_text() {
    let final_card = SlackBlocks::final_card(
        "slack-C123-1780792150-138669",
        SlackThreadStatus::Completed,
        Some(360),
        Some("Use `tail 10` for recent events."),
    );
    let status_card = SlackBlocks::status_card(
        "slack-C123-1780792150-138669",
        SlackThreadStatus::Running,
        "/repo/ralph",
        false,
        Some(4242),
    );
    let help_card = SlackBlocks::help_card();

    assert!(final_card.text.contains("completed"));
    assert!(status_card.text.contains("pending question: no"));
    assert!(help_card.text.contains("Ralph Slack commands"));
    assert!(help_card.text.contains("repo"));
    let final_actions = final_card
        .blocks
        .iter()
        .find(|block| block["type"] == "actions")
        .expect("final card should include interactive actions");
    assert!(final_actions.to_string().contains("ralph_slack_tail"));
    assert!(final_actions.to_string().contains("ralph_slack_status"));
    assert!(!final_actions.to_string().contains("ralph_slack_approve"));
    assert!(
        !final_actions
            .to_string()
            .contains("ralph_slack_request_changes")
    );
    assert!(
        status_card
            .blocks
            .iter()
            .any(|block| block["type"] == "section")
    );
    assert!(
        help_card
            .blocks
            .iter()
            .any(|block| block["type"] == "section")
    );
}
