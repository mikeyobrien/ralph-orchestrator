use anyhow::Result;
use reqwest::Client;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use ralph_api::{ApiConfig, AuthMode, RpcRuntime, serve_with_listener};

struct TestServer {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    join: tokio::task::JoinHandle<anyhow::Result<()>>,
    _workspace: TempDir,
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
            _workspace: workspace,
        }
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let result = self.join.await.expect("server task should join");
        result.expect("server should shutdown cleanly");
    }
}

async fn post_rpc(
    client: &Client,
    server: &TestServer,
    body: &Value,
    token: Option<&str>,
) -> Result<(u16, Value)> {
    let mut request = client
        .post(format!("{}/rpc/v1", server.base_url))
        .header("content-type", "application/json")
        .json(body);

    if let Some(token) = token {
        request = request.bearer_auth(token);
    }

    let response = request.send().await?;
    let status = response.status().as_u16();
    let payload = response.json::<Value>().await?;
    Ok((status, payload))
}

#[tokio::test]
async fn returns_invalid_params_for_schema_violations() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let request = json!({
        "apiVersion": "v1",
        "id": "req-invalid-1",
        "method": "system.health",
        "params": {
            "unexpected": true
        }
    });

    let (status, payload) = post_rpc(&client, &server, &request, None).await?;

    assert_eq!(status, 400);
    assert_eq!(payload["error"]["code"], "INVALID_PARAMS");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn returns_method_not_found_for_unknown_methods() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let request = json!({
        "apiVersion": "v1",
        "id": "req-unknown-1",
        "method": "system.not_real",
        "params": {}
    });

    let (status, payload) = post_rpc(&client, &server, &request, None).await?;

    assert_eq!(status, 404);
    assert_eq!(payload["error"]["code"], "METHOD_NOT_FOUND");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn rejects_requests_when_token_auth_is_enabled() -> Result<()> {
    let mut config = ApiConfig::default();
    config.auth_mode = AuthMode::Token;
    config.token = Some("super-secret-token".to_string());

    let server = TestServer::start(config).await;
    let client = Client::new();

    let request = json!({
        "apiVersion": "v1",
        "id": "req-auth-1",
        "method": "system.health",
        "params": {}
    });

    let (status, payload) = post_rpc(&client, &server, &request, None).await?;

    assert_eq!(status, 401);
    assert_eq!(payload["error"]["code"], "UNAUTHORIZED");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn deduplicates_mutating_requests_by_idempotency_key() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let first_request = json!({
        "apiVersion": "v1",
        "id": "req-idem-1",
        "method": "task.create",
        "params": {
            "id": "task-idem-1",
            "title": "bootstrap task"
        },
        "meta": {
            "idempotencyKey": "idem-12345678"
        }
    });

    let (first_status, first_payload) = post_rpc(&client, &server, &first_request, None).await?;
    let (second_status, second_payload) = post_rpc(&client, &server, &first_request, None).await?;

    assert_eq!(first_status, 200);
    assert_eq!(second_status, first_status);
    assert_eq!(first_payload, second_payload);
    assert_eq!(first_payload["result"]["task"]["id"], "task-idem-1");

    let conflicting_request = json!({
        "apiVersion": "v1",
        "id": "req-idem-2",
        "method": "task.create",
        "params": {
            "id": "task-idem-1",
            "title": "different title"
        },
        "meta": {
            "idempotencyKey": "idem-12345678"
        }
    });

    let (conflict_status, conflict_payload) =
        post_rpc(&client, &server, &conflicting_request, None).await?;

    assert_eq!(conflict_status, 409);
    assert_eq!(conflict_payload["error"]["code"], "IDEMPOTENCY_CONFLICT");

    server.stop().await;
    Ok(())
}
