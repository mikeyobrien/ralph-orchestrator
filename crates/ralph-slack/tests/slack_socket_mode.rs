use ralph_slack::handler::{ThreadCommand, parse_thread_command};
use ralph_slack::socket_mode::slack_message_event_from_payload;
use ralph_slack::{HandlerAction, SlackStateManager, handle_message};

#[test]
fn typed_tail_command_variants_still_parse() {
    for text in ["tail", "tail 10", "!tail 10", "/tail 10"] {
        assert_eq!(
            parse_thread_command(text),
            Some(ThreadCommand::Tail { n: 10 }),
            "{text} should route through the existing tail command path"
        );
    }
}

#[test]
fn block_action_tail_payload_maps_to_existing_thread_command_text() {
    let payload = serde_json::json!({
        "type": "block_actions",
        "trigger_id": "TrigTail",
        "user": {"id": "U123"},
        "channel": {"id": "C123"},
        "message": {"ts": "1780792150.138669", "thread_ts": "1780792150.138669"},
        "actions": [{"action_id": "ralph_slack_tail", "value": "tail:slack-C123-1780792150-138669"}],
        "action_ts": "1780792160.000100"
    });

    let event = slack_message_event_from_payload(&payload).expect("block action should parse");

    assert_eq!(event.channel_id, "C123");
    assert_eq!(event.user_id.as_deref(), Some("U123"));
    assert_eq!(event.thread_ts.as_deref(), Some("1780792150.138669"));
    assert_eq!(event.text, "tail 10");
    assert_eq!(
        parse_thread_command(&event.text),
        Some(ThreadCommand::Tail { n: 10 })
    );
}

#[test]
fn block_action_approve_payload_routes_as_human_response_on_pending_question() {
    let dir = tempfile::TempDir::new().unwrap();
    let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));
    let loop_id = "slack-C123-1780792150-138669";
    manager
        .bind_thread(loop_id, "C123", "1780792150.138669", "U123", dir.path())
        .unwrap();
    manager
        .add_pending_question(loop_id, "C123", "1780792150.138669", "1780792160.000100")
        .unwrap();
    let payload = serde_json::json!({
        "type": "block_actions",
        "trigger_id": "TrigApprove",
        "user": {"id": "U123"},
        "channel": {"id": "C123"},
        "message": {"ts": "1780792150.138669", "thread_ts": "1780792150.138669"},
        "actions": [{"action_id": "ralph_slack_approve", "value": "approve:slack-C123-1780792150-138669"}],
        "action_ts": "1780792160.000200"
    });
    let event = slack_message_event_from_payload(&payload).expect("approve action should parse");

    let action = handle_message(
        &manager,
        dir.path(),
        &["C123".to_string()],
        &["U123".to_string()],
        event,
    )
    .unwrap();

    assert_eq!(
        action,
        HandlerAction::Appended {
            topic: "human.response".to_string(),
            loop_id: loop_id.to_string()
        }
    );
    let events = std::fs::read_to_string(
        dir.path()
            .join(".worktrees/slack-C123-1780792150-138669/.ralph/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("approved"));
    assert!(!manager.has_pending_question(loop_id).unwrap());
}
