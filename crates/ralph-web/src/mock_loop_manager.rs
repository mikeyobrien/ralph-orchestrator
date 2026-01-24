//! Mock Loop Manager for E2E Testing
//!
//! Provides a mock implementation of `LoopManagerTrait` that simulates
//! AI interactions without making real API calls. Used for E2E testing
//! of the ralph-web dashboard.
//!
//! # Security
//!
//! This module is only compiled when the `test-mode` feature is enabled.
//! Production builds MUST NOT include this feature.

use crate::loop_manager::{ActiveLoopInfo, LoopConfig, LoopError, LoopManagerTrait};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock loop status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MockLoopStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// State for a mock loop.
#[derive(Debug, Clone)]
pub struct MockLoopState {
    /// Session ID (UUID v4)
    pub session_id: String,
    /// Config path used
    pub config_path: String,
    /// Prompt used
    pub prompt: String,
    /// Working directory
    pub working_dir: PathBuf,
    /// Current iteration number
    pub current_iteration: u32,
    /// Current hat
    pub current_hat: String,
    /// Loop status
    pub status: MockLoopStatus,
    /// Start time as ISO 8601 timestamp
    pub started_at: String,
    /// Loaded scenario name (if any)
    pub scenario_name: Option<String>,
}

impl MockLoopState {
    /// Convert to ActiveLoopInfo for API responses.
    fn to_active_loop_info(&self) -> ActiveLoopInfo {
        ActiveLoopInfo {
            session_id: self.session_id.clone(),
            config_path: self.config_path.clone(),
            prompt: self.prompt.clone(),
            working_dir: self.working_dir.clone(),
            // Mock loops don't have real PIDs - use 0
            pid: 0,
            started_at: self.started_at.clone(),
        }
    }
}

/// Mock event for scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockEvent {
    /// Event topic (e.g., "build.task", "confession.clean")
    pub topic: String,
    /// Event payload
    pub payload: String,
}

/// Mock output line for scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MockOutput {
    /// Text output from the agent
    Text { text: String },
    /// Tool call
    ToolCall {
        name: String,
        input: serde_json::Value,
    },
    /// Tool result
    ToolResult { output: String },
    /// Iteration complete with token counts
    Complete {
        input_tokens: u32,
        output_tokens: u32,
    },
    /// Error output
    Error { text: String },
}

/// Mock iteration for scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockIteration {
    /// Iteration number
    pub number: u32,
    /// Hat for this iteration
    pub hat: MockHat,
    /// Events emitted during this iteration
    #[serde(default)]
    pub events: Vec<MockEvent>,
    /// Output lines for this iteration
    #[serde(default)]
    pub output: Vec<MockOutput>,
    /// Duration in milliseconds (for timed playback)
    #[serde(default = "default_duration")]
    pub duration_ms: u64,
}

fn default_duration() -> u64 {
    100
}

/// Hat information for scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockHat {
    /// Hat ID (e.g., "builder", "ralph")
    pub id: String,
    /// Display name with emoji (e.g., "⚙️ Builder")
    pub display: String,
}

/// A complete mock scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockScenario {
    /// Scenario name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: String,
    /// Iterations in this scenario
    pub iterations: Vec<MockIteration>,
    /// Final status after all iterations
    pub final_status: MockLoopStatus,
}

/// Mock Loop Manager for E2E testing.
///
/// Simulates loop execution without making real API calls.
/// Writes diagnostic JSONL files that the FileWatcher can detect.
#[derive(Clone)]
pub struct MockLoopManager {
    /// Active loops (session_id -> state)
    active_loops: Arc<RwLock<HashMap<String, MockLoopState>>>,
    /// Loaded scenarios (name -> scenario)
    scenarios: Arc<RwLock<HashMap<String, MockScenario>>>,
    /// Path to diagnostics directory (used in Phase 2 for file writing)
    #[allow(dead_code)]
    diagnostics_path: PathBuf,
}

impl MockLoopManager {
    /// Create a new mock loop manager.
    pub fn new() -> Self {
        Self {
            active_loops: Arc::new(RwLock::new(HashMap::new())),
            scenarios: Arc::new(RwLock::new(HashMap::new())),
            diagnostics_path: PathBuf::from(".ralph/diagnostics"),
        }
    }

    /// Create with custom diagnostics path.
    pub fn with_diagnostics_path(diagnostics_path: PathBuf) -> Self {
        Self {
            active_loops: Arc::new(RwLock::new(HashMap::new())),
            scenarios: Arc::new(RwLock::new(HashMap::new())),
            diagnostics_path,
        }
    }

    /// Generate a timestamp-based session ID.
    ///
    /// Uses the same format as the real LoopManager (e.g., "2026-01-24T10-33-47")
    /// so the SessionStore can find and serve the session data.
    fn generate_session_id() -> String {
        Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string()
    }

    /// Load a scenario for later use.
    pub async fn load_scenario(&self, scenario: MockScenario) {
        let mut scenarios = self.scenarios.write().await;
        scenarios.insert(scenario.name.clone(), scenario);
    }

    /// Get a loaded scenario by name.
    pub async fn get_scenario(&self, name: &str) -> Option<MockScenario> {
        let scenarios = self.scenarios.read().await;
        scenarios.get(name).cloned()
    }

    /// Advance a loop to the next iteration.
    ///
    /// Returns error if loop not found or already completed.
    pub async fn advance_iteration(&self, session_id: &str) -> Result<(), LoopError> {
        let mut loops = self.active_loops.write().await;

        let state = loops
            .get_mut(session_id)
            .ok_or_else(|| LoopError::NotFound {
                session_id: session_id.to_string(),
            })?;

        if state.status != MockLoopStatus::Running {
            return Err(LoopError::StopFailed {
                message: "Loop is not running".to_string(),
            });
        }

        state.current_iteration += 1;
        let iteration = state.current_iteration;
        let hat = state.current_hat.clone();

        // Write iteration_started event to diagnostics
        // Note: Need to drop the lock before calling write methods
        drop(loops);
        self.write_orchestration_event(session_id, iteration, &hat, "iteration_started")?;

        // Write initial output for the new iteration
        self.write_agent_output(
            session_id,
            iteration,
            &hat,
            &MockOutput::Text {
                text: format!("🔄 Starting iteration {}", iteration),
            },
        )?;

        tracing::info!("Advanced loop {} to iteration {}", session_id, iteration);

        Ok(())
    }

    /// Inject an event into a running loop.
    pub async fn inject_event(&self, session_id: &str, event: MockEvent) -> Result<(), LoopError> {
        let loops = self.active_loops.read().await;

        let state = loops.get(session_id).ok_or_else(|| LoopError::NotFound {
            session_id: session_id.to_string(),
        })?;

        if state.status != MockLoopStatus::Running {
            return Err(LoopError::StopFailed {
                message: "Loop is not running".to_string(),
            });
        }

        let iteration = state.current_iteration;
        let hat = state.current_hat.clone();

        // Drop the lock before writing
        drop(loops);

        // Write event as agent output text
        self.write_agent_output(
            session_id,
            iteration,
            &hat,
            &MockOutput::Text {
                text: format!("📨 Event: {} - {}", event.topic, event.payload),
            },
        )?;

        tracing::info!("Injected event {} into loop {}", event.topic, session_id);

        Ok(())
    }

    /// Complete a loop with the specified status.
    pub async fn complete(
        &self,
        session_id: &str,
        status: MockLoopStatus,
    ) -> Result<(), LoopError> {
        let mut loops = self.active_loops.write().await;

        let state = loops
            .get_mut(session_id)
            .ok_or_else(|| LoopError::NotFound {
                session_id: session_id.to_string(),
            })?;

        state.status = status.clone();
        tracing::info!("Completed loop {} with status {:?}", session_id, status);

        // Remove from active loops if completed/failed/cancelled
        if status != MockLoopStatus::Running {
            loops.remove(session_id);
        }

        Ok(())
    }

    /// Reset all state (for test isolation).
    pub async fn reset(&self) {
        let mut loops = self.active_loops.write().await;
        loops.clear();

        let mut scenarios = self.scenarios.write().await;
        scenarios.clear();

        tracing::info!("MockLoopManager state reset");
    }

    /// Write diagnostic files to disk for a session.
    ///
    /// Creates the session directory and writes initial `orchestration.jsonl`
    /// with an iteration_started event. This allows the FileWatcher to detect
    /// the new session and stream events to the WebSocket.
    fn write_initial_diagnostics(&self, session_id: &str, hat: &str) -> Result<(), LoopError> {
        let session_dir = self.diagnostics_path.join(session_id);

        // Create session directory
        fs::create_dir_all(&session_dir).map_err(|e| LoopError::SpawnFailed {
            message: format!("Failed to create session directory: {}", e),
        })?;

        // Write orchestration.jsonl with iteration_started event
        let orchestration_path = session_dir.join("orchestration.jsonl");
        let event = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "iteration": 1,
            "hat": hat,
            "event": { "type": "iteration_started" }
        });

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&orchestration_path)
            .map_err(|e| LoopError::SpawnFailed {
                message: format!("Failed to open orchestration.jsonl: {}", e),
            })?;

        writeln!(file, "{}", event).map_err(|e| LoopError::SpawnFailed {
            message: format!("Failed to write to orchestration.jsonl: {}", e),
        })?;

        // Ensure content is flushed to disk for FileWatcher to detect
        file.sync_all().map_err(|e| LoopError::SpawnFailed {
            message: format!("Failed to sync orchestration.jsonl: {}", e),
        })?;

        tracing::debug!(
            "Wrote initial diagnostics for session {} to {:?}",
            session_id,
            session_dir
        );

        Ok(())
    }

    /// Write agent output to the diagnostics file.
    fn write_agent_output(
        &self,
        session_id: &str,
        iteration: u32,
        hat: &str,
        output: &MockOutput,
    ) -> Result<(), LoopError> {
        let session_dir = self.diagnostics_path.join(session_id);
        let output_path = session_dir.join("agent-output.jsonl");

        let event = match output {
            MockOutput::Text { text } => json!({
                "ts": Utc::now().to_rfc3339(),
                "iteration": iteration,
                "hat": hat,
                "type": "text",
                "text": text
            }),
            MockOutput::ToolCall { name, input } => json!({
                "ts": Utc::now().to_rfc3339(),
                "iteration": iteration,
                "hat": hat,
                "type": "tool_call",
                "name": name,
                "input": input
            }),
            MockOutput::ToolResult { output } => json!({
                "ts": Utc::now().to_rfc3339(),
                "iteration": iteration,
                "hat": hat,
                "type": "tool_result",
                "output": output
            }),
            MockOutput::Complete {
                input_tokens,
                output_tokens,
            } => json!({
                "ts": Utc::now().to_rfc3339(),
                "iteration": iteration,
                "hat": hat,
                "type": "complete",
                "input_tokens": input_tokens,
                "output_tokens": output_tokens
            }),
            MockOutput::Error { text } => json!({
                "ts": Utc::now().to_rfc3339(),
                "iteration": iteration,
                "hat": hat,
                "type": "error",
                "text": text
            }),
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&output_path)
            .map_err(|e| LoopError::SpawnFailed {
                message: format!("Failed to open agent-output.jsonl: {}", e),
            })?;

        writeln!(file, "{}", event).map_err(|e| LoopError::SpawnFailed {
            message: format!("Failed to write to agent-output.jsonl: {}", e),
        })?;

        // Ensure content is flushed to disk for FileWatcher to detect
        file.sync_all().map_err(|e| LoopError::SpawnFailed {
            message: format!("Failed to sync agent-output.jsonl: {}", e),
        })?;

        Ok(())
    }

    /// Write an orchestration event (iteration started, hat selected, etc.)
    fn write_orchestration_event(
        &self,
        session_id: &str,
        iteration: u32,
        hat: &str,
        event_type: &str,
    ) -> Result<(), LoopError> {
        let session_dir = self.diagnostics_path.join(session_id);
        let orchestration_path = session_dir.join("orchestration.jsonl");

        let event = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "iteration": iteration,
            "hat": hat,
            "event": { "type": event_type }
        });

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&orchestration_path)
            .map_err(|e| LoopError::SpawnFailed {
                message: format!("Failed to open orchestration.jsonl: {}", e),
            })?;

        writeln!(file, "{}", event).map_err(|e| LoopError::SpawnFailed {
            message: format!("Failed to write to orchestration.jsonl: {}", e),
        })?;

        // Ensure content is flushed to disk for FileWatcher to detect
        file.sync_all().map_err(|e| LoopError::SpawnFailed {
            message: format!("Failed to sync orchestration.jsonl: {}", e),
        })?;

        Ok(())
    }
}

impl Default for MockLoopManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LoopManagerTrait for MockLoopManager {
    async fn start(&self, config: LoopConfig) -> Result<String, LoopError> {
        // Validate config file exists (same as real implementation)
        let config_path = config.working_dir.join(&config.config_path);
        if !config_path.exists() {
            return Err(LoopError::ConfigNotFound {
                path: config.config_path,
            });
        }

        let session_id = Self::generate_session_id();
        let started_at = Utc::now().to_rfc3339();
        let initial_hat = "ralph".to_string();

        // Write initial diagnostics files so FileWatcher can detect the session
        self.write_initial_diagnostics(&session_id, &initial_hat)?;

        // Write initial agent output so something appears on the live page
        self.write_agent_output(
            &session_id,
            1,
            &initial_hat,
            &MockOutput::Text {
                text: format!("🎩 Starting mock loop with prompt: {}", config.prompt),
            },
        )?;

        let state = MockLoopState {
            session_id: session_id.clone(),
            config_path: config.config_path,
            prompt: config.prompt,
            working_dir: config.working_dir,
            current_iteration: 1,
            current_hat: initial_hat,
            status: MockLoopStatus::Running,
            started_at,
            scenario_name: None,
        };

        tracing::info!("Started mock loop {}", session_id);

        let mut loops = self.active_loops.write().await;
        loops.insert(session_id.clone(), state);

        Ok(session_id)
    }

    async fn stop(&self, session_id: &str) -> Result<(), LoopError> {
        let mut loops = self.active_loops.write().await;

        let state = loops
            .remove(session_id)
            .ok_or_else(|| LoopError::NotFound {
                session_id: session_id.to_string(),
            })?;

        tracing::info!("Stopped mock loop {}", state.session_id);

        Ok(())
    }

    async fn list_active(&self) -> Vec<ActiveLoopInfo> {
        let loops = self.active_loops.read().await;
        loops
            .values()
            .filter(|s| s.status == MockLoopStatus::Running)
            .map(|s| s.to_active_loop_info())
            .collect()
    }

    async fn get(&self, session_id: &str) -> Option<ActiveLoopInfo> {
        let loops = self.active_loops.read().await;
        loops.get(session_id).map(|s| s.to_active_loop_info())
    }

    async fn has_active_loops(&self) -> bool {
        let loops = self.active_loops.read().await;
        loops.values().any(|s| s.status == MockLoopStatus::Running)
    }

    async fn active_count(&self) -> usize {
        let loops = self.active_loops.read().await;
        loops
            .values()
            .filter(|s| s.status == MockLoopStatus::Running)
            .count()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config(dir: &std::path::Path) -> PathBuf {
        let config_path = dir.join("ralph.yml");
        std::fs::write(
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
    async fn test_mock_loop_manager_start() {
        let manager = MockLoopManager::new();
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        let config = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "test prompt".to_string(),
            working_dir: temp.path().to_path_buf(),
        };

        let session_id = manager.start(config).await.unwrap();

        // Session ID should be timestamp format (e.g., "2026-01-24T10-33-47")
        assert!(session_id.contains('T'));
        assert!(session_id.contains('-'));

        // Should be active
        assert!(manager.has_active_loops().await);
        assert_eq!(manager.active_count().await, 1);

        // Should be retrievable
        let info = manager.get(&session_id).await;
        assert!(info.is_some());
        assert_eq!(info.unwrap().session_id, session_id);
    }

    #[tokio::test]
    async fn test_mock_loop_manager_stop() {
        let manager = MockLoopManager::new();
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        let config = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "test".to_string(),
            working_dir: temp.path().to_path_buf(),
        };

        let session_id = manager.start(config).await.unwrap();
        assert!(manager.has_active_loops().await);

        manager.stop(&session_id).await.unwrap();
        assert!(!manager.has_active_loops().await);
    }

    #[tokio::test]
    async fn test_mock_loop_manager_config_not_found() {
        let manager = MockLoopManager::new();
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
    async fn test_mock_loop_manager_stop_not_found() {
        let manager = MockLoopManager::new();

        let result = manager.stop("nonexistent").await;
        assert!(matches!(result, Err(LoopError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_mock_loop_manager_advance_iteration() {
        let manager = MockLoopManager::new();
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        let config = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "test".to_string(),
            working_dir: temp.path().to_path_buf(),
        };

        let session_id = manager.start(config).await.unwrap();

        // Start at iteration 1
        {
            let loops = manager.active_loops.read().await;
            assert_eq!(loops.get(&session_id).unwrap().current_iteration, 1);
        }

        // Advance to iteration 2
        manager.advance_iteration(&session_id).await.unwrap();

        {
            let loops = manager.active_loops.read().await;
            assert_eq!(loops.get(&session_id).unwrap().current_iteration, 2);
        }
    }

    #[tokio::test]
    async fn test_mock_loop_manager_complete() {
        let manager = MockLoopManager::new();
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        let config = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "test".to_string(),
            working_dir: temp.path().to_path_buf(),
        };

        let session_id = manager.start(config).await.unwrap();
        assert!(manager.has_active_loops().await);

        // Complete the loop
        manager
            .complete(&session_id, MockLoopStatus::Completed)
            .await
            .unwrap();

        // Should no longer be active
        assert!(!manager.has_active_loops().await);
        assert!(manager.get(&session_id).await.is_none());
    }

    #[tokio::test]
    async fn test_mock_loop_manager_reset() {
        let manager = MockLoopManager::new();
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        // Start a loop
        let config = LoopConfig {
            config_path: "ralph.yml".to_string(),
            prompt: "test".to_string(),
            working_dir: temp.path().to_path_buf(),
        };
        let _ = manager.start(config).await.unwrap();

        // Load a scenario
        let scenario = MockScenario {
            name: "test".to_string(),
            description: "Test scenario".to_string(),
            iterations: vec![],
            final_status: MockLoopStatus::Completed,
        };
        manager.load_scenario(scenario).await;

        assert!(manager.has_active_loops().await);
        assert!(manager.get_scenario("test").await.is_some());

        // Reset
        manager.reset().await;

        assert!(!manager.has_active_loops().await);
        assert!(manager.get_scenario("test").await.is_none());
    }

    #[tokio::test]
    async fn test_session_ids_are_timestamps() {
        // Generate session IDs and verify they're valid timestamp format
        for _ in 0..3 {
            let id = MockLoopManager::generate_session_id();
            // Should be format like "2026-01-24T10-33-47"
            assert!(id.contains('T'), "Should contain T separator");
            assert_eq!(id.matches('-').count(), 4, "Should have 4 dashes");
            assert_eq!(id.len(), 19, "Should be 19 chars (YYYY-MM-DDTHH-MM-SS)");
        }
    }
}
