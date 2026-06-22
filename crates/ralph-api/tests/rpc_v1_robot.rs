use std::fs;
use std::time::Duration;

use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use ralph_api::{ApiConfig, RpcRuntime, serve_with_listener};

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

struct TestServer {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    join: tokio::task::JoinHandle<anyhow::Result<()>>,
    runtime: RpcRuntime,
    workspace: TempDir,
}

impl TestServer {
    async fn start(mut config: ApiConfig) -> Self {
        let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
        config.workspace_root = workspace.path().to_path_buf();

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let local_addr = listener
            .local_addr()
            .expect("listener local addr should exist");
        let runtime = RpcRuntime::new(config).expect("runtime should initialize");
        let runtime_handle = runtime.clone();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let join = tokio::spawn(async move {
            serve_with_listener(listener, runtime, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });

        Self {
            base_url: format!("http://{local_addr}"),
            shutdown: Some(shutdown_tx),
            join,
            runtime: runtime_handle,
            workspace,
        }
    }

    fn path(&self, relative: &str) -> std::path::PathBuf {
        self.workspace.path().join(relative)
    }

    fn ws_url(&self) -> String {
        self.base_url.replacen("http://", "ws://", 1)
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let result = self.join.await.expect("server task should join");
        result.expect("server should shutdown cleanly");
    }
}

async fn post_rpc(client: &Client, server: &TestServer, body: &Value) -> Result<(u16, Value)> {
    let response = client
        .post(format!("{}/rpc/v1", server.base_url))
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await?;

    let status = response.status().as_u16();
    let payload = response.json::<Value>().await?;
    Ok((status, payload))
}

fn rpc_request(id: &str, method: &str, params: Value, idempotency_key: Option<&str>) -> Value {
    let mut request = json!({
        "apiVersion": "v1",
        "id": id,
        "method": method,
        "params": params,
    });

    if let Some(idempotency_key) = idempotency_key {
        request["meta"] = json!({
            "idempotencyKey": idempotency_key,
        });
    }

    request
}

fn write_json(path: &std::path::Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

async fn open_stream(server: &TestServer, subscription_id: &str) -> Result<WsStream> {
    let url = format!(
        "{}/rpc/v1/stream?subscriptionId={subscription_id}",
        server.ws_url()
    );
    let (stream, _) = connect_async(url).await?;
    Ok(stream)
}

async fn recv_topic_event(stream: &mut WsStream, topic: &str) -> Value {
    loop {
        let maybe_message = timeout(Duration::from_secs(4), stream.next())
            .await
            .expect("timed out waiting for websocket message");

        let Some(message) = maybe_message else {
            panic!("websocket closed before receiving expected topic");
        };

        let message = message.expect("websocket message should be ok");
        let Message::Text(text) = message else {
            continue;
        };

        let payload: Value =
            serde_json::from_str(&text).expect("websocket event should be valid json");
        if payload["topic"] == topic {
            return payload;
        }
    }
}

#[tokio::test]
async fn robot_question_and_checkin_return_null_or_file_payload() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let question = rpc_request("robot-question-empty", "robot.question", json!({}), None);
    let (status, payload) = post_rpc(&client, &server, &question).await?;
    assert_eq!(status, 200);
    assert!(payload["result"]["question"].is_null());

    let checkin = rpc_request("robot-checkin-empty", "robot.checkin", json!({}), None);
    let (status, payload) = post_rpc(&client, &server, &checkin).await?;
    assert_eq!(status, 200);
    assert!(payload["result"]["checkin"].is_null());

    write_json(
        &server.path(".ralph/api/robot-question.json"),
        &json!({
            "id": 7,
            "loop_id": "loop-1",
            "response_token": "token-7",
            "payload": "Need review?",
            "hat": "executor",
            "iteration": 3
        }),
    )?;
    write_json(
        &server.path(".ralph/api/robot-checkin.json"),
        &json!({
            "iteration": 3,
            "elapsed_seconds": 12,
            "context": { "current_hat": "executor" }
        }),
    )?;

    let (status, payload) = post_rpc(&client, &server, &question).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["question"]["id"], 7);
    assert_eq!(payload["result"]["question"]["response_token"], "token-7");

    let (status, payload) = post_rpc(&client, &server, &checkin).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["checkin"]["iteration"], 3);

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn robot_respond_writes_web_robot_compatible_response_and_replays() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let request = rpc_request(
        "robot-respond-1",
        "robot.respond",
        json!({
            "id": 7,
            "loop_id": "loop-1",
            "response_token": "token-7",
            "response": "Approved"
        }),
        Some("idem-robot-respond-1"),
    );

    let (status, payload) = post_rpc(&client, &server, &request).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["questionId"], 7);
    assert_eq!(payload["result"]["response"], "Approved");

    let response_file: Value =
        serde_json::from_slice(&fs::read(server.path(".ralph/api/robot-response.json"))?)?;
    assert_eq!(response_file["id"], 7);
    assert_eq!(response_file["loop_id"], "loop-1");
    assert_eq!(response_file["response_token"], "token-7");
    assert_eq!(response_file["response"], "Approved");

    fs::remove_file(server.path(".ralph/api/robot-response.json"))?;
    let (replay_status, replay_payload) = post_rpc(&client, &server, &request).await?;
    assert_eq!(replay_status, status);
    assert_eq!(replay_payload, payload);
    assert!(
        !server.path(".ralph/api/robot-response.json").exists(),
        "idempotency replay should not rewrite response file"
    );

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn robot_guidance_appends_to_current_events_and_replays() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    fs::create_dir_all(server.path(".ralph"))?;
    fs::write(
        server.path(".ralph/current-events"),
        ".ralph/events-active.jsonl",
    )?;

    let request = rpc_request(
        "robot-guidance-1",
        "robot.guidance",
        json!({ "text": "Focus on cancellation" }),
        Some("idem-robot-guidance-1"),
    );

    let (status, payload) = post_rpc(&client, &server, &request).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["text"], "Focus on cancellation");

    let events = fs::read_to_string(server.path(".ralph/events-active.jsonl"))?;
    let lines = events.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let event: Value = serde_json::from_str(lines[0])?;
    assert_eq!(event["topic"], "human.guidance");
    assert_eq!(event["payload"], "Focus on cancellation");

    let (replay_status, replay_payload) = post_rpc(&client, &server, &request).await?;
    assert_eq!(replay_status, status);
    assert_eq!(replay_payload, payload);
    let replay_events = fs::read_to_string(server.path(".ralph/events-active.jsonl"))?;
    assert_eq!(
        replay_events.lines().count(),
        1,
        "idempotency replay should not append duplicate guidance"
    );

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn robot_guidance_rejects_parent_dir_current_events_marker() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    fs::create_dir_all(server.path(".ralph"))?;
    fs::write(server.path(".ralph/current-events"), "../outside.jsonl")?;

    let request = rpc_request(
        "robot-guidance-traversal",
        "robot.guidance",
        json!({ "text": "Do not escape" }),
        Some("idem-robot-guidance-traversal"),
    );

    let (status, payload) = post_rpc(&client, &server, &request).await?;
    assert_eq!(status, 400);
    assert_eq!(payload["error"]["code"], "INVALID_PARAMS");
    assert!(
        !server.path("../outside.jsonl").exists(),
        "guidance should not append outside the workspace"
    );

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn robot_methods_require_idempotency_for_mutations() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let respond = rpc_request(
        "robot-respond-no-idem",
        "robot.respond",
        json!({
            "questionId": 1,
            "loopId": "loop-1",
            "responseToken": "token-1",
            "response": "ok"
        }),
        None,
    );
    let (status, payload) = post_rpc(&client, &server, &respond).await?;
    assert_eq!(status, 400);
    assert_eq!(payload["error"]["code"], "INVALID_PARAMS");

    let guidance = rpc_request(
        "robot-guidance-no-idem",
        "robot.guidance",
        json!({ "text": "Keep going" }),
        None,
    );
    let (status, payload) = post_rpc(&client, &server, &guidance).await?;
    assert_eq!(status, 400);
    assert_eq!(payload["error"]["code"], "INVALID_PARAMS");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn robot_stream_topics_accept_rpc_side_effects_and_internal_publish() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let subscribe = rpc_request(
        "robot-stream-sub",
        "stream.subscribe",
        json!({ "topics": ["robot.response.received", "robot.question.asked"] }),
        None,
    );
    let (status, subscribe_payload) = post_rpc(&client, &server, &subscribe).await?;
    assert_eq!(status, 200);
    let subscription_id = subscribe_payload["result"]["subscriptionId"]
        .as_str()
        .unwrap();
    let mut stream = open_stream(&server, subscription_id).await?;

    let respond = rpc_request(
        "robot-stream-respond",
        "robot.respond",
        json!({
            "questionId": 9,
            "loopId": "loop-1",
            "responseToken": "token-9",
            "response": "yes"
        }),
        Some("idem-robot-stream-respond"),
    );
    let (status, response) = post_rpc(&client, &server, &respond).await?;
    assert_eq!(status, 200);
    assert_eq!(response["result"]["questionId"], 9);

    let response_event = recv_topic_event(&mut stream, "robot.response.received").await;
    assert_eq!(response_event["payload"]["question_id"], 9);
    assert_eq!(response_event["resource"]["type"], "robot");

    let publish_result = server
        .runtime
        .invoke_method(
            "robot-stream-internal",
            "_internal.publish",
            json!({
                "topic": "robot.question.asked",
                "resourceType": "robot",
                "resourceId": "9",
                "payload": {
                    "question_id": 9,
                    "payload": "Need approval?",
                    "hat": "executor",
                    "iteration": 4
                }
            }),
            "internal",
            None,
        )
        .expect("private internal publish should succeed");
    assert_eq!(publish_result["success"], true);

    let question_event = recv_topic_event(&mut stream, "robot.question.asked").await;
    assert_eq!(question_event["payload"]["question_id"], 9);
    assert_eq!(question_event["payload"]["hat"], "executor");

    stream.close(None).await?;
    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn robot_internal_publish_is_not_public_http_method() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let request = rpc_request(
        "robot-stream-internal-public",
        "_internal.publish",
        json!({
            "topic": "robot.question.asked",
            "resourceType": "robot",
            "resourceId": "9",
            "payload": {}
        }),
        None,
    );
    let (status, payload) = post_rpc(&client, &server, &request).await?;

    assert_eq!(status, 404);
    assert_eq!(payload["error"]["code"], "METHOD_NOT_FOUND");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn robot_internal_publish_rejects_malformed_private_requests() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;

    let cases = [
        json!({
            "topic": "robot.question.asked",
            "resourceType": "robot",
            "resourceId": "9",
            "payload": "not-object"
        }),
        json!({
            "topic": "robot.unknown",
            "resourceType": "robot",
            "resourceId": "9",
            "payload": {}
        }),
        json!({
            "topic": "robot.question.asked",
            "resourceType": "robot",
            "resourceId": "9",
            "payload": {},
            "extra": true
        }),
    ];

    for (index, params) in cases.into_iter().enumerate() {
        let error = server
            .runtime
            .invoke_method(
                format!("robot-stream-internal-invalid-{index}"),
                "_internal.publish",
                params,
                "internal",
                None,
            )
            .expect_err("malformed private publish should fail");
        assert_eq!(error.code.as_str(), "INVALID_PARAMS");
    }

    server.stop().await;
    Ok(())
}
