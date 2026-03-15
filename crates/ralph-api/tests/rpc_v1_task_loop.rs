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
            "status": "ready",
            "priority": 1
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
            "status": "ready",
            "priority": 2,
            "blockedBy": "task-blocker-1"
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

    let delete_ready = rpc_request(
        "req-task-delete-ready-1",
        "task.delete",
        json!({ "id": "task-blocked-1" }),
        Some("idem-task-delete-ready-1"),
    );
    let (status, delete_ready_payload) = post_rpc(&client, &server, &delete_ready).await?;
    assert_eq!(status, 412);
    assert_eq!(delete_ready_payload["error"]["code"], "PRECONDITION_FAILED");

    let close_blocked = rpc_request(
        "req-task-close-2",
        "task.close",
        json!({ "id": "task-blocked-1" }),
        Some("idem-task-close-blocked-1"),
    );
    let (status, _) = post_rpc(&client, &server, &close_blocked).await?;
    assert_eq!(status, 200);

    let delete_done = rpc_request(
        "req-task-delete-done-1",
        "task.delete",
        json!({ "id": "task-blocked-1" }),
        Some("idem-task-delete-done-1"),
    );
    let (status, delete_done_payload) = post_rpc(&client, &server, &delete_done).await?;
    assert_eq!(status, 200);
    assert_eq!(delete_done_payload["result"]["success"], true);

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_update_clears_terminal_fields_on_non_terminal_transition() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create = rpc_request(
        "req-task-state-create-1",
        "task.create",
        json!({
            "id": "task-state-1",
            "title": "State transition task"
        }),
        Some("idem-task-state-create-1"),
    );
    let (status, create_payload) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);
    assert_eq!(create_payload["result"]["task"]["status"], "ready");

    let update_done = rpc_request(
        "req-task-state-update-done-1",
        "task.update",
        json!({ "id": "task-state-1", "status": "done" }),
        Some("idem-task-state-update-done-1"),
    );
    let (status, update_done_payload) = post_rpc(&client, &server, &update_done).await?;
    assert_eq!(status, 200);
    assert_eq!(update_done_payload["result"]["task"]["status"], "done");
    assert!(update_done_payload["result"]["task"]["completedAt"].is_string());

    let reopen_ready = rpc_request(
        "req-task-state-reopen-ready-1",
        "task.update",
        json!({ "id": "task-state-1", "status": "ready" }),
        Some("idem-task-state-reopen-ready-1"),
    );
    let (status, reopen_ready_payload) = post_rpc(&client, &server, &reopen_ready).await?;
    assert_eq!(status, 200);
    assert_eq!(reopen_ready_payload["result"]["task"]["status"], "ready");
    assert!(reopen_ready_payload["result"]["task"]["completedAt"].is_null());
    assert!(reopen_ready_payload["result"]["task"]["errorMessage"].is_null());

    let update_in_progress = rpc_request(
        "req-task-state-update-in-progress-1",
        "task.update",
        json!({ "id": "task-state-1", "status": "in_progress" }),
        Some("idem-task-state-update-in-progress-1"),
    );
    let (status, update_in_progress_payload) =
        post_rpc(&client, &server, &update_in_progress).await?;
    assert_eq!(status, 200);
    assert_eq!(
        update_in_progress_payload["result"]["task"]["status"],
        "in_progress"
    );
    assert!(update_in_progress_payload["result"]["task"]["completedAt"].is_null());

    let cancel = rpc_request(
        "req-task-state-cancel-1",
        "task.cancel",
        json!({ "id": "task-state-1" }),
        Some("idem-task-state-cancel-1"),
    );
    let (status, cancel_payload) = post_rpc(&client, &server, &cancel).await?;
    assert_eq!(status, 200);
    assert_eq!(cancel_payload["result"]["task"]["status"], "cancelled");
    assert_eq!(
        cancel_payload["result"]["task"]["errorMessage"],
        "Task cancelled by user"
    );
    assert!(cancel_payload["result"]["task"]["completedAt"].is_string());

    let reopen_after_cancel = rpc_request(
        "req-task-state-reopen-after-cancel-1",
        "task.update",
        json!({ "id": "task-state-1", "status": "ready" }),
        Some("idem-task-state-reopen-after-cancel-1"),
    );
    let (status, reopen_after_cancel_payload) =
        post_rpc(&client, &server, &reopen_after_cancel).await?;
    assert_eq!(status, 200);
    assert_eq!(
        reopen_after_cancel_payload["result"]["task"]["status"],
        "ready"
    );
    assert!(reopen_after_cancel_payload["result"]["task"]["completedAt"].is_null());
    assert!(reopen_after_cancel_payload["result"]["task"]["errorMessage"].is_null());

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

    let trigger_result = trigger_payload["result"]
        .as_object()
        .expect("result should be an object");
    assert_eq!(trigger_result.get("success"), Some(&json!(true)));
    assert!(trigger_result.contains_key("taskId"));
    assert!(!trigger_result.contains_key("queuedTaskId"));

    let task_id = trigger_result
        .get("taskId")
        .and_then(|value| value.as_str())
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
    assert_eq!(get_task_payload["result"]["task"]["status"], "ready");
    assert!(
        get_task_payload["result"]["task"]["title"]
            .as_str()
            .is_some_and(|title| title.starts_with("Merge:"))
    );

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_board_state_transitions() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    // --- Happy path: ready → in_progress → cancelled → ready → done ---

    let create_a = rpc_request(
        "req-board-create-a",
        "task.create",
        json!({
            "id": "task-board-a",
            "title": "Board task A",
            "status": "ready",
            "priority": 1
        }),
        Some("idem-board-create-a"),
    );
    let (status, create_a_payload) = post_rpc(&client, &server, &create_a).await?;
    assert_eq!(status, 200);
    assert_eq!(create_a_payload["result"]["task"]["status"], "ready");

    let cancel_ready = rpc_request(
        "req-board-cancel-ready",
        "task.cancel",
        json!({ "id": "task-board-a" }),
        Some("idem-board-cancel-ready"),
    );
    let (status, cancel_ready_payload) = post_rpc(&client, &server, &cancel_ready).await?;
    assert_eq!(status, 412);
    assert_eq!(cancel_ready_payload["error"]["code"], "PRECONDITION_FAILED");
    assert_eq!(
        cancel_ready_payload["error"]["message"],
        "Only in_progress tasks can be cancelled"
    );
    assert_eq!(cancel_ready_payload["error"]["details"]["status"], "ready");
    assert_eq!(
        cancel_ready_payload["error"]["details"]["taskId"],
        "task-board-a"
    );

    let retry_ready = rpc_request(
        "req-board-retry-ready",
        "task.retry",
        json!({ "id": "task-board-a" }),
        Some("idem-board-retry-ready"),
    );
    let (status, retry_ready_payload) = post_rpc(&client, &server, &retry_ready).await?;
    assert_eq!(status, 412);
    assert_eq!(retry_ready_payload["error"]["code"], "PRECONDITION_FAILED");
    assert_eq!(
        retry_ready_payload["error"]["message"],
        "Only cancelled tasks can be retried"
    );
    assert_eq!(retry_ready_payload["error"]["details"]["status"], "ready");
    assert_eq!(
        retry_ready_payload["error"]["details"]["taskId"],
        "task-board-a"
    );

    let create_b = rpc_request(
        "req-board-create-b",
        "task.create",
        json!({
            "id": "task-board-b",
            "title": "Board task B",
            "status": "ready",
            "blockedBy": "task-board-a"
        }),
        Some("idem-board-create-b"),
    );
    let (status, _) = post_rpc(&client, &server, &create_b).await?;
    assert_eq!(status, 200);

    let ready_before_done =
        rpc_request("req-board-ready-before-done", "task.ready", json!({}), None);
    let (_, ready_before_done_payload) = post_rpc(&client, &server, &ready_before_done).await?;
    let ready_before_done_ids: Vec<&str> = ready_before_done_payload["result"]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|task| task["id"].as_str())
        .collect();
    assert!(
        !ready_before_done_ids.contains(&"task-board-b"),
        "task-board-b should stay blocked until task-board-a reaches done"
    );

    let update_in_progress = rpc_request(
        "req-board-update-in-progress",
        "task.update",
        json!({ "id": "task-board-a", "status": "in_progress" }),
        Some("idem-board-update-in-progress"),
    );
    let (status, update_payload) = post_rpc(&client, &server, &update_in_progress).await?;
    assert_eq!(status, 200);
    assert_eq!(update_payload["result"]["task"]["status"], "in_progress");

    let cancel = rpc_request(
        "req-board-cancel",
        "task.cancel",
        json!({ "id": "task-board-a" }),
        Some("idem-board-cancel"),
    );
    let (status, cancel_payload) = post_rpc(&client, &server, &cancel).await?;
    assert_eq!(status, 200);
    assert_eq!(cancel_payload["result"]["task"]["status"], "cancelled");

    let retry = rpc_request(
        "req-board-retry",
        "task.retry",
        json!({ "id": "task-board-a" }),
        Some("idem-board-retry"),
    );
    let (status, retry_payload) = post_rpc(&client, &server, &retry).await?;
    assert_eq!(status, 200);
    assert_eq!(retry_payload["result"]["task"]["status"], "ready");

    let update_done = rpc_request(
        "req-board-update-done",
        "task.update",
        json!({ "id": "task-board-a", "status": "done" }),
        Some("idem-board-update-done"),
    );
    let (status, done_payload) = post_rpc(&client, &server, &update_done).await?;
    assert_eq!(status, 200);
    assert_eq!(done_payload["result"]["task"]["status"], "done");
    assert!(done_payload["result"]["task"]["completedAt"].is_string());

    // --- Unblocking: task B becomes ready after blocker task A reaches done ---

    let ready_after_done = rpc_request("req-board-ready-after-done", "task.ready", json!({}), None);
    let (_, ready_after_done_payload) = post_rpc(&client, &server, &ready_after_done).await?;
    let ready_after_done_ids: Vec<&str> = ready_after_done_payload["result"]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|task| task["id"].as_str())
        .collect();
    assert!(
        ready_after_done_ids.contains(&"task-board-b"),
        "task-board-b should be unblocked after task-board-a is done"
    );

    // --- Guardrails: delete ready task → 412, delete done task → 200 ---

    let create_c = rpc_request(
        "req-board-create-c",
        "task.create",
        json!({
            "id": "task-board-c",
            "title": "Ready-only task",
            "status": "ready"
        }),
        Some("idem-board-create-c"),
    );
    let (status, _) = post_rpc(&client, &server, &create_c).await?;
    assert_eq!(status, 200);

    let delete_ready = rpc_request(
        "req-board-delete-ready",
        "task.delete",
        json!({ "id": "task-board-c" }),
        Some("idem-board-delete-ready"),
    );
    let (status, delete_ready_payload) = post_rpc(&client, &server, &delete_ready).await?;
    assert_eq!(status, 412);
    assert_eq!(delete_ready_payload["error"]["code"], "PRECONDITION_FAILED");

    let delete_done = rpc_request(
        "req-board-delete-done",
        "task.delete",
        json!({ "id": "task-board-a" }),
        Some("idem-board-delete-done"),
    );
    let (status, delete_done_payload) = post_rpc(&client, &server, &delete_done).await?;
    assert_eq!(status, 200);
    assert_eq!(delete_done_payload["result"]["success"], true);

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_worker_lease_fields_round_trip_persist_and_validate() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let create = rpc_request(
        "req-task-worker-fields-create-1",
        "task.create",
        json!({
            "id": "task-worker-fields-1",
            "title": "Task with worker lease metadata",
            "status": "ready",
            "assigneeWorkerId": "worker-alpha",
            "claimedAt": "2026-03-14T20:00:00Z",
            "leaseExpiresAt": "2026-03-14T20:02:00Z"
        }),
        Some("idem-task-worker-fields-create-1"),
    );
    let (status, create_payload) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);
    assert_eq!(
        create_payload["result"]["task"]["assigneeWorkerId"],
        "worker-alpha"
    );
    assert_eq!(
        create_payload["result"]["task"]["claimedAt"],
        "2026-03-14T20:00:00Z"
    );
    assert_eq!(
        create_payload["result"]["task"]["leaseExpiresAt"],
        "2026-03-14T20:02:00Z"
    );

    let get = rpc_request(
        "req-task-worker-fields-get-1",
        "task.get",
        json!({ "id": "task-worker-fields-1" }),
        None,
    );
    let (status, get_payload) = post_rpc(&client, &server, &get).await?;
    assert_eq!(status, 200);
    assert_eq!(
        get_payload["result"]["task"],
        create_payload["result"]["task"]
    );

    let update = rpc_request(
        "req-task-worker-fields-update-1",
        "task.update",
        json!({
            "id": "task-worker-fields-1",
            "assigneeWorkerId": "worker-beta",
            "claimedAt": "2026-03-14T20:05:00Z",
            "leaseExpiresAt": "2026-03-14T20:07:00Z"
        }),
        Some("idem-task-worker-fields-update-1"),
    );
    let (status, update_payload) = post_rpc(&client, &server, &update).await?;
    assert_eq!(status, 200);
    assert_eq!(
        update_payload["result"]["task"]["assigneeWorkerId"],
        "worker-beta"
    );
    assert_eq!(
        update_payload["result"]["task"]["claimedAt"],
        "2026-03-14T20:05:00Z"
    );
    assert_eq!(
        update_payload["result"]["task"]["leaseExpiresAt"],
        "2026-03-14T20:07:00Z"
    );

    let reloaded_after_update = ralph_api::task_domain::TaskDomain::new(server.workspace_path())
        .get("task-worker-fields-1")
        .map_err(|error| anyhow::anyhow!(error.message))?;
    assert_eq!(
        reloaded_after_update.assignee_worker_id.as_deref(),
        Some("worker-beta")
    );
    assert_eq!(
        reloaded_after_update.claimed_at.as_deref(),
        Some("2026-03-14T20:05:00Z")
    );
    assert_eq!(
        reloaded_after_update.lease_expires_at.as_deref(),
        Some("2026-03-14T20:07:00Z")
    );

    let snapshot_path = server.workspace_path().join(".ralph/api/tasks-v1.json");
    let snapshot: Value = serde_json::from_str(&std::fs::read_to_string(&snapshot_path)?)?;
    let persisted_after_update = snapshot["tasks"]
        .as_array()
        .and_then(|tasks| {
            tasks.iter().find(|task| {
                task["id"]
                    .as_str()
                    .is_some_and(|task_id| task_id == "task-worker-fields-1")
            })
        })
        .and_then(Value::as_object)
        .expect("task snapshot should contain task-worker-fields-1");
    assert_eq!(
        persisted_after_update.get("assigneeWorkerId"),
        Some(&json!("worker-beta"))
    );
    assert_eq!(
        persisted_after_update.get("claimedAt"),
        Some(&json!("2026-03-14T20:05:00Z"))
    );
    assert_eq!(
        persisted_after_update.get("leaseExpiresAt"),
        Some(&json!("2026-03-14T20:07:00Z"))
    );

    let clear = rpc_request(
        "req-task-worker-fields-clear-1",
        "task.update",
        json!({
            "id": "task-worker-fields-1",
            "assigneeWorkerId": null,
            "claimedAt": null,
            "leaseExpiresAt": null
        }),
        Some("idem-task-worker-fields-clear-1"),
    );
    let (status, clear_payload) = post_rpc(&client, &server, &clear).await?;
    assert_eq!(status, 200);
    assert!(clear_payload["result"]["task"]["assigneeWorkerId"].is_null());
    assert!(clear_payload["result"]["task"]["claimedAt"].is_null());
    assert!(clear_payload["result"]["task"]["leaseExpiresAt"].is_null());

    let reloaded_after_clear = ralph_api::task_domain::TaskDomain::new(server.workspace_path())
        .get("task-worker-fields-1")
        .map_err(|error| anyhow::anyhow!(error.message))?;
    assert_eq!(reloaded_after_clear.assignee_worker_id, None);
    assert_eq!(reloaded_after_clear.claimed_at, None);
    assert_eq!(reloaded_after_clear.lease_expires_at, None);

    let cleared_snapshot: Value = serde_json::from_str(&std::fs::read_to_string(&snapshot_path)?)?;
    let persisted_after_clear = cleared_snapshot["tasks"]
        .as_array()
        .and_then(|tasks| {
            tasks.iter().find(|task| {
                task["id"]
                    .as_str()
                    .is_some_and(|task_id| task_id == "task-worker-fields-1")
            })
        })
        .and_then(Value::as_object)
        .expect("task snapshot should still contain task-worker-fields-1 after clearing");
    assert!(persisted_after_clear.get("assigneeWorkerId").is_none());
    assert!(persisted_after_clear.get("claimedAt").is_none());
    assert!(persisted_after_clear.get("leaseExpiresAt").is_none());

    for (field, invalid_value) in [
        ("assigneeWorkerId", json!(42)),
        ("claimedAt", json!({ "bad": true })),
        ("leaseExpiresAt", json!(false)),
    ] {
        let mut invalid_params = serde_json::Map::new();
        invalid_params.insert("id".to_string(), json!("task-worker-fields-1"));
        invalid_params.insert(field.to_string(), invalid_value);

        let invalid_update = rpc_request(
            &format!("req-task-worker-fields-invalid-{field}"),
            "task.update",
            Value::Object(invalid_params),
            None,
        );
        let (status, invalid_payload) = post_rpc(&client, &server, &invalid_update).await?;
        assert_eq!(status, 400);
        assert_eq!(invalid_payload["error"]["code"], "INVALID_PARAMS");
        assert_eq!(
            invalid_payload["error"]["message"],
            "request does not match rpc-v1 schema"
        );
        assert!(
            invalid_payload["error"]["details"]["errors"]
                .as_array()
                .is_some_and(|errors| !errors.is_empty())
        );
    }

    let get_after_invalid = rpc_request(
        "req-task-worker-fields-get-after-invalid-1",
        "task.get",
        json!({ "id": "task-worker-fields-1" }),
        None,
    );
    let (status, get_after_invalid_payload) =
        post_rpc(&client, &server, &get_after_invalid).await?;
    assert_eq!(status, 200);
    assert!(get_after_invalid_payload["result"]["task"]["assigneeWorkerId"].is_null());
    assert!(get_after_invalid_payload["result"]["task"]["claimedAt"].is_null());
    assert!(get_after_invalid_payload["result"]["task"]["leaseExpiresAt"].is_null());

    server.stop().await;
    Ok(())
}

// ---------------------------------------------------------------------------
// 8.5 — Task enrichment fields (currentLoopId, currentHat, isClaimed, isStale)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn task_enrichment_fields_unclaimed_and_claimed() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();
    let ws = server.workspace_path().display().to_string();

    // ── Create an unclaimed task ──────────────────────────────────────────
    let create = rpc_request(
        "req-enrich-create-1",
        "task.create",
        json!({
            "id": "task-enrich-1",
            "title": "Unclaimed enrichment test",
            "status": "ready",
            "priority": 1
        }),
        Some("idem-enrich-create-1"),
    );
    let (status, create_payload) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);

    // Unclaimed → isClaimed false, isStale false, currentLoopId/currentHat null
    let task = &create_payload["result"]["task"];
    assert_eq!(task["isClaimed"], false);
    assert_eq!(task["isStale"], false);
    assert!(task["currentLoopId"].is_null());
    assert!(task["currentHat"].is_null());

    // task.get should also carry enrichment fields
    let get = rpc_request(
        "req-enrich-get-1",
        "task.get",
        json!({ "id": "task-enrich-1" }),
        None,
    );
    let (status, get_payload) = post_rpc(&client, &server, &get).await?;
    assert_eq!(status, 200);
    let task_get = &get_payload["result"]["task"];
    assert_eq!(task_get["isClaimed"], false);
    assert_eq!(task_get["isStale"], false);
    assert!(task_get["currentLoopId"].is_null());
    assert!(task_get["currentHat"].is_null());

    // task.list should carry enrichment fields
    let list = rpc_request("req-enrich-list-1", "task.list", json!({}), None);
    let (status, list_payload) = post_rpc(&client, &server, &list).await?;
    assert_eq!(status, 200);
    let tasks = list_payload["result"]["tasks"].as_array().unwrap();
    assert!(!tasks.is_empty());
    let listed = tasks
        .iter()
        .find(|t| t["id"] == "task-enrich-1")
        .expect("task-enrich-1 should appear in list");
    assert_eq!(listed["isClaimed"], false);
    assert_eq!(listed["isStale"], false);
    assert!(listed["currentLoopId"].is_null());
    assert!(listed["currentHat"].is_null());

    // task.ready should carry enrichment fields
    let ready = rpc_request("req-enrich-ready-1", "task.ready", json!({}), None);
    let (status, ready_payload) = post_rpc(&client, &server, &ready).await?;
    assert_eq!(status, 200);
    let ready_tasks = ready_payload["result"]["tasks"].as_array().unwrap();
    let ready_task = ready_tasks
        .iter()
        .find(|t| t["id"] == "task-enrich-1")
        .expect("task-enrich-1 should appear in ready");
    assert_eq!(ready_task["isClaimed"], false);
    assert_eq!(ready_task["isStale"], false);

    // ── Register a worker and assign it to the task ──────────────────────
    let reg = rpc_request(
        "req-enrich-reg-w1",
        "worker.register",
        json!({
            "workerId": "worker-enrich-1",
            "workerName": "Enrichment Worker",
            "loopId": "loop-enrich-1",
            "backend": "claude",
            "workspaceRoot": ws,
            "status": "idle",
            "lastHeartbeatAt": "2026-03-15T10:00:00Z"
        }),
        Some("idem-enrich-reg-w1"),
    );
    let (status, _) = post_rpc(&client, &server, &reg).await?;
    assert_eq!(status, 200);

    // Heartbeat to set currentHat
    let hb = rpc_request(
        "req-enrich-hb-w1",
        "worker.heartbeat",
        json!({
            "workerId": "worker-enrich-1",
            "status": "busy",
            "currentTaskId": "task-enrich-1",
            "currentHat": "builder",
            "lastHeartbeatAt": "2026-03-15T10:01:00Z"
        }),
        Some("idem-enrich-hb-w1"),
    );
    let (status, _) = post_rpc(&client, &server, &hb).await?;
    assert_eq!(status, 200);

    // Assign the worker to the task with a future lease
    let assign = rpc_request(
        "req-enrich-assign-1",
        "task.update",
        json!({
            "id": "task-enrich-1",
            "status": "in_progress",
            "assigneeWorkerId": "worker-enrich-1",
            "claimedAt": "2026-03-15T10:01:00Z",
            "leaseExpiresAt": "2099-12-31T23:59:59Z"
        }),
        Some("idem-enrich-assign-1"),
    );
    let (status, assign_payload) = post_rpc(&client, &server, &assign).await?;
    assert_eq!(status, 200);

    // Claimed task → isClaimed true, isStale false, currentLoopId/currentHat resolved
    let claimed = &assign_payload["result"]["task"];
    assert_eq!(claimed["isClaimed"], true);
    assert_eq!(claimed["isStale"], false);
    assert_eq!(claimed["currentLoopId"], "loop-enrich-1");
    assert_eq!(claimed["currentHat"], "builder");

    // task.get on claimed task
    let get2 = rpc_request(
        "req-enrich-get-2",
        "task.get",
        json!({ "id": "task-enrich-1" }),
        None,
    );
    let (status, get2_payload) = post_rpc(&client, &server, &get2).await?;
    assert_eq!(status, 200);
    let claimed_get = &get2_payload["result"]["task"];
    assert_eq!(claimed_get["isClaimed"], true);
    assert_eq!(claimed_get["isStale"], false);
    assert_eq!(claimed_get["currentLoopId"], "loop-enrich-1");
    assert_eq!(claimed_get["currentHat"], "builder");

    // task.list on claimed task
    let list2 = rpc_request("req-enrich-list-2", "task.list", json!({}), None);
    let (status, list2_payload) = post_rpc(&client, &server, &list2).await?;
    assert_eq!(status, 200);
    let listed2 = list2_payload["result"]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "task-enrich-1")
        .expect("task-enrich-1 should appear in list");
    assert_eq!(listed2["isClaimed"], true);
    assert_eq!(listed2["currentLoopId"], "loop-enrich-1");
    assert_eq!(listed2["currentHat"], "builder");

    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn task_enrichment_stale_lease_detected() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    // Create a task with an already-expired lease
    let create = rpc_request(
        "req-stale-create-1",
        "task.create",
        json!({
            "id": "task-stale-1",
            "title": "Stale lease enrichment test",
            "status": "in_progress",
            "assigneeWorkerId": "worker-ghost",
            "claimedAt": "2020-01-01T00:00:00Z",
            "leaseExpiresAt": "2020-01-01T00:05:00Z"
        }),
        Some("idem-stale-create-1"),
    );
    let (status, create_payload) = post_rpc(&client, &server, &create).await?;
    assert_eq!(status, 200);

    // Expired lease → isClaimed true, isStale true
    // Worker doesn't exist in registry so currentLoopId/currentHat are null
    let task = &create_payload["result"]["task"];
    assert_eq!(task["isClaimed"], true);
    assert_eq!(task["isStale"], true);
    assert!(task["currentLoopId"].is_null());
    assert!(task["currentHat"].is_null());

    // task.get confirms stale enrichment
    let get = rpc_request(
        "req-stale-get-1",
        "task.get",
        json!({ "id": "task-stale-1" }),
        None,
    );
    let (status, get_payload) = post_rpc(&client, &server, &get).await?;
    assert_eq!(status, 200);
    let stale_get = &get_payload["result"]["task"];
    assert_eq!(stale_get["isClaimed"], true);
    assert_eq!(stale_get["isStale"], true);
    assert!(stale_get["currentLoopId"].is_null());
    assert!(stale_get["currentHat"].is_null());

    server.stop().await;
    Ok(())
}

// ---------------------------------------------------------------------------
// 9.5 — Loop enrichment fields (workerId, workerStatus, currentTaskId,
//        currentHat, lastHeartbeatAt)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn loop_enrichment_fields_with_and_without_worker() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();
    let ws = server.workspace_path().display().to_string();

    // ── Set up two loops: one via registry, one via merge queue ──────────
    // (LoopRegistry::register dedupes by PID, so we can only register one
    //  entry from the test process; use the merge queue for the second.)
    let worktree_a = server.workspace_path().join(".worktrees/loop-enrich-a");
    std::fs::create_dir_all(&worktree_a)?;

    let loop_registry = LoopRegistry::new(server.workspace_path());
    loop_registry.register(LoopEntry::with_id(
        "loop-enrich-a",
        "Loop A with worker",
        Some(worktree_a.to_string_lossy().to_string()),
        ws.clone(),
    ))?;

    let merge_queue = MergeQueue::new(server.workspace_path());
    merge_queue.enqueue("loop-enrich-b", "Loop B without worker")?;

    // ── List before any workers — all enrichment fields should be null ───
    let list1 = rpc_request(
        "req-loop-enrich-list-1",
        "loop.list",
        json!({ "includeTerminal": false }),
        None,
    );
    let (status, list1_payload) = post_rpc(&client, &server, &list1).await?;
    assert_eq!(status, 200);

    let loops1 = list1_payload["result"]["loops"].as_array().unwrap();
    let loop_a1 = loops1
        .iter()
        .find(|l| l["id"] == "loop-enrich-a")
        .expect("loop-enrich-a should appear in list");
    assert!(loop_a1["workerId"].is_null());
    assert!(loop_a1["workerStatus"].is_null());
    assert!(loop_a1["currentTaskId"].is_null());
    assert!(loop_a1["currentHat"].is_null());
    assert!(loop_a1["lastHeartbeatAt"].is_null());

    let loop_b1 = loops1
        .iter()
        .find(|l| l["id"] == "loop-enrich-b")
        .expect("loop-enrich-b should appear in list");
    assert!(loop_b1["workerId"].is_null());
    assert!(loop_b1["workerStatus"].is_null());
    assert!(loop_b1["currentTaskId"].is_null());
    assert!(loop_b1["currentHat"].is_null());
    assert!(loop_b1["lastHeartbeatAt"].is_null());

    // ── Register a worker assigned to loop-enrich-a ─────────────────────
    let reg = rpc_request(
        "req-loop-enrich-reg-w1",
        "worker.register",
        json!({
            "workerId": "worker-loop-enrich-1",
            "workerName": "Loop Enrichment Worker",
            "loopId": "loop-enrich-a",
            "backend": "claude",
            "workspaceRoot": ws,
            "status": "idle",
            "lastHeartbeatAt": "2026-03-15T12:00:00Z"
        }),
        Some("idem-loop-enrich-reg-w1"),
    );
    let (status, _) = post_rpc(&client, &server, &reg).await?;
    assert_eq!(status, 200);

    // Heartbeat to set currentTaskId and currentHat
    let hb = rpc_request(
        "req-loop-enrich-hb-w1",
        "worker.heartbeat",
        json!({
            "workerId": "worker-loop-enrich-1",
            "status": "busy",
            "currentTaskId": "task-from-loop-a",
            "currentHat": "planner",
            "lastHeartbeatAt": "2026-03-15T12:01:00Z"
        }),
        Some("idem-loop-enrich-hb-w1"),
    );
    let (status, _) = post_rpc(&client, &server, &hb).await?;
    assert_eq!(status, 200);

    // ── List again — loop A should be enriched, loop B still null ───────
    let list2 = rpc_request(
        "req-loop-enrich-list-2",
        "loop.list",
        json!({ "includeTerminal": false }),
        None,
    );
    let (status, list2_payload) = post_rpc(&client, &server, &list2).await?;
    assert_eq!(status, 200);

    let loops2 = list2_payload["result"]["loops"].as_array().unwrap();

    // Loop A: enriched with worker data
    let loop_a2 = loops2
        .iter()
        .find(|l| l["id"] == "loop-enrich-a")
        .expect("loop-enrich-a should appear in list");
    assert_eq!(loop_a2["workerId"], "worker-loop-enrich-1");
    assert_eq!(loop_a2["workerStatus"], "busy");
    assert_eq!(loop_a2["currentTaskId"], "task-from-loop-a");
    assert_eq!(loop_a2["currentHat"], "planner");
    assert_eq!(loop_a2["lastHeartbeatAt"], "2026-03-15T12:01:00Z");

    // Loop B: no worker → all null
    let loop_b2 = loops2
        .iter()
        .find(|l| l["id"] == "loop-enrich-b")
        .expect("loop-enrich-b should appear in list");
    assert!(loop_b2["workerId"].is_null());
    assert!(loop_b2["workerStatus"].is_null());
    assert!(loop_b2["currentTaskId"].is_null());
    assert!(loop_b2["currentHat"].is_null());
    assert!(loop_b2["lastHeartbeatAt"].is_null());

    server.stop().await;
    Ok(())
}
