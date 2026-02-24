use std::path::Path;

use anyhow::Result;
use ralph_core::{LoopEntry, LoopRegistry, MergeQueue};
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

    if let Some(idempotency_key) = idempotency_key {
        request["meta"] = json!({
            "idempotencyKey": idempotency_key,
        });
    }

    request
}

#[tokio::test]
async fn task_crud_ready_and_guardrails_parity() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create_blocker = rpc_request(
        "req-task-create-1",
        "task.create",
        json!({
            "id": "task-blocker-1",
            "title": "Blocker task",
            "status": "open",
            "priority": 1,
            "autoExecute": false
        }),
        Some("idem-task-create-blocker-1"),
    );
    let (status, payload) = post_rpc(&client, &server, &create_blocker).await?;
    assert_eq!(status, 200);
    assert_eq!(payload["result"]["task"]["id"], "task-blocker-1");

    let create_blocked = rpc_request(
        "req-task-create-2",
        "task.create",
        json!({
            "id": "task-blocked-1",
            "title": "Blocked task",
            "status": "open",
            "priority": 2,
            "blockedBy": "task-blocker-1",
            "autoExecute": false
        }),
        Some("idem-task-create-blocked-1"),
    );
    let (status, _) = post_rpc(&client, &server, &create_blocked).await?;
    assert_eq!(status, 200);

    let ready_before = rpc_request("req-task-ready-1", "task.ready", json!({}), None);
    let (_, ready_before_payload) = post_rpc(&client, &server, &ready_before).await?;
    assert_eq!(
        ready_before_payload["result"]["tasks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        ready_before_payload["result"]["tasks"][0]["id"],
        "task-blocker-1"
    );

    let close_blocker = rpc_request(
        "req-task-close-1",
        "task.close",
        json!({ "id": "task-blocker-1" }),
        Some("idem-task-close-blocker-1"),
    );
    let (status, _) = post_rpc(&client, &server, &close_blocker).await?;
    assert_eq!(status, 200);

    let ready_after = rpc_request("req-task-ready-2", "task.ready", json!({}), None);
    let (_, ready_after_payload) = post_rpc(&client, &server, &ready_after).await?;
    assert_eq!(
        ready_after_payload["result"]["tasks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        ready_after_payload["result"]["tasks"][0]["id"],
        "task-blocked-1"
    );

    let archive_blocker = rpc_request(
        "req-task-archive-1",
        "task.archive",
        json!({ "id": "task-blocker-1" }),
        Some("idem-task-archive-blocker-1"),
    );
    let (status, _) = post_rpc(&client, &server, &archive_blocker).await?;
    assert_eq!(status, 200);

    let list_default = rpc_request("req-task-list-1", "task.list", json!({}), None);
    let (_, list_default_payload) = post_rpc(&client, &server, &list_default).await?;
    let listed = list_default_payload["result"]["tasks"].as_array().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["id"], "task-blocked-1");

    let list_archived = rpc_request(
        "req-task-list-2",
        "task.list",
        json!({ "includeArchived": true }),
        None,
    );
    let (_, list_archived_payload) = post_rpc(&client, &server, &list_archived).await?;
    assert_eq!(
        list_archived_payload["result"]["tasks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let delete_open = rpc_request(
        "req-task-delete-open-1",
        "task.delete",
        json!({ "id": "task-blocked-1" }),
        Some("idem-task-delete-open-1"),
    );
    let (status, delete_open_payload) = post_rpc(&client, &server, &delete_open).await?;
    assert_eq!(status, 412);
    assert_eq!(delete_open_payload["error"]["code"], "PRECONDITION_FAILED");

    let close_blocked = rpc_request(
        "req-task-close-2",
        "task.close",
        json!({ "id": "task-blocked-1" }),
        Some("idem-task-close-blocked-1"),
    );
    let (status, _) = post_rpc(&client, &server, &close_blocked).await?;
    assert_eq!(status, 200);

    let delete_closed = rpc_request(
        "req-task-delete-closed-1",
        "task.delete",
        json!({ "id": "task-blocked-1" }),
        Some("idem-task-delete-closed-1"),
    );
    let (status, delete_closed_payload) = post_rpc(&client, &server, &delete_closed).await?;
    assert_eq!(status, 200);
    assert_eq!(delete_closed_payload["result"]["success"], true);

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_run_retry_status_and_idempotency_parity() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create = rpc_request(
        "req-run-create-1",
        "task.create",
        json!({
            "id": "task-run-1",
            "title": "Run me",
            "autoExecute": false
        }),
        Some("idem-task-run-create-1"),
    );
    let (status, _) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);

    let run = rpc_request(
        "req-run-1",
        "task.run",
        json!({ "id": "task-run-1" }),
        Some("idem-task-run-1"),
    );
    let (run_status, run_payload) = post_rpc(&client, &server, &run).await?;
    assert_eq!(run_status, 200);
    assert_eq!(run_payload["result"]["success"], true);

    let (run_replay_status, run_replay_payload) = post_rpc(&client, &server, &run).await?;
    assert_eq!(run_replay_status, run_status);
    assert_eq!(run_replay_payload, run_payload);

    let status_request = rpc_request(
        "req-run-status-1",
        "task.status",
        json!({ "id": "task-run-1" }),
        None,
    );
    let (status_code, status_payload) = post_rpc(&client, &server, &status_request).await?;
    assert_eq!(status_code, 200);
    assert_eq!(status_payload["result"]["isQueued"], true);

    let cancel = rpc_request(
        "req-run-cancel-1",
        "task.cancel",
        json!({ "id": "task-run-1" }),
        Some("idem-task-run-cancel-1"),
    );
    let (cancel_status, cancel_payload) = post_rpc(&client, &server, &cancel).await?;
    assert_eq!(cancel_status, 200);
    assert_eq!(cancel_payload["result"]["task"]["status"], "failed");

    let retry = rpc_request(
        "req-run-retry-1",
        "task.retry",
        json!({ "id": "task-run-1" }),
        Some("idem-task-run-retry-1"),
    );
    let (retry_status, retry_payload) = post_rpc(&client, &server, &retry).await?;
    assert_eq!(retry_status, 200);
    assert_eq!(retry_payload["result"]["success"], true);

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_update_clears_terminal_and_queue_fields_on_non_terminal_transition() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create = rpc_request(
        "req-task-state-create-1",
        "task.create",
        json!({
            "id": "task-state-1",
            "title": "State transition task",
            "autoExecute": false
        }),
        Some("idem-task-state-create-1"),
    );
    let (status, _) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);

    let run = rpc_request(
        "req-task-state-run-1",
        "task.run",
        json!({ "id": "task-state-1" }),
        Some("idem-task-state-run-1"),
    );
    let (status, run_payload) = post_rpc(&client, &server, &run).await?;
    assert_eq!(status, 200);
    assert_eq!(run_payload["result"]["success"], true);

    let update_open = rpc_request(
        "req-task-state-update-open-1",
        "task.update",
        json!({ "id": "task-state-1", "status": "open" }),
        Some("idem-task-state-update-open-1"),
    );
    let (status, update_open_payload) = post_rpc(&client, &server, &update_open).await?;
    assert_eq!(status, 200);
    assert!(update_open_payload["result"]["task"]["queuedTaskId"].is_null());
    assert!(update_open_payload["result"]["task"]["completedAt"].is_null());

    let run_again = rpc_request(
        "req-task-state-run-2",
        "task.run",
        json!({ "id": "task-state-1" }),
        Some("idem-task-state-run-2"),
    );
    let (status, _) = post_rpc(&client, &server, &run_again).await?;
    assert_eq!(status, 200);

    let cancel = rpc_request(
        "req-task-state-cancel-1",
        "task.cancel",
        json!({ "id": "task-state-1" }),
        Some("idem-task-state-cancel-1"),
    );
    let (status, cancel_payload) = post_rpc(&client, &server, &cancel).await?;
    assert_eq!(status, 200);
    assert_eq!(cancel_payload["result"]["task"]["status"], "failed");
    assert_eq!(
        cancel_payload["result"]["task"]["errorMessage"],
        "Task cancelled by user"
    );

    let reopen = rpc_request(
        "req-task-state-reopen-1",
        "task.update",
        json!({ "id": "task-state-1", "status": "open" }),
        Some("idem-task-state-reopen-1"),
    );
    let (status, reopen_payload) = post_rpc(&client, &server, &reopen).await?;
    assert_eq!(status, 200);
    assert_eq!(reopen_payload["result"]["task"]["status"], "open");
    assert!(reopen_payload["result"]["task"]["completedAt"].is_null());
    assert!(reopen_payload["result"]["task"]["queuedTaskId"].is_null());
    assert!(reopen_payload["result"]["task"]["errorMessage"].is_null());

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_create_rejects_auto_execute_for_non_open_status() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let invalid_create = rpc_request(
        "req-task-create-invalid-status-1",
        "task.create",
        json!({
            "id": "task-invalid-auto-1",
            "title": "Invalid auto execute",
            "status": "failed",
            "autoExecute": true
        }),
        Some("idem-task-create-invalid-status-1"),
    );
    let (status, invalid_payload) = post_rpc(&client, &server, &invalid_create).await?;
    assert_eq!(status, 400);
    assert_eq!(invalid_payload["error"]["code"], "INVALID_PARAMS");

    let valid_create = rpc_request(
        "req-task-create-valid-status-1",
        "task.create",
        json!({
            "id": "task-valid-auto-1",
            "title": "Explicit terminal task",
            "status": "failed",
            "autoExecute": false
        }),
        Some("idem-task-create-valid-status-1"),
    );
    let (status, valid_payload) = post_rpc(&client, &server, &valid_create).await?;
    assert_eq!(status, 200);
    assert_eq!(valid_payload["result"]["task"]["status"], "failed");
    assert!(valid_payload["result"]["task"]["completedAt"].is_string());
    assert!(valid_payload["result"]["task"]["queuedTaskId"].is_null());

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn loop_methods_and_trigger_merge_task_parity() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let merge_queue = MergeQueue::new(server.workspace_path());
    merge_queue.enqueue("loop-queued-1", "Queued loop prompt")?;
    merge_queue.enqueue("loop-review-1", "Needs review loop")?;
    merge_queue.mark_merging("loop-review-1", std::process::id())?;
    merge_queue.mark_needs_review("loop-review-1", "conflict in src/lib.rs")?;

    let worktree_path = server.workspace_path().join(".worktrees/loop-worktree-1");
    std::fs::create_dir_all(&worktree_path)?;

    let loop_registry = LoopRegistry::new(server.workspace_path());
    loop_registry.register(LoopEntry::with_id(
        "loop-worktree-1",
        "Implement feature in worktree",
        Some(worktree_path.to_string_lossy().to_string()),
        server.workspace_path().display().to_string(),
    ))?;

    let list_request = rpc_request(
        "req-loop-list-1",
        "loop.list",
        json!({ "includeTerminal": false }),
        None,
    );
    let (status, list_payload) = post_rpc(&client, &server, &list_request).await?;
    assert_eq!(status, 200);
    let loop_ids: Vec<String> = list_payload["result"]["loops"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["id"].as_str().map(std::string::ToString::to_string))
        .collect();
    assert!(loop_ids.contains(&"loop-queued-1".to_string()));
    assert!(loop_ids.contains(&"loop-worktree-1".to_string()));

    let merge_button_state = rpc_request(
        "req-loop-merge-button-1",
        "loop.merge_button_state",
        json!({ "id": "loop-queued-1" }),
        None,
    );
    let (status, merge_button_payload) = post_rpc(&client, &server, &merge_button_state).await?;
    assert_eq!(status, 200);
    assert!(merge_button_payload["result"]["enabled"].is_boolean());

    let merge = rpc_request(
        "req-loop-merge-1",
        "loop.merge",
        json!({
            "id": "loop-queued-1",
            "force": false
        }),
        Some("idem-loop-merge-1"),
    );
    let (status, merge_payload) = post_rpc(&client, &server, &merge).await?;
    assert_eq!(status, 200);
    assert_eq!(merge_payload["result"]["success"], true);

    let list_non_terminal = rpc_request(
        "req-loop-list-2",
        "loop.list",
        json!({ "includeTerminal": false }),
        None,
    );
    let (_, list_non_terminal_payload) = post_rpc(&client, &server, &list_non_terminal).await?;
    let non_terminal_ids: Vec<String> = list_non_terminal_payload["result"]["loops"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["id"].as_str().map(std::string::ToString::to_string))
        .collect();
    assert!(!non_terminal_ids.contains(&"loop-queued-1".to_string()));

    let list_with_terminal = rpc_request(
        "req-loop-list-3",
        "loop.list",
        json!({ "includeTerminal": true }),
        None,
    );
    let (_, list_with_terminal_payload) = post_rpc(&client, &server, &list_with_terminal).await?;
    assert!(
        list_with_terminal_payload["result"]["loops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["id"] == "loop-queued-1" && entry["status"] == "merged")
    );

    let trigger_merge_task = rpc_request(
        "req-loop-trigger-task-1",
        "loop.trigger_merge_task",
        json!({ "loopId": "loop-worktree-1" }),
        Some("idem-loop-trigger-task-1"),
    );
    let (status, trigger_payload) = post_rpc(&client, &server, &trigger_merge_task).await?;
    assert_eq!(status, 200);
    assert_eq!(trigger_payload["result"]["success"], true);

    let task_id = trigger_payload["result"]["taskId"]
        .as_str()
        .expect("taskId should be present")
        .to_string();

    let get_task = rpc_request(
        "req-loop-trigger-task-get-1",
        "task.get",
        json!({ "id": task_id }),
        None,
    );
    let (status, get_task_payload) = post_rpc(&client, &server, &get_task).await?;
    assert_eq!(status, 200);
    assert!(
        get_task_payload["result"]["task"]["title"]
            .as_str()
            .is_some_and(|title| title.starts_with("Merge:"))
    );

    server.stop().await;
    Ok(())
}
