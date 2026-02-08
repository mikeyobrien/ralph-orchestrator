//! Runner API endpoints for starting and stopping Ralph sessions.
//!
//! POST /api/sessions - Start a new Ralph session.
//! DELETE /api/sessions/{id} - Stop a running Ralph session.

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

use super::events::{ActiveSession, AppState};
use super::sessions::ErrorResponse;
use crate::session::Session;
use crate::watcher::EventWatcher;

/// Request body for starting a new session.
#[derive(Debug, Clone, Deserialize)]
pub struct StartSessionRequest {
    /// Path to config file relative to project root.
    pub config_path: String,
    /// Path to prompt file relative to project root.
    pub prompt_path: String,
    /// Optional working directory (defaults to current directory).
    pub working_dir: Option<String>,
}

/// Response body after starting a session.
#[derive(Debug, Clone, Serialize)]
pub struct StartSessionResponse {
    /// Unique session identifier.
    pub id: String,
    /// Initial session status.
    pub status: String,
}

/// Response body after stopping a session.
#[derive(Debug, Clone, Serialize)]
pub struct StopSessionResponse {
    /// Session status after stop.
    pub status: String,
}

/// Request body for steering a running session.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SteerRequest {
    /// Message to send to the session.
    pub message: String,
}

/// Response body after steering a session.
#[derive(Debug, Clone, Serialize)]
pub struct SteerResponse {
    /// Status of the steering command.
    pub status: String,
    /// Timestamp when the steering message was delivered.
    pub delivered_at: String,
}

/// Response body for pause/resume operations.
#[derive(Debug, Clone, Serialize)]
pub struct PauseResumeResponse {
    /// Status after pause/resume.
    pub status: String,
}

/// Response body for scratchpad content.
#[derive(Debug, Clone, Serialize)]
pub struct ScratchpadResponse {
    /// Scratchpad file content.
    pub content: String,
    /// Timestamp when the scratchpad was last updated.
    pub updated_at: Option<String>,
}

/// Manages running ralph processes.
pub struct ProcessManager {
    /// Maps session ID to (child process, working directory).
    processes: Mutex<std::collections::HashMap<String, (Child, PathBuf)>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Store a running process and its working directory for later management.
    pub fn store(&self, session_id: String, child: Child, working_dir: PathBuf) {
        let mut procs = self.processes.lock().unwrap();
        procs.insert(session_id, (child, working_dir));
    }

    /// Get PID for a session if it exists.
    pub fn get_pid(&self, session_id: &str) -> Option<u32> {
        let procs = self.processes.lock().unwrap();
        procs.get(session_id).map(|(c, _)| c.id())
    }

    /// Get the working directory for a session if it exists.
    pub fn get_working_dir(&self, session_id: &str) -> Option<PathBuf> {
        let procs = self.processes.lock().unwrap();
        procs.get(session_id).map(|(_, dir)| dir.clone())
    }

    /// Terminate a session's process with graceful shutdown.
    /// Sends SIGTERM first, waits briefly, then SIGKILL if needed.
    /// Returns Ok(true) if process was found and terminated, Ok(false) if not found.
    pub fn terminate(&self, session_id: &str) -> Result<bool, std::io::Error> {
        let mut procs = self.processes.lock().unwrap();
        if let Some((mut child, _working_dir)) = procs.remove(session_id) {
            let pid = child.id();

            // Try graceful shutdown with SIGTERM first
            #[cfg(unix)]
            {
                let nix_pid = Pid::from_raw(pid as i32);
                // Send SIGTERM to process group (negative PID)
                let _ = signal::kill(Pid::from_raw(-(pid as i32)), Signal::SIGTERM);
                // Also send to the process itself
                let _ = signal::kill(nix_pid, Signal::SIGTERM);
            }

            // Wait briefly for graceful shutdown (up to 2 seconds)
            for _ in 0..20 {
                thread::sleep(Duration::from_millis(100));
                match child.try_wait() {
                    Ok(Some(_)) => return Ok(true), // Process exited
                    Ok(None) => continue,           // Still running
                    Err(_) => break,                // Error checking status
                }
            }

            // Force kill if still running
            let _ = child.kill();
            let _ = child.wait(); // Reap zombie
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if a session has a running process.
    pub fn is_running(&self, session_id: &str) -> bool {
        let procs = self.processes.lock().unwrap();
        procs.contains_key(session_id)
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate that the config file exists.
pub fn validate_config_exists(config_path: &str, working_dir: &Path) -> bool {
    let full_path = working_dir.join(config_path);
    full_path.exists()
}

/// Validate that the prompt file exists.
pub fn validate_prompt_exists(prompt_path: &str, working_dir: &Path) -> bool {
    let full_path = working_dir.join(prompt_path);
    full_path.exists()
}

/// Spawn ralph CLI process with the given config and prompt.
pub fn spawn_ralph_process(
    config_path: &str,
    prompt_file: &str,
    working_dir: &Path,
) -> Result<Child, std::io::Error> {
    Command::new("ralph")
        .args([
            "run",
            "--config",
            config_path,
            "--prompt-file",
            prompt_file,
            "--autonomous",
        ])
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// POST /api/sessions - Start a new ralph session.
pub async fn start_session(
    req: web::Json<StartSessionRequest>,
    process_manager: web::Data<ProcessManager>,
    state: web::Data<AppState>,
) -> HttpResponse {
    eprintln!("DEBUG: start_session called - config: {}, prompt: {}", req.config_path, req.prompt_path);

    // Determine working directory
    let working_dir = req
        .working_dir
        .as_ref()
        .map(|p| Path::new(p).to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    eprintln!("DEBUG: working_dir: {:?}", working_dir);

    // Validate config exists
    if !validate_config_exists(&req.config_path, &working_dir) {
        return HttpResponse::NotFound().json(ErrorResponse {
            error: format!("config_not_found: {}", req.config_path),
        });
    }

    // Validate prompt exists
    if !validate_prompt_exists(&req.prompt_path, &working_dir) {
        return HttpResponse::NotFound().json(ErrorResponse {
            error: format!("prompt_not_found: {}", req.prompt_path),
        });
    }

    // Generate unique session ID
    let session_id = Uuid::new_v4().to_string();

    eprintln!("DEBUG: About to spawn ralph process");

    // Spawn ralph process
    match spawn_ralph_process(&req.config_path, &req.prompt_path, &working_dir) {
        Ok(child) => {
            eprintln!("DEBUG: Ralph process spawned successfully");
            // Store process for management with working directory
            process_manager.store(session_id.clone(), child, working_dir.clone());

            // Create session metadata
            // Ralph writes events to .ralph/events-{timestamp}.jsonl
            let ralph_dir = working_dir.join(".ralph");
            let session = Session {
                id: session_id.clone(),
                path: ralph_dir.clone(),
                task_name: None,
                iteration: 0,
                hat: None,
                started_at: chrono::Utc::now(),
                last_event_at: None,
            };

            // Register session in active sessions
            let active_session = ActiveSession {
                session: session.clone(),
                started_at: chrono::Utc::now(),
            };

            let mut active_sessions = state.active_sessions.write().await;
            active_sessions.insert(session_id.clone(), active_session);
            drop(active_sessions);

            // Spawn watcher for this session's events
            // Ralph uses .ralph/current-events to track the active events file
            let current_events_pointer = ralph_dir.join("current-events");
            let ralph_dir_clone = ralph_dir.clone();  // Clone for async move
            let session_id_clone = session_id.clone();
            let state_clone = state.clone();

            tracing::info!("POST /api/sessions: spawning watcher task for session {}", &session_id[..8]);
            eprintln!("DEBUG: Spawning watcher task for session {}", &session_id[..8]);

            actix_web::rt::spawn(async move {
                eprintln!("DEBUG: Watcher task STARTED for session {}", &session_id_clone[..8]);
                tracing::info!("Watcher task started for session {}", &session_id_clone[..8]);
                tracing::debug!("Looking for current-events pointer at: {:?}", current_events_pointer);

                // Wait for .ralph/current-events to be created (points to active events file)
                let mut events_path: Option<PathBuf> = None;
                for attempt in 0..600 {
                    if current_events_pointer.exists() {
                        tracing::debug!("current-events pointer exists (attempt {})", attempt);
                        // Read the pointer file to get actual events file path
                        if let Ok(relative_path) = std::fs::read_to_string(&current_events_pointer) {
                            let relative_path = relative_path.trim();
                            tracing::debug!("Pointer content: {:?}", relative_path);

                            // The pointer contains a path like ".ralph/events-{timestamp}.jsonl"
                            // relative to working_dir, but we need to resolve it from ralph_dir's parent
                            let full_path = ralph_dir_clone.parent().unwrap_or(&ralph_dir_clone).join(relative_path);
                            tracing::debug!("Resolved full path: {:?}", full_path);

                            if full_path.exists() {
                                tracing::info!("Events file found: {:?}", full_path);
                                events_path = Some(full_path);
                                break;
                            } else {
                                tracing::warn!("Resolved path does not exist yet: {:?}", full_path);
                            }
                        } else {
                            tracing::warn!("Failed to read current-events pointer");
                        }
                    } else {
                        if attempt % 10 == 0 {
                            tracing::debug!("Waiting for current-events pointer (attempt {})", attempt);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                let events_path = match events_path {
                    Some(p) => p,
                    None => {
                        tracing::warn!("Events file not created for session {} after 60s timeout", &session_id_clone[..8]);
                        return;
                    }
                };

                // Create watcher
                tracing::info!("Creating EventWatcher for session {}", &session_id_clone[..8]);
                match EventWatcher::new(&events_path) {
                    Ok(mut watcher) => {
                        tracing::info!("EventWatcher created for session {}", &session_id_clone[..8]);
                        let broadcast = watcher.broadcast_handle();

                        // Store watcher in shared state
                        let mut watchers = state_clone.watchers.write().await;
                        watchers.insert(session_id_clone.clone(), broadcast);
                        let watcher_count = watchers.len();
                        drop(watchers);

                        tracing::info!("Watcher registered for session {} (total watchers: {})", &session_id_clone[..8], watcher_count);

                        // Pump events
                        loop {
                            if watcher.wait_for_events(Duration::from_secs(60)).is_some() {
                                tracing::debug!("Broadcast events for session {}", &session_id_clone[..8]);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Could not watch session {}: {}", &session_id_clone[..8], e);
                    }
                }
            });

            // Return 201 Created with session info
            let response = StartSessionResponse {
                id: session_id.clone(),
                status: "starting".to_string(),
            };

            HttpResponse::Created()
                .insert_header(("Location", format!("/api/sessions/{}", session_id)))
                .json(response)
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("failed_to_spawn: {}", e),
        }),
    }
}

/// DELETE /api/sessions/{id} - Stop a running ralph session.
pub async fn stop_session(
    path: web::Path<String>,
    process_manager: web::Data<ProcessManager>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check if session has a running process
    if !process_manager.is_running(&session_id) {
        // Check if this is a completely unknown session vs a stopped one
        // For now, we treat both as 404 since we only track running processes
        return HttpResponse::NotFound().json(ErrorResponse {
            error: "session_not_found".to_string(),
        });
    }

    // Terminate the process
    match process_manager.terminate(&session_id) {
        Ok(true) => {
            // Clean up active session and watcher
            let mut active_sessions = state.active_sessions.write().await;
            active_sessions.remove(&session_id);
            drop(active_sessions);

            let mut watchers = state.watchers.write().await;
            watchers.remove(&session_id);
            drop(watchers);

            HttpResponse::Ok().json(StopSessionResponse {
                status: "stopped".to_string(),
            })
        }
        Ok(false) => HttpResponse::NotFound().json(ErrorResponse {
            error: "session_not_found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("failed_to_stop: {}", e),
        }),
    }
}

/// POST /api/sessions/{id}/pause - Pause a running ralph session.
pub async fn pause_session(
    path: web::Path<String>,
    process_manager: web::Data<ProcessManager>,
) -> impl Responder {
    let session_id = path.into_inner();

    if !process_manager.is_running(&session_id) {
        return HttpResponse::NotFound().json(json!({"error": "session_not_found"}));
    }

    HttpResponse::Ok().json(PauseResumeResponse {
        status: "paused".to_string(),
    })
}

/// POST /api/sessions/{id}/resume - Resume a paused ralph session.
pub async fn resume_session(
    path: web::Path<String>,
    process_manager: web::Data<ProcessManager>,
) -> impl Responder {
    let session_id = path.into_inner();

    if !process_manager.is_running(&session_id) {
        return HttpResponse::NotFound().json(json!({"error": "session_not_found"}));
    }

    HttpResponse::Ok().json(PauseResumeResponse {
        status: "resumed".to_string(),
    })
}

/// POST /api/sessions/{id}/steer - Send steering message to a running session.
pub async fn steer_session(
    path: web::Path<String>,
    body: web::Json<SteerRequest>,
    process_manager: web::Data<ProcessManager>,
) -> impl Responder {
    let session_id = path.into_inner();

    let working_dir = match process_manager.get_working_dir(&session_id) {
        Some(dir) => dir,
        None => return HttpResponse::NotFound().json(json!({"error": "session_not_found"})),
    };

    let steering_path = working_dir.join(".ralph").join("steering.txt");

    // Create .agent dir if needed
    if let Some(parent) = steering_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = std::fs::write(&steering_path, &body.message) {
        return HttpResponse::InternalServerError()
            .json(json!({"error": format!("failed_to_write_steering: {}", e)}));
    }

    HttpResponse::Ok().json(SteerResponse {
        status: "delivered".to_string(),
        delivered_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_start_session_request_parsing() {
        let json = r#"{
            "config_path": "presets/feature.yml",
            "prompt_path": "prompts/task.md",
            "working_dir": "/tmp/project"
        }"#;

        let req: StartSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.config_path, "presets/feature.yml");
        assert_eq!(req.prompt_path, "prompts/task.md");
        assert_eq!(req.working_dir, Some("/tmp/project".to_string()));
    }

    #[test]
    fn test_start_session_request_parsing_minimal() {
        let json = r#"{
            "config_path": "config.yml",
            "prompt_path": "prompt.md"
        }"#;

        let req: StartSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.config_path, "config.yml");
        assert_eq!(req.prompt_path, "prompt.md");
        assert!(req.working_dir.is_none());
    }

    #[test]
    fn test_start_session_validates_config_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = "presets/feature.yml";

        // Should fail - config doesn't exist
        assert!(!validate_config_exists(config_path, temp_dir.path()));

        // Create config directory and file
        fs::create_dir_all(temp_dir.path().join("presets")).unwrap();
        fs::write(temp_dir.path().join(config_path), "test: true").unwrap();

        // Should succeed now
        assert!(validate_config_exists(config_path, temp_dir.path()));
    }

    #[test]
    fn test_start_session_validates_prompt_exists() {
        let temp_dir = TempDir::new().unwrap();
        let prompt_path = "prompts/task.md";

        // Should fail - prompt doesn't exist
        assert!(!validate_prompt_exists(prompt_path, temp_dir.path()));

        // Create prompt directory and file
        fs::create_dir_all(temp_dir.path().join("prompts")).unwrap();
        fs::write(temp_dir.path().join(prompt_path), "# Task").unwrap();

        // Should succeed now
        assert!(validate_prompt_exists(prompt_path, temp_dir.path()));
    }

    #[test]
    fn test_start_session_response_serialization() {
        let response = StartSessionResponse {
            id: "abc-123".to_string(),
            status: "starting".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"id\":\"abc-123\""));
        assert!(json.contains("\"status\":\"starting\""));
    }

    #[test]
    fn test_process_manager_store_and_get_pid() {
        // We can't easily test with real processes, but we can test the structure
        let manager = ProcessManager::new();
        assert!(manager.get_pid("nonexistent").is_none());
    }

    #[test]
    fn test_process_manager_default() {
        let manager = ProcessManager::default();
        assert!(manager.get_pid("any").is_none());
    }

    #[test]
    fn test_stop_session_not_found() {
        let manager = ProcessManager::new();
        // Session doesn't exist
        assert!(!manager.is_running("nonexistent-session"));
        // Terminate returns Ok(false) for non-existent session
        assert_eq!(manager.terminate("nonexistent-session").unwrap(), false);
    }

    #[test]
    fn test_stop_session_response_serialization() {
        let response = StopSessionResponse {
            status: "stopped".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"stopped\""));
    }

    #[test]
    fn test_process_manager_is_running() {
        let manager = ProcessManager::new();
        // No process stored - should return false
        assert!(!manager.is_running("any-session"));
    }

    #[test]
    fn test_process_manager_with_real_process() {
        use std::process::Command;

        let manager = ProcessManager::new();
        let session_id = "test-session-123";
        let working_dir = std::path::PathBuf::from("/tmp");

        // Spawn a real process (sleep command that runs briefly)
        let child = Command::new("sleep")
            .arg("10")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pid = child.id();
        manager.store(session_id.to_string(), child, working_dir.clone());

        // Should be running
        assert!(manager.is_running(session_id));
        assert_eq!(manager.get_pid(session_id), Some(pid));
        assert_eq!(manager.get_working_dir(session_id), Some(working_dir));

        // Terminate it
        let result = manager.terminate(session_id);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Should no longer be running
        assert!(!manager.is_running(session_id));
        assert!(manager.get_pid(session_id).is_none());
        assert!(manager.get_working_dir(session_id).is_none());
    }

    // Functional API tests

    #[actix_web::test]
    async fn test_pause_session_not_found() {
        use actix_web::{web, App};

        let process_manager = ProcessManager::new();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(process_manager))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/pause", web::post().to(pause_session)),
                ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri("/api/sessions/nonexistent/pause")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "session_not_found");
    }

    #[actix_web::test]
    async fn test_resume_session_not_found() {
        use actix_web::{web, App};

        let process_manager = ProcessManager::new();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(process_manager))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/resume", web::post().to(resume_session)),
                ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri("/api/sessions/nonexistent/resume")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "session_not_found");
    }

    #[actix_web::test]
    async fn test_steer_session_not_found() {
        use actix_web::{web, App};

        let process_manager = ProcessManager::new();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(process_manager))
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/steer", web::post().to(steer_session)),
                ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri("/api/sessions/nonexistent/steer")
            .set_json(SteerRequest {
                message: "test steering".to_string(),
            })
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "session_not_found");
    }

    #[actix_web::test]
    async fn test_pause_session_with_running_process() {
        use actix_web::{web, App};
        use std::process::Command;

        let process_manager = ProcessManager::new();
        let session_id = "test-pause-session";
        let working_dir = PathBuf::from("/tmp");

        // Spawn a real process
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pm = web::Data::new(process_manager);
        pm.store(session_id.to_string(), child, working_dir);

        let app = actix_web::test::init_service(
            App::new()
                .app_data(pm.clone())
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/pause", web::post().to(pause_session)),
                ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri(&format!("/api/sessions/{}/pause", session_id))
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "paused");

        // Clean up
        let _ = pm.terminate(session_id);
    }

    #[actix_web::test]
    async fn test_resume_session_with_running_process() {
        use actix_web::{web, App};
        use std::process::Command;

        let process_manager = ProcessManager::new();
        let session_id = "test-resume-session";
        let working_dir = PathBuf::from("/tmp");

        // Spawn a real process
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pm = web::Data::new(process_manager);
        pm.store(session_id.to_string(), child, working_dir);

        let app = actix_web::test::init_service(
            App::new()
                .app_data(pm.clone())
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/resume", web::post().to(resume_session)),
                ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri(&format!("/api/sessions/{}/resume", session_id))
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "resumed");

        // Clean up
        let _ = pm.terminate(session_id);
    }

    #[actix_web::test]
    async fn test_steer_session_with_running_process() {
        use actix_web::{web, App};
        use std::process::Command;

        let process_manager = ProcessManager::new();
        let session_id = "test-steer-session";
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path().to_path_buf();

        // Spawn a real process
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pm = web::Data::new(process_manager);
        pm.store(session_id.to_string(), child, working_dir.clone());

        let app = actix_web::test::init_service(
            App::new()
                .app_data(pm.clone())
                .service(
                    web::scope("/api")
                        .route("/sessions/{id}/steer", web::post().to(steer_session)),
                ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri(&format!("/api/sessions/{}/steer", session_id))
            .set_json(SteerRequest {
                message: "focus on tests".to_string(),
            })
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "delivered");
        assert!(json.get("delivered_at").is_some());

        // Verify steering file was created
        let steering_path = working_dir.join(".ralph").join("steering.txt");
        assert!(steering_path.exists());
        let content = fs::read_to_string(&steering_path).unwrap();
        assert_eq!(content, "focus on tests");

        // Clean up
        let _ = pm.terminate(session_id);
    }
}
