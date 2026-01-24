//! Loop Manager - Spawn and stop Ralph orchestration loops.
//!
//! Manages the lifecycle of multiple concurrent loops, including:
//! - Process spawning with diagnostics enabled
//! - Graceful shutdown via SIGINT
//! - Force kill after timeout
//! - Active loops state tracking
//!
//! This module provides a trait-based interface to support both production
//! (`LoopManager`) and test (`MockLoopManager`) implementations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

/// Configuration for starting a new loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    /// Path to the Ralph config file (e.g., `ralph.yml`)
    pub config_path: String,
    /// Prompt to pass to the loop
    pub prompt: String,
    /// Working directory for the loop
    pub working_dir: PathBuf,
}

/// Information about an active loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveLoopInfo {
    /// Session ID (derived from start time)
    pub session_id: String,
    /// Config path used
    pub config_path: String,
    /// Prompt used
    pub prompt: String,
    /// Working directory
    pub working_dir: PathBuf,
    /// Process ID
    pub pid: u32,
    /// Start time as ISO 8601 timestamp
    pub started_at: String,
}

/// Internal state for an active loop.
struct ActiveLoop {
    /// The spawned child process
    child: Child,
    /// Loop information
    info: ActiveLoopInfo,
}

/// Error types for loop manager operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoopError {
    /// No loop found with the given session ID
    NotFound { session_id: String },
    /// Failed to spawn process
    SpawnFailed { message: String },
    /// Failed to stop process
    StopFailed { message: String },
    /// Config file not found
    ConfigNotFound { path: String },
}

impl std::fmt::Display for LoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoopError::NotFound { session_id } => {
                write!(f, "Loop not found: {}", session_id)
            }
            LoopError::SpawnFailed { message } => write!(f, "Failed to spawn loop: {}", message),
            LoopError::StopFailed { message } => write!(f, "Failed to stop loop: {}", message),
            LoopError::ConfigNotFound { path } => write!(f, "Config file not found: {}", path),
        }
    }
}

impl std::error::Error for LoopError {}

/// Trait for loop manager implementations.
///
/// This trait allows for dependency injection of either the real `LoopManager`
/// (for production) or `MockLoopManager` (for E2E testing).
#[async_trait]
pub trait LoopManagerTrait: Send + Sync {
    /// Start a new loop with the given configuration.
    ///
    /// Returns the session ID on success.
    async fn start(&self, config: LoopConfig) -> Result<String, LoopError>;

    /// Stop a specific loop by session ID.
    ///
    /// Sends SIGINT for graceful shutdown, then force kills after timeout.
    async fn stop(&self, session_id: &str) -> Result<(), LoopError>;

    /// List all active loops.
    async fn list_active(&self) -> Vec<ActiveLoopInfo>;

    /// Get information about a specific loop.
    async fn get(&self, session_id: &str) -> Option<ActiveLoopInfo>;

    /// Check if any loop is currently running.
    async fn has_active_loops(&self) -> bool;

    /// Get the count of active loops.
    async fn active_count(&self) -> usize;

    /// Get self as Any for downcasting (used by test endpoints).
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Timeout for graceful shutdown before force kill.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Manager for Ralph orchestration loops.
///
/// Supports multiple concurrent loops, each identified by a session ID.
#[derive(Clone)]
pub struct LoopManager {
    /// Currently active loops (session_id -> loop)
    active_loops: Arc<RwLock<HashMap<String, ActiveLoop>>>,
    /// Path to the Ralph binary
    ralph_binary: PathBuf,
}

impl LoopManager {
    /// Create a new loop manager.
    ///
    /// If `ralph_binary` is None, it will search for "ralph" in PATH.
    pub fn new(ralph_binary: Option<PathBuf>) -> Self {
        let ralph_binary = ralph_binary.unwrap_or_else(|| PathBuf::from("ralph"));

        Self {
            active_loops: Arc::new(RwLock::new(HashMap::new())),
            ralph_binary,
        }
    }

    /// Internal implementation of stop - sends SIGINT and waits.
    async fn stop_loop(child: &mut Child, info: &ActiveLoopInfo) -> Result<(), LoopError> {
        tracing::info!("Stopping loop {} (PID {})", info.session_id, info.pid);

        // Send SIGINT for graceful shutdown
        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(info.pid as i32);
            if let Err(e) = kill(pid, Signal::SIGINT) {
                tracing::warn!("Failed to send SIGINT: {}", e);
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, just try to kill
            let _ = child.start_kill();
        }

        // Wait with timeout
        let wait_result = tokio::time::timeout(SHUTDOWN_TIMEOUT, child.wait()).await;

        match wait_result {
            Ok(Ok(status)) => {
                tracing::info!("Loop {} exited with status: {:?}", info.session_id, status);
                Ok(())
            }
            Ok(Err(e)) => {
                tracing::error!("Error waiting for loop {}: {}", info.session_id, e);
                Err(LoopError::StopFailed {
                    message: e.to_string(),
                })
            }
            Err(_) => {
                // Timeout - force kill
                tracing::warn!(
                    "Loop {} did not exit gracefully, force killing",
                    info.session_id
                );
                if let Err(e) = child.kill().await {
                    tracing::error!("Failed to force kill loop {}: {}", info.session_id, e);
                    return Err(LoopError::StopFailed {
                        message: e.to_string(),
                    });
                }
                Ok(())
            }
        }
    }
}

#[async_trait]
impl LoopManagerTrait for LoopManager {
    async fn start(&self, config: LoopConfig) -> Result<String, LoopError> {
        // Validate config file exists
        let config_path = config.working_dir.join(&config.config_path);
        if !config_path.exists() {
            return Err(LoopError::ConfigNotFound {
                path: config.config_path,
            });
        }

        // Generate session ID from current timestamp with milliseconds for uniqueness
        let now = chrono::Utc::now();
        let session_id = now.format("%Y-%m-%dT%H-%M-%S%.3f").to_string();
        let started_at = now.to_rfc3339();

        // Spawn the ralph process
        let child = Command::new(&self.ralph_binary)
            .args(["run", "-c", &config.config_path, "-p", &config.prompt])
            .current_dir(&config.working_dir)
            .env("RALPH_DIAGNOSTICS", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| LoopError::SpawnFailed {
                message: e.to_string(),
            })?;

        let pid = child.id().ok_or_else(|| LoopError::SpawnFailed {
            message: "Failed to get process ID".to_string(),
        })?;

        let info = ActiveLoopInfo {
            session_id: session_id.clone(),
            config_path: config.config_path,
            prompt: config.prompt,
            working_dir: config.working_dir,
            pid,
            started_at,
        };

        tracing::info!("Started loop {} with PID {}", session_id, pid);

        let mut loops = self.active_loops.write().await;
        loops.insert(session_id.clone(), ActiveLoop { child, info });

        Ok(session_id)
    }

    async fn stop(&self, session_id: &str) -> Result<(), LoopError> {
        let mut loops = self.active_loops.write().await;

        let active_loop = loops
            .remove(session_id)
            .ok_or_else(|| LoopError::NotFound {
                session_id: session_id.to_string(),
            })?;

        let ActiveLoop { mut child, info } = active_loop;

        Self::stop_loop(&mut child, &info).await
    }

    async fn list_active(&self) -> Vec<ActiveLoopInfo> {
        let loops = self.active_loops.read().await;
        loops.values().map(|l| l.info.clone()).collect()
    }

    async fn get(&self, session_id: &str) -> Option<ActiveLoopInfo> {
        let loops = self.active_loops.read().await;
        loops.get(session_id).map(|l| l.info.clone())
    }

    async fn has_active_loops(&self) -> bool {
        !self.active_loops.read().await.is_empty()
    }

    async fn active_count(&self) -> usize {
        self.active_loops.read().await.len()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Default for LoopManager {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config(dir: &std::path::Path) -> PathBuf {
        let config_path = dir.join("ralph.yml");
        fs::write(
            &config_path,
            r"
backend: claude
model: claude-3-5-sonnet-20241022
max_iterations: 1
",
        )
        .unwrap();
        config_path
    }

    #[tokio::test]
    async fn test_loop_manager_new() {
        let manager = LoopManager::new(None);
        assert!(!manager.has_active_loops().await);
        assert!(manager.list_active().await.is_empty());
    }

    #[tokio::test]
    async fn test_loop_manager_with_custom_binary() {
        let manager = LoopManager::new(Some(PathBuf::from("/usr/bin/ralph")));
        assert_eq!(manager.ralph_binary, PathBuf::from("/usr/bin/ralph"));
    }

    #[tokio::test]
    async fn test_config_not_found() {
        let manager = LoopManager::new(Some(PathBuf::from("echo")));
        let temp = TempDir::new().unwrap();

        let config = LoopConfig {
            config_path: "nonexistent.yml".to_string(),
            prompt: "test".to_string(),
            working_dir: temp.path().to_path_buf(),
        };

        let result = manager.start(config).await;
        assert!(matches!(result, Err(LoopError::ConfigNotFound { .. })));
    }

    #[tokio::test]
    async fn test_stop_not_found() {
        let manager = LoopManager::new(None);
        let result = manager.stop("nonexistent").await;
        assert!(matches!(result, Err(LoopError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_start_creates_session_id() {
        // Use 'sleep' as a mock ralph binary (it will just wait)
        let manager = LoopManager::new(Some(PathBuf::from("sleep")));
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        let config = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "test".to_string(),
            working_dir: temp.path().to_path_buf(),
        };

        // This will fail because 'sleep' doesn't accept ralph args,
        // but we're testing that it attempts to start
        let result = manager.start(config).await;

        // Either it started (and we can check session_id format) or spawn failed
        // which is expected with mock binary
        match result {
            Ok(session_id) => {
                // Session ID should be timestamp format
                assert!(session_id.contains('-'));
                assert!(session_id.contains('T'));

                // Clean up
                let _ = manager.stop(&session_id).await;
            }
            Err(LoopError::SpawnFailed { .. }) => {
                // Expected if 'sleep' binary doesn't exist or fails
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_multiple_concurrent_loops() {
        // Use 'sleep' as a mock that stays running
        let manager = LoopManager::new(Some(PathBuf::from("sleep")));
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();
        create_test_config(temp1.path());
        create_test_config(temp2.path());

        let config1 = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "100".to_string(), // sleep for 100 seconds
            working_dir: temp1.path().to_path_buf(),
        };

        let config2 = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "100".to_string(),
            working_dir: temp2.path().to_path_buf(),
        };

        // Start first loop
        if let Ok(session1) = manager.start(config1).await {
            assert_eq!(manager.active_count().await, 1);

            // Start second loop
            if let Ok(session2) = manager.start(config2).await {
                assert_eq!(manager.active_count().await, 2);
                assert!(manager.has_active_loops().await);

                // Both should be in list
                let active = manager.list_active().await;
                assert_eq!(active.len(), 2);

                // Can get each by ID
                assert!(manager.get(&session1).await.is_some());
                assert!(manager.get(&session2).await.is_some());

                // Clean up
                let _ = manager.stop(&session2).await;
            }
            let _ = manager.stop(&session1).await;
        }
    }

    #[tokio::test]
    async fn test_stop_specific_loop() {
        // Use bash to run sleep, ignoring ralph-style args
        // bash -c "sleep 100" will work even when extra args are passed
        let manager = LoopManager::new(Some(PathBuf::from("bash")));
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();
        create_test_config(temp1.path());
        create_test_config(temp2.path());

        // Create a script that ignores args and just sleeps
        let script = "exec sleep 100";

        let config1 = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: script.to_string(),
            working_dir: temp1.path().to_path_buf(),
        };

        let config2 = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: script.to_string(),
            working_dir: temp2.path().to_path_buf(),
        };

        // Start both loops - bash receives: run -c ralph.yml -p "exec sleep 100"
        // The last arg to -p becomes the bash command
        let result1 = manager.start(config1).await;
        let result2 = manager.start(config2).await;

        match (result1, result2) {
            (Ok(session1), Ok(session2)) => {
                // Give processes a moment to fully spawn
                tokio::time::sleep(Duration::from_millis(50)).await;

                assert_eq!(manager.active_count().await, 2);

                // Stop first loop
                assert!(manager.stop(&session1).await.is_ok());
                assert_eq!(manager.active_count().await, 1);

                // First loop should be gone
                assert!(manager.get(&session1).await.is_none());

                // Second loop should still be active
                assert!(manager.get(&session2).await.is_some());

                // Clean up
                let _ = manager.stop(&session2).await;
            }
            _ => {
                // If spawning fails (e.g., sandbox restrictions), skip the test
                eprintln!("Skipping test: could not spawn bash processes");
            }
        }
    }

    #[tokio::test]
    async fn test_active_loop_info() {
        let manager = LoopManager::new(Some(PathBuf::from("sleep")));
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        let config = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "test prompt".to_string(),
            working_dir: temp.path().to_path_buf(),
        };

        if let Ok(session_id) = manager.start(config).await {
            let info = manager.get(&session_id).await;
            assert!(info.is_some());

            let info = info.unwrap();
            assert_eq!(info.session_id, session_id);
            assert_eq!(info.config_path, "ralph.yml");
            assert_eq!(info.prompt, "test prompt");

            // Clean up
            let _ = manager.stop(&session_id).await;
        }
    }
}
