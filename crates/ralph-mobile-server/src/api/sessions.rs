//! Sessions API endpoints.
//!
//! GET /api/sessions - List all discovered Ralph sessions.
//! GET /api/sessions/{id}/status - Get detailed status for a specific session.

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use serde::Serialize;

use super::events::AppState;
use super::runner::{ProcessManager, ScratchpadResponse};
use crate::session::Session;

/// Response format for session list items.
#[derive(Debug, Serialize)]
pub struct SessionListItem {
    pub id: String,
    pub iteration: u32,
    pub hat: Option<String>,
    pub started_at: String, // ISO 8601 format
}

/// Response format for session status (detailed view).
#[derive(Debug, Serialize)]
pub struct SessionStatus {
    pub id: String,
    pub iteration: u32,
    pub total: Option<u32>,
    pub hat: Option<String>,
    pub elapsed_secs: u64,
    pub mode: String, // "live" | "complete"
}

/// Error response format.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl From<&Session> for SessionListItem {
    fn from(session: &Session) -> Self {
        SessionListItem {
            id: session.id.clone(),
            iteration: session.iteration,
            hat: session.hat.clone(),
            started_at: session.started_at.to_rfc3339(),
        }
    }
}

/// GET /api/sessions - List all discovered and active sessions.
pub async fn list_sessions(state: web::Data<AppState>) -> impl Responder {
    // Get discovered sessions
    let mut items: Vec<SessionListItem> = state.sessions.iter().map(SessionListItem::from).collect();

    // Add active sessions (created after startup)
    let active_sessions = state.active_sessions.read().await;
    for active_session in active_sessions.values() {
        items.push(SessionListItem::from(&active_session.session));
    }

    HttpResponse::Ok().json(items)
}

/// GET /api/sessions/{id}/status - Get detailed status for a specific session.
pub async fn get_session_status(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> impl Responder {
    let session_id = path.into_inner();

    // Find the session by ID in discovered sessions
    let discovered_session = state.sessions.iter().find(|s| s.id == session_id);

    // If not found in discovered, check active sessions
    let session_data = if let Some(session) = discovered_session {
        Some((
            session.id.clone(),
            session.iteration,
            session.hat.clone(),
            session.started_at,
        ))
    } else {
        let active_sessions = state.active_sessions.read().await;
        active_sessions.get(&session_id).map(|a| {
            (
                a.session.id.clone(),
                a.session.iteration,
                a.session.hat.clone(),
                a.session.started_at,
            )
        })
    };

    match session_data {
        Some((id, iteration, hat, started_at)) => {
            // Calculate elapsed time from session start
            let elapsed_secs = (Utc::now() - started_at).num_seconds().max(0) as u64;

            let status = SessionStatus {
                id,
                iteration,
                total: None, // Will be derived from events in future
                hat,
                elapsed_secs,
                mode: "live".to_string(), // Default to live for now, will be derived from watcher state
            };

            HttpResponse::Ok().json(status)
        }
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: "session_not_found".to_string(),
        }),
    }
}

/// GET /api/sessions/{id}/scratchpad - Get scratchpad content for a session.
pub async fn get_scratchpad(
    path: web::Path<String>,
    state: web::Data<AppState>,
    process_manager: web::Data<ProcessManager>,
) -> impl Responder {
    let session_id = path.into_inner();

    // First try discovered sessions (AppState)
    let scratchpad_path = if let Some(session) = state.sessions.iter().find(|s| s.id == session_id)
    {
        session.path.join("scratchpad.md")
    } else if let Some(working_dir) = process_manager.get_working_dir(&session_id) {
        // Fall back to ProcessManager for started sessions
        working_dir.join(".ralph").join("agent").join("scratchpad.md")
    } else {
        return HttpResponse::NotFound().json(ErrorResponse {
            error: "session_not_found".to_string(),
        });
    };

    match std::fs::read_to_string(&scratchpad_path) {
        Ok(content) => {
            let metadata = std::fs::metadata(&scratchpad_path).ok();
            let updated_at = metadata
                .and_then(|m| m.modified().ok())
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
            HttpResponse::Ok().json(ScratchpadResponse { content, updated_at })
        }
        Err(_) => HttpResponse::Ok().json(ScratchpadResponse {
            content: String::new(),
            updated_at: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use actix_web::{test, App};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn create_test_state() -> AppState {
        let sessions = vec![
            Session {
                id: "abc123".to_string(),
                path: PathBuf::from("/test/project1/.agent"),
                task_name: Some("my-task".to_string()),
                iteration: 3,
                hat: Some("builder".to_string()),
                started_at: Utc::now(),
                last_event_at: None,
            },
            Session {
                id: "def456".to_string(),
                path: PathBuf::from("/test/project2/.agent"),
                task_name: None,
                iteration: 1,
                hat: None,
                started_at: Utc::now(),
                last_event_at: None,
            },
        ];
        AppState {
            sessions,
            watchers: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[actix_web::test]
    async fn test_sessions_list_returns_json() {
        let state = create_test_state();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(
                    web::scope("/api")
                        .route("/sessions", web::get().to(list_sessions)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions")
            .to_request();

        let resp = test::call_service(&app, req).await;

        // Check status
        assert_eq!(resp.status(), 200);

        // Check content type
        let content_type = resp.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));

        // Check body is valid JSON array
        let body = test::read_body(resp).await;
        let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.len(), 2);
    }

    #[actix_web::test]
    async fn test_sessions_list_has_required_fields() {
        let sessions = create_test_state();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(sessions))
                .service(
                    web::scope("/api")
                        .route("/sessions", web::get().to(list_sessions)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions")
            .to_request();

        let resp = test::call_service(&app, req).await;
        let body = test::read_body(resp).await;
        let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

        // First session has all fields populated
        let session1 = &json[0];
        assert!(session1.get("id").is_some());
        assert!(session1.get("iteration").is_some());
        assert!(session1.get("hat").is_some());
        assert!(session1.get("started_at").is_some());

        assert_eq!(session1["id"], "abc123");
        assert_eq!(session1["iteration"], 3);
        assert_eq!(session1["hat"], "builder");

        // Second session has null hat
        let session2 = &json[1];
        assert_eq!(session2["id"], "def456");
        assert_eq!(session2["iteration"], 1);
        assert!(session2["hat"].is_null());
    }

    #[actix_web::test]
    async fn test_session_status_fields() {
        let sessions = create_test_state();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(sessions))
                .service(
                    web::scope("/api")
                        .route("/sessions", web::get().to(list_sessions))
                        .route("/sessions/{id}/status", web::get().to(get_session_status)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions/abc123/status")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Check all required fields are present
        assert!(json.get("id").is_some());
        assert!(json.get("iteration").is_some());
        assert!(json.get("total").is_some()); // Can be null
        assert!(json.get("hat").is_some()); // Can be null
        assert!(json.get("elapsed_secs").is_some());
        assert!(json.get("mode").is_some());

        // Check specific values
        assert_eq!(json["id"], "abc123");
        assert_eq!(json["iteration"], 3);
        assert_eq!(json["hat"], "builder");
        assert_eq!(json["mode"], "live");
    }

    #[actix_web::test]
    async fn test_session_not_found() {
        let sessions = create_test_state();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(sessions))
                .service(
                    web::scope("/api")
                        .route("/sessions", web::get().to(list_sessions))
                        .route("/sessions/{id}/status", web::get().to(get_session_status)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions/nonexistent/status")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "session_not_found");
    }

    #[actix_web::test]
    async fn test_scratchpad_session_not_found() {
        use crate::api::runner::ProcessManager;

        let state = create_test_state();
        let process_manager = ProcessManager::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .app_data(web::Data::new(process_manager))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/scratchpad", web::get().to(get_scratchpad)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions/nonexistent/scratchpad")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "session_not_found");
    }

    #[actix_web::test]
    async fn test_scratchpad_returns_empty_for_missing_file() {
        use crate::api::runner::ProcessManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let session = Session {
            id: "scratchpad-test".to_string(),
            path: temp_dir.path().to_path_buf(),
            task_name: None,
            iteration: 1,
            hat: None,
            started_at: Utc::now(),
            last_event_at: None,
        };

        let state = AppState {
            sessions: vec![session],
            watchers: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        };
        let process_manager = ProcessManager::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .app_data(web::Data::new(process_manager))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/scratchpad", web::get().to(get_scratchpad)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions/scratchpad-test/scratchpad")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["content"], "");
        assert!(json["updated_at"].is_null());
    }

    #[actix_web::test]
    async fn test_scratchpad_returns_content() {
        use crate::api::runner::ProcessManager;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create scratchpad file
        let scratchpad_path = temp_dir.path().join("scratchpad.md");
        fs::write(&scratchpad_path, "# My Scratchpad\n\nSome notes here.").unwrap();

        let session = Session {
            id: "scratchpad-content-test".to_string(),
            path: temp_dir.path().to_path_buf(),
            task_name: None,
            iteration: 1,
            hat: None,
            started_at: Utc::now(),
            last_event_at: None,
        };

        let state = AppState {
            sessions: vec![session],
            watchers: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        };
        let process_manager = ProcessManager::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .app_data(web::Data::new(process_manager))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/scratchpad", web::get().to(get_scratchpad)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/sessions/scratchpad-content-test/scratchpad")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["content"], "# My Scratchpad\n\nSome notes here.");
        assert!(json.get("updated_at").is_some());
    }
}
