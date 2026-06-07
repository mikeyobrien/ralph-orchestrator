use std::io::Write;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;

use ralph_proto::RobotService;
use ralph_slack::{SlackApi, SlackService};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

struct CapturedRequest {
    path: String,
    headers: String,
    body: String,
}

async fn run_http_double(
    responses: Vec<&'static str>,
) -> (String, mpsc::Receiver<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel(8);
    let responses: Vec<String> = responses.into_iter().map(ToString::to_string).collect();

    tokio::spawn(async move {
        for response in responses {
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

    (format!("http://{}", addr), rx)
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
async fn slack_api_surfaces_slack_error_payloads() {
    let (base_url, _requests) =
        run_http_double(vec![r#"{"ok":false,"error":"channel_not_found"}"#]).await;
    let api = SlackApi::new("bot-token".to_string(), Some(base_url));

    let err = api.post_message("C404", None, "hello").await.unwrap_err();

    assert!(err.to_string().contains("channel_not_found"));
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
