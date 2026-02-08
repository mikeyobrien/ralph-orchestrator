//! Human-in-the-Loop (RObot) API endpoints.
//!
//! GET /api/robot/questions - Get pending human.interact questions
//! POST /api/robot/response - Send response to a question
//! POST /api/robot/guidance - Send proactive guidance to the loop

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::sessions::ErrorResponse;

/// State tracking pending questions across all sessions.
pub struct RobotState {
    /// Maps session_id -> PendingQuestion.
    pending_questions: Mutex<HashMap<String, PendingQuestion>>,
}

impl RobotState {
    pub fn new() -> Self {
        Self {
            pending_questions: Mutex::new(HashMap::new()),
        }
    }

    /// Add a pending question for a session.
    pub fn add_question(&self, session_id: String, question: PendingQuestion) {
        let mut questions = self.pending_questions.lock().unwrap();
        questions.insert(session_id, question);
    }

    /// Remove a pending question for a session.
    pub fn remove_question(&self, session_id: &str) -> Option<PendingQuestion> {
        let mut questions = self.pending_questions.lock().unwrap();
        questions.remove(session_id)
    }

    /// Get all pending questions.
    pub fn get_all_questions(&self) -> Vec<(String, PendingQuestion)> {
        let questions = self.pending_questions.lock().unwrap();
        questions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get a specific pending question by session ID.
    pub fn get_question(&self, session_id: &str) -> Option<PendingQuestion> {
        let questions = self.pending_questions.lock().unwrap();
        questions.get(session_id).cloned()
    }
}

/// A pending question waiting for human response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingQuestion {
    /// Unique question identifier (UUID).
    pub id: String,
    /// The question text from the agent.
    pub question_text: String,
    /// Session ID this question belongs to.
    pub session_id: String,
    /// When the question was asked.
    pub asked_at: DateTime<Utc>,
    /// When the question will timeout.
    pub timeout_at: DateTime<Utc>,
    /// Current iteration number.
    pub iteration: u32,
    /// Current hat name.
    pub hat: Option<String>,
}

/// Response format for GET /api/robot/questions.
#[derive(Debug, Clone, Serialize)]
pub struct QuestionsResponse {
    pub questions: Vec<PendingQuestion>,
}

/// Request body for POST /api/robot/response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionResponse {
    /// The question ID being responded to.
    pub question_id: String,
    /// The response text from the human.
    pub response_text: String,
}

/// Response format for POST /api/robot/response.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseAck {
    pub success: bool,
    pub question_id: String,
    pub delivered_at: String,
}

/// Request body for POST /api/robot/guidance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceRequest {
    /// Session ID to send guidance to.
    pub session_id: String,
    /// The guidance text to inject.
    pub guidance_text: String,
}

/// Response format for POST /api/robot/guidance.
#[derive(Debug, Clone, Serialize)]
pub struct GuidanceAck {
    pub success: bool,
    pub session_id: String,
    pub delivered_at: String,
}

/// GET /api/robot/questions - Get all pending questions.
///
/// Returns a list of questions that are awaiting human response.
/// Questions are populated when the event loop detects `human.interact` events.
pub async fn get_questions(state: web::Data<RobotState>) -> impl Responder {
    let questions = state.get_all_questions();
    let response = QuestionsResponse {
        questions: questions.into_iter().map(|(_, q)| q).collect(),
    };
    HttpResponse::Ok().json(response)
}

/// POST /api/robot/response - Send response to a pending question.
///
/// Writes a `human.response` event to the session's events.jsonl file
/// and removes the question from the pending list.
pub async fn post_response(
    body: web::Json<QuestionResponse>,
    state: web::Data<RobotState>,
) -> HttpResponse {
    // Find the question by ID
    let questions = state.get_all_questions();
    let question = questions
        .into_iter()
        .find(|(_, q)| q.id == body.question_id)
        .map(|(_, q)| q);

    let Some(question) = question else {
        return HttpResponse::NotFound().json(ErrorResponse {
            error: "question_not_found".to_string(),
        });
    };

    // Write human.response event to events.jsonl
    let events_path = get_events_path(&question.session_id);
    let timestamp = Utc::now();
    let event = serde_json::json!({
        "topic": "human.response",
        "payload": body.response_text,
        "ts": timestamp.to_rfc3339(),
    });

    if let Err(e) = append_event(&events_path, &event) {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("failed to write event: {}", e),
        });
    }

    // Remove from pending questions
    state.remove_question(&question.session_id);

    HttpResponse::Ok().json(ResponseAck {
        success: true,
        question_id: body.question_id.clone(),
        delivered_at: timestamp.to_rfc3339(),
    })
}

/// POST /api/robot/guidance - Send proactive guidance to a session.
///
/// Writes a `human.guidance` event to the session's events.jsonl file.
/// This is non-blocking and does not require a pending question.
pub async fn post_guidance(body: web::Json<GuidanceRequest>) -> HttpResponse {
    let events_path = get_events_path(&body.session_id);
    let timestamp = Utc::now();
    let event = serde_json::json!({
        "topic": "human.guidance",
        "payload": body.guidance_text,
        "ts": timestamp.to_rfc3339(),
    });

    if let Err(e) = append_event(&events_path, &event) {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("failed to write event: {}", e),
        });
    }

    HttpResponse::Ok().json(GuidanceAck {
        success: true,
        session_id: body.session_id.clone(),
        delivered_at: timestamp.to_rfc3339(),
    })
}

/// Get the events file path for a session.
///
/// Reads the `current-events` marker to find the active events file,
/// falling back to `.ralph/events.jsonl` if the marker doesn't exist.
fn get_events_path(_session_id: &str) -> PathBuf {
    // TODO: This should read from a configured workspace root
    // For now, assume current directory is the workspace root
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let ralph_dir = workspace_root.join(".ralph");

    let marker_path = ralph_dir.join("current-events");
    if let Ok(contents) = std::fs::read_to_string(&marker_path) {
        let relative = contents.trim();
        if !relative.is_empty() {
            return workspace_root.join(relative);
        }
    }

    ralph_dir.join("events.jsonl")
}

/// Append an event to the events file.
fn append_event(path: &Path, event: &serde_json::Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let event_line = serde_json::to_string(event)?;
    writeln!(file, "{}", event_line)?;
    Ok(())
}

/// Scan events file for `human.interact` events and populate RobotState.
///
/// This should be called periodically or on server startup to detect
/// pending questions from the event stream.
pub fn scan_for_questions(events_path: &Path, session_id: &str, state: &RobotState) {
    let Ok(file) = std::fs::File::open(events_path) else {
        return;
    };

    let reader = BufReader::new(file);
    for line in reader.lines().filter_map(|l| l.ok()) {
        // Try to parse as JSON event
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
            if event.get("topic").and_then(|t| t.as_str()) == Some("human.interact") {
                let payload = event
                    .get("payload")
                    .and_then(|p| p.as_str())
                    .unwrap_or("")
                    .to_string();
                let timestamp = event
                    .get("ts")
                    .and_then(|t| t.as_str())
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);

                // Extract iteration and hat if available
                let iteration = event
                    .get("iteration")
                    .and_then(|i| i.as_u64())
                    .unwrap_or(0) as u32;
                let hat = event
                    .get("hat")
                    .and_then(|h| h.as_str())
                    .map(|s| s.to_string());

                // Create pending question with a timeout (e.g., 5 minutes from now)
                let timeout_at = Utc::now() + chrono::Duration::minutes(5);
                let question = PendingQuestion {
                    id: uuid::Uuid::new_v4().to_string(),
                    question_text: payload,
                    session_id: session_id.to_string(),
                    asked_at: timestamp,
                    timeout_at,
                    iteration,
                    hat,
                };

                state.add_question(session_id.to_string(), question);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use tempfile::TempDir;

    fn create_test_state() -> RobotState {
        RobotState::new()
    }

    fn create_temp_events_file() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let events_path = dir.path().join(".ralph/events.jsonl");
        std::fs::create_dir_all(events_path.parent().unwrap()).unwrap();
        (dir, events_path)
    }

    #[actix_web::test]
    async fn test_get_questions_empty() {
        let state = create_test_state();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(
                    web::scope("/api")
                        .route("/robot/questions", web::get().to(get_questions)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/robot/questions")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["questions"].as_array().unwrap().len(), 0);
    }

    #[actix_web::test]
    async fn test_get_questions_with_pending() {
        let state = create_test_state();
        let question = PendingQuestion {
            id: "q123".to_string(),
            question_text: "Should I use async?".to_string(),
            session_id: "session1".to_string(),
            asked_at: Utc::now(),
            timeout_at: Utc::now() + chrono::Duration::minutes(5),
            iteration: 3,
            hat: Some("builder".to_string()),
        };
        state.add_question("session1".to_string(), question);


        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(
                    web::scope("/api")
                        .route("/robot/questions", web::get().to(get_questions)),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/robot/questions")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let questions = json["questions"].as_array().unwrap();
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0]["id"], "q123");
        assert_eq!(questions[0]["question_text"], "Should I use async?");
        assert_eq!(questions[0]["session_id"], "session1");
        assert_eq!(questions[0]["iteration"], 3);
        assert_eq!(questions[0]["hat"], "builder");
    }

    #[actix_web::test]
    async fn test_post_response_not_found() {
        let state = create_test_state();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(
                    web::scope("/api")
                        .route("/robot/response", web::post().to(post_response)),
                ),
        )
        .await;

        let body = QuestionResponse {
            question_id: "nonexistent".to_string(),
            response_text: "yes".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/api/robot/response")
            .set_json(&body)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "question_not_found");
    }

    #[actix_web::test]
    #[serial_test::serial(cwd)]
    async fn test_post_response_success() {
        let (_temp, events_path) = create_temp_events_file();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });

        let state = create_test_state();
        let question = PendingQuestion {
            id: "q456".to_string(),
            question_text: "Which DB?".to_string(),
            session_id: "session2".to_string(),
            asked_at: Utc::now(),
            timeout_at: Utc::now() + chrono::Duration::minutes(5),
            iteration: 1,
            hat: Some("planner".to_string()),
        };
        state.add_question("session2".to_string(), question);

        // Mock get_events_path by setting working directory
        std::env::set_current_dir(_temp.path()).unwrap();


        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(
                    web::scope("/api")
                        .route("/robot/response", web::post().to(post_response)),
                ),
        )
        .await;

        let body = QuestionResponse {
            question_id: "q456".to_string(),
            response_text: "Use PostgreSQL".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/api/robot/response")
            .set_json(&body)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["question_id"], "q456");

        // Verify event was written
        let contents = std::fs::read_to_string(&events_path).unwrap();
        assert!(contents.contains("human.response"));
        assert!(contents.contains("Use PostgreSQL"));
    }

    #[actix_web::test]
    #[serial_test::serial(cwd)]
    async fn test_post_guidance_success() {
        let (_temp, events_path) = create_temp_events_file();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });

        // Mock get_events_path by setting working directory
        std::env::set_current_dir(_temp.path()).unwrap();


        let app = test::init_service(App::new().service(
            web::scope("/api")
                .route("/robot/guidance", web::post().to(post_guidance)),
        ))
        .await;

        let body = GuidanceRequest {
            session_id: "session3".to_string(),
            guidance_text: "Focus on error handling".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/api/robot/guidance")
            .set_json(&body)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["session_id"], "session3");

        // Verify event was written
        let contents = std::fs::read_to_string(&events_path).unwrap();
        assert!(contents.contains("human.guidance"));
        assert!(contents.contains("Focus on error handling"));
    }

    #[actix_web::test]
    async fn test_scan_for_questions() {
        let (_temp, events_path) = create_temp_events_file();

        // Write a human.interact event
        let event = serde_json::json!({
            "topic": "human.interact",
            "payload": "Which approach should I use?",
            "ts": "2026-02-03T10:00:00Z",
            "iteration": 2,
            "hat": "architect"
        });
        append_event(&events_path, &event).unwrap();

        let state = create_test_state();
        scan_for_questions(&events_path, "test-session", &state);

        let questions = state.get_all_questions();
        assert_eq!(questions.len(), 1);
        assert_eq!(
            questions[0].1.question_text,
            "Which approach should I use?"
        );
        assert_eq!(questions[0].1.session_id, "test-session");
        assert_eq!(questions[0].1.iteration, 2);
        assert_eq!(questions[0].1.hat, Some("architect".to_string()));
    }

}
