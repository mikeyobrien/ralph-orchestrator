use std::fs;
use std::path::Path;

use ralph_api::errors::RpcErrorCode;
use ralph_api::file_ownership::FileOwnershipRegistry;
use ralph_api::task_domain::{TaskCreateParams, TaskDomain, TaskUpdateInput};
use ralph_api::worker_domain::{
    WorkerDomain, WorkerHeartbeatInput, WorkerReclaimExpiredInput, WorkerRecord, WorkerStatus,
};
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
        iteration: None,
        max_iterations: None,
        registered_at: None,
    }
}

fn sample_heartbeat(worker_id: &str, status: WorkerStatus) -> WorkerHeartbeatInput {
    WorkerHeartbeatInput {
        worker_id: worker_id.to_string(),
        status,
        current_task_id: Some("task-123".to_string()),
        current_hat: Some("builder".to_string()),
        last_heartbeat_at: "2026-03-14T23:00:00Z".to_string(),
        iteration: None,
        max_iterations: None,
    }
}

fn create_task_with_lease(
    workspace_root: &Path,
    id: &str,
    title: &str,
    status: &str,
    assignee_worker_id: Option<&str>,
    claimed_at: Option<&str>,
    lease_expires_at: Option<&str>,
) {
    let mut domain = TaskDomain::new(workspace_root);

    // Determine creation status and transition path
    let (creation_status, transitions): (&str, &[&str]) = match status {
        "backlog" => ("backlog", &[]),
        "ready" => ("ready", &[]),
        "in_progress" => ("ready", &["in_progress"]),
        "in_review" => ("ready", &["in_progress", "in_review"]),
        "blocked" => ("ready", &["in_progress", "blocked"]),
        "done" => ("ready", &["in_progress", "done"]),
        "cancelled" => ("ready", &["cancelled"]),
        other => panic!("create_task_with_lease: unknown target status '{other}'"),
    };

    // Create with valid creation status
    domain
        .create(TaskCreateParams {
            id: id.to_string(),
            title: title.to_string(),
            status: Some(creation_status.to_string()),
            priority: None,
            blocked_by: None,
            merge_loop_prompt: None,
            assignee_worker_id: None,
            claimed_at: None,
            lease_expires_at: None,
            scope_files: None,
        })
        .expect("task fixture should persist");

    // Transition through valid states to reach the target status

    for &next_status in transitions {
        domain
            .update(TaskUpdateInput {
                id: id.to_string(),
                title: None,
                status: Some(next_status.to_string()),
                priority: None,
                blocked_by: None,
                assignee_worker_id: None,
                claimed_at: None,
                lease_expires_at: None,
            })
            .unwrap_or_else(|e| panic!("transition to '{next_status}' should succeed: {e:?}"));
    }

    // Apply lease fields if provided (separate update to avoid interfering with transitions)
    if assignee_worker_id.is_some() || claimed_at.is_some() || lease_expires_at.is_some() {
        domain
            .update(TaskUpdateInput {
                id: id.to_string(),
                title: None,
                status: None,
                priority: None,
                blocked_by: None,
                assignee_worker_id: assignee_worker_id.map(|v| Some(v.to_string())),
                claimed_at: claimed_at.map(|v| Some(v.to_string())),
                lease_expires_at: lease_expires_at.map(|v| Some(v.to_string())),
            })
            .expect("lease field update should succeed");
    }
}

fn create_task(workspace_root: &Path, id: &str, title: &str, status: &str) {
    create_task_with_lease(workspace_root, id, title, status, None, None, None);
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
fn reclaim_expired_requeues_stale_claims_and_marks_workers_dead_across_handles() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain_a =
        WorkerDomain::new(workspace.path()).expect("first worker domain should initialize");
    let mut domain_b =
        WorkerDomain::new(workspace.path()).expect("second worker domain should initialize");

    let stale_worker = WorkerRecord {
        current_task_id: Some("task-stale".to_string()),
        current_hat: Some("builder".to_string()),
        status: WorkerStatus::Busy,
        last_heartbeat_at: "2026-03-14T22:30:00Z".to_string(),
        ..sample_worker("worker-stale", "stale", &workspace_root, WorkerStatus::Idle)
    };
    domain_a
        .register(stale_worker.clone())
        .expect("stale worker should register before reclaiming");
    create_task_with_lease(
        workspace.path(),
        "task-stale",
        "Stale Task",
        "in_progress",
        Some("worker-stale"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:32:00Z"),
    );

    let reclaim = domain_b
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("expired claim should be reclaimed");
    assert_eq!(reclaim.tasks.len(), 1);
    let reclaimed_task = &reclaim.tasks[0];
    let expected_reason = "Task 'task-stale' reclaimed after worker 'worker-stale' lease expired at 2026-03-14T22:32:00Z (last heartbeat 2026-03-14T22:30:00Z, as of 2026-03-14T22:33:00Z)";
    assert_eq!(reclaimed_task.id, "task-stale");
    assert_eq!(reclaimed_task.status, "ready");
    assert_eq!(reclaimed_task.assignee_worker_id, None);
    assert_eq!(reclaimed_task.claimed_at, None);
    assert_eq!(reclaimed_task.lease_expires_at, None);
    assert_eq!(reclaimed_task.completed_at, None);
    assert_eq!(
        reclaimed_task.error_message.as_deref(),
        Some(expected_reason)
    );
    assert_eq!(reclaimed_task.updated_at, "2026-03-14T22:33:00Z");

    let reclaimed_worker = WorkerRecord {
        current_task_id: None,
        current_hat: None,
        status: WorkerStatus::Dead,
        ..stale_worker.clone()
    };
    assert_eq!(reclaim.workers, vec![reclaimed_worker.clone()]);

    let fresh_task = TaskDomain::new(workspace.path())
        .get("task-stale")
        .expect("reclaimed task should reload from disk");
    assert_eq!(fresh_task.status, "ready");
    assert_eq!(fresh_task.assignee_worker_id, None);
    assert_eq!(fresh_task.claimed_at, None);
    assert_eq!(fresh_task.lease_expires_at, None);
    assert_eq!(fresh_task.completed_at, None);
    assert_eq!(fresh_task.error_message.as_deref(), Some(expected_reason));
    assert_eq!(
        TaskDomain::new(workspace.path())
            .ready()
            .into_iter()
            .map(|task| task.id)
            .collect::<Vec<_>>(),
        vec!["task-stale".to_string()]
    );

    assert_eq!(
        domain_a
            .get("worker-stale")
            .expect("stale handle should refresh dead worker state after reclaim"),
        reclaimed_worker.clone()
    );
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded worker domain should initialize")
            .list()
            .expect("reloaded worker list should show the reclaimed worker"),
        vec![reclaimed_worker.clone()]
    );

    let task_snapshot: Value = serde_json::from_str(
        &fs::read_to_string(workspace.path().join(".ralph/api/tasks-v1.json"))
            .expect("task snapshot should be persisted"),
    )
    .expect("task snapshot JSON should parse after reclaim");
    let tasks = task_snapshot["tasks"]
        .as_array()
        .expect("task snapshot should store an array of tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], json!("task-stale"));
    assert_eq!(tasks[0]["status"], json!("ready"));
    assert!(tasks[0].get("assigneeWorkerId").is_none());
    assert!(tasks[0].get("claimedAt").is_none());
    assert!(tasks[0].get("leaseExpiresAt").is_none());
    assert_eq!(tasks[0]["errorMessage"], json!(expected_reason));

    let worker_snapshot: Value = serde_json::from_str(
        &fs::read_to_string(workspace.path().join(".ralph/workers.json"))
            .expect("worker registry should be persisted"),
    )
    .expect("worker registry JSON should parse after reclaim");
    let workers = worker_snapshot["workers"]
        .as_array()
        .expect("worker registry should store an array of workers");
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0]["workerId"], json!("worker-stale"));
    assert_eq!(workers[0]["status"], json!("dead"));
    assert!(workers[0].get("currentTaskId").is_none());
    assert!(workers[0].get("currentHat").is_none());
}

#[test]
fn reclaim_expired_skips_tasks_with_live_task_leases() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let worker = WorkerRecord {
        current_task_id: Some("task-live".to_string()),
        current_hat: Some("builder".to_string()),
        status: WorkerStatus::Busy,
        last_heartbeat_at: "2026-03-14T22:30:00Z".to_string(),
        ..sample_worker("worker-live", "live", &workspace_root, WorkerStatus::Idle)
    };
    domain
        .register(worker.clone())
        .expect("worker should register before reclaim checks");
    create_task_with_lease(
        workspace.path(),
        "task-live",
        "Live Task",
        "in_progress",
        Some("worker-live"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:35:00Z"),
    );

    let reclaim = domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("live task lease should skip reclaim");
    assert!(reclaim.tasks.is_empty());
    assert!(reclaim.workers.is_empty());

    let fresh_task = TaskDomain::new(workspace.path())
        .get("task-live")
        .expect("live task should remain readable");
    assert_eq!(fresh_task.status, "in_progress");
    assert_eq!(
        fresh_task.assignee_worker_id.as_deref(),
        Some("worker-live")
    );
    assert_eq!(
        fresh_task.claimed_at.as_deref(),
        Some("2026-03-14T22:30:00Z")
    );
    assert_eq!(
        fresh_task.lease_expires_at.as_deref(),
        Some("2026-03-14T22:35:00Z")
    );
    assert_eq!(fresh_task.error_message, None);
    assert!(TaskDomain::new(workspace.path()).ready().is_empty());
    assert_eq!(
        WorkerDomain::new(workspace.path())
            .expect("reloaded worker domain should initialize")
            .get("worker-live")
            .expect("worker should remain unchanged when reclaim is skipped"),
        worker
    );
}

#[test]
fn reclaim_expired_rejects_invalid_as_of_without_mutation() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let worker = WorkerRecord {
        current_task_id: Some("task-held".to_string()),
        current_hat: Some("builder".to_string()),
        status: WorkerStatus::Busy,
        last_heartbeat_at: "2026-03-14T22:30:00Z".to_string(),
        ..sample_worker("worker-held", "held", &workspace_root, WorkerStatus::Idle)
    };
    domain
        .register(worker.clone())
        .expect("worker should register before invalid reclaim input");
    create_task_with_lease(
        workspace.path(),
        "task-held",
        "Held Task",
        "in_progress",
        Some("worker-held"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:32:00Z"),
    );

    let error = domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "   ".to_string(),
        })
        .expect_err("invalid reclaim input should be rejected");
    assert_eq!(error.code, RpcErrorCode::InvalidParams);
    assert_eq!(error.message, "worker.asOf must be a non-empty string");

    let fresh_task = TaskDomain::new(workspace.path())
        .get("task-held")
        .expect("invalid reclaim input should leave the task untouched");
    assert_eq!(fresh_task.status, "in_progress");
    assert_eq!(
        fresh_task.assignee_worker_id.as_deref(),
        Some("worker-held")
    );
    assert_eq!(
        fresh_task.claimed_at.as_deref(),
        Some("2026-03-14T22:30:00Z")
    );
    assert_eq!(
        fresh_task.lease_expires_at.as_deref(),
        Some("2026-03-14T22:32:00Z")
    );
    assert_eq!(fresh_task.error_message, None);
    assert_eq!(
        domain
            .get("worker-held")
            .expect("invalid reclaim input should leave the worker untouched"),
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

// ── complete_task tests ────────────────────────────────────────────

fn busy_worker(
    worker_id: &str,
    worker_name: &str,
    workspace_root: &str,
    task_id: &str,
) -> WorkerRecord {
    WorkerRecord {
        current_task_id: Some(task_id.to_string()),
        status: WorkerStatus::Busy,
        ..sample_worker(worker_id, worker_name, workspace_root, WorkerStatus::Idle)
    }
}

#[test]
fn complete_task_success_sets_task_done_and_worker_idle() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    domain
        .register(busy_worker(
            "worker-alpha",
            "alpha",
            &workspace_root,
            "task-1",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-1",
        "Build feature",
        "in_progress",
        Some("worker-alpha"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:35:00Z"),
    );

    domain
        .complete_task("worker-alpha", "task-1", true, None)
        .expect("complete_task should succeed");

    let worker = domain
        .get("worker-alpha")
        .expect("worker should exist after completion");
    assert_eq!(worker.status, WorkerStatus::Idle);
    assert_eq!(worker.current_task_id, None);

    let task = TaskDomain::new(workspace.path())
        .get("task-1")
        .expect("task should exist after completion");
    assert_eq!(task.status, "done");
    assert!(task.completed_at.is_some(), "completed_at should be set");
}

#[test]
fn complete_task_failure_resets_task_to_ready_with_error() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    domain
        .register(busy_worker(
            "worker-alpha",
            "alpha",
            &workspace_root,
            "task-1",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-1",
        "Build feature",
        "in_progress",
        Some("worker-alpha"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:35:00Z"),
    );

    domain
        .complete_task(
            "worker-alpha",
            "task-1",
            false,
            Some("MaxIterations".to_string()),
        )
        .expect("complete_task with failure should succeed");

    let worker = domain
        .get("worker-alpha")
        .expect("worker should exist after failure completion");
    assert_eq!(worker.status, WorkerStatus::Idle);
    assert_eq!(worker.current_task_id, None);

    let task = TaskDomain::new(workspace.path())
        .get("task-1")
        .expect("task should exist after failure completion");
    assert_eq!(task.status, "ready");
    assert_eq!(task.assignee_worker_id, None);
    assert_eq!(task.error_message.as_deref(), Some("MaxIterations"));

    // Task should be re-claimable
    let ready_tasks = TaskDomain::new(workspace.path()).ready();
    assert_eq!(
        ready_tasks.into_iter().map(|t| t.id).collect::<Vec<_>>(),
        vec!["task-1".to_string()]
    );
}

#[test]
fn complete_task_failure_preserves_manual_cancellation() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    domain
        .register(busy_worker(
            "worker-alpha",
            "alpha",
            &workspace_root,
            "task-1",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-1",
        "Build feature",
        "in_progress",
        Some("worker-alpha"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:35:00Z"),
    );

    let mut tasks = TaskDomain::new(workspace.path());
    tasks
        .cancel("task-1")
        .expect("manual cancellation should succeed");

    domain
        .complete_task(
            "worker-alpha",
            "task-1",
            false,
            Some("Interrupted".to_string()),
        )
        .expect("complete_task should not overwrite cancellation");

    let task = TaskDomain::new(workspace.path())
        .get("task-1")
        .expect("task should remain readable after completion");
    assert_eq!(task.status, "cancelled");
    assert_eq!(task.assignee_worker_id, None);
    assert_eq!(task.claimed_at, None);
    assert_eq!(task.lease_expires_at, None);
    assert_eq!(
        task.error_message.as_deref(),
        Some("Task cancelled by user")
    );
}

#[test]
fn complete_task_releases_file_ownership() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    domain
        .register(busy_worker(
            "worker-alpha",
            "alpha",
            &workspace_root,
            "task-1",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-1",
        "Build feature",
        "in_progress",
        Some("worker-alpha"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:35:00Z"),
    );

    let registry = FileOwnershipRegistry::new(workspace.path());
    registry
        .claim(
            "worker-alpha",
            "task-1",
            vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
        )
        .expect("file claim for worker-alpha should succeed");
    registry
        .claim("worker-beta", "task-2", vec!["src/other.rs".to_string()])
        .expect("file claim for worker-beta should succeed");

    domain
        .complete_task("worker-alpha", "task-1", true, None)
        .expect("complete_task should succeed");

    let claims = registry
        .list_claims()
        .expect("listing claims should succeed after completion");
    assert_eq!(claims.len(), 1, "only worker-beta's claim should remain");
    assert_eq!(claims[0].worker_id, "worker-beta");
    assert_eq!(claims[0].files, vec!["src/other.rs".to_string()]);
}

#[test]
fn complete_task_rejects_unknown_worker() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    create_task_with_lease(
        workspace.path(),
        "task-1",
        "Build feature",
        "in_progress",
        None,
        None,
        None,
    );

    let error = domain
        .complete_task("worker-missing", "task-1", true, None)
        .expect_err("unknown worker should be rejected");
    assert_eq!(error.code, RpcErrorCode::NotFound);
    assert_eq!(error.message, "Worker with id 'worker-missing' not found");
}

#[test]
fn complete_task_then_claim_next_full_cycle() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let worker = sample_worker("worker-alpha", "alpha", &workspace_root, WorkerStatus::Idle);
    domain
        .register(worker.clone())
        .expect("idle worker should register");

    create_task(workspace.path(), "task-1", "First task", "ready");
    create_task(workspace.path(), "task-2", "Second task", "ready");

    // Claim first task
    let first_claim = domain
        .claim_next("worker-alpha")
        .expect("first claim_next should succeed");
    let first_task = first_claim.task.expect("should claim task-1");
    assert_eq!(first_task.id, "task-1");
    assert_eq!(first_claim.worker.status, WorkerStatus::Busy);
    assert_eq!(
        first_claim.worker.current_task_id.as_deref(),
        Some("task-1")
    );

    // Complete first task
    domain
        .complete_task("worker-alpha", "task-1", true, None)
        .expect("completing task-1 should succeed");

    let worker_after_complete = domain
        .get("worker-alpha")
        .expect("worker should exist after first completion");
    assert_eq!(worker_after_complete.status, WorkerStatus::Idle);
    assert_eq!(worker_after_complete.current_task_id, None);

    // Claim second task
    let second_claim = domain
        .claim_next("worker-alpha")
        .expect("second claim_next should succeed");
    let second_task = second_claim.task.expect("should claim task-2");
    assert_eq!(second_task.id, "task-2");
    assert_eq!(second_claim.worker.status, WorkerStatus::Busy);
    assert_eq!(
        second_claim.worker.current_task_id.as_deref(),
        Some("task-2")
    );

    // Verify task-1 is done
    let task_1 = TaskDomain::new(workspace.path())
        .get("task-1")
        .expect("task-1 should exist");
    assert_eq!(task_1.status, "done");
}

#[test]
fn failed_task_reclaimable_by_another_worker() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    let worker_alpha = sample_worker("worker-alpha", "alpha", &workspace_root, WorkerStatus::Idle);
    let worker_beta = sample_worker("worker-beta", "beta", &workspace_root, WorkerStatus::Idle);
    domain
        .register(worker_alpha.clone())
        .expect("worker-alpha should register");
    domain
        .register(worker_beta.clone())
        .expect("worker-beta should register");

    create_task(workspace.path(), "task-1", "Shared task", "ready");

    // worker-alpha claims and fails
    let claim_alpha = domain
        .claim_next("worker-alpha")
        .expect("worker-alpha claim_next should succeed");
    assert_eq!(
        claim_alpha.task.as_ref().expect("should claim task").id,
        "task-1"
    );

    domain
        .complete_task(
            "worker-alpha",
            "task-1",
            false,
            Some("Build failed".to_string()),
        )
        .expect("worker-alpha complete_task failure should succeed");

    // Verify error_message is set before reclaim
    let failed_task = TaskDomain::new(workspace.path())
        .get("task-1")
        .expect("failed task should exist");
    assert_eq!(
        failed_task.error_message.as_deref(),
        Some("Build failed"),
        "error_message should be set after failure"
    );

    // worker-beta picks up the failed task
    let claim_beta = domain
        .claim_next("worker-beta")
        .expect("worker-beta claim_next should succeed");
    let reclaimed_task = claim_beta
        .task
        .expect("worker-beta should reclaim the failed task");
    assert_eq!(reclaimed_task.id, "task-1");
    assert_eq!(reclaimed_task.status, "in_progress");
    assert_eq!(
        reclaimed_task.assignee_worker_id.as_deref(),
        Some("worker-beta")
    );
}

#[test]
fn claim_next_prefers_higher_priority_ready_tasks() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    domain
        .register(sample_worker(
            "worker-alpha",
            "alpha",
            &workspace_root,
            WorkerStatus::Idle,
        ))
        .expect("idle worker should register");

    let mut tasks = TaskDomain::new(workspace.path());
    tasks
        .create(TaskCreateParams {
            id: "task-low".to_string(),
            title: "Low priority".to_string(),
            status: Some("ready".to_string()),
            priority: Some(5),
            blocked_by: None,
            merge_loop_prompt: None,
            assignee_worker_id: None,
            claimed_at: None,
            lease_expires_at: None,
            scope_files: None,
        })
        .expect("low-priority task should persist");
    tasks
        .create(TaskCreateParams {
            id: "task-high".to_string(),
            title: "High priority".to_string(),
            status: Some("ready".to_string()),
            priority: Some(1),
            blocked_by: None,
            merge_loop_prompt: None,
            assignee_worker_id: None,
            claimed_at: None,
            lease_expires_at: None,
            scope_files: None,
        })
        .expect("high-priority task should persist");

    let claim = domain
        .claim_next("worker-alpha")
        .expect("claim_next should succeed");
    let task = claim.task.expect("worker should claim a task");
    assert_eq!(task.id, "task-high");
    assert_eq!(task.priority, 1);
}

#[test]
fn complete_task_visible_across_handles() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut handle_a =
        WorkerDomain::new(workspace.path()).expect("first worker domain should initialize");
    let handle_b =
        WorkerDomain::new(workspace.path()).expect("second worker domain should initialize");

    handle_a
        .register(busy_worker(
            "worker-alpha",
            "alpha",
            &workspace_root,
            "task-1",
        ))
        .expect("busy worker should register via handle_a");
    create_task_with_lease(
        workspace.path(),
        "task-1",
        "Build feature",
        "in_progress",
        Some("worker-alpha"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:35:00Z"),
    );

    handle_a
        .complete_task("worker-alpha", "task-1", true, None)
        .expect("complete_task via handle_a should succeed");

    // Verify handle_b sees the updated worker
    let worker_via_b = handle_b
        .get("worker-alpha")
        .expect("handle_b should see updated worker");
    assert_eq!(worker_via_b.status, WorkerStatus::Idle);
    assert_eq!(worker_via_b.current_task_id, None);

    // Verify fresh TaskDomain sees done task
    let task = TaskDomain::new(workspace.path())
        .get("task-1")
        .expect("task should be visible to fresh domain");
    assert_eq!(task.status, "done");
}

#[test]
fn complete_task_revives_dead_worker() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    // Register a busy worker and create an in_progress task
    domain
        .register(busy_worker(
            "worker-revive",
            "revive",
            &workspace_root,
            "task-revive",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-revive",
        "Revivable task",
        "in_progress",
        Some("worker-revive"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:32:00Z"),
    );

    // Simulate reclaim_expired marking the worker dead
    let mut domain2 = WorkerDomain::new(workspace.path()).expect("second domain should initialize");
    let reclaim = domain2
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("reclaim should succeed");
    assert_eq!(reclaim.workers.len(), 1);
    assert_eq!(reclaim.workers[0].status, WorkerStatus::Dead);

    // The task was already reset to "ready" by reclaim
    let task_before = TaskDomain::new(workspace.path())
        .get("task-revive")
        .expect("task should exist");
    assert_eq!(task_before.status, "ready");

    // Now the worker finishes and calls complete_task — should revive and force-close the task
    domain
        .complete_task("worker-revive", "task-revive", true, None)
        .expect("complete_task on dead worker should succeed (revive)");

    // Worker should be revived to Idle
    let worker = domain
        .get("worker-revive")
        .expect("worker should exist after revive");
    assert_eq!(worker.status, WorkerStatus::Idle);
    assert_eq!(worker.current_task_id, None);

    // Task should be "done" — the worker completed it successfully, so force-close
    // bypasses the state machine (ready → done is normally invalid)
    let task_after = TaskDomain::new(workspace.path())
        .get("task-revive")
        .expect("task should exist after revive");
    assert_eq!(task_after.status, "done");
    assert!(task_after.completed_at.is_some());
}

#[test]
fn purge_removes_stale_dead_workers() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    // Register a busy worker and create an in_progress task
    domain
        .register(busy_worker(
            "worker-purge",
            "purge-me",
            &workspace_root,
            "task-purge",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-purge",
        "Purgeable task",
        "in_progress",
        Some("worker-purge"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:32:00Z"),
    );

    // Reclaim marks worker dead (last_heartbeat_at stays at registration time)
    domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("reclaim should succeed");

    // Verify worker is dead
    let dead_worker = domain
        .get("worker-purge")
        .expect("dead worker should exist");
    assert_eq!(dead_worker.status, WorkerStatus::Dead);

    // Run reclaim again with as_of well past the purge threshold
    // (LEASE_DURATION=2min + DEAD_PURGE=5min = 7min after last_heartbeat)
    // last_heartbeat_at is "2026-03-14T22:30:00Z", so 22:38:00 is 8min later → should purge
    domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:38:00Z".to_string(),
        })
        .expect("second reclaim should succeed");

    // Worker should be purged (removed entirely)
    let err = domain.get("worker-purge").unwrap_err();
    assert_eq!(err.code, ralph_api::errors::RpcErrorCode::NotFound);
    assert!(domain.list().expect("list should work").is_empty());
}

#[test]
fn purge_keeps_recent_dead_workers() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    // Register a busy worker and create an in_progress task
    domain
        .register(busy_worker(
            "worker-recent",
            "recent",
            &workspace_root,
            "task-recent",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-recent",
        "Recent task",
        "in_progress",
        Some("worker-recent"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:32:00Z"),
    );

    // Reclaim marks worker dead
    domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("reclaim should succeed");
    assert_eq!(
        domain.get("worker-recent").unwrap().status,
        WorkerStatus::Dead
    );

    // Run reclaim again but only 3min after last_heartbeat (within the 7min grace period)
    // last_heartbeat_at is "2026-03-14T22:30:00Z", 22:33:00 is only 3min later → keep
    domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("reclaim within grace period should succeed");

    // Worker should still exist
    let worker = domain
        .get("worker-recent")
        .expect("recently dead worker should be kept");
    assert_eq!(worker.status, WorkerStatus::Dead);
}

/// Reproduces the exact bug: worker marked Dead by reclaim, idle heartbeat
/// revives it, then claim_next succeeds and picks up the ready task.
#[test]
fn idle_heartbeat_revives_dead_worker_and_claim_next_succeeds() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    // Register a busy worker with a task
    domain
        .register(busy_worker(
            "worker-stuck",
            "stuck",
            &workspace_root,
            "task-stuck",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-stuck",
        "Stuck task",
        "in_progress",
        Some("worker-stuck"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:32:00Z"),
    );

    // Reclaim marks worker Dead, resets task to "ready"
    domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("reclaim should succeed");
    assert_eq!(
        domain.get("worker-stuck").unwrap().status,
        WorkerStatus::Dead
    );
    assert_eq!(
        TaskDomain::new(workspace.path())
            .get("task-stuck")
            .unwrap()
            .status,
        "ready"
    );

    // claim_next FAILS because worker is Dead (not Idle)
    let claim_err = domain
        .claim_next("worker-stuck")
        .expect_err("claim_next should reject Dead worker");
    assert!(
        claim_err.message.contains("must be idle"),
        "error should mention idle requirement: {}",
        claim_err.message
    );

    // Idle heartbeat revives the Dead worker (simulates factory loop's send_idle_heartbeat)
    domain
        .heartbeat(WorkerHeartbeatInput {
            worker_id: "worker-stuck".to_string(),
            status: WorkerStatus::Idle,
            current_task_id: None,
            current_hat: None,
            last_heartbeat_at: "2026-03-14T22:33:30Z".to_string(),
            iteration: None,
            max_iterations: None,
        })
        .expect("idle heartbeat should revive dead worker");

    let revived = domain.get("worker-stuck").unwrap();
    assert_eq!(revived.status, WorkerStatus::Idle);
    assert_eq!(revived.current_task_id, None);

    // NOW claim_next succeeds and picks up the ready task
    let claim = domain
        .claim_next("worker-stuck")
        .expect("claim_next should succeed after idle heartbeat revival");
    let task = claim.task.expect("should claim the ready task");
    assert_eq!(task.id, "task-stuck");
    assert_eq!(task.status, "in_progress");
    assert_eq!(task.assignee_worker_id.as_deref(), Some("worker-stuck"));
    assert_eq!(claim.worker.status, WorkerStatus::Busy);
}

/// Proves busy heartbeats are blocked on Dead workers — they must NOT
/// revive the worker or re-establish a stale current_task_id.
#[test]
fn busy_heartbeat_does_not_revive_dead_worker() {
    let workspace = tempfile::tempdir().expect("workspace tempdir should be created");
    let workspace_root = workspace.path().display().to_string();
    let mut domain = WorkerDomain::new(workspace.path()).expect("worker domain should initialize");

    // Register a busy worker with a task, then reclaim it
    domain
        .register(busy_worker(
            "worker-dead",
            "dead",
            &workspace_root,
            "task-dead",
        ))
        .expect("busy worker should register");
    create_task_with_lease(
        workspace.path(),
        "task-dead",
        "Dead task",
        "in_progress",
        Some("worker-dead"),
        Some("2026-03-14T22:30:00Z"),
        Some("2026-03-14T22:32:00Z"),
    );

    domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2026-03-14T22:33:00Z".to_string(),
        })
        .expect("reclaim should succeed");
    assert_eq!(
        domain.get("worker-dead").unwrap().status,
        WorkerStatus::Dead
    );

    // Busy heartbeat (from background heartbeat task) should be a no-op
    let result = domain
        .heartbeat(WorkerHeartbeatInput {
            worker_id: "worker-dead".to_string(),
            status: WorkerStatus::Busy,
            current_task_id: Some("task-dead".to_string()),
            current_hat: None,
            last_heartbeat_at: "2026-03-14T22:33:30Z".to_string(),
            iteration: None,
            max_iterations: None,
        })
        .expect("busy heartbeat on dead worker should not error");

    // Worker should still be Dead with no task
    assert_eq!(result.status, WorkerStatus::Dead);
    assert_eq!(result.current_task_id, None);

    // Verify persisted state is also Dead
    let from_disk = domain.get("worker-dead").unwrap();
    assert_eq!(from_disk.status, WorkerStatus::Dead);
    assert_eq!(from_disk.current_task_id, None);
}

// ── Task event tracking ─────────────────────────────────────────────────

#[test]
fn task_events_on_create() {
    let dir = tempfile::tempdir().unwrap();
    // TaskDomain::new + create auto-initializes .ralph/api/

    let mut task_domain = TaskDomain::new(dir.path());
    let task = task_domain
        .create(TaskCreateParams {
            id: "task-evt-1".to_string(),
            title: "Test events".to_string(),
            status: None,
            priority: None,
            blocked_by: None,
            merge_loop_prompt: None,
            assignee_worker_id: None,
            claimed_at: None,
            lease_expires_at: None,
            scope_files: None,
        })
        .unwrap();

    assert_eq!(task.events.len(), 1);
    assert_eq!(task.events[0].event_type, "created");
    assert!(task.events[0].details.as_ref().unwrap().contains("ready"));
}

#[test]
fn task_events_on_claim_and_complete() {
    let dir = tempfile::tempdir().unwrap();
    // TaskDomain::new + create auto-initializes .ralph/api/

    // Create a task
    let mut task_domain = TaskDomain::new(dir.path());
    task_domain
        .create(TaskCreateParams {
            id: "task-evt-2".to_string(),
            title: "Claim test".to_string(),
            status: None,
            priority: None,
            blocked_by: None,
            merge_loop_prompt: None,
            assignee_worker_id: None,
            claimed_at: None,
            lease_expires_at: None,
            scope_files: None,
        })
        .unwrap();

    // Register a worker and claim
    let mut domain = WorkerDomain::new(dir.path()).unwrap();
    let ws = dir.path().to_string_lossy().to_string();
    domain
        .register(sample_worker(
            "worker-evt",
            "evt-worker",
            &ws,
            WorkerStatus::Idle,
        ))
        .unwrap();
    let claim = domain.claim_next("worker-evt").unwrap();
    assert!(claim.task.is_some());

    // Check claimed event
    let task_domain = TaskDomain::new(dir.path());
    let task = task_domain.get("task-evt-2").unwrap();
    let claimed_events: Vec<_> = task
        .events
        .iter()
        .filter(|e| e.event_type == "claimed")
        .collect();
    assert_eq!(claimed_events.len(), 1);
    assert_eq!(claimed_events[0].worker_id.as_deref(), Some("worker-evt"));

    // Complete the task
    domain
        .complete_task("worker-evt", "task-evt-2", true, None)
        .unwrap();

    let task_domain = TaskDomain::new(dir.path());
    let task = task_domain.get("task-evt-2").unwrap();
    let completed_events: Vec<_> = task
        .events
        .iter()
        .filter(|e| e.event_type == "completed")
        .collect();
    assert_eq!(completed_events.len(), 1);
    assert_eq!(completed_events[0].worker_id.as_deref(), Some("worker-evt"));
}

#[test]
fn task_events_on_reclaim() {
    let dir = tempfile::tempdir().unwrap();
    // TaskDomain::new + create auto-initializes .ralph/api/

    // Create an in_progress task with expired lease
    create_task_with_lease(
        dir.path(),
        "task-evt-3",
        "Reclaim events",
        "in_progress",
        Some("worker-stale"),
        Some("2026-03-14T22:00:00Z"),
        Some("2026-03-14T22:01:00Z"),
    );

    let mut domain = WorkerDomain::new(dir.path()).unwrap();
    let ws = dir.path().to_string_lossy().to_string();
    domain
        .register(sample_worker(
            "worker-stale",
            "stale",
            &ws,
            WorkerStatus::Busy,
        ))
        .unwrap();

    // Set current_task_id via modify_workers (not heartbeat, which extends leases)
    domain
        .heartbeat(WorkerHeartbeatInput {
            worker_id: "worker-stale".to_string(),
            status: WorkerStatus::Idle,
            current_task_id: Some("task-evt-3".to_string()),
            current_hat: None,
            last_heartbeat_at: "2026-03-14T22:00:30Z".to_string(),
            iteration: None,
            max_iterations: None,
        })
        .unwrap();

    // Reclaim — use a far-future timestamp so the lease is definitely expired
    let result = domain
        .reclaim_expired(WorkerReclaimExpiredInput {
            as_of: "2099-01-01T00:00:00Z".to_string(),
        })
        .unwrap();
    assert_eq!(result.tasks.len(), 1);

    // Check reclaimed event
    let task_domain = TaskDomain::new(dir.path());
    let task = task_domain.get("task-evt-3").unwrap();
    let reclaimed_events: Vec<_> = task
        .events
        .iter()
        .filter(|e| e.event_type == "reclaimed")
        .collect();
    assert_eq!(reclaimed_events.len(), 1);
    assert_eq!(
        reclaimed_events[0].worker_id.as_deref(),
        Some("worker-stale")
    );
}

#[test]
fn task_events_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    // TaskDomain::new + create auto-initializes .ralph/api/

    let mut task_domain = TaskDomain::new(dir.path());
    task_domain
        .create(TaskCreateParams {
            id: "task-evt-4".to_string(),
            title: "Fail test".to_string(),
            status: None,
            priority: None,
            blocked_by: None,
            merge_loop_prompt: None,
            assignee_worker_id: None,
            claimed_at: None,
            lease_expires_at: None,
            scope_files: None,
        })
        .unwrap();

    let mut domain = WorkerDomain::new(dir.path()).unwrap();
    let ws = dir.path().to_string_lossy().to_string();
    domain
        .register(sample_worker(
            "worker-fail",
            "fail-worker",
            &ws,
            WorkerStatus::Idle,
        ))
        .unwrap();
    domain.claim_next("worker-fail").unwrap();

    // Fail the task
    domain
        .complete_task(
            "worker-fail",
            "task-evt-4",
            false,
            Some("build failed".to_string()),
        )
        .unwrap();

    let task_domain = TaskDomain::new(dir.path());
    let task = task_domain.get("task-evt-4").unwrap();
    let failed_events: Vec<_> = task
        .events
        .iter()
        .filter(|e| e.event_type == "failed")
        .collect();
    assert_eq!(failed_events.len(), 1);
    assert_eq!(failed_events[0].worker_id.as_deref(), Some("worker-fail"));
    assert!(
        failed_events[0]
            .details
            .as_ref()
            .unwrap()
            .contains("failed")
    );
}
