//! Ralph Web Dashboard
//!
//! Web-based dashboard for Ralph orchestration loops, providing live monitoring,
//! session review, and loop management capabilities.

pub mod loop_manager;
pub mod models;
pub mod routes;
pub mod store;
pub mod watcher;
pub mod websocket;

#[cfg(feature = "test-mode")]
pub mod mock_loop_manager;

#[cfg(feature = "test-mode")]
pub mod test_endpoints;

use axum::{Router, routing::get};
use loop_manager::LoopManagerTrait;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tower_http::cors::{Any, CorsLayer};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Session store
    pub sessions: Arc<RwLock<store::SessionStore>>,
    /// WebSocket hub for real-time updates
    pub ws_hub: websocket::WebSocketHub,
    /// Loop manager for starting/stopping orchestration loops
    pub loop_manager: Arc<dyn LoopManagerTrait>,
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Port to listen on
    pub port: u16,
    /// Path to static files (frontend build output)
    pub static_dir: Option<PathBuf>,
    /// Path to diagnostics directory (defaults to .ralph/diagnostics)
    pub diagnostics_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 3000,
            static_dir: None,
            diagnostics_dir: None,
        }
    }
}

/// Check if test mode is enabled via truthy environment variable.
///
/// SECURITY: Only accepts explicit "1" or "true" values to avoid
/// accidental activation.
fn is_test_mode_enabled() -> bool {
    std::env::var("RALPH_WEB_TEST_MODE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Create the loop manager based on configuration.
///
/// When compiled with `test-mode` feature AND `RALPH_WEB_TEST_MODE=1`,
/// uses `MockLoopManager`. Otherwise uses the real `LoopManager`.
fn create_loop_manager() -> Arc<dyn LoopManagerTrait> {
    #[cfg(feature = "test-mode")]
    if is_test_mode_enabled() {
        tracing::info!("Test mode enabled: using MockLoopManager");
        return Arc::new(mock_loop_manager::MockLoopManager::new());
    }

    #[cfg(not(feature = "test-mode"))]
    let _ = is_test_mode_enabled(); // Silence unused function warning

    Arc::new(loop_manager::LoopManager::default())
}

/// Create the application router (without file watcher, for testing)
pub fn create_app(config: &Config) -> Router {
    let diagnostics_path = config
        .diagnostics_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ralph/diagnostics"));

    let mut session_store = store::SessionStore::new(diagnostics_path);
    let _ = session_store.load_from_disk();

    let ws_hub = websocket::WebSocketHub::new();
    let loop_manager = create_loop_manager();

    let state = AppState {
        sessions: Arc::new(RwLock::new(session_store)),
        ws_hub: ws_hub.clone(),
        loop_manager,
    };

    create_app_with_state(config, state)
}

/// Create the application router with provided state (for dependency injection)
pub fn create_app_with_state(config: &Config, state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let ws_hub = state.ws_hub.clone();

    let mut app = Router::new().merge(routes::api_routes(state.clone()));

    // Add test endpoints when in test mode
    #[cfg(feature = "test-mode")]
    if is_test_mode_enabled() {
        app = app.merge(test_endpoints::test_api_routes(state.clone()));
    }

    app = app
        .route("/ws", get(websocket::ws_handler).with_state(ws_hub))
        .layer(cors);

    // Add static file serving if configured
    if let Some(ref static_dir) = config.static_dir
        && static_dir.exists()
    {
        app = app.fallback_service(
            tower_http::services::ServeDir::new(static_dir).not_found_service(
                tower_http::services::ServeFile::new(static_dir.join("index.html")),
            ),
        );
    }

    app
}

/// Transform diagnostic events to WebSocket messages
fn diagnostic_to_server_message(event: watcher::DiagnosticEvent) -> websocket::ServerMessage {
    match event {
        watcher::DiagnosticEvent::AgentOutput(output) => {
            // Extract text content if it's a text event
            if let watcher::AgentOutputContent::Text { text } = output.content {
                websocket::ServerMessage::Output { lines: vec![text] }
            } else {
                // For other output types, serialize as JSON
                let payload = serde_json::to_string(&output).unwrap_or_default();
                websocket::ServerMessage::Event {
                    topic: "agent_output".to_string(),
                    payload,
                }
            }
        }
        watcher::DiagnosticEvent::Orchestration(orch) => {
            match &orch.event {
                watcher::OrchestrationEventType::IterationStarted => {
                    websocket::ServerMessage::IterationStarted {
                        iteration: orch.iteration,
                        hat: orch.hat.clone(),
                    }
                }
                watcher::OrchestrationEventType::LoopTerminated { reason } => {
                    websocket::ServerMessage::LoopCompleted {
                        reason: reason.clone(),
                    }
                }
                watcher::OrchestrationEventType::EventPublished { topic } => {
                    websocket::ServerMessage::Event {
                        topic: topic.clone(),
                        payload: String::new(),
                    }
                }
                _ => {
                    // For other orchestration events, serialize as generic event
                    let payload = serde_json::to_string(&orch).unwrap_or_default();
                    websocket::ServerMessage::Event {
                        topic: "orchestration".to_string(),
                        payload,
                    }
                }
            }
        }
    }
}

/// Start the server with file watcher for live updates
pub async fn serve(config: Config) -> Result<(), std::io::Error> {
    let diagnostics_path = config
        .diagnostics_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ralph/diagnostics"));

    // Ensure diagnostics directory exists so file watcher can be initialized.
    // This is required because loops started via the API will create session
    // subdirectories here, and the watcher needs to be watching before that happens.
    if !diagnostics_path.exists() {
        std::fs::create_dir_all(&diagnostics_path)?;
        tracing::info!("Created diagnostics directory: {:?}", diagnostics_path);
    }

    let mut session_store = store::SessionStore::new(diagnostics_path.clone());
    let _ = session_store.load_from_disk();

    let ws_hub = websocket::WebSocketHub::new();
    let loop_manager = create_loop_manager();

    let state = AppState {
        sessions: Arc::new(RwLock::new(session_store)),
        ws_hub: ws_hub.clone(),
        loop_manager,
    };

    let app = create_app_with_state(&config, state);
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    tracing::info!("Starting ralph-web server on {}", addr);

    // Start file watcher for diagnostics directory.
    // Uses RecursiveMode::Recursive to watch for changes in session subdirectories.
    let _watcher = match watcher::FileWatcher::new(diagnostics_path.clone()) {
        Ok(file_watcher) => {
            let mut rx = file_watcher.subscribe();
            let hub = ws_hub.clone();

            // Spawn task to forward file watcher events to WebSocket
            tokio::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    let msg = diagnostic_to_server_message(event);
                    hub.broadcast(msg).await;
                }
            });

            tracing::info!("File watcher started for {:?}", diagnostics_path);
            Some(file_watcher)
        }
        Err(e) => {
            tracing::warn!("Failed to start file watcher: {}", e);
            None
        }
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    // ==================== diagnostic_to_server_message Tests ====================

    #[test]
    fn test_diagnostic_to_server_message_text() {
        let event = watcher::DiagnosticEvent::AgentOutput(watcher::AgentOutputEvent {
            timestamp: "2026-01-23T10:00:00Z".to_string(),
            iteration: 1,
            hat: "ralph".to_string(),
            content: watcher::AgentOutputContent::Text {
                text: "Hello world".to_string(),
            },
        });

        let msg = diagnostic_to_server_message(event);
        assert!(matches!(
            msg,
            websocket::ServerMessage::Output { lines } if lines == vec!["Hello world"]
        ));
    }

    #[test]
    fn test_diagnostic_to_server_message_iteration_started() {
        let event = watcher::DiagnosticEvent::Orchestration(watcher::OrchestrationEvent {
            timestamp: "2026-01-23T10:00:00Z".to_string(),
            iteration: 5,
            hat: "builder".to_string(),
            event: watcher::OrchestrationEventType::IterationStarted,
        });

        let msg = diagnostic_to_server_message(event);
        assert!(matches!(
            msg,
            websocket::ServerMessage::IterationStarted { iteration: 5, hat } if hat == "builder"
        ));
    }

    #[test]
    fn test_diagnostic_to_server_message_loop_terminated() {
        let event = watcher::DiagnosticEvent::Orchestration(watcher::OrchestrationEvent {
            timestamp: "2026-01-23T10:00:00Z".to_string(),
            iteration: 10,
            hat: "ralph".to_string(),
            event: watcher::OrchestrationEventType::LoopTerminated {
                reason: "LOOP_COMPLETE".to_string(),
            },
        });

        let msg = diagnostic_to_server_message(event);
        assert!(matches!(
            msg,
            websocket::ServerMessage::LoopCompleted { reason } if reason == "LOOP_COMPLETE"
        ));
    }

    #[test]
    fn test_diagnostic_to_server_message_event_published() {
        let event = watcher::DiagnosticEvent::Orchestration(watcher::OrchestrationEvent {
            timestamp: "2026-01-23T10:00:00Z".to_string(),
            iteration: 3,
            hat: "ralph".to_string(),
            event: watcher::OrchestrationEventType::EventPublished {
                topic: "build.start".to_string(),
            },
        });

        let msg = diagnostic_to_server_message(event);
        assert!(matches!(
            msg,
            websocket::ServerMessage::Event { topic, .. } if topic == "build.start"
        ));
    }

    #[test]
    fn test_diagnostic_to_server_message_tool_call() {
        let event = watcher::DiagnosticEvent::AgentOutput(watcher::AgentOutputEvent {
            timestamp: "2026-01-23T10:00:00Z".to_string(),
            iteration: 1,
            hat: "builder".to_string(),
            content: watcher::AgentOutputContent::ToolCall {
                name: "Read".to_string(),
                id: "t1".to_string(),
                input: serde_json::json!({"file": "test.rs"}),
            },
        });

        let msg = diagnostic_to_server_message(event);
        assert!(matches!(
            msg,
            websocket::ServerMessage::Event { topic, .. } if topic == "agent_output"
        ));
    }

    // ==================== API Tests ====================

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = Config::default();
        let app = create_app(&config);

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
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"], "ok");
        assert!(json["version"].is_string());
    }

    #[tokio::test]
    async fn test_cors_headers() {
        let config = Config::default();
        let app = create_app(&config);

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/health")
                    .header("Origin", "http://localhost:5173")
                    .header("Access-Control-Request-Method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // CORS preflight should return 200
        assert_eq!(response.status(), StatusCode::OK);

        // Should have CORS headers
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-origin")
        );
    }
}
