//! API routes for ralph-web

use crate::AppState;
use crate::loop_manager::{ActiveLoopInfo, LoopConfig, LoopError};
use crate::models::{IterationContent, SessionSummary};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Health check endpoint
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// List all sessions
async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<SessionSummary>>, StatusCode> {
    let store = state
        .sessions
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(store.list()))
}

/// Get session detail
async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<crate::models::Session>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.sessions.read().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Internal error".to_string(),
            }),
        )
    })?;

    store.get(&session_id).cloned().map(Json).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Session not found".to_string(),
            }),
        )
    })
}

/// List iterations for a session
async fn list_iterations(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Vec<crate::models::Iteration>>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.sessions.read().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Internal error".to_string(),
            }),
        )
    })?;

    store
        .get(&session_id)
        .map(|s| Json(s.iterations.clone()))
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Session not found".to_string(),
                }),
            )
        })
}

/// Get iteration content
async fn get_iteration_content(
    State(state): State<AppState>,
    Path((session_id, iteration_num)): Path<(String, u32)>,
) -> Result<Json<IterationContent>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.sessions.read().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Internal error".to_string(),
            }),
        )
    })?;

    store
        .get_iteration_content(&session_id, iteration_num)
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Iteration not found".to_string(),
                }),
            )
        })
}

// ==================== Loop Management Endpoints ====================

/// Request body for starting a loop
#[derive(Debug, Deserialize)]
pub struct StartLoopRequest {
    /// Path to the Ralph config file (relative to working_dir)
    pub config_path: String,
    /// Prompt to pass to the loop
    pub prompt: String,
    /// Working directory for the loop
    pub working_dir: String,
}

/// Response for starting a loop
#[derive(Debug, Serialize)]
pub struct StartLoopResponse {
    /// Session ID of the started loop
    pub session_id: String,
}

/// Start a new orchestration loop
async fn start_loop(
    State(state): State<AppState>,
    Json(req): Json<StartLoopRequest>,
) -> Result<Json<StartLoopResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config = LoopConfig {
        config_path: req.config_path,
        prompt: req.prompt,
        working_dir: std::path::PathBuf::from(req.working_dir),
    };

    match state.loop_manager.start(config).await {
        Ok(session_id) => Ok(Json(StartLoopResponse { session_id })),
        Err(LoopError::ConfigNotFound { path }) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Config file not found: {}", path),
            }),
        )),
        Err(LoopError::SpawnFailed { message }) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to start loop: {}", message),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Stop a specific loop by session ID
async fn stop_loop(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.loop_manager.stop(&session_id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(LoopError::NotFound { session_id }) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Loop not found: {}", session_id),
            }),
        )),
        Err(LoopError::StopFailed { message }) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to stop loop: {}", message),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Stop the current/only loop (simplified endpoint for frontend)
async fn stop_current_loop(
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let active = state.loop_manager.list_active().await;

    if active.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "No active loop to stop".to_string(),
            }),
        ));
    }

    // Stop the first active loop (for backward compatibility with single-loop UI)
    let session_id = &active[0].session_id;
    match state.loop_manager.stop(session_id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(LoopError::StopFailed { message }) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to stop loop: {}", message),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// List all active loops
async fn list_active_loops(State(state): State<AppState>) -> Json<Vec<ActiveLoopInfo>> {
    Json(state.loop_manager.list_active().await)
}

/// Get a specific loop by session ID
async fn get_loop(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<ActiveLoopInfo>, (StatusCode, Json<ErrorResponse>)> {
    state
        .loop_manager
        .get(&session_id)
        .await
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Loop not found: {}", session_id),
                }),
            )
        })
}

// ==================== Config Discovery Endpoints ====================

/// A single config file option
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigOption {
    /// Display name (e.g., "confession-loop" or "minimal/claude")
    pub name: String,
    /// Full path to the config file (relative to working_dir for local, absolute for presets)
    pub path: String,
    /// Optional description from the YAML file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Config options grouped by source
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigGroup {
    /// Source name (e.g., "Local", "Presets", "Presets/minimal")
    pub source: String,
    /// Configs in this group
    pub configs: Vec<ConfigOption>,
}

/// Response for the configs endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigsResponse {
    /// All config groups
    pub groups: Vec<ConfigGroup>,
}

/// Query parameters for listing configs
#[derive(Debug, Deserialize)]
pub struct ConfigsQuery {
    /// Working directory to search for local configs (defaults to current dir)
    pub working_dir: Option<String>,
}

/// List available config files (local yml files and presets)
async fn list_configs(
    Query(query): Query<ConfigsQuery>,
) -> Result<Json<ConfigsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let working_dir = query
        .working_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let mut groups: Vec<ConfigGroup> = Vec::new();

    // 1. Find local yml files in working_dir
    let local_configs = discover_local_configs(&working_dir);
    if !local_configs.is_empty() {
        groups.push(ConfigGroup {
            source: "Local".to_string(),
            configs: local_configs,
        });
    }

    // 2. Find presets from the presets directory
    let preset_groups = discover_presets();
    groups.extend(preset_groups);

    Ok(Json(ConfigsResponse { groups }))
}

/// Discover local .yml/.yaml config files in the working directory (non-recursive)
fn discover_local_configs(dir: &std::path::Path) -> Vec<ConfigOption> {
    let mut configs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if (ext_str == "yml" || ext_str == "yaml") && path.is_file() {
                    // Check if it looks like a Ralph config (has cli: or event_loop: key)
                    if is_likely_ralph_config(&path) {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let description = extract_yaml_description(&path);
                        configs.push(ConfigOption {
                            name,
                            path: path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            description,
                        });
                    }
                }
            }
        }
    }

    configs.sort_by(|a, b| a.name.cmp(&b.name));
    configs
}

/// Check if a YAML file looks like a Ralph config
fn is_likely_ralph_config(path: &std::path::Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(path) {
        // Check for common Ralph config keys
        content.contains("cli:") || content.contains("event_loop:") || content.contains("hats:")
    } else {
        false
    }
}

/// Extract description from the first comment lines of a YAML file
fn extract_yaml_description(path: &std::path::Path) -> Option<String> {
    if let Ok(content) = std::fs::read_to_string(path) {
        let mut description_lines = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let comment = trimmed.trim_start_matches('#').trim();
                // Skip empty comments and usage instructions
                if !comment.is_empty()
                    && !comment.to_lowercase().starts_with("usage:")
                    && !comment.starts_with("ralph ")
                {
                    description_lines.push(comment.to_string());
                    // Just take the first meaningful comment line as description
                    break;
                }
            } else if !trimmed.is_empty() {
                // Hit actual YAML content, stop looking
                break;
            }
        }
        if !description_lines.is_empty() {
            return Some(description_lines.join(" "));
        }
    }
    None
}

/// Discover presets from the built-in presets directory
fn discover_presets() -> Vec<ConfigGroup> {
    // Try to find presets relative to the executable or in common locations
    let preset_paths = [
        // Development: relative to working dir
        PathBuf::from("presets"),
        // Installed: relative to executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("presets")))
            .unwrap_or_else(|| PathBuf::from("presets")),
    ];

    for preset_dir in preset_paths {
        if preset_dir.exists() && preset_dir.is_dir() {
            return discover_presets_in_dir(&preset_dir);
        }
    }

    Vec::new()
}

/// Discover presets in a specific directory, grouping by subdirectory
fn discover_presets_in_dir(dir: &std::path::Path) -> Vec<ConfigGroup> {
    let mut grouped: HashMap<String, Vec<ConfigOption>> = HashMap::new();

    // Walk the presets directory
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();

            if path.is_dir() {
                // This is a subdirectory (e.g., "minimal")
                let subdir_name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.filter_map(Result::ok) {
                        let sub_path = sub_entry.path();
                        if let Some(ext) = sub_path.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if (ext_str == "yml" || ext_str == "yaml") && sub_path.is_file() {
                                let name = format!(
                                    "{}/{}",
                                    subdir_name,
                                    sub_path
                                        .file_stem()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_default()
                                );
                                let description = extract_yaml_description(&sub_path);
                                let group_key = format!("Presets/{}", subdir_name);
                                grouped.entry(group_key).or_default().push(ConfigOption {
                                    name,
                                    path: format!(
                                        "preset:{}",
                                        sub_path
                                            .strip_prefix(dir)
                                            .unwrap_or(&sub_path)
                                            .to_string_lossy()
                                    ),
                                    description,
                                });
                            }
                        }
                    }
                }
            } else if let Some(ext) = path.extension() {
                // Top-level preset file
                let ext_str = ext.to_string_lossy().to_lowercase();
                if (ext_str == "yml" || ext_str == "yaml") && path.is_file() {
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let description = extract_yaml_description(&path);
                    grouped
                        .entry("Presets".to_string())
                        .or_default()
                        .push(ConfigOption {
                            name,
                            path: format!(
                                "preset:{}",
                                path.file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            ),
                            description,
                        });
                }
            }
        }
    }

    // Convert to sorted groups
    let mut groups: Vec<ConfigGroup> = grouped
        .into_iter()
        .map(|(source, mut configs)| {
            configs.sort_by(|a, b| a.name.cmp(&b.name));
            ConfigGroup { source, configs }
        })
        .collect();

    // Sort groups: "Presets" first, then subdirs alphabetically
    groups.sort_by(|a, b| {
        if a.source == "Presets" {
            std::cmp::Ordering::Less
        } else if b.source == "Presets" {
            std::cmp::Ordering::Greater
        } else {
            a.source.cmp(&b.source)
        }
    });

    groups
}

/// Create API routes
pub fn api_routes(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/api/health", get(health))
        // Session management
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/{id}", get(get_session))
        .route("/api/sessions/{id}/iterations", get(list_iterations))
        .route(
            "/api/sessions/{id}/iterations/{num}/content",
            get(get_iteration_content),
        )
        // Loop management
        .route("/api/loops/start", post(start_loop))
        .route("/api/loops/stop", post(stop_current_loop))
        .route("/api/loops/active", get(list_active_loops))
        .route("/api/loops/{id}", get(get_loop))
        .route("/api/loops/{id}/stop", post(stop_loop))
        // Config discovery
        .route("/api/configs", get(list_configs))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SessionStore;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use std::io::Write;
    use std::sync::{Arc, RwLock};
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn create_test_app(temp_dir: &TempDir) -> Router {
        let mut store = SessionStore::new(temp_dir.path());
        let _ = store.load_from_disk();

        let state = AppState {
            sessions: Arc::new(RwLock::new(store)),
            ws_hub: crate::websocket::WebSocketHub::new(),
            loop_manager: Arc::new(crate::loop_manager::LoopManager::default()),
        };

        api_routes(state)
    }

    fn create_session_with_data(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let session_dir = dir.join(name);
        std::fs::create_dir_all(&session_dir).unwrap();

        // Write orchestration data
        let orch_path = session_dir.join("orchestration.jsonl");
        let mut file = std::fs::File::create(orch_path).unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2026-01-21T10:33:47Z","iteration":1,"hat":"ralph","event":{{"type":"iteration_started"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2026-01-21T10:33:48Z","iteration":1,"hat":"ralph","event":{{"type":"hat_selected","hat":"builder","reason":"tasks.ready"}}}}"#
        )
        .unwrap();

        session_dir
    }

    #[tokio::test]
    async fn test_sessions_api_returns_json() {
        let temp = TempDir::new().unwrap();
        create_session_with_data(temp.path(), "2026-01-21T10-33-47");

        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sessions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let sessions: Vec<SessionSummary> = serde_json::from_slice(&body).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "2026-01-21T10-33-47");
    }

    #[tokio::test]
    async fn test_session_detail_returns_json() {
        let temp = TempDir::new().unwrap();
        create_session_with_data(temp.path(), "2026-01-21T10-33-47");

        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sessions/2026-01-21T10-33-47")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let session: crate::models::Session = serde_json::from_slice(&body).unwrap();

        assert_eq!(session.id, "2026-01-21T10-33-47");
        assert_eq!(session.iterations.len(), 1);
    }

    #[tokio::test]
    async fn test_session_not_found_returns_404() {
        let temp = TempDir::new().unwrap();
        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sessions/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: ErrorResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.error, "Session not found");
    }

    #[tokio::test]
    async fn test_iterations_api_returns_json() {
        let temp = TempDir::new().unwrap();
        create_session_with_data(temp.path(), "2026-01-21T10-33-47");

        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sessions/2026-01-21T10-33-47/iterations")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let iterations: Vec<crate::models::Iteration> = serde_json::from_slice(&body).unwrap();

        assert_eq!(iterations.len(), 1);
        assert_eq!(iterations[0].number, 1);
    }

    // ==================== Loop Management API Tests ====================

    #[tokio::test]
    async fn test_list_active_loops_empty() {
        let temp = TempDir::new().unwrap();
        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/loops/active")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let loops: Vec<ActiveLoopInfo> = serde_json::from_slice(&body).unwrap();

        assert!(loops.is_empty());
    }

    #[tokio::test]
    async fn test_get_loop_not_found() {
        let temp = TempDir::new().unwrap();
        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/loops/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: ErrorResponse = serde_json::from_slice(&body).unwrap();

        assert!(error.error.contains("not found"));
    }

    #[tokio::test]
    async fn test_stop_loop_not_found() {
        let temp = TempDir::new().unwrap();
        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/nonexistent/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_stop_current_loop_no_active() {
        let temp = TempDir::new().unwrap();
        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: ErrorResponse = serde_json::from_slice(&body).unwrap();

        assert!(error.error.contains("No active loop"));
    }

    #[tokio::test]
    async fn test_start_loop_config_not_found() {
        let temp = TempDir::new().unwrap();
        let app = create_test_app(&temp);

        let body = serde_json::json!({
            "config_path": "nonexistent.yml",
            "prompt": "test",
            "working_dir": temp.path().to_string_lossy()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/start")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: ErrorResponse = serde_json::from_slice(&body).unwrap();

        assert!(error.error.contains("not found"));
    }

    // ==================== Config Discovery API Tests ====================

    #[tokio::test]
    async fn test_list_configs_returns_json() {
        let temp = TempDir::new().unwrap();
        let app = create_test_app(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/configs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let configs: ConfigsResponse = serde_json::from_slice(&body).unwrap();

        // Should have groups (at least presets if the presets dir exists)
        assert!(configs.groups.iter().all(|g| !g.source.is_empty()));
    }

    #[tokio::test]
    async fn test_list_configs_with_working_dir() {
        let temp = TempDir::new().unwrap();

        // Create a local config file
        let config_content = r#"# Test Config
cli:
  backend: "claude"
event_loop:
  prompt_file: "PROMPT.md"
"#;
        std::fs::write(temp.path().join("test-config.yml"), config_content).unwrap();

        let app = create_test_app(&temp);

        let uri = format!(
            "/api/configs?working_dir={}",
            urlencoding::encode(temp.path().to_str().unwrap())
        );

        let response = app
            .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let configs: ConfigsResponse = serde_json::from_slice(&body).unwrap();

        // Should have Local group with our test config
        let local_group = configs.groups.iter().find(|g| g.source == "Local");
        assert!(local_group.is_some(), "Should have Local group");
        let local = local_group.unwrap();
        assert!(
            local.configs.iter().any(|c| c.name == "test-config"),
            "Should find test-config"
        );
    }

    #[tokio::test]
    async fn test_list_configs_extracts_description() {
        let temp = TempDir::new().unwrap();

        // Create a local config file with a description comment
        let config_content = r#"# My awesome workflow config
# Usage: ralph run -c my-config.yml
cli:
  backend: "claude"
"#;
        std::fs::write(temp.path().join("my-config.yml"), config_content).unwrap();

        let app = create_test_app(&temp);

        let uri = format!(
            "/api/configs?working_dir={}",
            urlencoding::encode(temp.path().to_str().unwrap())
        );

        let response = app
            .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let configs: ConfigsResponse = serde_json::from_slice(&body).unwrap();

        let local_group = configs.groups.iter().find(|g| g.source == "Local").unwrap();
        let my_config = local_group
            .configs
            .iter()
            .find(|c| c.name == "my-config")
            .unwrap();

        assert_eq!(
            my_config.description,
            Some("My awesome workflow config".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_configs_ignores_non_ralph_yaml() {
        let temp = TempDir::new().unwrap();

        // Create a non-Ralph YAML file
        let non_ralph_content = r"name: some-package
version: 1.0.0
";
        std::fs::write(temp.path().join("package.yml"), non_ralph_content).unwrap();

        // Create a Ralph config file
        let ralph_content = r#"cli:
  backend: "claude"
"#;
        std::fs::write(temp.path().join("ralph.yml"), ralph_content).unwrap();

        let app = create_test_app(&temp);

        let uri = format!(
            "/api/configs?working_dir={}",
            urlencoding::encode(temp.path().to_str().unwrap())
        );

        let response = app
            .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let configs: ConfigsResponse = serde_json::from_slice(&body).unwrap();

        let local_group = configs.groups.iter().find(|g| g.source == "Local");

        if let Some(local) = local_group {
            // Should have ralph.yml but not package.yml
            assert!(
                local.configs.iter().any(|c| c.name == "ralph"),
                "Should find ralph config"
            );
            assert!(
                !local.configs.iter().any(|c| c.name == "package"),
                "Should not include non-Ralph YAML"
            );
        }
    }
}
