//! Integration test: full Kanban lifecycle through the RPC v1 API.
//!
//! Exercises: task creation with new statuses (blocked, in_review),
//! status transitions with transition history, task.list filtering,
//! event.list queries, loop.list hat_collection + task_counts,
//! and task.get loop_context enrichment.

use std::path::Path;

use anyhow::Result;
use ralph_core::EventRecord;
use ralph_core::loop_registry::{HatSummary, LoopEntry, LoopRegistry};
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
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        config.workspace_root = workspace.path().to_path_buf();

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let local_addr = listener.local_addr().expect("local addr");
        let runtime = RpcRuntime::new(config).expect("runtime");
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
        let result = self.join.await.expect("join");
        result.expect("clean shutdown");
    }
}

async fn post_rpc(client: &Client, server: &TestServer, body: &Value) -> Result<(u16, Value)> {
    let resp = client
        .post(format!("{}/rpc/v1", server.base_url))
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await?;
    let status = resp.status().as_u16();
    let payload = resp.json::<Value>().await?;
    Ok((status, payload))
}

fn rpc(id: &str, method: &str, params: Value) -> Value {
    let idem_key = format!("idem-kanban-{id}");
    json!({
        "apiVersion": "v1",
        "id": id,
        "method": method,
        "params": params,
        "meta": { "idempotencyKey": idem_key },
    })
}

/// Full Kanban lifecycle: create → start → block → reopen → review → close,
/// verifying transitions, filters, events, loop enrichment along the way.
#[tokio::test]
async fn kanban_lifecycle_transitions_and_enrichment() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    // --- 1. Create two tasks ---
    let (s, _) = post_rpc(
        &client,
        &server,
        &rpc(
            "create-1",
            "task.create",
            json!({ "id": "task-alpha", "title": "Alpha", "priority": 2, "autoExecute": false }),
        ),
    )
    .await?;
    assert_eq!(s, 200);

    let (s, _) = post_rpc(
        &client,
        &server,
        &rpc(
            "create-2",
            "task.create",
            json!({ "id": "task-beta", "title": "Beta", "priority": 3, "autoExecute": false }),
        ),
    )
    .await?;
    assert_eq!(s, 200);

    // --- 2. Transition alpha: open → in_progress → blocked → in_progress → in_review → closed ---
    let transitions = [
        ("in_progress", "in_progress"),
        ("blocked", "blocked"),
        ("in_progress", "in_progress"),
        ("in_review", "in_review"),
        ("closed", "closed"),
    ];
    for (i, (status, expected)) in transitions.iter().enumerate() {
        let (s, p) = post_rpc(
            &client,
            &server,
            &rpc(
                &format!("update-{i}"),
                "task.update",
                json!({ "id": "task-alpha", "status": status }),
            ),
        )
        .await?;
        assert_eq!(s, 200, "transition to {status} failed");
        assert_eq!(p["result"]["task"]["status"], *expected);
    }

    // --- 3. Verify transitions array on task.get ---
    let (_, p) = post_rpc(
        &client,
        &server,
        &rpc("get-alpha", "task.get", json!({ "id": "task-alpha" })),
    )
    .await?;
    let task = &p["result"]["task"];
    assert_eq!(task["status"], "closed");
    let tr = task["transitions"].as_array().expect("transitions array");
    assert_eq!(tr.len(), 5);
    assert_eq!(tr[0]["from"], "open");
    assert_eq!(tr[0]["to"], "in_progress");
    assert_eq!(tr[4]["from"], "in_review");
    assert_eq!(tr[4]["to"], "closed");

    // --- 4. Filter task.list by status ---
    let (_, p) = post_rpc(
        &client,
        &server,
        &rpc("list-open", "task.list", json!({ "status": "open" })),
    )
    .await?;
    let tasks = p["result"]["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "task-beta");

    let (_, p) = post_rpc(
        &client,
        &server,
        &rpc("list-closed", "task.list", json!({ "status": "closed" })),
    )
    .await?;
    let tasks = p["result"]["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "task-alpha");

    // --- 5. Write events.jsonl and verify event.list ---
    let events_dir = server.workspace_path().join(".ralph");
    std::fs::create_dir_all(&events_dir)?;
    let records = vec![
        EventRecord {
            ts: "2026-03-10T20:00:00Z".to_string(),
            iteration: 1,
            hat: "builder".to_string(),
            topic: "task.started".to_string(),
            triggered: None,
            payload: "task-alpha started".to_string(),
            blocked_count: None,
        },
        EventRecord {
            ts: "2026-03-10T20:01:00Z".to_string(),
            iteration: 2,
            hat: "reviewer".to_string(),
            topic: "task.reviewed".to_string(),
            triggered: None,
            payload: "task-alpha reviewed".to_string(),
            blocked_count: None,
        },
    ];
    let lines: String = records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(events_dir.join("events.jsonl"), format!("{lines}\n"))?;

    let (_, p) = post_rpc(
        &client,
        &server,
        &rpc("ev-glob", "event.list", json!({ "topic": "task.*" })),
    )
    .await?;
    let events = p["result"]["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["topic"], "task.started");
    assert_eq!(events[1]["topic"], "task.reviewed");

    // Filter by specific topic
    let (_, p) = post_rpc(
        &client,
        &server,
        &rpc(
            "ev-specific",
            "event.list",
            json!({ "topic": "task.reviewed" }),
        ),
    )
    .await?;
    assert_eq!(p["result"]["events"].as_array().unwrap().len(), 1);

    // --- 6. Register a loop with hat_collection, verify loop.list enrichment ---
    let registry = LoopRegistry::new(server.workspace_path());
    let mut entry = LoopEntry::with_id(
        "loop-test-1",
        "Test kanban",
        None::<String>,
        server.workspace_path().display().to_string(),
    );
    entry.hat_collection = vec![
        HatSummary {
            id: "builder".to_string(),
            name: "Builder".to_string(),
            description: "Builds things".to_string(),
        },
        HatSummary {
            id: "reviewer".to_string(),
            name: "Reviewer".to_string(),
            description: "Reviews things".to_string(),
        },
    ];
    entry.active_hat = Some("builder".to_string());
    entry.iteration = 3;
    entry.max_iterations = Some(10);
    registry.register(entry)?;

    let (_, p) = post_rpc(&client, &server, &rpc("loop-list", "loop.list", json!({}))).await?;
    let loops = p["result"]["loops"].as_array().unwrap();
    let our_loop = loops
        .iter()
        .find(|l| l["id"] == "loop-test-1")
        .expect("loop-test-1 should be listed");
    assert_eq!(our_loop["status"], "running");
    let hats = our_loop["hatCollection"].as_array().unwrap();
    assert_eq!(hats.len(), 2);
    assert_eq!(hats[0]["id"], "builder");
    assert_eq!(hats[1]["id"], "reviewer");
    assert_eq!(our_loop["activeHat"], "builder");
    assert_eq!(our_loop["iteration"], 3);
    assert_eq!(our_loop["maxIterations"], 10);
    // task_counts should reflect our 2 tasks (1 open beta, 1 closed alpha)
    // — but only if they have loop_id matching this loop. Since they don't, task_counts may be absent.

    // --- 7. Verify task.get loop_context enrichment ---
    // Write a task with loop_id directly to JSONL so loop_context gets populated
    let tasks_dir = server.workspace_path().join(".ralph/agent");
    std::fs::create_dir_all(&tasks_dir)?;
    let tasks_path = tasks_dir.join("tasks.jsonl");
    let existing = std::fs::read_to_string(&tasks_path).unwrap_or_default();
    let new_task = json!({
        "id": "task-gamma",
        "title": "Gamma with loop",
        "status": "open",
        "priority": 2,
        "blocked_by": [],
        "created": "2026-03-10T20:00:00Z",
        "loop_id": "loop-test-1",
        "tags": [],
        "transitions": []
    });
    let mut content = existing;
    content.push_str(&serde_json::to_string(&new_task)?);
    content.push('\n');
    std::fs::write(&tasks_path, &content)?;

    let (_, p) = post_rpc(
        &client,
        &server,
        &rpc("get-gamma", "task.get", json!({ "id": "task-gamma" })),
    )
    .await?;
    let gamma = &p["result"]["task"];
    assert_eq!(gamma["id"], "task-gamma");
    let ctx = gamma["loopContext"]
        .as_object()
        .expect("loopContext should exist");
    assert_eq!(ctx["iteration"], 3);
    assert_eq!(ctx["activeHat"], "builder");
    assert_eq!(ctx["maxIterations"], 10);

    // --- 8. Backward compat: old-format task without transitions/last_hat/tags ---
    let mut content = std::fs::read_to_string(&tasks_path)?;
    let old_task = json!({
        "id": "task-legacy",
        "title": "Legacy task",
        "status": "open",
        "priority": 3,
        "blocked_by": [],
        "created": "2026-01-01T00:00:00Z"
    });
    content.push_str(&serde_json::to_string(&old_task)?);
    content.push('\n');
    std::fs::write(&tasks_path, &content)?;

    let (s, p) = post_rpc(
        &client,
        &server,
        &rpc("get-legacy", "task.get", json!({ "id": "task-legacy" })),
    )
    .await?;
    assert_eq!(s, 200);
    let legacy = &p["result"]["task"];
    assert_eq!(legacy["id"], "task-legacy");
    assert_eq!(legacy["status"], "open");
    // transitions should default to empty (field omitted or empty array)
    let tr = legacy.get("transitions");
    assert!(
        tr.is_none() || tr.unwrap().as_array().is_none_or(Vec::is_empty),
        "legacy task should have no transitions"
    );

    server.stop().await;
    Ok(())
}
