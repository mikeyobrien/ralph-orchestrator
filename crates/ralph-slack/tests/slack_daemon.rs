use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ralph_slack::HandlerAction;
use ralph_slack::daemon::{
    LoopSpawner, SlackDaemon, SlackDaemonConfig, StartLoopRequest, ThreadNotifier,
};
use ralph_slack::handler::SlackMessageEvent;
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
        channel_repos: BTreeMap::from([("C123".to_string(), root.to_path_buf())]),
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
            channel_repos: BTreeMap::from([("C999".to_string(), temp_dir.path().to_path_buf())]),
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
            channel_repos: BTreeMap::from([("C123".to_string(), repo_root.path().to_path_buf())]),
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
    assert_eq!(requests[0].workspace_root, repo_root.path());
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
    assert_eq!(bound.workspace_root, repo_root.path());

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
            .contains("not configured")
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
            channel_repos: BTreeMap::from([("C123".to_string(), repo_root.path().to_path_buf())]),
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
    let notifier = FakeNotifier::default();
    let daemon = SlackDaemon::new(
        daemon_config(temp_dir.path()),
        state.clone(),
        FakeSpawner::default(),
        notifier.clone(),
    );

    for (idx, text) in ["help", "status", "!tail 1"].iter().enumerate() {
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
    assert_eq!(messages.len(), 3);
    assert!(messages[0].2.contains("help"));
    assert!(messages[1].2.contains(loop_id));
    assert!(messages[1].2.contains("pending question: yes"));
    assert!(messages[2].2.contains("checkpoint"));
    assert!(!messages[2].2.contains("secret-token-1234567890"));
    assert!(state.has_pending_question(loop_id).unwrap());
    let events = std::fs::read_to_string(events_dir.join("events-live.jsonl")).unwrap();
    assert!(!events.contains("human.guidance"));
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
            channel_repos: BTreeMap::from([("C123".to_string(), temp_dir.path().to_path_buf())]),
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
