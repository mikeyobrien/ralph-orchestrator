use anyhow::Result;
use reqwest::Client;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use ralph_api::{ApiConfig, RpcRuntime, serve_with_listener};

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

#[tokio::test]
async fn system_version_returns_expected_shape() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let request = rpc_request("sys-ver-1", "system.version", json!({}), None);
    let (status, payload) = post_rpc(&client, &server, &request).await?;

    assert_eq!(status, 200);
    assert_eq!(payload["result"]["apiVersion"], "v1");
    assert!(payload["result"]["serverVersion"].is_string());

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn system_capabilities_returns_expected_shape() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let request = rpc_request("sys-cap-1", "system.capabilities", json!({}), None);
    let (status, payload) = post_rpc(&client, &server, &request).await?;

    assert_eq!(status, 200);
    assert!(payload["result"]["methods"].is_array());
    assert!(payload["result"]["streamTopics"].is_array());
    assert!(payload["result"]["auth"]["mode"].is_string());
    assert!(payload["result"]["auth"]["supportedModes"].is_array());

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_unarchive_restores_archived_task() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create = rpc_request(
        "unarch-create",
        "task.create",
        json!({
            "id": "task-unarchive-test",
            "title": "Task to unarchive",
            "status": "open",
            "priority": 1,
            "autoExecute": false
        }),
        Some("idem-unarch-create"),
    );
    let (status, _) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);

    let archive = rpc_request(
        "unarch-arc",
        "task.archive",
        json!({ "id": "task-unarchive-test" }),
        Some("idem-unarch-arc"),
    );
    let (status, _) = post_rpc(&client, &server, &archive).await?;
    assert_eq!(status, 200);

    let unarchive = rpc_request(
        "unarch-unarc",
        "task.unarchive",
        json!({ "id": "task-unarchive-test" }),
        Some("idem-unarch-unarc"),
    );
    let (status, payload) = post_rpc(&client, &server, &unarchive).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["task"]["id"], "task-unarchive-test");

    // Verify it appears in regular list now
    let list = rpc_request("unarch-list", "task.list", json!({}), None);
    let (status, payload) = post_rpc(&client, &server, &list).await?;
    assert_eq!(status, 200);
    let listed = payload["result"]["tasks"].as_array().unwrap();
    assert!(listed.iter().any(|t| t["id"] == "task-unarchive-test"));

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_clear_removes_all_tasks() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create1 = rpc_request(
        "clr-create-1",
        "task.create",
        json!({ "id": "task-clear-open", "title": "Open", "status": "open", "priority": 1, "autoExecute": false }),
        Some("idem-clr-create-1"),
    );
    let (status, _) = post_rpc(&client, &server, &create1).await?;
    assert_eq!(status, 200);

    let create2 = rpc_request(
        "clr-create-2",
        "task.create",
        json!({ "id": "task-clear-done", "title": "Done", "status": "done", "priority": 1, "autoExecute": false }),
        Some("idem-clr-create-2"),
    );
    let (status, _) = post_rpc(&client, &server, &create2).await?;
    assert_eq!(status, 200);

    let create3 = rpc_request(
        "clr-create-3",
        "task.create",
        json!({ "id": "task-clear-closed", "title": "Closed", "status": "closed", "priority": 1, "autoExecute": false }),
        Some("idem-clr-create-3"),
    );
    let (status, _) = post_rpc(&client, &server, &create3).await?;
    assert_eq!(status, 200);

    let clear = rpc_request("clr-clear", "task.clear", json!({}), Some("idem-clr-clear"));
    let (status, payload) = post_rpc(&client, &server, &clear).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["success"], true);

    let list = rpc_request(
        "clr-list",
        "task.list",
        json!({ "includeArchived": true }),
        None,
    );
    let (status, payload) = post_rpc(&client, &server, &list).await?;
    assert_eq!(status, 200);
    let listed = payload["result"]["tasks"].as_array().unwrap();
    assert!(listed.is_empty(), "task.clear should remove all tasks");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_run_all_processes_queued_or_open_tasks() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create = rpc_request(
        "ra-create",
        "task.create",
        json!({ "id": "task-run-all-1", "title": "Run all", "status": "open", "priority": 1, "autoExecute": false }),
        Some("idem-ra-create"),
    );
    let (status, _) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);

    let run_all = rpc_request(
        "ra-run-all",
        "task.run_all",
        json!({}),
        Some("idem-ra-run-all"),
    );
    let (status, payload) = post_rpc(&client, &server, &run_all).await?;
    assert_eq!(status, 200);

    // According to TaskRunAllResult it should return enqueued and errors
    assert!(payload["result"]["enqueued"].is_u64());
    assert!(payload["result"]["errors"].is_array());

    // Task should be running or queued now
    let list = rpc_request("ra-list", "task.list", json!({}), None);
    let (status, payload) = post_rpc(&client, &server, &list).await?;
    assert_eq!(status, 200);
    let listed = payload["result"]["tasks"].as_array().unwrap();
    let task = listed.iter().find(|t| t["id"] == "task-run-all-1").unwrap();
    assert!(task["status"] == "pending" || task["status"] == "running" || task["status"] == "done");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn loop_prune_cleans_up() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let prune = rpc_request("lp-prune", "loop.prune", json!({}), Some("idem-lp-prune"));
    let (status, payload) = post_rpc(&client, &server, &prune).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["success"], true);

    server.stop().await;
    Ok(())
}
