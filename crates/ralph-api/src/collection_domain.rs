mod yaml;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

use crate::errors::ApiError;
use crate::loop_support::now_ts;

use self::yaml::{export_collection_yaml, graph_from_yaml};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCreateParams {
    pub name: String,
    pub description: Option<String>,
    pub graph: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub graph: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionImportParams {
    pub yaml: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRunParams {
    pub id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRunResult {
    pub success: bool,
    pub config_path: String,
    pub pid: u32,
    /// The hat that will activate first, derived from the graph topology.
    /// The frontend uses this to highlight the entry node immediately
    /// without waiting for the first WebSocket event (timing-race fix).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting_hat: Option<String>,
}

/// Bounded in-memory tail of a spawned run's stderr, shared with the
/// drain thread. Only CAP bytes are ever retained no matter how much the
/// child logs; small outputs — the common failure case — pass through
/// verbatim and unmarked, large ones carry a truncation note naming the
/// discarded prefix length.
///
/// Counter and buffer live under one mutex so a snapshot always pairs a
/// byte count with the exact contents it describes.
#[derive(Debug, Default)]
struct StderrTail {
    state: std::sync::Mutex<StderrTailState>,
}

#[derive(Debug, Default)]
struct StderrTailState {
    buf: Vec<u8>,
    total: u64,
}

impl StderrTail {
    /// Maximum retained bytes; enough for failure diagnostics.
    const CAP: usize = 65536;

    fn push(&self, chunk: &[u8]) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.total += chunk.len() as u64;
        state.buf.extend_from_slice(chunk);
        if state.buf.len() > Self::CAP {
            let drop = state.buf.len() - Self::CAP;
            state.buf.drain(..drop);
        }
    }

    fn snapshot_string(&self) -> String {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        // A cut point inside a UTF-8 sequence degrades to replacement
        // characters rather than failing the diagnostic.
        let tail = String::from_utf8_lossy(&state.buf);
        if state.total > state.buf.len() as u64 {
            format!(
                "[earliest {} bytes truncated]\n{tail}",
                state.total - state.buf.len() as u64
            )
        } else {
            tail.into_owned()
        }
    }
}

/// Continuously read `reader` into `tail` until EOF or read error.
/// Interrupted reads are retried; every other outcome ends the drain.
/// Runs on a dedicated thread; see `StderrDrain` for lifetime control.
fn drain_to_tail(mut reader: impl std::io::Read, tail: &std::sync::Arc<StderrTail>) {
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => tail.push(&chunk[..n]),
            // A signaled read is not EOF: retry the loop instead of
            // ending the drain (which would drop the read end and
            // break the pipe for subsequent child writes).
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

/// Lifetime control for a spawned run's stderr drain.
///
/// The drain thread reads continuously, which prevents the undrained-buffer
/// deadlock where a chatty child blocks forever (momentary backpressure
/// while the drain catches up remains possible). After the direct child
/// exits, [`StderrDrain::cancel`]
/// unblocks the drain even if a descendant inherited the write end and is
/// still alive — no EOF wait, no join, on any path. Memory stays bounded
/// by [`StderrTail::CAP`] regardless of how much is logged.
///
/// Cancelling closes the run's stderr consumer: a descendant that keeps
/// logging afterwards gets EPIPE on its next write (standard dead-consumer
/// behavior, same as `producer | head`) instead of blocking forever. Its
/// earlier output is already captured in the tail.
#[cfg(unix)]
struct StderrDrain {
    /// Shutting the read end down unblocks the drain thread's read.
    shutdown: std::os::unix::net::UnixStream,
    tail: std::sync::Arc<StderrTail>,
    done_rx: std::sync::mpsc::Receiver<()>,
}

#[cfg(unix)]
impl StderrDrain {
    /// Spawn a drain for `read_end`; returns the control handle. The
    /// caller keeps `write_end` for the child. Thread creation failure is
    /// an ordinary I/O error on the API path, never a panic.
    fn spawn(read_end: std::os::unix::net::UnixStream) -> std::io::Result<Self> {
        let shutdown = read_end.try_clone()?;
        let tail = std::sync::Arc::new(StderrTail::default());
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("ralph-stderr-drain".to_string())
            .spawn({
                let tail = tail.clone();
                move || {
                    drain_to_tail(read_end, &tail);
                    let _ = done_tx.send(());
                }
            })?;
        Ok(Self {
            shutdown,
            tail,
            done_rx,
        })
    }

    fn snapshot(&self) -> String {
        self.tail.snapshot_string()
    }

    /// Unblock the drain thread; never waits.
    fn cancel(&self) {
        let _ = self.shutdown.shutdown(std::net::Shutdown::Read);
    }

    /// Bounded wait for drain exit (graceful shutdown proof, testing).
    fn wait_done(&self, timeout: std::time::Duration) -> bool {
        self.done_rx.recv_timeout(timeout).is_ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRecord {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub graph: GraphData,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub viewport: Viewport,
}

impl Default for GraphData {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub position: NodePosition,
    pub data: HatNodeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HatNodeData {
    pub key: String,
    pub name: String,
    pub description: String,
    pub triggers_on: Vec<String>,
    pub publishes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CollectionSnapshot {
    collections: Vec<CollectionRecord>,
    id_counter: u64,
}

pub struct CollectionDomain {
    store_path: PathBuf,
    collections: BTreeMap<String, CollectionRecord>,
    id_counter: u64,
}

impl CollectionDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let store_path = workspace_root
            .as_ref()
            .join(".ralph/api/collections-v1.json");
        let mut domain = Self {
            store_path,
            collections: BTreeMap::new(),
            id_counter: 0,
        };
        domain.load();
        domain
    }

    pub fn list(&self) -> Vec<CollectionSummary> {
        let mut entries: Vec<_> = self
            .collections
            .values()
            .map(|collection| CollectionSummary {
                id: collection.id.clone(),
                name: collection.name.clone(),
                description: collection.description.clone(),
                created_at: collection.created_at.clone(),
                updated_at: collection.updated_at.clone(),
            })
            .collect();

        entries.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        entries
    }

    pub fn get(&self, id: &str) -> Result<CollectionRecord, ApiError> {
        self.collections
            .get(id)
            .cloned()
            .ok_or_else(|| collection_not_found_error(id))
    }

    pub fn create(&mut self, params: CollectionCreateParams) -> Result<CollectionRecord, ApiError> {
        if params.name.trim().is_empty() {
            return Err(ApiError::invalid_params(
                "collection name must not be empty",
            ));
        }

        let graph = params
            .graph
            .map(parse_graph)
            .transpose()?
            .unwrap_or_default();

        let now = now_ts();
        let id = self.next_collection_id();

        let record = CollectionRecord {
            id: id.clone(),
            name: params.name,
            description: params.description,
            graph,
            created_at: now.clone(),
            updated_at: now,
        };

        self.collections.insert(id.clone(), record);
        self.persist()?;
        self.get(&id)
    }

    pub fn update(&mut self, params: CollectionUpdateParams) -> Result<CollectionRecord, ApiError> {
        let record = self
            .collections
            .get_mut(&params.id)
            .ok_or_else(|| collection_not_found_error(&params.id))?;

        if let Some(ref name) = params.name {
            if name.trim().is_empty() {
                return Err(ApiError::invalid_params(
                    "collection name must not be empty",
                ));
            }
            record.name = name.clone();
        }

        if let Some(description) = params.description {
            record.description = Some(description);
        }

        if let Some(graph) = params.graph {
            record.graph = parse_graph(graph)?;
        }

        record.updated_at = now_ts();
        self.persist()?;
        self.get(&params.id)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), ApiError> {
        if self.collections.remove(id).is_none() {
            return Err(collection_not_found_error(id));
        }

        self.persist()
    }

    pub fn import(&mut self, params: CollectionImportParams) -> Result<CollectionRecord, ApiError> {
        let graph = graph_from_yaml(&params.yaml)?;
        self.create(CollectionCreateParams {
            name: params.name,
            description: params.description,
            graph: Some(serde_json::to_value(graph).map_err(|error| {
                ApiError::internal(format!("failed serializing graph: {error}"))
            })?),
        })
    }

    pub fn export(&self, id: &str) -> Result<String, ApiError> {
        let collection = self.get(id)?;
        export_collection_yaml(&collection)
    }

    /// Export the collection's hats to a temp YAML file and spawn `ralph run`
    /// with it via the `-H` flag. The user's existing `ralph.yml` provides
    /// core config (backend, max_iterations, backpressure). The collection
    /// only provides hats and events.
    ///
    /// Returns the PID so the frontend can track the process.
    pub fn run(
        &self,
        params: CollectionRunParams,
        ralph_command: &str,
        workspace_root: &Path,
    ) -> Result<CollectionRunResult, ApiError> {
        let yaml = self.export(&params.id)?;

        // Write the exported YAML to a predictable path.
        let collections_dir = workspace_root.join(".ralph/collections");
        fs::create_dir_all(&collections_dir).map_err(|error| {
            ApiError::internal(format!(
                "failed creating collections run directory: {error}"
            ))
        })?;

        let config_path = collections_dir.join(format!("{}-run.yml", params.id));
        fs::write(&config_path, &yaml).map_err(|error| {
            ApiError::internal(format!(
                "failed writing collection run config '{}': {error}",
                config_path.display()
            ))
        })?;

        // Stderr flows through a Unix socket pair drained continuously
        // into a bounded in-memory tail: continuous reading prevents the
        // undrained-buffer deadlock where a chatty child blocks forever,
        // and memory stays capped no matter how much is logged. After the
        // direct child exits the read end is shut down, which unblocks the
        // drain even if a descendant inherited the write end and is still
        // alive — no EOF wait, no join, on any path. (Non-unix platforms
        // keep a plain pipe drained into the same bounded tail; see below.)
        #[cfg(unix)]
        let (mut child, stderr_drain) = {
            use std::os::unix::io::OwnedFd;
            use std::os::unix::net::UnixStream;
            let (read_end, write_end) = UnixStream::pair().map_err(|error| {
                ApiError::internal(format!("cannot create stderr socket pair: {error}"))
            })?;
            let drain = StderrDrain::spawn(read_end).map_err(|error| {
                ApiError::internal(format!("cannot start stderr drain: {error}"))
            })?;
            let child = std::process::Command::new(ralph_command)
                .current_dir(workspace_root)
                .args([
                    "run",
                    "-H",
                    &config_path.to_string_lossy(),
                    "-a",
                    "-p",
                    &params.prompt,
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::from(OwnedFd::from(write_end)))
                .spawn()
                .map_err(|error| {
                    // Spawn failed: unblock the drain so no thread lingers.
                    drain.cancel();
                    ApiError::internal(format!(
                        "ralph CLI not found or failed to start. Install ralph or set RALPH_API_RALPH_COMMAND. Error: {error}"
                    ))
                })?;
            (child, drain)
        };
        // Spawn ralph run with -H (hats overlay) so the user's ralph.yml
        // provides backend/max_iterations/backpressure and the collection
        // provides hats/events. -a (autonomous) forces headless mode, which
        // is required when the API spawns ralph: interactive mode tries to
        // read from a tty the background process doesn't own and gets
        // SIGSTOP'd by the OS. Autonomous mode implies --no-tui.
        //
        // Non-unix fallback: plain pipe drained into the same bounded
        // tail, so failure diagnostics are preserved there too. Without
        // socket shutdown the drain cannot be cancelled on these
        // platforms and ends on EOF (pre-existing semantics); unix —
        // Ralph's supported platforms — uses the cancellable drain above.
        #[cfg(not(unix))]
        let (mut child, stderr_tail) = {
            let tail = std::sync::Arc::new(StderrTail::default());
            let mut child = std::process::Command::new(ralph_command)
                .current_dir(workspace_root)
                .args([
                    "run",
                    "-H",
                    &config_path.to_string_lossy(),
                    "-a",
                    "-p",
                    &params.prompt,
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|error| {
                    ApiError::internal(format!(
                        "ralph CLI not found or failed to start. Install ralph or set RALPH_API_RALPH_COMMAND. Error: {error}"
                    ))
                })?;
            let stderr = child.stderr.take();
            std::thread::spawn({
                let tail = tail.clone();
                move || {
                    if let Some(pipe) = stderr {
                        drain_to_tail(pipe, &tail);
                    }
                }
            });
            (child, tail)
        };

        let pid = child.id();

        // Wait briefly to check if the process died immediately.
        std::thread::sleep(std::time::Duration::from_millis(500));
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                // Bounded snapshot, never an EOF wait: a descendant may
                // hold the write end open indefinitely. Give the drain a
                // bounded grace period for trailing output (returning
                // sooner if it already reached EOF), then read whatever
                // arrived and unblock the drain.
                #[cfg(unix)]
                let stderr_output = {
                    let _ = stderr_drain.wait_done(std::time::Duration::from_millis(100));
                    let output = stderr_drain.snapshot();
                    stderr_drain.cancel();
                    output
                };
                #[cfg(not(unix))]
                let stderr_output = {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    stderr_tail.snapshot_string()
                };
                {
                    let trimmed = stderr_output.trim();
                    // `status.code()` is `Some(code)` on normal exit; `None` means
                    // signal-terminated on Unix. We format both cleanly to avoid
                    // the double "exit status:" prefix that `ExitStatus: Display`
                    // would produce.
                    let status_label = match status.code() {
                        Some(code) => format!("exit code {code}"),
                        None => format!("{status}"),
                    };
                    let message = if trimmed.is_empty() {
                        format!("ralph run exited with {status_label} (no stderr)")
                    } else {
                        // Pass ralph's stderr through (bounded tail: small
                        // outputs are verbatim, large ones carry a
                        // truncation note so the newest lines, where the
                        // actual error line usually is, survive).
                        format!("ralph run exited with {status_label}:\n{trimmed}")
                    };
                    return Err(ApiError::internal(message));
                }
            }
            _ => {
                // Still running or exited successfully. Detach a reaper so
                // the eventual exit doesn't leave a zombie process — the
                // API may outlive many loop runs. The reaper unblocks the
                // drain once the direct child is reaped: a descendant
                // holding the write end open cannot pin request handling
                // or reaper resources afterwards.
                #[cfg(unix)]
                std::thread::spawn(move || {
                    let _ = child.wait();
                    stderr_drain.cancel();
                });
                #[cfg(not(unix))]
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
        }

        // Compute the starting hat from the collection's topology so the
        // frontend can highlight it immediately (timing-race fix).
        let collection = self.get(&params.id)?;
        let starting_hat = yaml::starting_hat_for_collection(&collection);

        Ok(CollectionRunResult {
            success: true,
            config_path: config_path.to_string_lossy().to_string(),
            pid,
            starting_hat,
        })
    }

    fn next_collection_id(&mut self) -> String {
        self.id_counter = self.id_counter.saturating_add(1);
        format!(
            "collection-{}-{:04x}",
            Utc::now().timestamp_millis(),
            self.id_counter
        )
    }

    fn load(&mut self) {
        if !self.store_path.exists() {
            return;
        }

        let content = match fs::read_to_string(&self.store_path) {
            Ok(content) => content,
            Err(error) => {
                warn!(
                    path = %self.store_path.display(),
                    %error,
                    "failed reading collection snapshot"
                );
                return;
            }
        };

        let snapshot: CollectionSnapshot = match serde_json::from_str(&content) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                warn!(
                    path = %self.store_path.display(),
                    %error,
                    "failed parsing collection snapshot"
                );
                return;
            }
        };

        self.collections = snapshot
            .collections
            .into_iter()
            .map(|collection| (collection.id.clone(), collection))
            .collect();
        self.id_counter = snapshot.id_counter;
    }

    fn persist(&self) -> Result<(), ApiError> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed creating collection snapshot directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let snapshot = CollectionSnapshot {
            collections: self.sorted_records(),
            id_counter: self.id_counter,
        };

        let payload = serde_json::to_string_pretty(&snapshot).map_err(|error| {
            ApiError::internal(format!("failed serializing collections snapshot: {error}"))
        })?;

        fs::write(&self.store_path, payload).map_err(|error| {
            ApiError::internal(format!(
                "failed writing collection snapshot '{}': {error}",
                self.store_path.display()
            ))
        })
    }

    fn sorted_records(&self) -> Vec<CollectionRecord> {
        let mut records: Vec<_> = self.collections.values().cloned().collect();
        records.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        records
    }
}

fn parse_graph(raw: Value) -> Result<GraphData, ApiError> {
    serde_json::from_value(raw)
        .map_err(|error| ApiError::invalid_params(format!("invalid collection graph: {error}")))
}

fn collection_not_found_error(collection_id: &str) -> ApiError {
    ApiError::collection_not_found(format!("Collection with id '{collection_id}' not found"))
        .with_details(serde_json::json!({ "collectionId": collection_id }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a minimal collection in a fresh domain so `run()` has something
    /// to export.
    fn fixture_with_collection() -> (TempDir, CollectionDomain, String) {
        let temp = TempDir::new().expect("tempdir");
        let mut domain = CollectionDomain::new(temp.path());
        let record = domain
            .create(CollectionCreateParams {
                name: "Test".to_string(),
                description: None,
                graph: None,
            })
            .expect("create collection");
        (temp, domain, record.id)
    }

    #[test]
    fn run_surfaces_stderr_from_failed_spawn() {
        // A stub "ralph" that prints a distinctive error and exits non-zero.
        // /bin/sh is universally available on macOS + Linux (Ralph's only
        // supported platforms).
        let (temp, domain, id) = fixture_with_collection();

        // We invoke /bin/sh with `-c` so the arg list ralph.run() builds
        // (`run -H <path> --no-tui -p <prompt>`) gets passed as extra
        // positional args; sh echoes to stderr and exits 1 regardless.
        let script = r#"echo "pi: command not found (simulated)" >&2; exit 1"#;

        // Build a wrapper script that always writes the marker to stderr
        // and exits 1. We'll point ralph_command at it.
        let wrapper_path = temp.path().join("fake-ralph.sh");
        std::fs::write(&wrapper_path, format!("#!/bin/sh\n{script}\n")).expect("write wrapper");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&wrapper_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&wrapper_path, perms).expect("chmod");

        let result = domain.run(
            CollectionRunParams {
                id,
                prompt: "hello".to_string(),
            },
            wrapper_path.to_str().expect("wrapper path"),
            temp.path(),
        );

        let err = result.expect_err("fake ralph should fail");
        let message = format!("{err:?}");
        assert!(
            message.contains("pi: command not found (simulated)"),
            "error should contain the underlying stderr; got: {message}"
        );
        assert!(
            message.contains("exit code 1"),
            "error should name the exit code; got: {message}"
        );
        assert!(
            !message.contains("..."),
            "error should not truncate stderr; got: {message}"
        );
    }

    /// Best-effort cleanup of a test-owned background process whose pid
    /// was recorded to a file. Runs on drop, so cleanup happens on
    /// success, timeout, and assertion failure alike — never skipped
    /// precisely when a regression occurs. Only the recorded pid is
    /// signaled.
    struct KillPidFile {
        path: std::path::PathBuf,
    }

    impl Drop for KillPidFile {
        fn drop(&mut self) {
            if let Ok(pid) = std::fs::read_to_string(&self.path) {
                let pid = pid.trim().to_string();
                if !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()) {
                    let _ = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!("kill {pid} 2>/dev/null"))
                        .status();
                }
            }
        }
    }

    /// A reader that fails with `Interrupted` exactly once, then behaves.
    /// Protects the drain's retry behavior without processes or timing.
    struct InterruptOnceThen<Inner> {
        inner: Inner,
        interrupted: bool,
    }

    impl<Inner: std::io::Read> std::io::Read for InterruptOnceThen<Inner> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "simulated signal",
                ));
            }
            self.inner.read(buf)
        }
    }

    #[test]
    fn drain_retries_interrupted_reads() {
        let tail = std::sync::Arc::new(StderrTail::default());
        let reader = InterruptOnceThen {
            inner: &b"after-signal"[..],
            interrupted: false,
        };
        drain_to_tail(reader, &tail);
        assert_eq!(tail.snapshot_string(), "after-signal");
    }

    #[cfg(unix)]
    #[test]
    fn drain_exits_on_cancel_while_writer_alive() {
        // The production cancellation handle itself: cancelling unblocks
        // the drain even though the writer stays alive and holds the write
        // end open. This is what lets request handling and the reaper
        // proceed without waiting for inherited-handle EOF.
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        let (read_end, mut write_end) = UnixStream::pair().expect("socket pair");
        let drain = StderrDrain::spawn(read_end).expect("spawn drain");
        write_end.write_all(b"partial output").expect("write");
        write_end.flush().expect("flush");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !drain.snapshot().contains("partial output") && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            drain.snapshot().contains("partial output"),
            "drain must collect while the writer is alive"
        );
        drain.cancel();
        assert!(
            drain.wait_done(std::time::Duration::from_secs(5)),
            "drain must exit on cancel while the writer is still alive"
        );
        // After cancel the writer gets EPIPE on its next write (standard
        // dead-consumer behavior, same as `producer | head`): the run's
        // stderr has no consumer anymore. Nothing hangs either way.
        assert!(
            write_end.write_all(b"more").is_err(),
            "writes after cancel must fail fast, not block"
        );
    }

    /// How a test producer stopped writing.
    #[cfg(unix)]
    #[derive(Debug, PartialEq, Eq)]
    enum ProducerStop {
        /// Writes started failing: carries the I/O error kind so the test
        /// can require `BrokenPipe` (EPIPE after cancel) specifically.
        WriteError(std::io::ErrorKind),
        /// The loop ran to completion without any write failing: cancel
        /// arrived too late and nothing was proven.
        Completed,
    }

    /// Cancels the drain when dropped, so every exit path — including a
    /// panicking assertion — still unblocks the drain and EPIPEs the
    /// producer instead of leaking either thread.
    #[cfg(unix)]
    struct CancelOnDrop<'a> {
        drain: &'a StderrDrain,
    }

    #[cfg(unix)]
    impl Drop for CancelOnDrop<'_> {
        fn drop(&mut self) {
            self.drain.cancel();
        }
    }

    /// Shuts the test socket down when dropped, independent of the drain
    /// mechanism under test: even if cancellation regressed, a producer
    /// blocked in a write is released instead of leaking its thread.
    #[cfg(unix)]
    struct ShutdownSocketOnDrop {
        stream: std::os::unix::net::UnixStream,
    }

    #[cfg(unix)]
    impl Drop for ShutdownSocketOnDrop {
        fn drop(&mut self) {
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
    }

    #[cfg(unix)]
    #[test]
    fn drain_cancelled_during_sustained_output() {
        // Cancellation while the producer is actively streaming (not idle):
        // the drain must still exit promptly with bounded retained output,
        // and the producer must stop on EPIPE instead of leaking. The
        // producer writes without bound until cancellation takes effect,
        // and reports why it stopped — so a too-late cancel fails the
        // test instead of silently proving nothing.
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        let (read_end, write_end) = UnixStream::pair().expect("socket pair");
        // A finite write timeout bounds any single stuck write; any error
        // (including a timeout) stops the producer and is reported, so a
        // blocked writer fails the test instead of hanging it.
        write_end
            .set_write_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("write timeout");
        // Independent of the drain mechanism under test: releases a
        // blocked writer even if cancellation regressed.
        let _socket_guard = ShutdownSocketOnDrop {
            stream: write_end.try_clone().expect("clone write end"),
        };
        let drain = StderrDrain::spawn(read_end).expect("spawn drain");
        let _cancel_guard = CancelOnDrop { drain: &drain };
        let (stop_tx, stop_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut write_end = write_end;
            let chunk = [b'x'; 1024];
            // Unbounded in principle, capped in practice: 512 MiB is far
            // beyond any legitimate grace period, so hitting the cap means
            // cancellation never took effect.
            let mut stop = ProducerStop::Completed;
            for _ in 0..524_288 {
                if let Err(error) = write_end.write_all(&chunk) {
                    stop = ProducerStop::WriteError(error.kind());
                    break;
                }
            }
            let _ = stop_tx.send(stop);
        });
        // Cancel only once streaming has been observed, so the test cannot
        // pass by cancelling before the first byte. A full tail proves the
        // producer had been streaming (not that it still is — the
        // exit-reason assertion below covers that distinction).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while drain.snapshot().len() < 64 * 1024 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            drain.snapshot().len() >= 64 * 1024,
            "producer must have been streaming before cancel"
        );
        drain.cancel();
        assert!(
            drain.wait_done(std::time::Duration::from_secs(5)),
            "drain must exit on cancel during sustained output"
        );
        assert!(
            drain.snapshot().len() <= StderrTail::CAP + 64,
            "retained output must stay bounded (got {} bytes)",
            drain.snapshot().len()
        );
        assert_eq!(
            stop_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("producer must terminate after cancel"),
            ProducerStop::WriteError(std::io::ErrorKind::BrokenPipe),
            "producer must stop on EPIPE after cancel, not run unbounded"
        );
    }

    #[test]
    fn run_succeeds_when_child_fills_stderr_buffer() {
        // A child logging more than the buffer must not deadlock the run.
        // The marker file proves the child ran to completion (not merely
        // that run() returned: the API answers once the child exits *or*
        // outlives the startup window).
        let (temp, domain, id) = fixture_with_collection();
        let script = r#"echo $$ > noisy.pid; i=0; while [ "$i" -lt 4000 ]; do echo "log padding to exceed the stderr buffer" >&2; i=$((i + 1)); done; echo done > noise-done; exit 0"#;
        let wrapper_path = temp.path().join("noisy-ralph.sh");
        std::fs::write(&wrapper_path, format!("#!/bin/sh\n{script}\n")).expect("write wrapper");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&wrapper_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&wrapper_path, perms).expect("chmod");
        }

        // Cleanup first: the guard drops (and kills the recorded child, if
        // still alive) on every exit path, including a wedged pre-fix run.
        // The TempDir likewise stays owned by this thread: moving it into
        // the worker would delete the child's working directory (and the
        // marker) while the child may still be running.
        let _cleanup = KillPidFile {
            path: temp.path().join("noisy.pid"),
        };
        let workspace = temp.path().to_path_buf();
        let marker = workspace.join("noise-done");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = domain.run(
                CollectionRunParams {
                    id,
                    prompt: "hello".to_string(),
                },
                wrapper_path.to_str().expect("wrapper path"),
                &workspace,
            );
            let _ = tx.send(result);
        });
        // Collect the outcome first, assert afterwards: without a
        // continuous drain the child wedges on the full buffer and the
        // marker never appears; with it both land promptly.
        let run_result = rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("run must answer instead of deadlocking on a full stderr pipe");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let marker_present = marker.exists();
        let run = run_result.expect("noisy run should succeed");
        assert!(run.success, "noisy run should report success");
        assert!(marker_present, "backend must have run to completion");
    }

    #[test]
    fn stderr_tail_passes_small_output_verbatim() {
        let tail = StderrTail::default();
        tail.push(b"boom\n");
        assert_eq!(tail.snapshot_string(), "boom\n");
    }

    #[test]
    fn stderr_tail_reports_exact_truncation() {
        // Counter and buffer snapshot together: 80_000 bytes in, CAP
        // retained, and the note names exactly the discarded prefix.
        let tail = StderrTail::default();
        tail.push(&vec![b'a'; 40_000]);
        tail.push(&vec![b'b'; 40_000]);
        let snapshot = tail.snapshot_string();
        let note = "[earliest 14464 bytes truncated]\n";
        assert!(
            snapshot.starts_with(note),
            "note must name the discarded prefix; got: {}",
            &snapshot[..note.len().min(snapshot.len())]
        );
        assert_eq!(snapshot.len(), note.len() + StderrTail::CAP);
        // Newest bytes survive in full: the whole second push plus exactly
        // the tail of the first — the entire retained suffix, not just its
        // edges.
        let mut expected = "a".repeat(25536);
        expected.push_str(&"b".repeat(40_000));
        assert_eq!(&snapshot[note.len()..], &expected[..]);
    }

    #[test]
    fn run_failure_with_inherited_stderr_returns_promptly() {
        // A failing child whose descendant inherits the stderr handle must
        // not stall the error path: diagnostics come from a bounded
        // snapshot, never from waiting for EOF.
        let (temp, domain, id) = fixture_with_collection();
        // The descendant inherits stderr and outlives the exiting parent;
        // its pid is recorded so the drop-guard cleans it up on every exit
        // path (it holds the handle open by design; the run must not wait
        // for it, but the test must not leave it behind either).
        let script = r"(sleep 30 >&2 & echo $! > sleep.pid); echo boom >&2; exit 1";
        let wrapper_path = temp.path().join("inheriting-ralph.sh");
        std::fs::write(&wrapper_path, format!("#!/bin/sh\n{script}\n")).expect("write wrapper");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&wrapper_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&wrapper_path, perms).expect("chmod");
        }

        let _cleanup = KillPidFile {
            path: temp.path().join("sleep.pid"),
        };
        let started = std::time::Instant::now();
        let result = domain.run(
            CollectionRunParams {
                id,
                prompt: "hello".to_string(),
            },
            wrapper_path.to_str().expect("wrapper path"),
            temp.path(),
        );
        let elapsed = started.elapsed();
        // Collect before asserting so the guard above still cleans up when
        // an assertion fails.
        let message = match result {
            Err(error) => format!("{error:?}"),
            Ok(_) => panic!("failing backend should error"),
        };
        assert!(
            message.contains("boom"),
            "error should contain the backend stderr; got: {message}"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(20),
            "error must return without waiting for the inherited handle (took {elapsed:?})"
        );
        // Proof of cleanup (the drop-guard is the backstop, not the proof):
        // terminate the recorded descendant and require it gone.
        let pid = std::fs::read_to_string(temp.path().join("sleep.pid"))
            .expect("descendant pid recorded")
            .trim()
            .to_string();
        assert!(
            !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()),
            "descendant pid must be recorded"
        );
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("kill {pid} 2>/dev/null"))
            .status();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut gone = false;
        while std::time::Instant::now() < deadline {
            let still_alive = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("kill -0 {pid} 2>/dev/null"))
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if !still_alive {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(gone, "test descendant {pid} must be gone after cleanup");
    }

    #[test]
    fn create_rejects_empty_name() {
        let temp = TempDir::new().expect("tempdir");
        let mut domain = CollectionDomain::new(temp.path());
        let err = domain
            .create(CollectionCreateParams {
                name: "  ".to_string(),
                description: None,
                graph: None,
            })
            .expect_err("empty name should fail");
        assert!(format!("{err:?}").contains("name must not be empty"));
    }

    #[test]
    fn update_rejects_empty_name() {
        let (_temp, mut domain, id) = fixture_with_collection();
        let err = domain
            .update(CollectionUpdateParams {
                id,
                name: Some(String::new()),
                description: None,
                graph: None,
            })
            .expect_err("empty name should fail");
        assert!(format!("{err:?}").contains("name must not be empty"));
    }

    #[test]
    fn run_handles_missing_ralph_binary() {
        let (temp, domain, id) = fixture_with_collection();
        let missing = temp.path().join("definitely-not-here");

        let result = domain.run(
            CollectionRunParams {
                id,
                prompt: "hello".to_string(),
            },
            missing.to_str().expect("missing path"),
            temp.path(),
        );

        let err = result.expect_err("missing binary should fail");
        let message = format!("{err:?}");
        assert!(
            message.contains("ralph CLI not found"),
            "error should name the spawn failure; got: {message}"
        );
    }
}
