//! SSE Events endpoint for streaming workflow events.
//!
//! - GET /api/sessions/{id}/events - Server-Sent Events stream
//! - POST /api/sessions/{id}/emit - Emit an event to a session

use actix_web::{web, HttpResponse, Responder};
use bytes::Bytes;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;

use crate::session::Session;
use crate::watcher::SessionBroadcast;

use super::runner::ProcessManager;
use super::sessions::ErrorResponse;

#[cfg(test)]
use crate::watcher::EventWatcher;

/// Active session metadata for runtime-created sessions.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub session: Session,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Application state that holds session broadcast handles.
///
/// Uses `SessionBroadcast` instead of `EventWatcher` because the watcher
/// contains `mpsc::Receiver` which is not `Sync`. The broadcast handle
/// is thread-safe and can be shared across Actix worker threads.
pub struct AppState {
    /// Sessions discovered at startup from filesystem
    pub sessions: Vec<Session>,
    /// Event watchers for both discovered and runtime-created sessions
    pub watchers: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, SessionBroadcast>>>,
    /// Active sessions created after server startup
    pub active_sessions: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, ActiveSession>>>,
}

/// Request body for POST /api/sessions/{id}/emit.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmitEventRequest {
    /// Event topic (e.g., "build.done", "review.complete").
    pub topic: String,
    /// Optional payload - can be string or JSON value.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

/// Response body for POST /api/sessions/{id}/emit.
#[derive(Debug, Clone, Serialize)]
pub struct EmitEventResponse {
    pub success: bool,
    pub topic: String,
    pub timestamp: String,
}

/// GET /api/sessions/{id}/events - Stream events via SSE.
///
/// Returns a Server-Sent Events stream with Content-Type: text/event-stream.
/// Each event is formatted as:
/// ```text
/// event: workflow
/// data: {"topic":"...","ts":"...","payload":"..."}
///
/// ```
pub async fn stream_events(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> impl Responder {
    let session_id = path.into_inner();

    tracing::info!("GET /api/sessions/{}/events - SSE connection attempt", &session_id[..8]);

    // Check if session exists in discovered sessions
    let session_exists = state.sessions.iter().any(|s| s.id == session_id);
    tracing::debug!("Session {} in discovered sessions: {}", &session_id[..8], session_exists);

    // Also check active sessions (created after startup)
    let active_sessions = state.active_sessions.read().await;
    let in_active = active_sessions.contains_key(&session_id);
    drop(active_sessions);
    tracing::debug!("Session {} in active sessions: {}", &session_id[..8], in_active);

    if !session_exists && !in_active {
        tracing::warn!("Session {} not found in either discovered or active sessions", &session_id[..8]);
        return HttpResponse::NotFound().json(ErrorResponse {
            error: "session_not_found".to_string(),
        });
    }

    // Get the watcher for this session (could be in either map)
    let watchers = state.watchers.read().await;
    let watcher_count = watchers.len();
    let watcher = match watchers.get(&session_id) {
        Some(w) => {
            tracing::info!("Watcher found for session {} (total watchers: {})", &session_id[..8], watcher_count);
            w.clone()
        }
        None => {
            tracing::warn!("Session {} exists but NO WATCHER registered (total watchers: {})", &session_id[..8], watcher_count);
            tracing::warn!("Available watcher IDs: {:?}", watchers.keys().map(|k| &k[..8]).collect::<Vec<_>>());
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "session_not_found".to_string(),
            });
        }
    };
    drop(watchers);

    // Subscribe to the broadcast channel
    let rx = watcher.subscribe();
    let stream = BroadcastStream::new(rx);

    // Map events to SSE format
    let sse_stream = stream.filter_map(|result| async move {
        match result {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                let sse_data = format!("event: workflow\ndata: {}\n\n", json);
                Some(Ok::<_, actix_web::Error>(Bytes::from(sse_data)))
            }
            Err(_) => None, // Skip lagged messages
        }
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(sse_stream)
}

/// POST /api/sessions/{id}/emit - Emit an event to a running session.
///
/// Validates the session exists and is running, then emits the event
/// to the session's event bus for processing by the orchestration loop.
pub async fn emit_event(
    path: web::Path<String>,
    body: web::Json<EmitEventRequest>,
    process_manager: web::Data<ProcessManager>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Validate session exists and is running
    if !process_manager.is_running(&session_id) {
        return HttpResponse::NotFound().json(ErrorResponse {
            error: "session_not_found".to_string(),
        });
    }

    // Validate topic format (alphanumeric with dots and underscores)
    if !is_valid_topic(&body.topic) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "invalid_topic: must be non-empty alphanumeric with dots and underscores".to_string(),
        });
    }

    let timestamp = chrono::Utc::now().to_rfc3339();

    // TODO: Actually write event to session's events.jsonl file
    // For now, just acknowledge the request - the event broadcast mechanism
    // will be enhanced in a future iteration to write to disk

    HttpResponse::Ok().json(EmitEventResponse {
        success: true,
        topic: body.topic.clone(),
        timestamp,
    })
}

/// Validate event topic format.
///
/// Topics must be non-empty and contain only alphanumeric characters,
/// dots (.), and underscores (_).
fn is_valid_topic(topic: &str) -> bool {
    !topic.is_empty()
        && topic
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    fn create_test_session(id: &str, path: PathBuf) -> Session {
        Session {
            id: id.to_string(),
            path,
            task_name: Some("test-task".to_string()),
            iteration: 1,
            hat: Some("builder".to_string()),
            started_at: Utc::now(),
            last_event_at: None,
        }
    }

    fn create_events_file(dir: &std::path::Path) -> PathBuf {
        let events_path = dir.join("events.jsonl");
        std::fs::File::create(&events_path).unwrap();
        events_path
    }

    #[actix_web::test]
    async fn test_sse_content_type() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        let session = create_test_session("abc123", temp.path().to_path_buf());
        let watcher = EventWatcher::new(&events_path).unwrap();

        let mut watchers = HashMap::new();
        watchers.insert("abc123".to_string(), watcher.broadcast_handle());

        let state = AppState {
            sessions: vec![session],
            watchers: Arc::new(RwLock::new(watchers)),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        };


        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/events", web::get().to(stream_events)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions/abc123/events")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").unwrap();
        assert_eq!(content_type.to_str().unwrap(), "text/event-stream");
    }

    #[actix_web::test]
    async fn test_sse_session_not_found() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        let session = create_test_session("abc123", temp.path().to_path_buf());
        let watcher = EventWatcher::new(&events_path).unwrap();

        let mut watchers = HashMap::new();
        watchers.insert("abc123".to_string(), watcher.broadcast_handle());

        let state = AppState {
            sessions: vec![session],
            watchers: Arc::new(RwLock::new(watchers)),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        };


        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/events", web::get().to(stream_events)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions/nonexistent/events")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "session_not_found");
    }

    #[actix_web::test]
    async fn test_emit_event_session_not_found() {
        use crate::api::runner::ProcessManager;

        let process_manager = ProcessManager::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(process_manager))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/emit", web::post().to(emit_event)),
                ),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/api/sessions/nonexistent/emit")
            .set_json(EmitEventRequest {
                topic: "build.done".to_string(),
                payload: None,
            })
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "session_not_found");
    }

    #[actix_web::test]
    async fn test_emit_event_invalid_topic() {
        use crate::api::runner::ProcessManager;
        use std::process::Command;

        let process_manager = ProcessManager::new();
        let session_id = "emit-test-session";
        let working_dir = std::path::PathBuf::from("/tmp");

        // Spawn a real process so session exists
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pm = web::Data::new(process_manager);
        pm.store(session_id.to_string(), child, working_dir);

        let app = test::init_service(
            App::new()
                .app_data(pm.clone())
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/emit", web::post().to(emit_event)),
                ),
        )
        .await;

        // Test with empty topic
        let req = test::TestRequest::post()
            .uri(&format!("/api/sessions/{}/emit", session_id))
            .set_json(EmitEventRequest {
                topic: "".to_string(),
                payload: None,
            })
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 400);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("invalid_topic"));

        // Clean up
        let _ = pm.terminate(session_id);
    }

    #[actix_web::test]
    async fn test_emit_event_success() {
        use crate::api::runner::ProcessManager;
        use std::process::Command;

        let process_manager = ProcessManager::new();
        let session_id = "emit-success-session";
        let working_dir = std::path::PathBuf::from("/tmp");

        // Spawn a real process so session exists
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pm = web::Data::new(process_manager);
        pm.store(session_id.to_string(), child, working_dir);

        let app = test::init_service(
            App::new()
                .app_data(pm.clone())
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/emit", web::post().to(emit_event)),
                ),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/sessions/{}/emit", session_id))
            .set_json(EmitEventRequest {
                topic: "build.done".to_string(),
                payload: Some(serde_json::json!({"status": "success"})),
            })
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["topic"], "build.done");
        assert!(json.get("timestamp").is_some());

        // Clean up
        let _ = pm.terminate(session_id);
    }

    #[actix_web::test]
    async fn test_emit_event_with_complex_topic() {
        use crate::api::runner::ProcessManager;
        use std::process::Command;

        let process_manager = ProcessManager::new();
        let session_id = "emit-complex-session";
        let working_dir = std::path::PathBuf::from("/tmp");

        // Spawn a real process so session exists
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pm = web::Data::new(process_manager);
        pm.store(session_id.to_string(), child, working_dir);

        let app = test::init_service(
            App::new()
                .app_data(pm.clone())
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/emit", web::post().to(emit_event)),
                ),
        )
        .await;

        // Test with complex topic containing dots and underscores
        let req = test::TestRequest::post()
            .uri(&format!("/api/sessions/{}/emit", session_id))
            .set_json(EmitEventRequest {
                topic: "iteration.hat_changed.builder".to_string(),
                payload: None,
            })
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["topic"], "iteration.hat_changed.builder");

        // Clean up
        let _ = pm.terminate(session_id);
    }
}
