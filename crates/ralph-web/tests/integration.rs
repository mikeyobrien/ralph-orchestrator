//! Integration tests for ralph-web
//!
//! Tests end-to-end flows including:
//! - Starting and monitoring loops
//! - Stopping running loops
//! - Session persistence
//! - File watcher updates
//! - Concurrent WebSocket connections

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use ralph_web::{AppState, Config, create_app_with_state};
use std::io::Write;
use std::sync::{Arc, RwLock};
use tempfile::TempDir;
use tower::ServiceExt;

/// Create a test server with fresh state
fn create_test_server(temp_dir: &TempDir) -> axum::Router {
    let mut store = ralph_web::store::SessionStore::new(temp_dir.path());
    let _ = store.load_from_disk();

    let state = AppState {
        sessions: Arc::new(RwLock::new(store)),
        ws_hub: ralph_web::websocket::WebSocketHub::new(),
        loop_manager: Arc::new(ralph_web::loop_manager::LoopManager::default()),
    };

    let config = Config {
        port: 3000,
        static_dir: None,
        diagnostics_dir: Some(temp_dir.path().to_path_buf()),
    };

    create_app_with_state(&config, state)
}

/// Create a test session with diagnostics data
fn create_test_session(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let session_dir = dir.join(name);
    std::fs::create_dir_all(&session_dir).unwrap();

    // Write orchestration data
    let orch_path = session_dir.join("orchestration.jsonl");
    let mut file = std::fs::File::create(orch_path).unwrap();
    writeln!(
        file,
        r#"{{"timestamp":"2026-01-21T10:33:47Z","iteration":1,"hat":"ralph","event":{{"type":"iteration_started"}}}}"#
    ).unwrap();
    writeln!(
        file,
        r#"{{"timestamp":"2026-01-21T10:33:48Z","iteration":1,"hat":"ralph","event":{{"type":"hat_selected","hat":"builder","reason":"tasks.ready"}}}}"#
    ).unwrap();
    writeln!(
        file,
        r#"{{"timestamp":"2026-01-21T10:33:50Z","iteration":2,"hat":"builder","event":{{"type":"iteration_started"}}}}"#
    ).unwrap();

    session_dir
}

/// Create a config file for testing
#[cfg(feature = "test-mode")]
fn create_test_config(dir: &std::path::Path) -> std::path::PathBuf {
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

// ==================== Session API Tests ====================

#[tokio::test]
async fn test_session_list_after_creation() {
    let temp = TempDir::new().unwrap();
    create_test_session(temp.path(), "2026-01-21T10-33-47");
    create_test_session(temp.path(), "2026-01-21T14-00-00");

    let app = create_test_server(&temp);

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
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(sessions.len(), 2);
    // Should be sorted newest first
    assert_eq!(sessions[0]["id"], "2026-01-21T14-00-00");
    assert_eq!(sessions[1]["id"], "2026-01-21T10-33-47");
}

#[tokio::test]
async fn test_session_detail_includes_iterations() {
    let temp = TempDir::new().unwrap();
    create_test_session(temp.path(), "2026-01-21T10-33-47");

    let app = create_test_server(&temp);

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
    let session: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(session["id"], "2026-01-21T10-33-47");
    assert_eq!(session["iterations"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_session_not_found() {
    let temp = TempDir::new().unwrap();
    let app = create_test_server(&temp);

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
}

// ==================== Health Check Tests ====================

#[tokio::test]
async fn test_health_check() {
    let temp = TempDir::new().unwrap();
    let app = create_test_server(&temp);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(health["status"], "ok");
    assert!(health["version"].is_string());
}

// ==================== CORS Tests ====================

#[tokio::test]
async fn test_cors_preflight() {
    let temp = TempDir::new().unwrap();
    let app = create_test_server(&temp);

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/sessions")
                .header("Origin", "http://localhost:5173")
                .header("Access-Control-Request-Method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .contains_key("access-control-allow-origin")
    );
}

// ==================== Iteration Content Tests ====================

#[tokio::test]
async fn test_iteration_content_not_found() {
    let temp = TempDir::new().unwrap();
    create_test_session(temp.path(), "2026-01-21T10-33-47");

    let app = create_test_server(&temp);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions/2026-01-21T10-33-47/iterations/99/content")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_iterations_list() {
    let temp = TempDir::new().unwrap();
    create_test_session(temp.path(), "2026-01-21T10-33-47");

    let app = create_test_server(&temp);

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
    let iterations: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(iterations.len(), 2);
    assert_eq!(iterations[0]["number"], 1);
    assert_eq!(iterations[1]["number"], 2);
}

// ==================== MockLoopManager E2E Tests ====================

/// These tests require the `test-mode` feature to be enabled.
/// Run with: cargo test -p ralph-web --features test-mode test_mock_loop
#[cfg(feature = "test-mode")]
mod mock_loop_tests {
    use super::*;
    use ralph_web::mock_loop_manager::MockLoopManager;

    /// Create a test server with MockLoopManager for E2E testing
    fn create_mock_loop_test_server(temp_dir: &TempDir) -> axum::Router {
        let mut store = ralph_web::store::SessionStore::new(temp_dir.path());
        let _ = store.load_from_disk();

        let mock_manager = MockLoopManager::with_diagnostics_path(temp_dir.path().to_path_buf());

        let state = AppState {
            sessions: Arc::new(RwLock::new(store)),
            ws_hub: ralph_web::websocket::WebSocketHub::new(),
            loop_manager: Arc::new(mock_manager),
        };

        let config = Config {
            port: 3000,
            static_dir: None,
            diagnostics_dir: Some(temp_dir.path().to_path_buf()),
        };

        create_app_with_state(&config, state)
    }

    #[tokio::test]
    async fn test_mock_loop_full_lifecycle() {
        let temp = TempDir::new().unwrap();

        // Create a config file (required by MockLoopManager)
        create_test_config(temp.path());

        let app = create_mock_loop_test_server(&temp);

        // Step 1: Verify no active loops initially
        let response = app
            .clone()
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
        let active_loops: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(
            active_loops.is_empty(),
            "Should have no active loops initially"
        );

        // Step 2: Start a loop via POST /api/loops/start
        let start_body = serde_json::json!({
            "config_path": "ralph.yml",
            "prompt": "Test prompt for E2E",
            "working_dir": temp.path().to_string_lossy()
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/start")
                    .header("Content-Type", "application/json")
                    .body(Body::from(start_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Start loop should succeed"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let start_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let session_id = start_response["session_id"]
            .as_str()
            .expect("Should have session_id");

        // Verify session ID is UUID v4 format
        assert!(
            uuid::Uuid::parse_str(session_id).is_ok(),
            "Session ID should be valid UUID"
        );

        // Step 3: Verify loop is active via GET /api/loops/active
        let response = app
            .clone()
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
        let active_loops: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

        assert_eq!(active_loops.len(), 1, "Should have exactly one active loop");
        assert_eq!(
            active_loops[0]["session_id"].as_str().unwrap(),
            session_id,
            "Active loop should have same session ID"
        );
        assert_eq!(
            active_loops[0]["prompt"].as_str().unwrap(),
            "Test prompt for E2E",
            "Active loop should have correct prompt"
        );

        // Step 4: Stop the loop via POST /api/loops/{id}/stop
        let stop_uri = format!("/api/loops/{}/stop", session_id);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&stop_uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NO_CONTENT,
            "Stop loop should return 204 No Content"
        );

        // Step 5: Verify loop is stopped (no longer active)
        let response = app
            .clone()
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
        let active_loops: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

        assert!(
            active_loops.is_empty(),
            "Should have no active loops after stop"
        );

        // Verify loop is no longer retrievable by ID
        let get_uri = format!("/api/loops/{}", session_id);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(&get_uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Stopped loop should not be found"
        );
    }

    #[tokio::test]
    async fn test_mock_loop_start_without_config_fails() {
        let temp = TempDir::new().unwrap();
        // Note: NOT creating config file

        let app = create_mock_loop_test_server(&temp);

        let start_body = serde_json::json!({
            "config_path": "nonexistent.yml",
            "prompt": "Test prompt",
            "working_dir": temp.path().to_string_lossy()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/start")
                    .header("Content-Type", "application/json")
                    .body(Body::from(start_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Should fail without config file"
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            error["error"].as_str().unwrap().contains("not found"),
            "Error should mention config not found"
        );
    }

    #[tokio::test]
    async fn test_mock_loop_stop_nonexistent_fails() {
        let temp = TempDir::new().unwrap();
        let app = create_mock_loop_test_server(&temp);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/nonexistent-uuid/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Should fail to stop nonexistent loop"
        );
    }

    #[tokio::test]
    async fn test_mock_loop_multiple_concurrent_loops() {
        let temp = TempDir::new().unwrap();
        create_test_config(temp.path());

        let app = create_mock_loop_test_server(&temp);

        // Start first loop
        let start_body1 = serde_json::json!({
            "config_path": "ralph.yml",
            "prompt": "First loop",
            "working_dir": temp.path().to_string_lossy()
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/start")
                    .header("Content-Type", "application/json")
                    .body(Body::from(start_body1.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp1: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let session_id1 = resp1["session_id"].as_str().unwrap().to_string();

        // Start second loop
        let start_body2 = serde_json::json!({
            "config_path": "ralph.yml",
            "prompt": "Second loop",
            "working_dir": temp.path().to_string_lossy()
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loops/start")
                    .header("Content-Type", "application/json")
                    .body(Body::from(start_body2.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp2: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let session_id2 = resp2["session_id"].as_str().unwrap().to_string();

        // Verify both loops are active
        let response = app
            .clone()
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
        let active_loops: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

        assert_eq!(active_loops.len(), 2, "Should have two active loops");

        // Stop first loop
        let stop_uri = format!("/api/loops/{}/stop", session_id1);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&stop_uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // Verify only second loop is still active
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/loops/active")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let active_loops: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            active_loops.len(),
            1,
            "Should have one active loop remaining"
        );
        assert_eq!(
            active_loops[0]["session_id"].as_str().unwrap(),
            session_id2,
            "Remaining loop should be the second one"
        );

        // Stop second loop
        let stop_uri = format!("/api/loops/{}/stop", session_id2);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&stop_uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
}
