use std::io::Write;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;

use ralph_proto::RobotService;
use ralph_slack::{SlackApi, SlackBlocks, SlackError, SlackService, SlackStreamChunk};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

struct CapturedRequest {
    path: String,
    headers: String,
    body: String,
}

async fn run_http_double<I, S>(responses: I) -> (String, mpsc::Receiver<CapturedRequest>)
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    let response_base_url = base_url.clone();
    let (tx, rx) = mpsc::channel(8);
    let responses: Vec<String> = responses.into_iter().map(Into::into).collect();

    tokio::spawn(async move {
        for response in responses {
            let response = response.replace("__BASE_URL__", &response_base_url);
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0; 8192];
            let n = stream.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
            let path = head
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_string();
            tx.send(CapturedRequest {
                path,
                headers: head.to_string(),
                body: body.to_string(),
            })
            .await
            .unwrap();
            let http = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                response.len(),
                response
            );
            stream.write_all(http.as_bytes()).await.unwrap();
        }
    });

    (base_url, rx)
}

#[tokio::test]
async fn post_message_sends_expected_slack_payload_and_auth_header() {
    let (base_url, mut requests) =
        run_http_double(vec![r#"{"ok":true,"ts":"1780792160.000100"}"#]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    let ts = api
        .post_message("C123", Some("1780792150.138669"), "hello slack")
        .await
        .unwrap();

    assert_eq!(ts, "1780792160.000100");
    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/chat.postMessage");
    assert!(request.headers.to_lowercase().contains("authorization"));
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel"], "C123");
    assert_eq!(body["thread_ts"], "1780792150.138669");
    assert_eq!(body["text"], "hello slack");
}

#[tokio::test]
async fn post_blocks_sends_block_kit_payload_with_plain_text_fallback() {
    let (base_url, mut requests) =
        run_http_double(vec![r#"{"ok":true,"ts":"1780792160.000200"}"#]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));
    let blocks = SlackBlocks::start_card(
        "slack-C123-1780792150-138669",
        "polish Slack output",
        Some("/repo/ralph"),
        Some("feat/slack-thread-surface"),
    );

    let ts = api
        .post_blocks("C123", Some("1780792150.138669"), &blocks)
        .await
        .unwrap();

    assert_eq!(ts, "1780792160.000200");
    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/chat.postMessage");
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel"], "C123");
    assert_eq!(body["thread_ts"], "1780792150.138669");
    assert!(
        body["text"]
            .as_str()
            .unwrap()
            .contains("Ralph loop started")
    );
    assert_eq!(body["blocks"][0]["type"], "header");
    assert_eq!(body["blocks"][1]["type"], "context");
    assert!(body["blocks"].as_array().unwrap().iter().any(|block| {
        block["type"] == "actions"
            && block["elements"].as_array().unwrap().iter().any(|element| {
                element["action_id"] == "ralph_slack_status"
                    && element["value"] == "slack-C123-1780792150-138669"
            })
    }));
}

#[tokio::test]
async fn update_blocks_sends_chat_update_with_existing_message_ts() {
    let (base_url, mut requests) = run_http_double(vec!["{\"ok\":true}"]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));
    let blocks = SlackBlocks::progress_card(
        "slack-C123-1780792150-138669",
        Some(2),
        Some("executor"),
        "agent.message",
        "working on Slack UX",
        Some(12),
    );

    api.update_blocks("C123", "1780792160.000300", &blocks)
        .await
        .unwrap();

    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/chat.update");
    assert!(request.headers.to_lowercase().contains("authorization"));
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel"], "C123");
    assert_eq!(body["ts"], "1780792160.000300");
    assert!(body["thread_ts"].is_null());
    assert!(body["text"].as_str().unwrap().contains("Ralph update"));
    assert_eq!(body["blocks"][0]["type"], "header");
}

#[tokio::test]
async fn set_assistant_thread_status_sends_status_payload_and_auth_header() {
    let (base_url, mut requests) = run_http_double(vec!["{\"ok\":true}"]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    api.set_assistant_thread_status("C123", "1780792150.138669", "is working in ralph")
        .await
        .unwrap();

    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/assistant.threads.setStatus");
    assert!(request.headers.to_lowercase().contains("authorization"));
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel_id"], "C123");
    assert_eq!(body["thread_ts"], "1780792150.138669");
    assert_eq!(body["status"], "is working in ralph");
}

#[tokio::test]
async fn set_assistant_thread_status_clears_with_empty_status() {
    let (base_url, mut requests) = run_http_double(vec!["{\"ok\":true}"]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    api.set_assistant_thread_status("C123", "1780792150.138669", "")
        .await
        .unwrap();

    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/assistant.threads.setStatus");
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel_id"], "C123");
    assert_eq!(body["thread_ts"], "1780792150.138669");
    assert_eq!(body["status"], "");
}

#[tokio::test]
async fn slack_api_surfaces_slack_error_payloads() {
    let (base_url, _requests) =
        run_http_double(vec![r#"{"ok":false,"error":"channel_not_found"}"#]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    let err = api.post_message("C404", None, "hello").await.unwrap_err();

    assert!(err.to_string().contains("channel_not_found"));
}

#[tokio::test]
async fn start_stream_sends_markdown_and_task_update_chunks_to_thread() {
    let (base_url, mut requests) = run_http_double(vec![
        serde_json::json!({"ok": true, "ts": "1780792160.000400"}).to_string(),
    ])
    .await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    let ts = api
        .start_stream(
            "C123",
            "1780792150.138669",
            Some("U123"),
            Some("T123"),
            Some("*Starting Ralph loop*"),
            vec![SlackStreamChunk::task_update(
                "slack-C123-1780792150-138669",
                "Run Ralph loop",
                "in_progress",
                Some("executor is inspecting the repo"),
                None,
            )],
        )
        .await
        .unwrap();

    assert_eq!(ts, "1780792160.000400");
    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/chat.startStream");
    assert!(request.headers.to_lowercase().contains("authorization"));
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel"], "C123");
    assert_eq!(body["thread_ts"], "1780792150.138669");
    assert_eq!(body["recipient_user_id"], "U123");
    assert_eq!(body["recipient_team_id"], "T123");
    assert_eq!(body["markdown_text"], "*Starting Ralph loop*");
    assert_eq!(body["task_display_mode"], "timeline");
    assert_eq!(body["chunks"][0]["type"], "task_update");
    assert_eq!(body["chunks"][0]["status"], "in_progress");
}

#[tokio::test]
async fn append_stream_sends_markdown_text_and_chunks_to_stream_ts() {
    let (base_url, mut requests) =
        run_http_double(vec![serde_json::json!({"ok": true}).to_string()]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    api.append_stream(
        "C123",
        "1780792160.000400",
        "Loop made progress",
        vec![SlackStreamChunk::markdown_text("*executor* finished tests")],
    )
    .await
    .unwrap();

    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/chat.appendStream");
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel"], "C123");
    assert_eq!(body["ts"], "1780792160.000400");
    assert_eq!(body["markdown_text"], "Loop made progress");
    assert_eq!(body["chunks"][0]["type"], "markdown_text");
    assert_eq!(body["chunks"][0]["text"], "*executor* finished tests");
}

#[tokio::test]
async fn stop_stream_sends_final_markdown_blocks_and_metadata() {
    let (base_url, mut requests) =
        run_http_double(vec![serde_json::json!({"ok": true}).to_string()]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));
    let final_blocks = SlackBlocks::help_card();

    api.stop_stream(
        "C123",
        "1780792160.000400",
        Some("Loop complete"),
        vec![SlackStreamChunk::task_update(
            "final-review",
            "Final review",
            "complete",
            None,
            Some("accepted"),
        )],
        Some(final_blocks.blocks.clone()),
        Some(serde_json::json!({
            "event_type": "ralph_loop_completed",
            "event_payload": {"loop_id": "slack-C123-1780792150-138669"}
        })),
    )
    .await
    .unwrap();

    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/chat.stopStream");
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["channel"], "C123");
    assert_eq!(body["ts"], "1780792160.000400");
    assert_eq!(body["markdown_text"], "Loop complete");
    assert_eq!(body["chunks"][0]["type"], "task_update");
    assert_eq!(body["blocks"][0]["type"], "header");
    assert_eq!(body["metadata"]["event_type"], "ralph_loop_completed");
}

#[tokio::test]
async fn streaming_api_errors_can_be_classified_for_update_card_fallback() {
    let (base_url, _requests) = run_http_double(vec![
        serde_json::json!({"ok": false, "error": "unsupported_method"}).to_string(),
    ])
    .await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    let err = api
        .append_stream("C123", "1780792160.000400", "hello", vec![])
        .await
        .unwrap_err();

    assert!(SlackApi::is_streaming_fallback_error(&err));
    assert!(SlackApi::is_streaming_fallback_error(&SlackError::Api(
        "missing_scope".to_string()
    )));
    assert!(!SlackApi::is_streaming_fallback_error(&SlackError::Api(
        "channel_not_found".to_string()
    )));
}

#[tokio::test]
async fn open_socket_mode_url_posts_to_apps_connections_open_with_app_token() {
    let (base_url, mut requests) =
        run_http_double(vec![r#"{"ok":true,"url":"wss://socket.slack.test/abc"}"#]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    let url = api.open_socket_mode_url("app-token").await.unwrap();

    assert_eq!(url, "wss://socket.slack.test/abc");
    let request = requests.recv().await.unwrap();
    assert_eq!(request.path, "/api/apps.connections.open");
    assert!(request.headers.to_lowercase().contains("authorization"));
}

#[tokio::test]
async fn upload_file_external_uses_current_slack_external_upload_flow() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("artifact.txt");
    std::fs::write(&file_path, "hello artifact").unwrap();
    let (base_url, mut requests) = run_http_double(vec![
        r#"{"ok":true,"upload_url":"__BASE_URL__/upload/F123","file_id":"F123"}"#,
        r"{}",
        r#"{"ok":true}"#,
    ])
    .await;
    let upload_url = format!("{base_url}/upload/F123");
    let api = SlackApi::new("bot-token".to_string(), Some(base_url.clone()));

    api.upload_file_external(
        "C123",
        "1780792150.138669",
        &file_path,
        "artifact.txt",
        14,
        Some("Artifact caption"),
    )
    .await
    .unwrap();

    let get_url = requests.recv().await.unwrap();
    assert_eq!(get_url.path, "/api/files.getUploadURLExternal");
    assert!(get_url.headers.to_lowercase().contains("authorization"));
    assert!(get_url.body.contains("filename=artifact.txt"));
    assert!(get_url.body.contains("length=14"));

    let upload = requests.recv().await.unwrap();
    assert_eq!(upload.path, "/upload/F123");
    assert!(!upload.headers.to_lowercase().contains("authorization"));
    assert_eq!(upload.body, "hello artifact");
    assert_eq!(upload_url, format!("{base_url}/upload/F123"));

    let complete = requests.recv().await.unwrap();
    assert_eq!(complete.path, "/api/files.completeUploadExternal");
    let body: serde_json::Value = serde_json::from_str(&complete.body).unwrap();
    assert_eq!(body["channel_id"], "C123");
    assert_eq!(body["thread_ts"], "1780792150.138669");
    assert_eq!(body["files"][0]["id"], "F123");
    assert_eq!(body["files"][0]["title"], "Artifact caption");
}

#[test]
fn slack_service_waits_for_human_response_and_removes_pending_question() {
    let dir = TempDir::new().unwrap();
    let events_path = dir.path().join(".ralph/events.jsonl");
    std::fs::create_dir_all(events_path.parent().unwrap()).unwrap();
    std::fs::write(&events_path, "").unwrap();

    let service = SlackService::new(
        dir.path().to_path_buf(),
        Some("bot-token".to_string()),
        5,
        "slack-C123-1780792150-138669".to_string(),
        "C123".to_string(),
        "1780792150.138669".to_string(),
        Some("http://127.0.0.1:9".to_string()),
    )
    .unwrap();
    service
        .state_manager()
        .add_pending_question(
            "slack-C123-1780792150-138669",
            "C123",
            "1780792150.138669",
            "1780792160.000100",
        )
        .unwrap();

    let writer_path = events_path.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&writer_path)
            .unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({"topic":"human.response","payload":"ship it","ts":"2026-06-06T19:30:27Z"})
        )
        .unwrap();
    });

    let response = service.wait_for_response(Path::new(&events_path)).unwrap();

    assert_eq!(response, Some("ship it".to_string()));
    let state = service.state_manager().load_or_default().unwrap();
    assert!(
        !state
            .pending_questions
            .contains_key("slack-C123-1780792150-138669")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn slack_service_posts_question_and_checkin_to_bound_thread() {
    let (base_url, mut requests) = run_http_double(vec![
        r#"{"ok":true,"ts":"1780792160.000100"}"#,
        r#"{"ok":true,"ts":"1780792170.000100"}"#,
    ])
    .await;
    let dir = TempDir::new().unwrap();
    let service = SlackService::new(
        dir.path().to_path_buf(),
        Some("bot-token".to_string()),
        5,
        "slack-C123-1780792150-138669".to_string(),
        "C123".to_string(),
        "1780792150.138669".to_string(),
        Some(base_url),
    )
    .unwrap();

    let question_id = tokio::task::block_in_place(|| service.send_question("Proceed?")).unwrap();
    let checkin_id =
        tokio::task::block_in_place(|| service.send_checkin(3, Duration::from_secs(125), None))
            .unwrap();

    assert_eq!(question_id, 1);
    assert_eq!(checkin_id, 1);
    let question = requests.recv().await.unwrap();
    let question_body: serde_json::Value = serde_json::from_str(&question.body).unwrap();
    assert_eq!(question_body["channel"], "C123");
    assert_eq!(question_body["thread_ts"], "1780792150.138669");
    assert_eq!(question_body["text"], "Proceed?");
    let checkin = requests.recv().await.unwrap();
    let checkin_body: serde_json::Value = serde_json::from_str(&checkin.body).unwrap();
    assert!(
        checkin_body["text"]
            .as_str()
            .unwrap()
            .contains("iteration 3")
    );

    let state = service.state_manager().load_or_default().unwrap();
    assert_eq!(
        state.pending_questions["slack-C123-1780792150-138669"].message_ts,
        "1780792160.000100"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn slack_service_can_write_pending_questions_to_shared_state_path() {
    let (base_url, mut requests) =
        run_http_double(vec![r#"{"ok":true,"ts":"1780792160.000100"}"#]).await;
    let worktree = TempDir::new().unwrap();
    let root = TempDir::new().unwrap();
    let shared_state = root.path().join(".ralph/slack-state.json");
    let service = SlackService::new_with_state_path(
        worktree.path().to_path_buf(),
        Some("bot-token".to_string()),
        5,
        "slack-C123-1780792150-138669".to_string(),
        "C123".to_string(),
        "1780792150.138669".to_string(),
        Some(base_url),
        Some(shared_state.clone()),
    )
    .unwrap();

    tokio::task::block_in_place(|| service.send_question("Proceed?")).unwrap();
    let _question = requests.recv().await.unwrap();

    assert!(!worktree.path().join(".ralph/slack-state.json").exists());
    let state = ralph_slack::SlackStateManager::new(shared_state)
        .load_or_default()
        .unwrap();
    assert!(
        state
            .pending_questions
            .contains_key("slack-C123-1780792150-138669")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn slack_service_uploads_files_only_to_bound_thread_and_workspace_paths() {
    let (base_url, mut requests) = run_http_double(vec![
        r#"{"ok":true,"upload_url":"__BASE_URL__/upload/F123","file_id":"F123"}"#,
        r"{}",
        r#"{"ok":true}"#,
    ])
    .await;
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("artifact.txt");
    std::fs::write(&file_path, "loop-local artifact").unwrap();
    let service = SlackService::new(
        dir.path().to_path_buf(),
        Some("bot-token".to_string()),
        5,
        "slack-C123-1780792150-138669".to_string(),
        "C123".to_string(),
        "1780792150.138669".to_string(),
        Some(base_url),
    )
    .unwrap();

    let sent =
        tokio::task::block_in_place(|| service.send_file(&file_path, Some("caption"))).unwrap();

    assert_eq!(sent, 1);
    let _get_url = requests.recv().await.unwrap();
    let _upload = requests.recv().await.unwrap();
    let complete = requests.recv().await.unwrap();
    let body: serde_json::Value = serde_json::from_str(&complete.body).unwrap();
    assert_eq!(body["channel_id"], "C123");
    assert_eq!(body["thread_ts"], "1780792150.138669");
}

#[test]
fn slack_service_rejects_non_workspace_files_and_redacts_token_debug() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let outside_file = outside.path().join("secret.txt");
    std::fs::write(&outside_file, "do not upload").unwrap();
    let service = SlackService::new(
        dir.path().to_path_buf(),
        Some("bot-token-secret".to_string()),
        5,
        "slack-C123-1780792150-138669".to_string(),
        "C123".to_string(),
        "1780792150.138669".to_string(),
        Some("http://127.0.0.1:9".to_string()),
    )
    .unwrap();

    let err = service.send_file(&outside_file, None).unwrap_err();
    assert!(err.to_string().contains("outside workspace"));
    let debug = format!("{service:?}");
    assert!(!debug.contains("bot-token-secret"));
}

#[test]
fn stop_sets_shutdown_flag() {
    let dir = TempDir::new().unwrap();
    let service = SlackService::new(
        dir.path().to_path_buf(),
        Some("bot-token".to_string()),
        5,
        "slack-C123-1780792150-138669".to_string(),
        "C123".to_string(),
        "1780792150.138669".to_string(),
        Some("http://127.0.0.1:9".to_string()),
    )
    .unwrap();
    let flag = service.shutdown_flag();

    Box::new(service).stop();

    assert!(flag.load(Ordering::Relaxed));
}
