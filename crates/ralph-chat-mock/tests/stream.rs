//! The mock speaks the wire the page speaks: send, stream, cancel.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ralph_chat_mock::{AppState, MockConfig};
use serde_json::{Value, json};
use tokio::net::TcpListener;

async fn spawn(delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new(MockConfig {
        chunk_delay: delay,
        dist: None,
    });
    tokio::spawn(async move {
        ralph_chat_mock::serve(listener, state).await.unwrap();
    });
    let client = client();
    let url = format!("http://{addr}/events");
    for _ in 0..50 {
        if client
            .get(&url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .send()
            .await
            .is_ok()
        {
            return addr.to_string();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("mock did not accept connections on {addr}");
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap()
}

async fn open_events(addr: &str) -> reqwest::Response {
    client()
        .get(format!("http://{addr}/events"))
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .send()
        .await
        .unwrap()
}

async fn post(addr: &str, body: Value) -> Value {
    let response = client()
        .post(format!("http://{addr}/commands"))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    assert!(status.is_success(), "{status} {text}");
    serde_json::from_str(&text).unwrap()
}

async fn read_until(response: &mut reqwest::Response, pred: impl Fn(&str) -> bool) -> String {
    let mut buf = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if pred(&buf) {
            return buf;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "timed out, saw: {buf}");
        match tokio::time::timeout(remaining, response.chunk()).await {
            Ok(Ok(Some(chunk))) => buf.push_str(&String::from_utf8_lossy(&chunk)),
            Ok(Ok(None)) => panic!("stream ended, saw: {buf}"),
            Ok(Err(err)) => panic!("{err}, saw: {buf}"),
            Err(elapsed) => panic!("{elapsed}, saw: {buf}"),
        }
    }
}

fn collect(mut response: reqwest::Response) -> Arc<Mutex<String>> {
    let buf = Arc::new(Mutex::new(String::new()));
    let write = Arc::clone(&buf);
    tokio::spawn(async move {
        while let Ok(Some(chunk)) = response.chunk().await {
            write
                .lock()
                .expect("buffer")
                .push_str(&String::from_utf8_lossy(&chunk));
        }
    });
    buf
}

#[tokio::test]
async fn send_streams_then_cancel_aborts_the_run() {
    let addr = spawn(Duration::from_millis(120)).await;
    let mut events = open_events(&addr).await;
    let snapshot = read_until(&mut events, |buf| buf.contains("\"type\":\"snapshot\"")).await;
    assert!(snapshot.contains("mock-root"));
    assert!(snapshot.contains("\"model\":\"mock\""));

    let ack = post(
        &addr,
        json!({
            "id": "c1",
            "command": { "type": "submit", "text": "Add a header", "requestId": "req-1" }
        }),
    )
    .await;
    assert_eq!(ack["ok"], true);
    assert_eq!(ack["id"], "c1");
    assert_eq!(ack["data"]["type"], "ack");

    let streamed = read_until(&mut events, |buf| buf.contains("\"type\":\"text_delta\"")).await;
    assert!(streamed.contains("mock"));

    let aborted = post(&addr, json!({ "id": "c2", "command": { "type": "abort" } })).await;
    assert_eq!(aborted["ok"], true);

    let end = read_until(&mut events, |buf| buf.contains("\"status\":\"aborted\"")).await;
    assert!(end.contains("run_end"));
    assert!(!end.contains("\"status\":\"done\""));
}

#[tokio::test]
async fn repeated_request_id_does_not_start_a_second_run() {
    let addr = spawn(Duration::from_millis(80)).await;
    let buf = collect(open_events(&addr).await);
    let body = json!({
        "id": "c1",
        "command": { "type": "submit", "text": "once", "requestId": "same" }
    });
    assert_eq!(post(&addr, body.clone()).await["ok"], true);
    let again = json!({
        "id": "c2",
        "command": { "type": "submit", "text": "different", "requestId": "same" }
    });
    let replay = post(&addr, again).await;
    assert_eq!(replay["id"], "c2");
    assert_eq!(replay["ok"], true);

    tokio::time::sleep(Duration::from_millis(250)).await;
    let text = buf.lock().expect("buffer").clone();
    assert_eq!(text.matches("\"type\":\"run_start\"").count(), 1, "{text}");
}

#[tokio::test]
async fn wake_prefix_is_rejected() {
    let addr = spawn(Duration::from_millis(10)).await;
    let response = client()
        .post(format!("http://{addr}/commands"))
        .json(&json!({
            "id": "c1",
            "command": { "type": "submit", "text": "no", "requestId": "wake:1" }
        }))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "invalid");
}

#[tokio::test]
async fn steer_is_taken_at_the_next_chunk() {
    let addr = spawn(Duration::from_millis(80)).await;
    let mut events = open_events(&addr).await;
    let _ = read_until(&mut events, |buf| buf.contains("snapshot")).await;
    let _ = post(
        &addr,
        json!({
            "id": "c1",
            "command": { "type": "submit", "text": "go", "requestId": "run" }
        }),
    )
    .await;
    let _ = read_until(&mut events, |buf| buf.contains("text_delta")).await;
    let ack = post(
        &addr,
        json!({
            "id": "c2",
            "command": {
                "type": "submit",
                "text": "shorter",
                "requestId": "steer-1",
                "whenBusy": "steer"
            }
        }),
    )
    .await;
    assert_eq!(ack["ok"], true);
    let folded = read_until(&mut events, |buf| buf.contains("[steer] shorter")).await;
    assert!(folded.contains("inbox_update"), "{folded}");
}
