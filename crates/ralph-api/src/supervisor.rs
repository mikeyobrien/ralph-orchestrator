// ABOUTME: Process supervisor for managed factory instances.
// ABOUTME: Spawns `ralph factory` as child processes and tracks their lifecycle.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command as AsyncCommand};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Grace period for factory processes to shut down before SIGKILL.
const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(10);

/// Reaper poll interval — how often we check child exit status.
const REAPER_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactorySpawnConfig {
    pub backend: Option<String>,
    pub max_iterations: Option<u32>,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactoryStatus {
    Running,
    Exited,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedFactory {
    pub id: String,
    pub pid: u32,
    pub num_workers: u32,
    pub config: FactorySpawnConfig,
    pub started_at: String,
    pub status: FactoryStatus,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactoryCreateParams {
    pub num_workers: u32,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub max_iterations: Option<u32>,
}

#[derive(Clone)]
pub struct ProcessSupervisor {
    factories: Arc<Mutex<HashMap<String, ManagedFactory>>>,
    children: Arc<Mutex<HashMap<String, Child>>>,
    workspace_root: PathBuf,
    ralph_binary: PathBuf,
    api_port: u16,
}

impl ProcessSupervisor {
    pub fn new(workspace_root: PathBuf, api_port: u16) -> Self {
        let ralph_binary =
            std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ralph"));

        Self {
            factories: Arc::new(Mutex::new(HashMap::new())),
            children: Arc::new(Mutex::new(HashMap::new())),
            workspace_root,
            ralph_binary,
            api_port,
        }
    }

    pub async fn create_factory(
        &self,
        params: FactoryCreateParams,
    ) -> anyhow::Result<ManagedFactory> {
        let now = Utc::now();
        let id = format!("factory-{}", now.timestamp_millis());

        let api_url = format!("http://127.0.0.1:{}", self.api_port);

        let mut args = vec![
            "factory".to_string(),
            "-w".to_string(),
            params.num_workers.to_string(),
            "--api-url".to_string(),
            api_url.clone(),
        ];

        if let Some(ref backend) = params.backend {
            args.push("--backend".to_string());
            args.push(backend.clone());
        }

        if let Some(max_iter) = params.max_iterations {
            args.push("--max-iterations".to_string());
            args.push(max_iter.to_string());
        }

        let child = AsyncCommand::new(&self.ralph_binary)
            .args(&args)
            .current_dir(&self.workspace_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let pid = child.id().unwrap_or(0);

        let config = FactorySpawnConfig {
            backend: params.backend,
            max_iterations: params.max_iterations,
            api_url: Some(api_url),
        };

        let factory = ManagedFactory {
            id: id.clone(),
            pid,
            num_workers: params.num_workers,
            config,
            started_at: now.to_rfc3339(),
            status: FactoryStatus::Running,
            exit_code: None,
        };

        info!(
            factory_id = %id,
            pid = pid,
            num_workers = params.num_workers,
            "Spawned factory process"
        );

        self.factories.lock().await.insert(id.clone(), factory.clone());
        self.children.lock().await.insert(id, child);

        Ok(factory)
    }

    pub async fn stop_factory(&self, id: &str) -> anyhow::Result<()> {
        let mut children = self.children.lock().await;
        let child = children
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("factory '{}' not found or already exited", id))?;

        info!(factory_id = %id, "Stopping factory process");
        terminate_gracefully(child, SHUTDOWN_GRACE_PERIOD).await;

        // Update status
        let mut factories = self.factories.lock().await;
        if let Some(factory) = factories.get_mut(id) {
            factory.status = FactoryStatus::Exited;
        }

        children.remove(id);

        Ok(())
    }

    pub async fn list_factories(&self) -> Vec<ManagedFactory> {
        let factories = self.factories.lock().await;
        let mut list: Vec<ManagedFactory> = factories.values().cloned().collect();
        list.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        list
    }

    /// Stops all running factories. Called during shutdown.
    pub async fn stop_all(&self) {
        let ids: Vec<String> = self.children.lock().await.keys().cloned().collect();
        for id in ids {
            if let Err(e) = self.stop_factory(&id).await {
                warn!(factory_id = %id, error = %e, "Failed to stop factory during shutdown");
            }
        }
    }

    /// Start background reaper task that polls child processes for exit.
    pub fn start_reaper(&self) -> tokio::task::JoinHandle<()> {
        let factories = self.factories.clone();
        let children = self.children.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(REAPER_INTERVAL).await;

                let mut children_guard = children.lock().await;
                let mut factories_guard = factories.lock().await;

                let mut exited = Vec::new();

                for (id, child) in children_guard.iter_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let code = status.code();
                            info!(
                                factory_id = %id,
                                exit_code = ?code,
                                "Factory process exited"
                            );
                            if let Some(factory) = factories_guard.get_mut(id) {
                                factory.status = FactoryStatus::Exited;
                                factory.exit_code = code;
                            }
                            exited.push(id.clone());
                        }
                        Ok(None) => {
                            // Still running
                        }
                        Err(e) => {
                            warn!(
                                factory_id = %id,
                                error = %e,
                                "Failed to check factory process status"
                            );
                        }
                    }
                }

                for id in exited {
                    children_guard.remove(&id);
                }
            }
        })
    }
}

/// Gracefully terminate a child process: SIGTERM → grace period → SIGKILL.
#[cfg(unix)]
async fn terminate_gracefully(child: &mut Child, grace_period: Duration) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    if let Some(pid) = child.id() {
        let pid = Pid::from_raw(pid as i32);

        if kill(pid, Signal::SIGTERM).is_err() {
            let _ = child.wait().await;
            return;
        }

        match tokio::time::timeout(grace_period, child.wait()).await {
            Ok(_) => {}
            Err(_) => {
                warn!("Factory grace period elapsed, forcing termination");
                let _ = kill(pid, Signal::SIGKILL);
                let _ = child.wait().await;
            }
        }
    } else {
        let _ = child.wait().await;
    }
}

#[cfg(not(unix))]
async fn terminate_gracefully(child: &mut Child, _grace_period: Duration) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn list_factories_empty_by_default() {
        let temp = TempDir::new().unwrap();
        let supervisor = ProcessSupervisor::new(temp.path().to_path_buf(), 3000);
        let list = supervisor.list_factories().await;
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn stop_nonexistent_factory_returns_error() {
        let temp = TempDir::new().unwrap();
        let supervisor = ProcessSupervisor::new(temp.path().to_path_buf(), 3000);
        let result = supervisor.stop_factory("nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn factory_status_serializes_as_snake_case() {
        let json = serde_json::to_string(&FactoryStatus::Running).unwrap();
        assert_eq!(json, "\"running\"");
        let json = serde_json::to_string(&FactoryStatus::Exited).unwrap();
        assert_eq!(json, "\"exited\"");
    }

    #[test]
    fn managed_factory_serializes_to_camel_case() {
        let factory = ManagedFactory {
            id: "factory-123".to_string(),
            pid: 42,
            num_workers: 2,
            config: FactorySpawnConfig {
                backend: Some("claude".to_string()),
                max_iterations: Some(10),
                api_url: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            status: FactoryStatus::Running,
            exit_code: None,
        };
        let val = serde_json::to_value(&factory).unwrap();
        assert_eq!(val["numWorkers"], 2);
        assert_eq!(val["startedAt"], "2026-01-01T00:00:00Z");
        assert_eq!(val["status"], "running");
        assert!(val.get("exitCode").is_some());
    }

    #[test]
    fn factory_create_params_deserializes_minimal() {
        let json = serde_json::json!({ "numWorkers": 3 });
        let params: FactoryCreateParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.num_workers, 3);
        assert!(params.backend.is_none());
        assert!(params.max_iterations.is_none());
    }

    #[test]
    fn factory_create_params_deserializes_full() {
        let json = serde_json::json!({
            "numWorkers": 4,
            "backend": "gemini",
            "maxIterations": 20
        });
        let params: FactoryCreateParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.num_workers, 4);
        assert_eq!(params.backend.as_deref(), Some("gemini"));
        assert_eq!(params.max_iterations, Some(20));
    }
}
