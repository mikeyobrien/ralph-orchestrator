use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ralph_slack::HandlerAction;
use ralph_slack::daemon::{
    LoopSpawner, SlackDaemon, SlackDaemonConfig, StartLoopRequest, ThreadNotifier,
};
use ralph_slack::handler::SlackMessageEvent;
use ralph_slack::socket_mode::slack_message_event_from_payload;
use ralph_slack::state::{SlackStateManager, SlackThreadStatus};

#[derive(Default, Clone)]
struct FakeSpawner {
    requests: Arc<Mutex<Vec<StartLoopRequest>>>,
    stopped: Arc<Mutex<Vec<u32>>>,
    delay_ms: u64,
}

#[async_trait]
impl LoopSpawner for FakeSpawner {
    async fn spawn_loop(&self, request: StartLoopRequest) -> ralph_slack::SlackResult<Option<u32>> {
        if self.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        }
        self.requests.lock().unwrap().push(request);
        Ok(Some(4242))
    }

    async fn stop_loop(&self, process_id: u32) -> ralph_slack::SlackResult<()> {
        self.stopped.lock().unwrap().push(process_id);
        Ok(())
    }
}

#[derive(Default, Clone)]
struct FakeNotifier {
    messages: Arc<Mutex<Vec<(String, String, String)>>>,
    updates: Arc<Mutex<Vec<(String, String, String)>>>,
    statuses: Arc<Mutex<Vec<(String, String, String)>>>,
}

#[async_trait]
impl ThreadNotifier for FakeNotifier {
    async fn post_thread_message(
        &self,
        channel_id: &str,
        thread_ts: &str,
        text: &str,
    ) -> ralph_slack::SlackResult<String> {
        self.messages.lock().unwrap().push((
            channel_id.to_string(),
            thread_ts.to_string(),
            text.to_string(),
        ));
        Ok("1780799999.000100".to_string())
    }

    async fn update_thread_blocks(
        &self,
        channel_id: &str,
        message_ts: &str,
        message: &ralph_slack::SlackRenderedMessage,
    ) -> ralph_slack::SlackResult<()> {
        self.updates.lock().unwrap().push((
            channel_id.to_string(),
            message_ts.to_string(),
            message.text.clone(),
        ));
        Ok(())
    }

    async fn set_assistant_thread_status(
        &self,
        channel_id: &str,
        thread_ts: &str,
        status: &str,
    ) -> ralph_slack::SlackResult<()> {
        self.statuses.lock().unwrap().push((
            channel_id.to_string(),
            thread_ts.to_string(),
            status.to_string(),
        ));
        Ok(())
    }
}

#[tokio::test]
async fn progress_events_create_once_then_update_same_message_ts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    let notifier = FakeNotifier::default();
    let first = r#"{"iteration":1,"hat":"planner","topic":"plan.ready","payload":"first"}"#;
    let second = concat!(
        r#"{"iteration":1,"hat":"planner","topic":"plan.ready","payload":"first"}"#,
        "\n",
        r#"{"iteration":2,"hat":"executor","topic":"agent.message","payload":"second"}"#,
        "\n"
    );

    let mut checkpoint = ralph_slack::daemon::ProgressCheckpoint::default();
    ralph_slack::daemon::sync_loop_progress_once(
        &state,
        notifier.clone(),
        loop_id,
        "C123",
        "1780792150.138669",
        first,
        std::time::Duration::from_secs(0),
        &mut checkpoint,
    )
    .await
    .unwrap();
    ralph_slack::daemon::sync_loop_progress_once(
        &state,
        notifier.clone(),
        loop_id,
        "C123",
        "1780792150.138669",
        second,
        std::time::Duration::from_secs(61),
        &mut checkpoint,
    )
    .await
    .unwrap();

    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].2.contains("plan.ready"));
    drop(messages);
    let updates = notifier.updates.lock().unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].1, "1780799999.000100");
    assert!(updates[0].2.contains("agent.message"));
    assert_eq!(
        state.load_or_default().unwrap().threads[loop_id]
            .progress_message_ts
            .as_deref(),
        Some("1780799999.000100")
    );
}

#[tokio::test]
async fn assistant_progress_status_uses_semantics_not_raw_payload_paths() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    let notifier = FakeNotifier::default();
    let progress = r#"{"topic":"unknown","payload":"running /Users/rook/projects/ralph-orchestrator/.ralph/slack-loop-logs/slack-C123.log && cat raw-output.txt"}"#;

    let mut checkpoint = ralph_slack::daemon::ProgressCheckpoint::default();
    ralph_slack::daemon::sync_loop_progress_once(
        &state,
        notifier.clone(),
        loop_id,
        "C123",
        "1780792150.138669",
        progress,
        std::time::Duration::from_secs(0),
        &mut checkpoint,
    )
    .await
    .unwrap();

    let statuses = notifier.statuses.lock().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].2, "is working");
    assert!(!statuses[0].2.contains("/Users/rook"));
    assert!(!statuses[0].2.contains("slack-loop-logs"));
    assert!(!statuses[0].2.contains("raw-output"));
}

#[tokio::test]
async fn pending_human_status_is_not_overwritten_by_later_progress() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    let notifier = FakeNotifier::default();
    let human =
        r#"{"iteration":3,"hat":"builder","topic":"human.interact","payload":"Need approval"}"#;
    let later = concat!(
        r#"{"iteration":3,"hat":"builder","topic":"human.interact","payload":"Need approval"}"#,
        "\n",
        r#"{"iteration":4,"hat":"builder","topic":"agent.message","payload":"Still running"}"#,
        "\n"
    );

    let mut checkpoint = ralph_slack::daemon::ProgressCheckpoint::default();
    ralph_slack::daemon::sync_loop_progress_once(
        &state,
        notifier.clone(),
        loop_id,
        "C123",
        "1780792150.138669",
        human,
        std::time::Duration::from_secs(0),
        &mut checkpoint,
    )
    .await
    .unwrap();
    state
        .add_pending_question(loop_id, "C123", "1780792150.138669", "1780800000.000100")
        .unwrap();
    ralph_slack::daemon::sync_loop_progress_once(
        &state,
        notifier.clone(),
        loop_id,
        "C123",
        "1780792150.138669",
        later,
        std::time::Duration::from_secs(11),
        &mut checkpoint,
    )
    .await
    .unwrap();

    let statuses = notifier.statuses.lock().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].2, "needs your answer");
}

#[tokio::test]
async fn stop_command_clears_assistant_status_even_after_recent_status() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    state.set_thread_process_id(loop_id, Some(4242)).unwrap();
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    notifier
        .set_assistant_thread_status("C123", "1780792150.138669", "needs your answer")
        .await
        .unwrap();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvStopClear".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "stop".to_string(),
            ts: "1780800000.000200".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();

    let statuses = notifier.statuses.lock().unwrap();
    assert_eq!(
        statuses.last().map(|(_, _, status)| status.as_str()),
        Some("")
    );
    assert_eq!(*spawner.stopped.lock().unwrap(), vec![4242]);
    assert_eq!(
        state.load_or_default().unwrap().threads[loop_id].status,
        SlackThreadStatus::Stopped
    );
}

#[tokio::test]
async fn socket_mode_ack_is_sent_before_slow_loop_spawn_work() {
    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner {
        delay_ms: 500,
        ..FakeSpawner::default()
    };
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state,
        spawner.clone(),
        FakeNotifier::default(),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(stream).await.unwrap();
        ws.send(Message::Text(
            serde_json::json!({
                "envelope_id": "EnvAckFirst",
                "payload": {
                    "event_id": "EvAckFirst",
                    "event": {
                        "type": "app_mention",
                        "channel": "C123",
                        "user": "U123",
                        "text": "<@B123> slow spawn",
                        "ts": "1780792150.138669"
                    }
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

        let started = std::time::Instant::now();
        let ack = tokio::time::timeout(std::time::Duration::from_millis(150), ws.next())
            .await
            .expect("Socket Mode ack must be sent before slow loop spawn work")
            .unwrap()
            .unwrap();
        assert!(started.elapsed() < std::time::Duration::from_millis(250));
        assert_eq!(
            ack.into_text().unwrap(),
            serde_json::json!({"envelope_id":"EnvAckFirst"}).to_string()
        );
        ws.close(None).await.unwrap();
    });

    ralph_slack::socket_mode::run_socket_mode(&url, daemon)
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(spawner.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn socket_mode_ignores_hello_envelopes_without_ack_id() {
    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state,
        spawner.clone(),
        FakeNotifier::default(),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(stream).await.unwrap();
        ws.send(Message::Text(
            serde_json::json!({"type":"hello","num_connections":1,"debug_info":{}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
        let no_ack = tokio::time::timeout(std::time::Duration::from_millis(100), ws.next()).await;
        assert!(
            no_ack.is_err(),
            "hello messages without envelope_id should not be acked"
        );
        ws.close(None).await.unwrap();
    });

    ralph_slack::socket_mode::run_socket_mode(&url, daemon)
        .await
        .unwrap();
    server.await.unwrap();
    assert!(spawner.requests.lock().unwrap().is_empty());
}

fn app_mention(event_id: &str, text: &str) -> SlackMessageEvent {
    SlackMessageEvent {
        event_id: Some(event_id.to_string()),
        channel_id: "C123".to_string(),
        user_id: Some("U123".to_string()),
        text: text.to_string(),
        ts: "1780792150.138669".to_string(),
        thread_ts: None,
        bot_id: None,
        app_mention: true,
    }
}

fn daemon_config(root: &std::path::Path) -> SlackDaemonConfig {
    SlackDaemonConfig {
        workspace_root: root.to_path_buf(),
        allowed_channels: vec!["C123".to_string()],
        allowed_users: vec!["U123".to_string()],
        repo_aliases: BTreeMap::from([("ralph".to_string(), root.to_path_buf())]),
        channel_repos: BTreeMap::from([("C123".to_string(), "ralph".to_string())]),
    }
}

#[tokio::test]
async fn fake_socket_root_event_starts_fake_loop_and_posts_thread_reply() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state,
        spawner.clone(),
        notifier.clone(),
    );

    let action = daemon
        .handle_event(app_mention("Ev1", "<@B123> build a Slack surface"))
        .await
        .unwrap();

    let HandlerAction::StartLoop {
        loop_id, prompt, ..
    } = action
    else {
        panic!("expected StartLoop action");
    };
    assert_eq!(prompt, "build a Slack surface");

    let requests = spawner.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].loop_id, loop_id);
    assert_eq!(requests[0].prompt, "build a Slack surface");
    assert_eq!(
        requests[0].env.get("RALPH_SLACK_CHANNEL_ID").unwrap(),
        "C123"
    );
    assert_eq!(
        requests[0].env.get("RALPH_SLACK_THREAD_TS").unwrap(),
        "1780792150.138669"
    );

    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].0, "C123");
    assert_eq!(messages[0].1, "1780792150.138669");
    assert!(messages[0].2.contains("Ralph loop started"));
    drop(messages);

    let statuses = notifier.statuses.lock().unwrap();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].0, "C123");
    assert_eq!(statuses[0].1, "1780792150.138669");
    assert!(statuses[0].2.starts_with("is starting loop slack-C123"));
    assert_eq!(statuses[1].2, "is working in ralph");
}

#[tokio::test]
async fn root_event_can_override_repo_alias_and_subdir_and_binds_thread_target() {
    let daemon_root = tempfile::tempdir().unwrap();
    let ralph_repo = tempfile::tempdir().unwrap();
    let tonic_repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tonic_repo.path().join("crates/ralph-slack")).unwrap();
    let state = SlackStateManager::new(daemon_root.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        SlackDaemonConfig {
            workspace_root: daemon_root.path().to_path_buf(),
            allowed_channels: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string()],
            repo_aliases: BTreeMap::from([
                ("ralph".to_string(), ralph_repo.path().to_path_buf()),
                ("tonic".to_string(), tonic_repo.path().to_path_buf()),
            ]),
            channel_repos: BTreeMap::from([("C123".to_string(), "ralph".to_string())]),
        },
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    let action = daemon
        .handle_event(app_mention(
            "EvRepoOverride",
            "<@B123> repo=tonic dir=crates/ralph-slack test status command",
        ))
        .await
        .unwrap();

    let HandlerAction::StartLoop { prompt, .. } = action else {
        panic!("expected StartLoop");
    };
    assert_eq!(prompt, "test status command");
    let request = spawner.requests.lock().unwrap()[0].clone();
    assert_eq!(
        request.workspace_root,
        tonic_repo.path().canonicalize().unwrap()
    );
    assert_eq!(
        request.state_path,
        daemon_root.path().join(".ralph/slack-state.json")
    );
    assert_eq!(
        request.env.get("RALPH_SLACK_STATE_PATH").unwrap(),
        &daemon_root
            .path()
            .join(".ralph/slack-state.json")
            .to_string_lossy()
            .to_string()
    );
    assert_eq!(request.repo_alias.as_deref(), Some("tonic"));
    assert_eq!(
        request.repo_dir.as_deref(),
        Some(std::path::Path::new("crates/ralph-slack"))
    );
    let binding = state
        .load_or_default()
        .unwrap()
        .threads
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(binding.repo_alias.as_deref(), Some("tonic"));
    assert_eq!(
        binding.repo_dir.as_deref(),
        Some(std::path::Path::new("crates/ralph-slack"))
    );

    let statuses = notifier.statuses.lock().unwrap();
    assert_eq!(statuses.len(), 2);
    assert_eq!(
        statuses[0].2,
        format!("is starting loop {} in tonic", binding.loop_id)
    );
    assert_eq!(statuses[1].2, "is working in tonic");
    for (_, _, status) in statuses.iter() {
        assert!(!status.contains("crates/ralph-slack"));
        assert!(!status.contains('/'));
        assert!(!status.contains(&tonic_repo.path().to_string_lossy().to_string()));
    }
}

#[tokio::test]
async fn in_alias_colon_start_syntax_uses_alias_and_strips_prefix_from_prompt() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state,
        spawner.clone(),
        FakeNotifier::default(),
    );

    daemon
        .handle_event(app_mention("EvInAlias", "<@B123> in ralph: fix the UX"))
        .await
        .unwrap();

    let request = spawner.requests.lock().unwrap()[0].clone();
    assert_eq!(request.prompt, "fix the UX");
    assert_eq!(request.repo_alias.as_deref(), Some("ralph"));
}

#[tokio::test]
async fn unsafe_repo_dir_is_rejected_before_thread_binding_or_spawn() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    let event = app_mention("EvBadDir", "<@B123> repo=ralph dir=../escape fix");
    let action = daemon.handle_event(event.clone()).await.unwrap();
    let duplicate = daemon.handle_event(event).await.unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    assert_eq!(duplicate, HandlerAction::Ignored);
    assert!(spawner.requests.lock().unwrap().is_empty());
    assert!(
        !state
            .load_or_default()
            .unwrap()
            .thread_to_loop
            .contains_key("C123:1780792150.138669")
    );
    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].2.contains("cannot contain"));
}

#[tokio::test]
async fn invalid_explicit_repo_alias_is_rejected_without_falling_back_to_channel_default() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    let event = app_mention("EvBadAlias", "<@B123> repo=../ralph fix");
    let action = daemon.handle_event(event.clone()).await.unwrap();
    let duplicate = daemon.handle_event(event).await.unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    assert_eq!(duplicate, HandlerAction::Ignored);
    assert!(spawner.requests.lock().unwrap().is_empty());
    assert!(
        !state
            .load_or_default()
            .unwrap()
            .thread_to_loop
            .contains_key("C123:1780792150.138669")
    );
    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].2.contains("repo alias `../ralph` is invalid"));
}

#[tokio::test]
async fn malformed_in_repo_selector_is_rejected_without_falling_back_to_channel_default() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    let event = app_mention("EvMalformedIn", "<@B123> in ralph fix this");
    let action = daemon.handle_event(event.clone()).await.unwrap();
    let duplicate = daemon.handle_event(event).await.unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    assert_eq!(duplicate, HandlerAction::Ignored);
    assert!(spawner.requests.lock().unwrap().is_empty());
    assert!(
        !state
            .load_or_default()
            .unwrap()
            .thread_to_loop
            .contains_key("C123:1780792150.138669")
    );
    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].2.contains("must include `:`"));
}

#[tokio::test]
async fn empty_repo_dir_directive_is_rejected_without_falling_back_to_channel_default() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    let event = app_mention("EvEmptyDir", "<@B123> repo=ralph dir= fix this");
    let action = daemon.handle_event(event.clone()).await.unwrap();
    let duplicate = daemon.handle_event(event).await.unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    assert_eq!(duplicate, HandlerAction::Ignored);
    assert!(spawner.requests.lock().unwrap().is_empty());
    assert!(
        !state
            .load_or_default()
            .unwrap()
            .thread_to_loop
            .contains_key("C123:1780792150.138669")
    );
    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].2.contains("repo dir cannot be empty"));
}

#[tokio::test]
async fn missing_repo_dir_is_rejected_before_thread_binding_or_spawn() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    let event = app_mention("EvMissingDir", "<@B123> repo=ralph dir=missing fix");
    let action = daemon.handle_event(event.clone()).await.unwrap();
    let duplicate = daemon.handle_event(event).await.unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    assert_eq!(duplicate, HandlerAction::Ignored);
    assert!(spawner.requests.lock().unwrap().is_empty());
    assert!(
        !state
            .load_or_default()
            .unwrap()
            .thread_to_loop
            .contains_key("C123:1780792150.138669")
    );
    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0]
            .2
            .contains("Slack repo dir `missing` is not usable")
    );
}

#[test]
fn repo_dir_validation_rejects_symlink_escape() {
    let repo = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), repo.path().join("escape")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(outside.path(), repo.path().join("escape")).unwrap();

    let err = ralph_slack::daemon::resolve_repo_target(
        &BTreeMap::from([("ralph".to_string(), repo.path().to_path_buf())]),
        Some("ralph"),
        None,
        Some(std::path::Path::new("escape")),
    )
    .unwrap_err();

    assert!(err.contains("inside the configured repo root"));
}

#[tokio::test]
async fn missing_repo_resolution_posts_human_interact_clarification_with_aliases() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        SlackDaemonConfig {
            workspace_root: temp_dir.path().to_path_buf(),
            allowed_channels: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string()],
            repo_aliases: BTreeMap::from([("ralph".to_string(), temp_dir.path().to_path_buf())]),
            channel_repos: BTreeMap::new(),
        },
        state,
        FakeSpawner::default(),
        notifier.clone(),
    );

    let action = daemon
        .handle_event(app_mention("EvClarify", "<@B123> build somewhere"))
        .await
        .unwrap();

    assert_eq!(action, HandlerAction::Ignored);
    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].2.contains("human.interact"));
    assert!(messages[0].2.contains("`ralph`"));
}

#[tokio::test]
async fn reply_event_writes_to_bound_loop_events_file_and_duplicate_is_ignored() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let worktree_ralph = temp_dir
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph");
    std::fs::create_dir_all(&worktree_ralph).unwrap();
    std::fs::write(
        worktree_ralph.join("current-events"),
        ".ralph/events-live.jsonl",
    )
    .unwrap();

    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state,
        spawner.clone(),
        notifier,
    );

    let reply = SlackMessageEvent {
        event_id: Some("Ev2".to_string()),
        channel_id: "C123".to_string(),
        user_id: Some("U123".to_string()),
        text: "please steer this way".to_string(),
        ts: "1780792160.000100".to_string(),
        thread_ts: Some("1780792150.138669".to_string()),
        bot_id: None,
        app_mention: false,
    };

    let action = daemon.handle_event(reply.clone()).await.unwrap();
    assert!(matches!(action, HandlerAction::Appended { .. }));
    let duplicate = daemon.handle_event(reply).await.unwrap();
    assert_eq!(duplicate, HandlerAction::Duplicate);
    assert!(spawner.requests.lock().unwrap().is_empty());

    let events_path = temp_dir
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph/events-live.jsonl");
    let events = std::fs::read_to_string(events_path).unwrap();
    assert!(events.contains("human.guidance"));
    assert!(events.contains("please steer this way"));
}

#[tokio::test]
async fn unauthorized_root_event_does_not_spawn_or_bind_thread() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        SlackDaemonConfig {
            workspace_root: temp_dir.path().to_path_buf(),
            allowed_channels: vec!["C999".to_string()],
            allowed_users: vec!["U123".to_string()],
            repo_aliases: BTreeMap::from([("ralph".to_string(), temp_dir.path().to_path_buf())]),
            channel_repos: BTreeMap::from([("C999".to_string(), "ralph".to_string())]),
        },
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    let action = daemon
        .handle_event(app_mention("Ev3", "<@B123> no side effects"))
        .await
        .unwrap();
    assert_eq!(action, HandlerAction::Ignored);
    assert!(spawner.requests.lock().unwrap().is_empty());
    assert!(notifier.messages.lock().unwrap().is_empty());
    assert!(state.load().unwrap().is_none());
}

#[tokio::test]
async fn root_event_resolves_repo_from_channel_mapping_and_unknown_channel_does_not_spawn() {
    let daemon_root = tempfile::tempdir().unwrap();
    let repo_root = tempfile::tempdir().unwrap();
    let state = SlackStateManager::new(daemon_root.path().join(".ralph/slack-state.json"));
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        SlackDaemonConfig {
            workspace_root: daemon_root.path().to_path_buf(),
            allowed_channels: vec!["C123".to_string(), "C999".to_string()],
            allowed_users: vec!["U123".to_string()],
            repo_aliases: BTreeMap::from([("ralph".to_string(), repo_root.path().to_path_buf())]),
            channel_repos: BTreeMap::from([("C123".to_string(), "ralph".to_string())]),
        },
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    daemon
        .handle_event(app_mention("EvRepo", "<@B123> build here"))
        .await
        .unwrap();

    let requests = spawner.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].workspace_root,
        repo_root.path().canonicalize().unwrap()
    );
    assert!(
        !requests[0].env.contains_key("RALPH_WORKSPACE_ROOT"),
        "Slack-spawned loops run from their worktree; do not force repo root via RALPH_WORKSPACE_ROOT"
    );
    drop(requests);
    let bound = state
        .load_or_default()
        .unwrap()
        .threads
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(
        bound.workspace_root,
        repo_root.path().canonicalize().unwrap()
    );

    let denied = SlackMessageEvent {
        event_id: Some("EvUnknownRepo".to_string()),
        channel_id: "C999".to_string(),
        user_id: Some("U123".to_string()),
        text: "<@B123> build nowhere".to_string(),
        ts: "1780792151.000000".to_string(),
        thread_ts: None,
        bot_id: None,
        app_mention: true,
    };
    let action = daemon.handle_event(denied).await.unwrap();
    assert_eq!(action, HandlerAction::Ignored);
    assert_eq!(spawner.requests.lock().unwrap().len(), 1);
    assert_eq!(notifier.messages.lock().unwrap().len(), 2);
    assert!(
        notifier.messages.lock().unwrap()[1]
            .2
            .contains("human.interact")
    );
    let state_after_denial = state.load_or_default().unwrap();
    assert!(
        !state_after_denial
            .thread_to_loop
            .contains_key("C999:1780792151.000000"),
        "missing repo mapping must not bind a thread"
    );
}

#[tokio::test]
async fn thread_reply_uses_persisted_repo_root_not_daemon_workspace_root() {
    let daemon_root = tempfile::tempdir().unwrap();
    let repo_root = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let worktree_ralph = repo_root
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph");
    std::fs::create_dir_all(&worktree_ralph).unwrap();
    std::fs::write(
        worktree_ralph.join("current-events"),
        ".ralph/events-live.jsonl",
    )
    .unwrap();
    let state = SlackStateManager::new(daemon_root.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            repo_root.path(),
        )
        .unwrap();
    let daemon = SlackDaemon::new(
        SlackDaemonConfig {
            workspace_root: daemon_root.path().to_path_buf(),
            allowed_channels: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string()],
            repo_aliases: BTreeMap::from([("ralph".to_string(), repo_root.path().to_path_buf())]),
            channel_repos: BTreeMap::from([("C123".to_string(), "ralph".to_string())]),
        },
        state,
        FakeSpawner::default(),
        FakeNotifier::default(),
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvPersistedRepo".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "use the bound repo".to_string(),
            ts: "1780792160.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();

    let events = std::fs::read_to_string(
        repo_root
            .path()
            .join(".worktrees")
            .join(loop_id)
            .join(".ralph/events-live.jsonl"),
    )
    .unwrap();
    assert!(events.contains("use the bound repo"));
    assert!(!daemon_root.path().join(".worktrees").join(loop_id).exists());
}

#[tokio::test]
async fn known_thread_help_status_tail_are_replies_not_guidance_and_status_wins_over_pending() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let events_dir = temp_dir
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph");
    std::fs::create_dir_all(&events_dir).unwrap();
    std::fs::write(
        events_dir.join("current-events"),
        ".ralph/events-live.jsonl",
    )
    .unwrap();
    std::fs::write(
        events_dir.join("events-live.jsonl"),
        "{\"topic\":\"checkpoint\",\"payload\":\"secret-token-1234567890 latest\"}\n",
    )
    .unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread_with_repo(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
            Some("ralph"),
            Some(std::path::Path::new("crates/ralph-slack")),
        )
        .unwrap();
    state
        .add_pending_question(loop_id, "C123", "1780792150.138669", "1780792200.000100")
        .unwrap();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        FakeSpawner::default(),
        notifier.clone(),
    );

    for (idx, text) in ["help", "repo", "status", "!tail 1"].iter().enumerate() {
        daemon
            .handle_event(SlackMessageEvent {
                event_id: Some(format!("EvCmd{idx}")),
                channel_id: "C123".to_string(),
                user_id: Some("U123".to_string()),
                text: text.to_string(),
                ts: format!("178079216{idx}.000100"),
                thread_ts: Some("1780792150.138669".to_string()),
                bot_id: None,
                app_mention: false,
            })
            .await
            .unwrap();
    }

    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 4);
    assert!(messages[0].2.contains("help"));
    assert!(messages[1].2.contains("alias: `ralph`"));
    assert!(messages[1].2.contains("dir: `crates/ralph-slack`"));
    assert!(messages[1].2.contains(loop_id));
    assert!(
        messages[1]
            .2
            .contains(&format!("branch: `ralph-slack-{loop_id}`"))
    );
    assert!(messages[2].2.contains(loop_id));
    assert!(messages[2].2.contains("pending question: yes"));
    assert!(messages[3].2.contains("checkpoint"));
    assert!(!messages[3].2.contains("secret-token-1234567890"));
    assert!(state.has_pending_question(loop_id).unwrap());
    let events = std::fs::read_to_string(events_dir.join("events-live.jsonl")).unwrap();
    assert!(!events.contains("human.guidance"));
}

#[tokio::test]
async fn obs_command_reports_loop_snapshot_with_latest_event_and_log() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let events_dir = temp_dir
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph");
    std::fs::create_dir_all(&events_dir).unwrap();
    std::fs::write(
        events_dir.join("current-events"),
        ".ralph/events-live.jsonl",
    )
    .unwrap();
    std::fs::write(
        events_dir.join("events-live.jsonl"),
        concat!(
            r#"{"ts":"2026-06-07T23:01:01.000000+00:00","iteration":5,"hat":"executor","topic":"work.start","payload":"started"}"#,
            "\n",
            r#"{"ts":"2026-06-07T23:02:02.000000+00:00","iteration":6,"hat":"reviewer","topic":"agent.message","payload":"finished tests with secret-token-1234567890"}"#,
            "\n"
        ),
    )
    .unwrap();
    let log_dir = temp_dir.path().join(".ralph/slack-loop-logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    std::fs::write(
        log_dir.join(format!("{loop_id}.log")),
        "booting\nreviewer saw xoxb-1234567890abcdef\n",
    )
    .unwrap();

    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread_with_repo(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
            Some("ralph"),
            Some(std::path::Path::new("crates/ralph-slack")),
        )
        .unwrap();
    state.set_thread_process_id(loop_id, Some(4242)).unwrap();
    state
        .set_thread_message_timestamps(
            loop_id,
            Some("1780799999.000100"),
            Some("1780799999.000200"),
            Some("1780799999.000300"),
            None,
        )
        .unwrap();
    state
        .add_pending_question(loop_id, "C123", "1780792150.138669", "1780792200.000100")
        .unwrap();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        FakeSpawner::default(),
        notifier.clone(),
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvObs".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "!obs".to_string(),
            ts: "1780792164.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();

    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    let obs = &messages[0].2;
    assert!(obs.contains("Ralph observable"));
    assert!(obs.contains(loop_id));
    assert!(obs.contains("status: `running`"));
    assert!(obs.contains("pending question: `yes`"));
    assert!(obs.contains("process id: `4242`"));
    assert!(obs.contains("repo alias: `ralph`"));
    assert!(obs.contains("repo dir: `crates/ralph-slack`"));
    assert!(obs.contains("latest event: iter `6` · hat `reviewer` · topic `agent.message`"));
    assert!(obs.contains("finished tests with [redacted]"));
    assert!(obs.contains("latest log:"));
    assert!(obs.contains("reviewer saw [redacted]"));
    assert!(!obs.contains("secret-token-1234567890"));
    assert!(!obs.contains("xoxb-1...cdef"));
    assert!(state.has_pending_question(loop_id).unwrap());
}

#[tokio::test]
async fn obs_command_on_archived_thread_reports_no_pending_and_missing_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread_with_repo(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
            Some("ralph"),
            Some(std::path::Path::new("crates/ralph-slack")),
        )
        .unwrap();
    state
        .finish_thread(loop_id, SlackThreadStatus::Completed)
        .unwrap();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state,
        FakeSpawner::default(),
        notifier.clone(),
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvObsArchived".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "observe".to_string(),
            ts: "1780792165.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();

    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    let obs = &messages[0].2;
    assert!(obs.contains("status: `completed`"));
    assert!(obs.contains("pending question: `no`"));
    assert!(obs.contains("process id: `none`"));
    assert!(obs.contains("latest event: none"));
    assert!(obs.contains("latest log: none"));
}

#[tokio::test]
async fn pending_question_non_command_routes_response_and_command_does_not_clear_pending() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    state
        .add_pending_question(loop_id, "C123", "1780792150.138669", "1780792200.000100")
        .unwrap();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        FakeSpawner::default(),
        FakeNotifier::default(),
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvStatusPending".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "status".to_string(),
            ts: "1780792160.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();
    assert!(state.has_pending_question(loop_id).unwrap());

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvResponsePending".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "the answer".to_string(),
            ts: "1780792161.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();
    assert!(!state.has_pending_question(loop_id).unwrap());
    let events = std::fs::read_to_string(
        temp_dir
            .path()
            .join(".worktrees")
            .join(loop_id)
            .join(".ralph/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("human.response"));
    assert!(events.contains("the answer"));
}

#[tokio::test]
async fn startup_reconciliation_finishes_running_thread_with_dead_process() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let events_dir = temp_dir
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph");
    std::fs::create_dir_all(&events_dir).unwrap();
    std::fs::write(
        events_dir.join("events.jsonl"),
        "{\"topic\":\"LOOP_COMPLETE\",\"payload\":\"done\"}\n",
    )
    .unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    state
        .set_thread_process_id(loop_id, Some(u32::MAX))
        .unwrap();
    state
        .add_pending_question(loop_id, "C123", "1780792150.138669", "1780792200.000100")
        .unwrap();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        FakeSpawner::default(),
        notifier.clone(),
    );

    daemon.reconcile_stale_threads().await.unwrap();

    let loaded = state.load_or_default().unwrap();
    let binding = &loaded.threads[loop_id];
    assert_eq!(binding.status, SlackThreadStatus::Completed);
    assert_eq!(binding.process_id, None);
    assert!(binding.final_card_ts.is_some());
    assert!(!loaded.pending_questions.contains_key(loop_id));
    let messages = notifier.messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].0, "C123");
    assert_eq!(messages[0].1, "1780792150.138669");
    assert!(messages[0].2.contains("completed"));
}

#[tokio::test]
async fn stale_reconciliation_updates_existing_final_card_for_failed_dead_process() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let events_dir = temp_dir
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph");
    std::fs::create_dir_all(&events_dir).unwrap();
    std::fs::write(
        events_dir.join("events.jsonl"),
        "{\"topic\":\"agent.message\",\"payload\":\"exited before completion\"}\n",
    )
    .unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    state
        .set_thread_process_id(loop_id, Some(u32::MAX))
        .unwrap();
    state
        .set_thread_message_timestamps(loop_id, None, None, None, Some("1780799999.000200"))
        .unwrap();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        FakeSpawner::default(),
        notifier.clone(),
    );

    daemon.reconcile_stale_threads().await.unwrap();

    let loaded = state.load_or_default().unwrap();
    let binding = &loaded.threads[loop_id];
    assert_eq!(binding.status, SlackThreadStatus::Failed);
    assert_eq!(binding.process_id, None);
    assert_eq!(binding.final_card_ts.as_deref(), Some("1780799999.000200"));
    assert!(notifier.messages.lock().unwrap().is_empty());
    let updates = notifier.updates.lock().unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].0, "C123");
    assert_eq!(updates[0].1, "1780799999.000200");
    assert!(updates[0].2.contains("failed"));
}

#[tokio::test]
async fn stale_reconciliation_marks_completed_from_loop_log_when_event_file_is_missing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let log_dir = temp_dir.path().join(".ralph/slack-loop-logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    std::fs::write(
        log_dir.join(format!("{loop_id}.log")),
        "Completion event detected in JSONL topic=LOOP_COMPLETE\n",
    )
    .unwrap();
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    state
        .set_thread_process_id(loop_id, Some(u32::MAX))
        .unwrap();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        FakeSpawner::default(),
        FakeNotifier::default(),
    );

    daemon.reconcile_stale_threads().await.unwrap();

    assert_eq!(
        state.load_or_default().unwrap().threads[loop_id].status,
        SlackThreadStatus::Completed
    );
}

#[tokio::test]
async fn stop_cancel_is_creator_only_marks_stopped_and_blocks_future_guidance() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    state.set_thread_process_id(loop_id, Some(4242)).unwrap();
    let spawner = FakeSpawner::default();
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        SlackDaemonConfig {
            workspace_root: temp_dir.path().to_path_buf(),
            allowed_channels: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string(), "U999".to_string()],
            repo_aliases: BTreeMap::from([("ralph".to_string(), temp_dir.path().to_path_buf())]),
            channel_repos: BTreeMap::from([("C123".to_string(), "ralph".to_string())]),
        },
        state.clone(),
        spawner.clone(),
        notifier.clone(),
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvStopDenied".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U999".to_string()),
            text: "stop".to_string(),
            ts: "1780792160.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();
    assert!(spawner.stopped.lock().unwrap().is_empty());
    assert_eq!(
        state.load_or_default().unwrap().threads[loop_id].status,
        SlackThreadStatus::Running
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvStopAllowed".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "cancel".to_string(),
            ts: "1780792161.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();
    assert_eq!(*spawner.stopped.lock().unwrap(), vec![4242]);
    assert_eq!(
        state.load_or_default().unwrap().threads[loop_id].status,
        SlackThreadStatus::Stopped
    );

    daemon
        .handle_event(SlackMessageEvent {
            event_id: Some("EvAfterStop".to_string()),
            channel_id: "C123".to_string(),
            user_id: Some("U123".to_string()),
            text: "keep going".to_string(),
            ts: "1780792162.000100".to_string(),
            thread_ts: Some("1780792150.138669".to_string()),
            bot_id: None,
            app_mention: false,
        })
        .await
        .unwrap();
    let events_path = temp_dir
        .path()
        .join(".worktrees")
        .join(loop_id)
        .join(".ralph/events.jsonl");
    assert!(!events_path.exists());
    assert!(
        notifier
            .messages
            .lock()
            .unwrap()
            .iter()
            .any(|(_, _, text)| text.contains("stopped"))
    );
}

#[tokio::test]
async fn stop_button_is_creator_only_like_typed_stop() {
    let temp_dir = tempfile::tempdir().unwrap();
    let loop_id = "slack-C123-1780792150-138669";
    let state = SlackStateManager::new(temp_dir.path().join(".ralph/slack-state.json"));
    state
        .bind_thread(
            loop_id,
            "C123",
            "1780792150.138669",
            "U123",
            temp_dir.path(),
        )
        .unwrap();
    state.set_thread_process_id(loop_id, Some(4242)).unwrap();
    let spawner = FakeSpawner::default();
    let daemon = SlackDaemon::new(
        SlackDaemonConfig {
            workspace_root: temp_dir.path().to_path_buf(),
            allowed_channels: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string(), "U999".to_string()],
            repo_aliases: BTreeMap::from([("ralph".to_string(), temp_dir.path().to_path_buf())]),
            channel_repos: BTreeMap::from([("C123".to_string(), "ralph".to_string())]),
        },
        state.clone(),
        spawner.clone(),
        FakeNotifier::default(),
    );
    let payload = serde_json::json!({
        "type": "block_actions",
        "trigger_id": "TrigStopDenied",
        "user": {"id": "U999"},
        "channel": {"id": "C123"},
        "message": {"ts": "1780792150.138669", "thread_ts": "1780792150.138669"},
        "actions": [{"action_id": "ralph_slack_stop", "value": "stop:slack-C123-1780792150-138669"}],
        "action_ts": "1780792160.000100"
    });
    let event = slack_message_event_from_payload(&payload).expect("stop button should parse");

    daemon.handle_event(event).await.unwrap();

    assert!(spawner.stopped.lock().unwrap().is_empty());
    assert_eq!(
        state.load_or_default().unwrap().threads[loop_id].status,
        SlackThreadStatus::Running
    );
}
