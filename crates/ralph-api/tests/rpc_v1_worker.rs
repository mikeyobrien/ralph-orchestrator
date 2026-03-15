use std::path::Path;

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
            workspace,
        }
    }

    fn workspace_path(&self) -> &Path {
        self.workspace.path()
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

    if let Some(key) = idempotency_key {
        request["meta"] = json!({ "idempotencyKey": key });
    }

    request
}

fn sample_register_params(worker_id: &str, worker_name: &str, workspace_root: &str) -> Value {
    json!({
        "workerId": worker_id,
        "workerName": worker_name,
        "loopId": format!("loop-{worker_id}"),
        "backend": "claude",
        "workspaceRoot": workspace_root,
        "status": "idle",
        "lastHeartbeatAt": "2026-03-14T22:30:00Z"
    })
}

fn create_ready_task_params(id: &str, title: &str) -> Value {
    rpc_request(
        &format!("req-setup-{id}"),
        "task.create",
        json!({
            "id": id,
            "title": title,
            "status": "ready",
            "priority": 1
        }),
        Some(&format!("idem-setup-{id}")),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn register_list_get_deregister_round_trip() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();
    let ws = server.workspace_path().display().to_string();

    // Register two workers
    let reg_a = rpc_request(
        "req-reg-a",
        "worker.register",
        sample_register_params("worker-a", "Alpha", &ws),
        Some("idem-reg-a"),
    );
    let (status, reg_a_payload) = post_rpc(&client, &server, &reg_a).await?;
    assert_eq!(status, 200);
    assert_eq!(reg_a_payload["result"]["worker"]["workerId"], "worker-a");
    assert_eq!(reg_a_payload["result"]["worker"]["workerName"], "Alpha");
    assert_eq!(reg_a_payload["result"]["worker"]["status"], "idle");

    let reg_b = rpc_request(
        "req-reg-b",
        "worker.register",
        sample_register_params("worker-b", "Beta", &ws),
        Some("idem-reg-b"),
    );
    let (status, _) = post_rpc(&client, &server, &reg_b).await?;
    assert_eq!(status, 200);

    // List — should return both
    let list = rpc_request("req-list-1", "worker.list", json!({}), None);
    let (status, list_payload) = post_rpc(&client, &server, &list).await?;
    assert_eq!(status, 200);
    let workers = list_payload["result"]["workers"].as_array().unwrap();
    assert_eq!(workers.len(), 2);
    let ids: Vec<&str> = workers
        .iter()
        .filter_map(|w| w["workerId"].as_str())
        .collect();
    assert!(ids.contains(&"worker-a"));
    assert!(ids.contains(&"worker-b"));

    // Get — specific worker
    let get = rpc_request(
        "req-get-a",
        "worker.get",
        json!({ "workerId": "worker-a" }),
        None,
    );
    let (status, get_payload) = post_rpc(&client, &server, &get).await?;
    assert_eq!(status, 200);
    assert_eq!(get_payload["result"]["worker"]["workerId"], "worker-a");
    assert_eq!(get_payload["result"]["worker"]["workerName"], "Alpha");

    // Get — unknown worker → 404
    let get_unknown = rpc_request(
        "req-get-unknown",
        "worker.get",
        json!({ "workerId": "worker-nope" }),
        None,
    );
    let (status, get_unknown_payload) = post_rpc(&client, &server, &get_unknown).await?;
    assert_eq!(status, 404);
    assert_eq!(get_unknown_payload["error"]["code"], "NOT_FOUND");

    // Deregister worker-b
    let dereg = rpc_request(
        "req-dereg-b",
        "worker.deregister",
        json!({ "workerId": "worker-b" }),
        Some("idem-dereg-b"),
    );
    let (status, dereg_payload) = post_rpc(&client, &server, &dereg).await?;
    assert_eq!(status, 200);
    assert_eq!(dereg_payload["result"]["success"], true);

    // List — only worker-a remains
    let list2 = rpc_request("req-list-2", "worker.list", json!({}), None);
    let (_, list2_payload) = post_rpc(&client, &server, &list2).await?;
    let workers2 = list2_payload["result"]["workers"].as_array().unwrap();
    assert_eq!(workers2.len(), 1);
    assert_eq!(workers2[0]["workerId"], "worker-a");

    // Deregister unknown → 404
    let dereg_unknown = rpc_request(
        "req-dereg-unknown",
        "worker.deregister",
        json!({ "workerId": "worker-nope" }),
        Some("idem-dereg-unknown"),
    );
    let (status, dereg_unknown_payload) = post_rpc(&client, &server, &dereg_unknown).await?;
    assert_eq!(status, 404);
    assert_eq!(dereg_unknown_payload["error"]["code"], "NOT_FOUND");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn heartbeat_updates_worker_state() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();
    let ws = server.workspace_path().display().to_string();

    // Register
    let reg = rpc_request(
        "req-hb-reg",
        "worker.register",
        sample_register_params("worker-hb", "Heartbeat Worker", &ws),
        Some("idem-hb-reg"),
    );
    let (status, _) = post_rpc(&client, &server, &reg).await?;
    assert_eq!(status, 200);

    // Heartbeat — transition to busy with task context
    let hb = rpc_request(
        "req-hb-1",
        "worker.heartbeat",
        json!({
            "workerId": "worker-hb",
            "status": "busy",
            "currentTaskId": "task-42",
            "currentHat": "builder",
            "lastHeartbeatAt": "2026-03-14T23:00:00Z"
        }),
        Some("idem-hb-1"),
    );
    let (status, hb_payload) = post_rpc(&client, &server, &hb).await?;
    assert_eq!(status, 200);
    assert_eq!(hb_payload["result"]["worker"]["status"], "busy");
    assert_eq!(hb_payload["result"]["worker"]["currentTaskId"], "task-42");
    assert_eq!(hb_payload["result"]["worker"]["currentHat"], "builder");
    assert_eq!(
        hb_payload["result"]["worker"]["lastHeartbeatAt"],
        "2026-03-14T23:00:00Z"
    );

    // Verify via get
    let get = rpc_request(
        "req-hb-get",
        "worker.get",
        json!({ "workerId": "worker-hb" }),
        None,
    );
    let (_, get_payload) = post_rpc(&client, &server, &get).await?;
    assert_eq!(get_payload["result"]["worker"]["status"], "busy");
    assert_eq!(get_payload["result"]["worker"]["currentTaskId"], "task-42");

    // Heartbeat unknown worker → 404
    let hb_unknown = rpc_request(
        "req-hb-unknown",
        "worker.heartbeat",
        json!({
            "workerId": "worker-nope",
            "status": "idle",
            "lastHeartbeatAt": "2026-03-14T23:00:00Z"
        }),
        Some("idem-hb-unknown"),
    );
    let (status, hb_unknown_payload) = post_rpc(&client, &server, &hb_unknown).await?;
    assert_eq!(status, 404);
    assert_eq!(hb_unknown_payload["error"]["code"], "NOT_FOUND");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn claim_next_claims_ready_task_and_transitions_worker() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();
    let ws = server.workspace_path().display().to_string();

    // Create two ready tasks via RPC
    let (status, _) = post_rpc(
        &client,
        &server,
        &create_ready_task_params("task-claim-1", "First task"),
    )
    .await?;
    assert_eq!(status, 200);
    let (status, _) = post_rpc(
        &client,
        &server,
        &create_ready_task_params("task-claim-2", "Second task"),
    )
    .await?;
    assert_eq!(status, 200);

    // Register an idle worker
    let reg = rpc_request(
        "req-claim-reg",
        "worker.register",
        sample_register_params("worker-claimer", "Claimer", &ws),
        Some("idem-claim-reg"),
    );
    let (status, _) = post_rpc(&client, &server, &reg).await?;
    assert_eq!(status, 200);

    // claim_next — should claim one task
    let claim = rpc_request(
        "req-claim-1",
        "worker.claim_next",
        json!({ "workerId": "worker-claimer" }),
        Some("idem-claim-1"),
    );
    let (status, claim_payload) = post_rpc(&client, &server, &claim).await?;
    assert_eq!(status, 200);
    assert!(claim_payload["result"]["task"].is_object());
    let claimed_task_id = claim_payload["result"]["task"]["id"]
        .as_str()
        .expect("claimed task should have id");
    assert_eq!(claim_payload["result"]["task"]["status"], "in_progress");
    assert_eq!(
        claim_payload["result"]["task"]["assigneeWorkerId"],
        "worker-claimer"
    );
    assert!(claim_payload["result"]["task"]["claimedAt"].is_string());
    assert!(claim_payload["result"]["task"]["leaseExpiresAt"].is_string());

    // Worker should now be busy
    assert_eq!(claim_payload["result"]["worker"]["status"], "busy");
    assert_eq!(
        claim_payload["result"]["worker"]["currentTaskId"],
        claimed_task_id
    );

    // Second claim_next from same worker should fail (not idle)
    let claim2 = rpc_request(
        "req-claim-2",
        "worker.claim_next",
        json!({ "workerId": "worker-claimer" }),
        Some("idem-claim-2"),
    );
    let (status, claim2_payload) = post_rpc(&client, &server, &claim2).await?;
    assert_eq!(status, 412);
    assert_eq!(claim2_payload["error"]["code"], "PRECONDITION_FAILED");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn claim_next_returns_null_task_when_no_ready_tasks() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();
    let ws = server.workspace_path().display().to_string();

    // Register idle worker, no tasks exist
    let reg = rpc_request(
        "req-empty-reg",
        "worker.register",
        sample_register_params("worker-empty", "Empty", &ws),
        Some("idem-empty-reg"),
    );
    let (status, _) = post_rpc(&client, &server, &reg).await?;
    assert_eq!(status, 200);

    let claim = rpc_request(
        "req-empty-claim",
        "worker.claim_next",
        json!({ "workerId": "worker-empty" }),
        Some("idem-empty-claim"),
    );
    let (status, claim_payload) = post_rpc(&client, &server, &claim).await?;
    assert_eq!(status, 200);
    assert!(claim_payload["result"]["task"].is_null());
    // Worker should remain idle
    assert_eq!(claim_payload["result"]["worker"]["status"], "idle");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn reclaim_expired_requeues_stale_claims() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();
    let ws = server.workspace_path().display().to_string();

    // Create a ready task and register a worker
    let (status, _) = post_rpc(
        &client,
        &server,
        &create_ready_task_params("task-reclaim-1", "Reclaimable task"),
    )
    .await?;
    assert_eq!(status, 200);

    let reg = rpc_request(
        "req-reclaim-reg",
        "worker.register",
        sample_register_params("worker-stale", "Stale Worker", &ws),
        Some("idem-reclaim-reg"),
    );
    let (status, _) = post_rpc(&client, &server, &reg).await?;
    assert_eq!(status, 200);

    // Claim the task
    let claim = rpc_request(
        "req-reclaim-claim",
        "worker.claim_next",
        json!({ "workerId": "worker-stale" }),
        Some("idem-reclaim-claim"),
    );
    let (status, claim_payload) = post_rpc(&client, &server, &claim).await?;
    assert_eq!(status, 200);
    assert_eq!(claim_payload["result"]["task"]["id"], "task-reclaim-1");
    let _lease_expires = claim_payload["result"]["task"]["leaseExpiresAt"]
        .as_str()
        .expect("leaseExpiresAt should be set");

    // Reclaim with as_of far in the future (well past lease expiry)
    let reclaim = rpc_request(
        "req-reclaim-1",
        "worker.reclaim_expired",
        json!({ "asOf": "2099-01-01T00:00:00Z" }),
        Some("idem-reclaim-1"),
    );
    let (status, reclaim_payload) = post_rpc(&client, &server, &reclaim).await?;
    assert_eq!(status, 200);

    // Should have reclaimed one task and one worker
    let reclaimed_tasks = reclaim_payload["result"]["tasks"].as_array().unwrap();
    assert_eq!(reclaimed_tasks.len(), 1);
    assert_eq!(reclaimed_tasks[0]["id"], "task-reclaim-1");
    assert_eq!(reclaimed_tasks[0]["status"], "ready");
    assert!(reclaimed_tasks[0]["assigneeWorkerId"].is_null());
    assert!(reclaimed_tasks[0]["claimedAt"].is_null());
    assert!(reclaimed_tasks[0]["leaseExpiresAt"].is_null());

    let reclaimed_workers = reclaim_payload["result"]["workers"].as_array().unwrap();
    assert_eq!(reclaimed_workers.len(), 1);
    assert_eq!(reclaimed_workers[0]["workerId"], "worker-stale");
    assert_eq!(reclaimed_workers[0]["status"], "dead");
    assert!(reclaimed_workers[0]["currentTaskId"].is_null());

    // Verify worker is dead via worker.get
    let get_worker = rpc_request(
        "req-reclaim-get-worker",
        "worker.get",
        json!({ "workerId": "worker-stale" }),
        None,
    );
    let (status, get_worker_payload) = post_rpc(&client, &server, &get_worker).await?;
    assert_eq!(status, 200);
    assert_eq!(get_worker_payload["result"]["worker"]["status"], "dead");
    assert!(get_worker_payload["result"]["worker"]["currentTaskId"].is_null());

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn reclaim_expired_no_op_when_nothing_stale() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    // No workers registered, reclaim should return empty
    let reclaim = rpc_request(
        "req-reclaim-noop",
        "worker.reclaim_expired",
        json!({ "asOf": "2099-01-01T00:00:00Z" }),
        Some("idem-reclaim-noop"),
    );
    let (status, reclaim_payload) = post_rpc(&client, &server, &reclaim).await?;
    assert_eq!(status, 200);
    assert_eq!(
        reclaim_payload["result"]["tasks"].as_array().unwrap().len(),
        0
    );
    assert_eq!(
        reclaim_payload["result"]["workers"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn schema_validation_rejects_bad_params() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    // worker.get with missing workerId
    let bad_get = rpc_request("req-bad-get", "worker.get", json!({}), None);
    let (status, bad_get_payload) = post_rpc(&client, &server, &bad_get).await?;
    assert_eq!(status, 400);
    assert_eq!(bad_get_payload["error"]["code"], "INVALID_PARAMS");

    // worker.register with missing required fields
    let bad_reg = rpc_request(
        "req-bad-reg",
        "worker.register",
        json!({ "workerId": "w1" }),
        Some("idem-bad-reg"),
    );
    let (status, bad_reg_payload) = post_rpc(&client, &server, &bad_reg).await?;
    assert_eq!(status, 400);
    assert_eq!(bad_reg_payload["error"]["code"], "INVALID_PARAMS");

    // worker.heartbeat with missing required fields
    let bad_hb = rpc_request(
        "req-bad-hb",
        "worker.heartbeat",
        json!({ "workerId": "w1" }),
        Some("idem-bad-hb"),
    );
    let (status, bad_hb_payload) = post_rpc(&client, &server, &bad_hb).await?;
    assert_eq!(status, 400);
    assert_eq!(bad_hb_payload["error"]["code"], "INVALID_PARAMS");

    // worker.reclaim_expired with wrong type
    let bad_reclaim = rpc_request(
        "req-bad-reclaim",
        "worker.reclaim_expired",
        json!({ "asOf": 12345 }),
        Some("idem-bad-reclaim"),
    );
    let (status, bad_reclaim_payload) = post_rpc(&client, &server, &bad_reclaim).await?;
    assert_eq!(status, 400);
    assert_eq!(bad_reclaim_payload["error"]["code"], "INVALID_PARAMS");

    server.stop().await;
    Ok(())
}
