use std::fs;

use ralph_api::errors::RpcErrorCode;
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
