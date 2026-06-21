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

        // Spawn ralph run with -H (hats overlay) so the user's ralph.yml
        // provides backend/max_iterations/backpressure and the collection
        // provides hats/events. -a (autonomous) forces headless mode, which
        // is required when the API spawns ralph: interactive mode tries to
        // read from a tty the background process doesn't own and gets
        // SIGSTOP'd by the OS. Autonomous mode implies --no-tui.
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
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| {
                ApiError::internal(format!(
                    "ralph CLI not found or failed to start. Install ralph or set RALPH_API_RALPH_COMMAND. Error: {error}"
                ))
            })?;

        let pid = child.id();

        // Wait briefly to check if the process died immediately.
        let mut child = child;
        std::thread::sleep(std::time::Duration::from_millis(500));
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                let mut stderr_output = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    use std::io::Read;
                    let _ = stderr.read_to_string(&mut stderr_output);
                }
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
                    // Pass ralph's stderr through verbatim. Spawn-failure
                    // output is small; truncation risks hiding the actual
                    // error line.
                    format!("ralph run exited with {status_label}:\n{trimmed}")
                };
                return Err(ApiError::internal(message));
            }
            _ => {
                // Still running or exited successfully. Detach a reaper so
                // the eventual exit doesn't leave a zombie process — the
                // API may outlive many loop runs.
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
