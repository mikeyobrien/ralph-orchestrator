use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};
use ralph_core::{
    LoopEntry, LoopLock, LoopRegistry, MergeQueue, MergeState, WorktreeConfig, create_worktree,
};
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

fn init_git_repo(path: &Path) -> Result<()> {
    run_git(path, &["init", "--initial-branch=main"])?;
    run_git(path, &["config", "user.email", "test@test.local"])?;
    run_git(path, &["config", "user.name", "Test User"])?;

    fs::write(path.join("README.md"), "# Test\n")?;
    run_git(path, &["add", "README.md"])?;
    run_git(path, &["commit", "-m", "Initial commit"])?;
    Ok(())
}

fn run_git(path: &Path, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .status()?;
    ensure!(status.success(), "git {:?} failed", args);
    Ok(())
}

#[cfg(unix)]
fn create_fake_ralph_command() -> Result<(TempDir, PathBuf, PathBuf)> {
    use std::os::unix::fs::PermissionsExt;

    let fake_bin = tempfile::tempdir()?;
    let command_path = fake_bin.path().join("fake-ralph");
    let call_log_path = fake_bin.path().join("calls.log");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"{}\"\n",
        call_log_path.display()
    );

    fs::write(&command_path, script)?;
    let mut permissions = fs::metadata(&command_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command_path, permissions)?;

    Ok((fake_bin, command_path, call_log_path))
}

#[tokio::test]
async fn loop_stop_unknown_id_returns_loop_not_found_with_primary_lock() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    let _primary_lock = LoopLock::try_acquire(server.workspace_path(), "primary lock")?;

    let stop_unknown = rpc_request(
        "req-loop-stop-unknown-1",
        "loop.stop",
        json!({ "id": "loop-does-not-exist", "force": false }),
        Some("idem-loop-stop-unknown-1"),
    );
    let (status, payload) = post_rpc(&client, &server, &stop_unknown).await?;

    assert_eq!(status, 404);
    assert_eq!(payload["error"]["code"], "LOOP_NOT_FOUND");
    assert!(
        !server
            .workspace_path()
            .join(".ralph/stop-requested")
            .exists()
    );

    server.stop().await;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn loop_retry_spawns_merge_flow_command() -> Result<()> {
    let (fake_bin, fake_ralph, call_log_path) = create_fake_ralph_command()?;

    let mut config = ApiConfig::default();
    config.ralph_command = fake_ralph.to_string_lossy().to_string();

    let server = TestServer::start(config).await;
    let client = Client::new();

    let merge_queue = MergeQueue::new(server.workspace_path());
    merge_queue.enqueue("loop-review-1", "Needs review")?;
    merge_queue.mark_merging("loop-review-1", std::process::id())?;
    merge_queue.mark_needs_review("loop-review-1", "conflict in src/lib.rs")?;

    let retry_request = rpc_request(
        "req-loop-retry-regression-1",
        "loop.retry",
        json!({ "id": "loop-review-1", "steeringInput": "Prefer ours" }),
        Some("idem-loop-retry-regression-1"),
    );
    let (status, payload) = post_rpc(&client, &server, &retry_request).await?;

    assert_eq!(status, 200);
    assert_eq!(payload["result"]["success"], true);

    let calls = fs::read_to_string(&call_log_path)?;
    assert!(
        calls
            .lines()
            .any(|line| line.trim() == "loops retry loop-review-1"),
        "expected retry command invocation, got: {calls}"
    );

    let steering = fs::read_to_string(server.workspace_path().join(".ralph/merge-steering.txt"))?;
    assert_eq!(steering, "Prefer ours");

    drop(fake_bin);
    server.stop().await;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn loop_process_delegates_to_cli_without_marking_queued_entries_merged() -> Result<()> {
    let (fake_bin, fake_ralph, call_log_path) = create_fake_ralph_command()?;

    let mut config = ApiConfig::default();
    config.ralph_command = fake_ralph.to_string_lossy().to_string();

    let server = TestServer::start(config).await;
    let client = Client::new();

    let merge_queue = MergeQueue::new(server.workspace_path());
    merge_queue.enqueue("loop-process-1", "Queued merge")?;

    let process_request = rpc_request(
        "req-loop-process-1",
        "loop.process",
        json!({}),
        Some("idem-loop-process-1"),
    );
    let (status, payload) = post_rpc(&client, &server, &process_request).await?;

    assert_eq!(status, 200);
    assert_eq!(payload["result"]["success"], true);

    let calls = fs::read_to_string(&call_log_path)?;
    assert!(
        calls.lines().any(|line| line.trim() == "loops process"),
        "expected loops process command invocation, got: {calls}"
    );

    let queue_entry = merge_queue
        .get_entry("loop-process-1")?
        .expect("queued loop should remain in queue until merge execution completes");
    assert_eq!(queue_entry.state, MergeState::Queued);

    drop(fake_bin);
    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn loop_discard_removes_worktree_and_marks_discarded() -> Result<()> {
    let server = TestServer::start(ApiConfig::default()).await;
    let client = Client::new();

    init_git_repo(server.workspace_path())?;

    let worktree = create_worktree(
        server.workspace_path(),
        "loop-discard-1",
        &WorktreeConfig::default(),
    )?;

    let merge_queue = MergeQueue::new(server.workspace_path());
    merge_queue.enqueue("loop-discard-1", "Discard this loop")?;

    let registry = LoopRegistry::new(server.workspace_path());
    registry.register(LoopEntry::with_id(
        "loop-discard-1",
        "Implement disposable changes",
        Some(worktree.path.to_string_lossy().to_string()),
        server.workspace_path().display().to_string(),
    ))?;

    assert!(worktree.path.exists());

    let discard_request = rpc_request(
        "req-loop-discard-1",
        "loop.discard",
        json!({ "id": "loop-discard-1" }),
        Some("idem-loop-discard-1"),
    );
    let (status, payload) = post_rpc(&client, &server, &discard_request).await?;

    assert_eq!(status, 200);
    assert_eq!(payload["result"]["success"], true);
    assert!(!worktree.path.exists());
    assert!(registry.get("loop-discard-1")?.is_none());

    let queue_entry = merge_queue
        .get_entry("loop-discard-1")?
        .expect("discarded loop should remain in merge queue history");
    assert_eq!(queue_entry.state, MergeState::Discarded);

    server.stop().await;
    Ok(())
}

// ── Real-flow integration helpers ────────────────────────────────────────────

/// Locate the `ralph` binary built by this workspace.
///
/// Cargo places the binary at `target/{profile}/ralph`, while the test binary
/// itself lives at `target/{profile}/deps/<test-name>-<hash>`. We navigate
/// up one directory (deps → profile dir) to find the sibling binary.
#[cfg(unix)]
fn find_workspace_ralph_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    // exe: .../target/{profile}/deps/<test-binary>
    let deps_dir = exe.parent()?;
    // deps_dir: .../target/{profile}/deps
    let build_dir = deps_dir.parent()?;
    // build_dir: .../target/{profile}
    let ralph = build_dir.join("ralph");
    if ralph.exists() { Some(ralph) } else { None }
}

/// Create a wrapper script named `ralph` inside a fresh temp directory.
///
/// The wrapper:
///  - Intercepts `ralph run ...` (exits 0 immediately) to prevent long-lived
///    merge-ralph processes from being spawned during integration tests.
///  - Prepends its own directory to `PATH` before delegating any other
///    sub-command to the real binary, so that child processes spawned by the
///    real ralph (e.g. the internal `ralph run` spawn in `loops process`) also
///    hit the interceptor.
///
/// Returns `(TempDir, wrapper_path)`.  The caller must keep `TempDir` alive.
#[cfg(unix)]
fn create_ralph_run_interceptor() -> Option<(TempDir, PathBuf)> {
    use std::os::unix::fs::PermissionsExt;

    let real_ralph = find_workspace_ralph_binary()?;
    let bin_dir = tempfile::tempdir().ok()?;
    let wrapper_path = bin_dir.path().join("ralph");

    let script = format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
             run) exit 0 ;;\n\
             *)\n\
                 PATH=\"$(dirname \"$0\"):$PATH\"\n\
                 export PATH\n\
                 exec \"{real_ralph}\" \"$@\"\n\
                 ;;\n\
         esac\n",
        real_ralph = real_ralph.display()
    );

    fs::write(&wrapper_path, &script).ok()?;
    let mut perms = fs::metadata(&wrapper_path).ok()?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&wrapper_path, perms).ok()?;

    Some((bin_dir, wrapper_path))
}

// ── Real-flow integration test ────────────────────────────────────────────────

/// Regression: `loop.process` must invoke the *real* `ralph loops process`
/// path, not just a stub.
///
/// Setup:
///   - Workspace with a queued merge entry.
///   - `ralph_command` points to a wrapper that delegates `loops process` to
///     the real ralph binary, but intercepts any `ralph run` spawns so no
///     long-running merge-ralph processes are started.
///
/// Assertions:
///   - API returns HTTP 200 with `result.success = true`.
///   - `loop.status` shows `lastProcessedAt` was set (confirming the domain
///     completed the flow, not an early-return short-circuit).
///   - The queue entry remains in `Queued` state because no actual merge ran.
#[cfg(unix)]
#[tokio::test]
async fn loop_process_real_ralph_flow_succeeds_with_queued_entry() -> Result<()> {
    let Some((_bin_dir, wrapper)) = create_ralph_run_interceptor() else {
        // Built binary not found; skip rather than fail (e.g. cross-compile CI).
        eprintln!(
            "skipping loop_process_real_ralph_flow_succeeds_with_queued_entry: ralph binary not found"
        );
        return Ok(());
    };

    let mut config = ApiConfig::default();
    config.ralph_command = wrapper.to_string_lossy().to_string();

    let server = TestServer::start(config).await;
    let client = Client::new();

    // A git repo is not strictly required by `ralph loops process` itself, but
    // having one avoids spurious warnings from the real binary.
    init_git_repo(server.workspace_path())?;

    // Enqueue an entry so loop_domain.process() actually invokes the wrapper
    // (an empty queue returns immediately without touching ralph).
    let merge_queue = MergeQueue::new(server.workspace_path());
    merge_queue.enqueue("loop-real-flow-1", "Real flow integration test")?;

    let process_request = rpc_request(
        "req-loop-real-flow-1",
        "loop.process",
        json!({}),
        Some("idem-loop-real-flow-1"),
    );
    let (status, payload) = post_rpc(&client, &server, &process_request).await?;

    assert_eq!(status, 200, "loop.process must succeed: {payload}");
    assert_eq!(
        payload["result"]["success"], true,
        "loop.process result.success must be true"
    );

    // Confirm lastProcessedAt was stamped — proves we went through the full
    // process() code path rather than an early-return no-op.
    let status_req = rpc_request("req-loop-status-real-1", "loop.status", json!({}), None);
    let (http_status, status_payload) = post_rpc(&client, &server, &status_req).await?;
    assert_eq!(http_status, 200);
    assert!(
        status_payload["result"]["lastProcessedAt"].is_string(),
        "lastProcessedAt must be set after loop.process: {status_payload}"
    );

    // Queue entry must still be Queued: the interceptor exited the spawned
    // merge-ralph immediately, so no state transition occurred.
    let entry = merge_queue
        .get_entry("loop-real-flow-1")?
        .expect("queue entry must still exist");
    assert_eq!(
        entry.state,
        MergeState::Queued,
        "entry should remain Queued after intercepted run"
    );

    drop(_bin_dir);
    server.stop().await;
    Ok(())
}
