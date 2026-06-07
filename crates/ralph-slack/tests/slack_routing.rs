use ralph_slack::{
    HandlerAction, SlackMessageEvent, SlackStateManager, SlackThreadStatus, handle_message,
};
use tempfile::TempDir;

#[test]
fn slack_state_round_trips_thread_binding_pending_question_and_event_dedupe() {
    let dir = TempDir::new().unwrap();
    let state_path = dir.path().join(".ralph/slack-state.json");
    let manager = SlackStateManager::new(&state_path);

    manager
        .bind_thread(
            "slack-C123-1780792150-138669",
            "C123",
            "1780792150.138669",
            "U123",
            dir.path(),
        )
        .unwrap();
    manager
        .add_pending_question(
            "slack-C123-1780792150-138669",
            "C123",
            "1780792150.138669",
            "1780792160.000100",
        )
        .unwrap();
    assert!(manager.mark_event_seen("Ev123").unwrap());
    assert!(!manager.mark_event_seen("Ev123").unwrap());

    let loaded = manager.load().unwrap().unwrap();
    let binding = loaded.threads.get("slack-C123-1780792150-138669").unwrap();
    assert_eq!(binding.channel_id, "C123");
    assert_eq!(binding.thread_ts, "1780792150.138669");
    assert_eq!(binding.created_by, "U123");
    assert_eq!(binding.workspace_root, dir.path());
    assert_eq!(binding.status, SlackThreadStatus::Running);
    assert_eq!(binding.start_card_ts.as_deref(), None);
    assert_eq!(binding.progress_message_ts.as_deref(), None);
    assert_eq!(binding.stream_ts.as_deref(), None);
    assert_eq!(binding.final_card_ts.as_deref(), None);
    assert_eq!(
        loaded.thread_to_loop["C123:1780792150.138669"],
        "slack-C123-1780792150-138669"
    );
    assert_eq!(
        loaded.pending_questions["slack-C123-1780792150-138669"].message_ts,
        "1780792160.000100"
    );
    assert!(loaded.seen_event_ids.contains(&"Ev123".to_string()));
}

#[test]
fn slack_state_loads_old_json_without_message_timestamps_and_can_set_card_timestamps() {
    let dir = TempDir::new().unwrap();
    let state_path = dir.path().join(".ralph/slack-state.json");
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    std::fs::write(
        &state_path,
        serde_json::json!({
            "team_id": null,
            "last_socket_envelope_id": null,
            "threads": {
                "slack-C123-1780792150-138669": {
                    "loop_id": "slack-C123-1780792150-138669",
                    "channel_id": "C123",
                    "thread_ts": "1780792150.138669",
                    "root_ts": "1780792150.138669",
                    "created_by": "U123",
                    "created_at": "2026-06-06T19:30:27Z",
                    "workspace_root": dir.path(),
                    "status": "running",
                    "process_id": 4242
                }
            },
            "thread_to_loop": {"C123:1780792150.138669": "slack-C123-1780792150-138669"},
            "pending_questions": {},
            "seen_event_ids": []
        })
        .to_string(),
    )
    .unwrap();
    let manager = SlackStateManager::new(&state_path);

    let loaded = manager.load_or_default().unwrap();
    let binding = loaded.threads.get("slack-C123-1780792150-138669").unwrap();
    assert_eq!(binding.start_card_ts, None);
    assert_eq!(binding.progress_message_ts, None);
    assert_eq!(binding.stream_ts, None);
    assert_eq!(binding.final_card_ts, None);

    manager
        .set_thread_message_timestamps(
            "slack-C123-1780792150-138669",
            Some("1780792160.000100"),
            Some("1780792170.000100"),
            Some("stream-1"),
            Some("1780792180.000100"),
        )
        .unwrap();
    let updated = manager.load_or_default().unwrap();
    let binding = updated.threads.get("slack-C123-1780792150-138669").unwrap();
    assert_eq!(binding.start_card_ts.as_deref(), Some("1780792160.000100"));
    assert_eq!(
        binding.progress_message_ts.as_deref(),
        Some("1780792170.000100")
    );
    assert_eq!(binding.stream_ts.as_deref(), Some("stream-1"));
    assert_eq!(binding.final_card_ts.as_deref(), Some("1780792180.000100"));
}

#[test]
fn known_thread_with_pending_question_writes_human_response_and_clears_pending() {
    let dir = TempDir::new().unwrap();
    let state_path = dir.path().join(".ralph/slack-state.json");
    let manager = SlackStateManager::new(&state_path);
    manager
        .bind_thread(
            "slack-C123-1780792150-138669",
            "C123",
            "1780792150.138669",
            "U123",
            dir.path(),
        )
        .unwrap();
    manager
        .add_pending_question(
            "slack-C123-1780792150-138669",
            "C123",
            "1780792150.138669",
            "1780792160.000100",
        )
        .unwrap();

    let action = handle_message(
        &manager,
        dir.path(),
        &["C123".to_string()],
        &["U123".to_string()],
        SlackMessageEvent {
            event_id: Some("Ev-response".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "ship it".to_string(),
            ts: "1780792161.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        },
    )
    .unwrap();

    assert_eq!(
        action,
        HandlerAction::Appended {
            topic: "human.response".to_string(),
            loop_id: "slack-C123-1780792150-138669".to_string()
        }
    );
    let events = std::fs::read_to_string(
        dir.path()
            .join(".worktrees/slack-C123-1780792150-138669/.ralph/events.jsonl"),
    )
    .unwrap();
    let event: serde_json::Value = serde_json::from_str(events.trim()).unwrap();
    assert_eq!(event["topic"], "human.response");
    assert_eq!(event["payload"], "ship it");
    let state = manager.load_or_default().unwrap();
    assert!(
        !state
            .pending_questions
            .contains_key("slack-C123-1780792150-138669")
    );
}

#[test]
fn known_thread_without_pending_question_writes_human_guidance() {
    let dir = TempDir::new().unwrap();
    let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));
    manager
        .bind_thread(
            "slack-C123-1780792150-138669",
            "C123",
            "1780792150.138669",
            "U123",
            dir.path(),
        )
        .unwrap();

    let action = handle_message(
        &manager,
        dir.path(),
        &["C123".to_string()],
        &["U123".to_string()],
        SlackMessageEvent {
            event_id: Some("Ev-guidance".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "prefer small diffs".to_string(),
            ts: "1780792161.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        },
    )
    .unwrap();

    assert_eq!(
        action,
        HandlerAction::Appended {
            topic: "human.guidance".to_string(),
            loop_id: "slack-C123-1780792150-138669".to_string()
        }
    );
}

#[test]
fn unauthorized_user_is_rejected_before_state_changes() {
    let dir = TempDir::new().unwrap();
    let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));

    let action = handle_message(
        &manager,
        dir.path(),
        &["C123".to_string()],
        &["U123".to_string()],
        SlackMessageEvent {
            event_id: Some("Ev-reject".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U999".to_string()),
            text: "malicious".to_string(),
            ts: "1780792161.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        },
    )
    .unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    assert!(manager.load().unwrap().is_none());
}

#[test]
fn empty_authorization_lists_reject_before_event_dedupe_or_state_changes() {
    let dir = TempDir::new().unwrap();
    let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));

    let action = handle_message(
        &manager,
        dir.path(),
        &[],
        &[],
        SlackMessageEvent {
            event_id: Some("Ev-empty-auth".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "<@URALPH> do not auto-trust me".to_string(),
            ts: "1780792161.000100".to_string(),
            thread_ts: None,
            bot_id: None,
            app_mention: true,
        },
    )
    .unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    assert!(manager.load().unwrap().is_none());
}

#[test]
fn root_app_mention_returns_start_loop_action_and_binds_thread() {
    let dir = TempDir::new().unwrap();
    let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));

    let action = handle_message(
        &manager,
        dir.path(),
        &["C123".to_string()],
        &["U123".to_string()],
        SlackMessageEvent {
            event_id: Some("Ev-start".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "<@URALPH> build a plan".to_string(),
            ts: "1780792150.138669".to_string(),
            thread_ts: None,
            bot_id: None,
            app_mention: true,
        },
    )
    .unwrap();

    assert_eq!(
        action,
        HandlerAction::StartLoop {
            loop_id: "slack-C123-1780792150-138669".to_string(),
            prompt: "build a plan".to_string(),
            channel_id: "C123".to_string(),
            thread_ts: "1780792150.138669".to_string(),
        }
    );
    assert_eq!(
        manager.load_or_default().unwrap().thread_to_loop["C123:1780792150.138669"],
        "slack-C123-1780792150-138669"
    );
}

#[test]
fn path_traversal_loop_id_is_rejected() {
    let dir = TempDir::new().unwrap();
    let manager = SlackStateManager::new(dir.path().join(".ralph/slack-state.json"));
    manager
        .bind_thread("../escape", "C123", "1780792150.138669", "U123", dir.path())
        .unwrap();

    let err = handle_message(
        &manager,
        dir.path(),
        &["C123".to_string()],
        &["U123".to_string()],
        SlackMessageEvent {
            event_id: Some("Ev-traversal".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "try escape".to_string(),
            ts: "1780792161.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        },
    )
    .unwrap_err();

    assert!(err.to_string().contains("invalid Slack loop id"));
}
