use std::fs;
use std::path::Path;

use ralph_api::errors::RpcErrorCode;
use ralph_api::task_domain::{TaskCreateParams, TaskDomain};
use ralph_api::worker_domain::{WorkerDomain, WorkerHeartbeatInput, WorkerRecord, WorkerStatus};
use serde_json::{Value, json};

fn sample_worker(
    worker_id: &str,
    worker_name: &str,
    workspace_root: &str,
    status: WorkerStatus,
) -> WorkerRecord {
    WorkerRecord {
        worker_id: worker_id.to_string(),
        worker_name: worker_name.to_string(),
        loop_id: format!("loop-{worker_id}"),
        backend: "claude".to_string(),
        workspace_root: workspace_root.to_string(),
        current_task_id: None,
        current_hat: None,
        status,
        last_heartbeat_at: "2026-03-14T22:30:00Z".to_string(),
    }
}

fn sample_heartbeat(worker_id: &str, status: WorkerStatus) -> WorkerHeartbeatInput {
    WorkerHeartbeatInput {
        worker_id: worker_id.to_string(),
        status,
        current_task_id: Some("task-123".to_string()),
        current_hat: Some("builder".to_string()),
        last_heartbeat_at: "2026-03-14T23:00:00Z".to_string(),
    }
}

fn create_task(workspace_root: &Path, id: &str, title: &str, status: &str) {
    TaskDomain::new(workspace_root)
        .create(TaskCreateParams {
            id: id.to_string(),
            title: title.to_string(),
            status: Some(status.to_string()),
            priority: None,
            blocked_by: None,
            merge_loop_prompt: None,
            assignee_worker_id: None,
            claimed_at: None,
            lease_expires_at: None,
        })
        .expect("task fixture should persist");
}

#[test]
fn register_list_get_and_deregister_round_trip() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    assert!(
        domain
            .list()
            .expect("empty registry should list cleanly")
            .is_empty()
    );

    let worker = sample_worker(
        "worker-brisk-maple",
        "brisk-maple",
        &workspace_root,
        WorkerStatus::Idle,
    );
    assert_eq!(
        domain
            .register(worker.clone())
            .expect("register should succeed"),
        worker
    );
    assert_eq!(
        domain
            .list()
            .expect("registered worker should appear in list"),
        vec![worker.clone()]
    );
    assert_eq!(
        domain
            .get("worker-brisk-maple")
            .expect("worker should load"),
        worker.clone()
    );

    let updated_worker = WorkerRecord {
        current_task_id: Some("task-123".to_string()),
        current_hat: Some("builder".to_string()),
        status: WorkerStatus::Busy,
        last_heartbeat_at: "2026-03-14T22:35:00Z".to_string(),
        ..worker.clone()
    };
    assert_eq!(
        domain
            .register(updated_worker.clone())
            .expect("upsert should succeed"),
        updated_worker.clone()
    );
    assert_eq!(
        domain.list().expect("updated worker should appear in list"),
        vec![updated_worker.clone()]
    );
    assert_eq!(
        domain
            .get("worker-brisk-maple")
            .expect("updated worker should load"),
        updated_worker.clone()
    );

    let registry_path = workspace.path().join(".ralph/workers.json");
    let snapshot: Value = serde_json::from_str(
        &fs::read_to_string(&registry_path).expect("worker registry should be persisted"),
    )
    .expect("worker registry JSON should parse");
    assert_eq!(snapshot["workers"], json!([updated_worker.clone()]));

    domain
        .deregister("worker-brisk-maple")
        .expect("deregister should succeed");
    assert!(
        domain
            .list()
            .expect("empty registry should list after deregister")
            .is_empty()
    );

    let snapshot: Value = serde_json::from_str(
        &fs::read_to_string(&registry_path).expect("worker registry should remain readable"),
    )
    .expect("worker registry JSON should still parse");
    assert_eq!(snapshot["workers"], json!([]));

    let error = domain
        .get("worker-brisk-maple")
        .expect_err("removed worker should not be returned");
    assert_eq!(error.code, RpcErrorCode::NotFound);
    assert_eq!(
        error.message,
        "Worker with id 'worker-brisk-maple' not found"
    );
    assert_eq!(
        error.details,
        Some(json!({ "workerId": "worker-brisk-maple" }))
    );
}

#[test]
fn reloads_persisted_workers_from_disk() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let worker_zeta = WorkerRecord {
        current_hat: Some("reviewer".to_string()),
        status: WorkerStatus::Blocked,
        last_heartbeat_at: "2026-03-14T22:40:00Z".to_string(),
        ..sample_worker("worker-zeta", "zeta", &workspace_root, WorkerStatus::Idle)
    };
    let worker_alpha = WorkerRecord {
        current_task_id: Some("task-7".to_string()),
        status: WorkerStatus::Busy,
        last_heartbeat_at: "2026-03-14T22:45:00Z".to_string(),
        ..sample_worker("worker-alpha", "alpha", &workspace_root, WorkerStatus::Idle)
    };

    domain
        .register(worker_zeta.clone())
        .expect("first worker should persist");
    domain
        .register(worker_alpha.clone())
        .expect("second worker should persist");
    drop(domain);

    let reloaded = WorkerDomain::new(workspace.path()).expect("worker domain should reload");
    assert_eq!(
        reloaded
            .list()
            .expect("reloaded registry should list cleanly"),
        vec![worker_alpha.clone(), worker_zeta.clone()]
    );
    assert_eq!(
        reloaded
            .get("worker-alpha")
            .expect("reloaded worker should be returned"),
        worker_alpha
    );
    assert_eq!(
        reloaded
            .get("worker-zeta")
            .expect("reloaded worker should be returned"),
        worker_zeta
    );
}

#[test]
fn multiple_handles_merge_registry_updates_and_refresh_reads() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain_a =
        WorkerDomain::new(workspace.path()).expect("first worker domain should initialize");
    let mut domain_b =
        WorkerDomain::new(workspace.path()).expect("second worker domain should initialize");

    let worker_alpha = sample_worker("worker-alpha", "alpha", &workspace_root, WorkerStatus::Idle);
    let worker_beta = WorkerRecord {
        current_task_id: Some("task-9".to_string()),
        current_hat: Some("builder".to_string()),
        status: WorkerStatus::Busy,
        last_heartbeat_at: "2026-03-14T22:47:00Z".to_string(),
        ..sample_worker("worker-beta", "beta", &workspace_root, WorkerStatus::Idle)
    };

    domain_a
        .register(worker_alpha.clone())
        .expect("first handle should register its worker");
    assert_eq!(
        domain_b
            .list()
            .expect("second handle should see first handle writes"),
        vec![worker_alpha.clone()]
    );
    assert_eq!(
        domain_b
            .get("worker-alpha")
            .expect("second handle should read first handle worker"),
        worker_alpha.clone()
    );

    domain_b
        .register(worker_beta.clone())
        .expect("second handle should merge instead of clobbering the registry");

    assert_eq!(
        domain_a
            .list()
            .expect("first handle should refresh reads after second handle writes"),
        vec![worker_alpha.clone(), worker_beta.clone()]
    );
    assert_eq!(
        domain_a
            .get("worker-beta")
            .expect("first handle should read second handle worker without reloading"),
        worker_beta.clone()
    );
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded domain should include both workers")
            .list()
            .expect("reloaded registry should list both workers"),
        vec![worker_alpha.clone(), worker_beta.clone()]
    );

    domain_a
        .deregister("worker-alpha")
        .expect("first handle should remove only its worker");

    assert_eq!(
        domain_b
            .list()
            .expect("second handle should refresh after first handle removes a worker"),
        vec![worker_beta.clone()]
    );
    let error = domain_b
        .get("worker-alpha")
        .expect_err("removed worker should not remain visible to stale handles");
    assert_eq!(error.code, RpcErrorCode::NotFound);
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded domain should preserve the remaining worker")
            .list()
            .expect("reloaded registry should list the survivor"),
        vec![worker_beta]
    );
}

#[test]
fn claim_next_claims_one_ready_task_and_prevents_duplicate_claims_across_handles() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain_a =
        WorkerDomain::new(workspace.path()).expect("first worker domain should initialize");
    let mut domain_b =
        WorkerDomain::new(workspace.path()).expect("second worker domain should initialize");

    let worker_alpha = sample_worker("worker-alpha", "alpha", &workspace_root, WorkerStatus::Idle);
    let worker_beta = sample_worker("worker-beta", "beta", &workspace_root, WorkerStatus::Idle);
    domain_a
        .register(worker_alpha.clone())
        .expect("first worker should register");
    domain_b
        .register(worker_beta.clone())
        .expect("second worker should register");

    create_task(workspace.path(), "task-ready", "Ready Task", "ready");

    let first_claim = domain_a
        .claim_next("worker-alpha")
        .expect("idle worker should claim the only ready task");
    let claimed_task = first_claim
        .task
        .expect("claiming a ready task should return that task");
    assert_eq!(claimed_task.id, "task-ready");
    assert_eq!(claimed_task.status, "in_progress");
    assert_eq!(
        claimed_task.assignee_worker_id.as_deref(),
        Some("worker-alpha")
    );
    assert!(
        claimed_task
            .claimed_at
            .as_deref()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        claimed_task
            .lease_expires_at
            .as_deref()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(first_claim.worker.worker_id, "worker-alpha");
    assert_eq!(first_claim.worker.status, WorkerStatus::Busy);
    assert_eq!(
        first_claim.worker.current_task_id.as_deref(),
        Some("task-ready")
    );
    assert_eq!(first_claim.worker.current_hat, None);

    let fresh_task = TaskDomain::new(workspace.path())
        .get("task-ready")
        .expect("claimed task should reload from disk");
    assert_eq!(fresh_task.status, "in_progress");
    assert_eq!(
        fresh_task.assignee_worker_id.as_deref(),
        Some("worker-alpha")
    );
    assert_eq!(fresh_task.claimed_at, claimed_task.claimed_at);
    assert_eq!(fresh_task.lease_expires_at, claimed_task.lease_expires_at);
    assert!(TaskDomain::new(workspace.path()).ready().is_empty());

    assert_eq!(
        domain_b
            .get("worker-alpha")
            .expect("other handle should see claimed worker state")
            .current_task_id
            .as_deref(),
        Some("task-ready")
    );

    let second_claim = domain_b
        .claim_next("worker-beta")
        .expect("second worker should see an empty ready queue after the first claim");
    assert!(
        second_claim.task.is_none(),
        "second worker must not be able to double-claim the same ready task"
    );
    assert_eq!(second_claim.worker, worker_beta.clone());
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded worker domain should initialize")
            .list()
            .expect("reloaded worker list should stay readable"),
        vec![first_claim.worker.clone(), worker_beta.clone()]
    );

    let task_snapshot: Value = serde_json::from_str(
        &fs::read_to_string(workspace.path().join(".ralph/api/tasks-v1.json"))
            .expect("task snapshot should be persisted"),
    )
    .expect("task snapshot JSON should parse");
    let tasks = task_snapshot["tasks"]
        .as_array()
        .expect("task snapshot should store an array of tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], json!("task-ready"));
    assert_eq!(tasks[0]["status"], json!("in_progress"));
    assert_eq!(tasks[0]["assigneeWorkerId"], json!("worker-alpha"));
    assert!(
        tasks[0]["claimedAt"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        tasks[0]["leaseExpiresAt"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

#[test]
fn claim_next_returns_none_when_no_ready_tasks_exist_without_mutation() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let worker = sample_worker("worker-idle", "idle", &workspace_root, WorkerStatus::Idle);
    domain
        .register(worker.clone())
        .expect("worker should register before claiming");
    create_task(workspace.path(), "task-backlog", "Backlog Task", "backlog");

    let claim = domain
        .claim_next("worker-idle")
        .expect("worker should get an empty result when no ready tasks exist");
    assert!(claim.task.is_none());
    assert_eq!(claim.worker, worker.clone());

    let fresh_task = TaskDomain::new(workspace.path())
        .get("task-backlog")
        .expect("non-ready task should remain readable");
    assert_eq!(fresh_task.status, "backlog");
    assert_eq!(fresh_task.assignee_worker_id, None);
    assert_eq!(fresh_task.claimed_at, None);
    assert_eq!(fresh_task.lease_expires_at, None);
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded worker domain should initialize")
            .get("worker-idle")
            .expect("worker should remain idle after empty claim"),
        worker
    );
}

#[test]
fn claim_next_rejects_unknown_and_non_idle_workers_without_mutating_ready_queue() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let busy_worker = WorkerRecord {
        current_task_id: Some("task-held".to_string()),
        current_hat: Some("builder".to_string()),
        status: WorkerStatus::Busy,
        last_heartbeat_at: "2026-03-14T23:10:00Z".to_string(),
        ..sample_worker("worker-busy", "busy", &workspace_root, WorkerStatus::Idle)
    };
    domain
        .register(busy_worker.clone())
        .expect("busy worker should register before rejection checks");
    create_task(workspace.path(), "task-ready", "Ready Task", "ready");

    let missing_error = domain
        .claim_next("worker-missing")
        .expect_err("unknown workers should be rejected");
    assert_eq!(missing_error.code, RpcErrorCode::NotFound);
    assert_eq!(
        missing_error.message,
        "Worker with id 'worker-missing' not found"
    );
    assert_eq!(
        missing_error.details,
        Some(json!({ "workerId": "worker-missing" }))
    );

    let busy_error = domain
        .claim_next("worker-busy")
        .expect_err("busy workers should not claim a new task");
    assert_eq!(busy_error.code, RpcErrorCode::PreconditionFailed);
    assert_eq!(
        busy_error.message,
        "Worker with id 'worker-busy' must be idle and unassigned before claiming the next task"
    );
    assert_eq!(
        busy_error.details,
        Some(json!({
            "workerId": "worker-busy",
            "status": "busy",
            "currentTaskId": "task-held"
        }))
    );

    let ready_task = TaskDomain::new(workspace.path())
        .get("task-ready")
        .expect("ready task should remain readable after rejected claims");
    assert_eq!(ready_task.status, "ready");
    assert_eq!(ready_task.assignee_worker_id, None);
    assert_eq!(ready_task.claimed_at, None);
    assert_eq!(ready_task.lease_expires_at, None);
    assert_eq!(
        TaskDomain::new(workspace.path())
            .ready()
            .into_iter()
            .map(|task| task.id)
            .collect::<Vec<_>>(),
        vec!["task-ready".to_string()]
    );
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded worker domain should initialize")
            .get("worker-busy")
            .expect("busy worker should remain unchanged after rejected claims"),
        busy_worker
    );
}

#[test]
fn heartbeat_updates_worker_state_across_handles_and_reload() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain_a =
        WorkerDomain::new(workspace.path()).expect("first worker domain should initialize");
    let mut domain_b =
        WorkerDomain::new(workspace.path()).expect("second worker domain should initialize");

    let worker = sample_worker(
        "worker-heartbeat",
        "heartbeat",
        &workspace_root,
        WorkerStatus::Idle,
    );
    domain_a
        .register(worker.clone())
        .expect("worker should register before heartbeats");

    let busy_heartbeat = sample_heartbeat("worker-heartbeat", WorkerStatus::Busy);
    let busy_worker = WorkerRecord {
        current_task_id: busy_heartbeat.current_task_id.clone(),
        current_hat: busy_heartbeat.current_hat.clone(),
        status: busy_heartbeat.status,
        last_heartbeat_at: busy_heartbeat.last_heartbeat_at.clone(),
        ..worker.clone()
    };

    assert_eq!(
        domain_a
            .heartbeat(busy_heartbeat)
            .expect("heartbeat should update live worker state"),
        busy_worker.clone()
    );
    assert_eq!(
        domain_b
            .get("worker-heartbeat")
            .expect("stale handle should read refreshed heartbeat state"),
        busy_worker.clone()
    );
    assert_eq!(
        domain_b
            .list()
            .expect("stale handle should list refreshed heartbeat state"),
        vec![busy_worker.clone()]
    );

    let registry_path = workspace.path().join(".ralph/workers.json");
    let snapshot: Value = serde_json::from_str(
        &fs::read_to_string(&registry_path).expect("worker registry should be persisted"),
    )
    .expect("worker registry JSON should parse after heartbeat update");
    assert_eq!(snapshot["workers"], json!([busy_worker.clone()]));

    let cleared_heartbeat = WorkerHeartbeatInput {
        current_task_id: None,
        current_hat: None,
        last_heartbeat_at: "2026-03-14T23:05:00Z".to_string(),
        ..sample_heartbeat("worker-heartbeat", WorkerStatus::Idle)
    };
    let cleared_worker = WorkerRecord {
        current_task_id: None,
        current_hat: None,
        status: WorkerStatus::Idle,
        last_heartbeat_at: "2026-03-14T23:05:00Z".to_string(),
        ..worker.clone()
    };

    assert_eq!(
        domain_b
            .heartbeat(cleared_heartbeat)
            .expect("heartbeat should clear optional live fields"),
        cleared_worker.clone()
    );
    assert_eq!(
        domain_a
            .get("worker-heartbeat")
            .expect("other handle should read cleared heartbeat state"),
        cleared_worker.clone()
    );
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded worker domain should initialize")
            .list()
            .expect("reloaded domain should show latest heartbeat state"),
        vec![cleared_worker.clone()]
    );

    let snapshot: Value = serde_json::from_str(
        &fs::read_to_string(&registry_path).expect("worker registry should remain readable"),
    )
    .expect("worker registry JSON should parse after clearing live fields");
    assert_eq!(snapshot["workers"], json!([cleared_worker]));
}

#[test]
fn heartbeat_rejects_unknown_workers_and_invalid_live_state_inputs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let worker = sample_worker("worker-gamma", "gamma", &workspace_root, WorkerStatus::Idle);
    domain
        .register(worker.clone())
        .expect("worker should register before validation checks");

    let error = domain
        .heartbeat(sample_heartbeat("worker-missing", WorkerStatus::Busy))
        .expect_err("unknown workers should not be heartbeated implicitly");
    assert_eq!(error.code, RpcErrorCode::NotFound);
    assert_eq!(error.message, "Worker with id 'worker-missing' not found");
    assert_eq!(error.details, Some(json!({ "workerId": "worker-missing" })));

    let invalid_cases = [
        (
            WorkerHeartbeatInput {
                worker_id: "   ".to_string(),
                ..sample_heartbeat("worker-gamma", WorkerStatus::Busy)
            },
            "worker.workerId must be a non-empty string",
        ),
        (
            WorkerHeartbeatInput {
                current_task_id: Some("   ".to_string()),
                ..sample_heartbeat("worker-gamma", WorkerStatus::Busy)
            },
            "worker.currentTaskId must be a non-empty string",
        ),
        (
            WorkerHeartbeatInput {
                current_hat: Some("   ".to_string()),
                ..sample_heartbeat("worker-gamma", WorkerStatus::Busy)
            },
            "worker.currentHat must be a non-empty string",
        ),
        (
            WorkerHeartbeatInput {
                last_heartbeat_at: "   ".to_string(),
                ..sample_heartbeat("worker-gamma", WorkerStatus::Busy)
            },
            "worker.lastHeartbeatAt must be a non-empty string",
        ),
    ];

    for (input, expected_message) in invalid_cases {
        let error = domain
            .heartbeat(input)
            .expect_err("invalid heartbeat input should be rejected");
        assert_eq!(error.code, RpcErrorCode::InvalidParams);
        assert_eq!(error.message, expected_message);
    }

    assert_eq!(
        domain
            .get("worker-gamma")
            .expect("invalid heartbeats should not mutate persisted worker state"),
        worker
    );
}

#[test]
fn rejects_partial_registry_entries_on_reload() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let registry_path = workspace.path().join(".ralph/workers.json");
    fs::create_dir_all(
        registry_path
            .parent()
            .expect("worker registry should have a parent directory"),
    )
    .expect("worker registry directory should be created");
    fs::write(
        &registry_path,
        serde_json::to_string_pretty(&json!({
            "workers": [{
                "workerId": "   ",
                "workerName": "broken-worker",
                "loopId": "loop-broken-worker",
                "backend": "claude",
                "workspaceRoot": workspace.path().display().to_string(),
                "status": "idle",
                "lastHeartbeatAt": "2026-03-14T22:50:00Z"
            }]
        }))
        .expect("registry fixture should serialize"),
    )
    .expect("registry fixture should write");

    let result = WorkerDomain::new(workspace.path());
    assert!(
        result.is_err(),
        "partial registry entries should fail to load"
    );

    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("expected partial registry entry to be rejected"),
    };
    assert_eq!(error.code, RpcErrorCode::Internal);
    assert!(error.message.contains("failed parsing worker registry"));
    assert!(
        error
            .message
            .contains("worker.workerId must be a non-empty string")
    );
}
