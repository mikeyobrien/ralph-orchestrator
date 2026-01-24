//! Test-only API endpoints for E2E testing
//!
//! These endpoints are only compiled when the `test-mode` feature is enabled.
//! They provide control over the MockLoopManager for E2E test scenarios.
//!
//! # Security
//!
//! - Compile-time guarded with `#[cfg(feature = "test-mode")]`
//! - Requires `X-Ralph-Test-Secret` header with constant-time comparison
//! - All endpoints use the `/__test__/` prefix for easy identification

use crate::AppState;
use crate::mock_loop_manager::{MockEvent, MockLoopManager, MockLoopStatus, MockScenario};
use crate::routes::ErrorResponse;
use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use subtle::ConstantTimeEq;

/// Minimum required length for the test secret.
const MIN_SECRET_LENGTH: usize = 32;

/// Maximum file size for scenario YAML files (1 MB).
const MAX_SCENARIO_SIZE: u64 = 1024 * 1024;

/// Maximum number of iterations per scenario.
const MAX_ITERATIONS: usize = 100;

/// Maximum length for scenario names.
const MAX_SCENARIO_NAME_LENGTH: usize = 64;

/// Minimum length for scenario names.
const MIN_SCENARIO_NAME_LENGTH: usize = 2;

/// Path to scenario fixtures directory (relative to working directory).
fn fixtures_dir() -> PathBuf {
    PathBuf::from("crates/ralph-web/tests/fixtures/scenarios")
}

/// Error type for scenario loading.
#[derive(Debug)]
pub enum ScenarioError {
    /// Scenario name is invalid.
    InvalidName(String),
    /// Scenario file not found.
    NotFound(String),
    /// File too large.
    TooLarge,
    /// Too many iterations.
    TooManyIterations,
    /// I/O error.
    IoError(std::io::Error),
    /// YAML parse error.
    ParseError(String),
    /// Internal error.
    InternalError,
}

impl std::fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScenarioError::InvalidName(msg) => write!(f, "Invalid scenario name: {}", msg),
            ScenarioError::NotFound(name) => write!(f, "Scenario not found: {}", name),
            ScenarioError::TooLarge => write!(f, "Scenario file too large (max 1MB)"),
            ScenarioError::TooManyIterations => {
                write!(f, "Too many iterations (max {})", MAX_ITERATIONS)
            }
            ScenarioError::IoError(e) => write!(f, "I/O error: {}", e),
            ScenarioError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ScenarioError::InternalError => write!(f, "Internal error"),
        }
    }
}

impl From<std::io::Error> for ScenarioError {
    fn from(e: std::io::Error) -> Self {
        ScenarioError::IoError(e)
    }
}

/// Validate scenario name character-by-character (no regex - immune to ReDoS).
///
/// Pattern: ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$
/// - Must be 2-64 characters
/// - Only lowercase alphanumeric and hyphens
/// - Must start and end with lowercase alphanumeric
fn validate_scenario_name(name: &str) -> Result<(), ScenarioError> {
    let bytes = name.as_bytes();

    // Length check
    if bytes.len() < MIN_SCENARIO_NAME_LENGTH {
        return Err(ScenarioError::InvalidName(format!(
            "Name must be at least {} characters",
            MIN_SCENARIO_NAME_LENGTH
        )));
    }
    if bytes.len() > MAX_SCENARIO_NAME_LENGTH {
        return Err(ScenarioError::InvalidName(format!(
            "Name must be at most {} characters",
            MAX_SCENARIO_NAME_LENGTH
        )));
    }

    // First character must be lowercase alphanumeric
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return Err(ScenarioError::InvalidName(
            "Must start with lowercase alphanumeric".into(),
        ));
    }

    // Last character must be lowercase alphanumeric
    if !bytes[bytes.len() - 1].is_ascii_lowercase() && !bytes[bytes.len() - 1].is_ascii_digit() {
        return Err(ScenarioError::InvalidName(
            "Must end with lowercase alphanumeric".into(),
        ));
    }

    // All characters must be lowercase alphanumeric or hyphen
    for &b in bytes {
        if !b.is_ascii_lowercase() && !b.is_ascii_digit() && b != b'-' {
            return Err(ScenarioError::InvalidName(
                "Only lowercase alphanumeric and hyphens allowed".into(),
            ));
        }
    }

    Ok(())
}

/// Open and validate a scenario file atomically (TOCTOU-safe).
///
/// SECURITY: Opens file first with O_NOFOLLOW to prevent symlink attacks,
/// then validates properties on the open file handle.
#[cfg(unix)]
fn open_scenario_file(name: &str, fixtures_dir: &std::path::Path) -> Result<File, ScenarioError> {
    use std::os::unix::fs::OpenOptionsExt;

    // Validate name first (no filesystem operations yet)
    validate_scenario_name(name)?;

    // Construct path (no user input in extension)
    let scenario_path = fixtures_dir.join(format!("{}.yml", name));

    // SECURITY: Open file first with O_NOFOLLOW to prevent symlink attacks
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&scenario_path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ScenarioError::NotFound(name.into()),
            _ => ScenarioError::IoError(e),
        })?;

    // Get metadata from the open file handle (not the path) to prevent TOCTOU
    let metadata = file.metadata().map_err(|_| ScenarioError::InternalError)?;

    // Verify it's a regular file (not a directory, device, etc.)
    if !metadata.is_file() {
        return Err(ScenarioError::InvalidName("Not a regular file".into()));
    }

    // Check file size limit
    if metadata.len() > MAX_SCENARIO_SIZE {
        return Err(ScenarioError::TooLarge);
    }

    // Canonicalize the fixtures directory and scenario path, verify containment
    let canonical_fixtures = fixtures_dir
        .canonicalize()
        .map_err(|_| ScenarioError::InternalError)?;
    let canonical = scenario_path
        .canonicalize()
        .map_err(|_| ScenarioError::NotFound(name.into()))?;

    if !canonical.starts_with(&canonical_fixtures) {
        return Err(ScenarioError::InvalidName("Path traversal detected".into()));
    }

    Ok(file)
}

/// Open and validate a scenario file (non-Unix fallback).
#[cfg(not(unix))]
fn open_scenario_file(name: &str, fixtures_dir: &std::path::Path) -> Result<File, ScenarioError> {
    // Validate name first
    validate_scenario_name(name)?;

    // Construct path
    let scenario_path = fixtures_dir.join(format!("{}.yml", name));

    // Open file
    let file = File::open(&scenario_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ScenarioError::NotFound(name.into()),
        _ => ScenarioError::IoError(e),
    })?;

    // Get metadata
    let metadata = file.metadata().map_err(|_| ScenarioError::InternalError)?;

    if !metadata.is_file() {
        return Err(ScenarioError::InvalidName("Not a regular file".into()));
    }

    if metadata.len() > MAX_SCENARIO_SIZE {
        return Err(ScenarioError::TooLarge);
    }

    // Verify path containment
    let canonical_fixtures = fixtures_dir
        .canonicalize()
        .map_err(|_| ScenarioError::InternalError)?;
    let canonical = scenario_path
        .canonicalize()
        .map_err(|_| ScenarioError::NotFound(name.into()))?;

    if !canonical.starts_with(&canonical_fixtures) {
        return Err(ScenarioError::InvalidName("Path traversal detected".into()));
    }

    Ok(file)
}

/// Load a scenario from a file.
fn load_scenario_from_file(file: File) -> Result<MockScenario, ScenarioError> {
    use std::io::Read;

    let mut content = String::new();
    std::io::BufReader::new(file)
        .read_to_string(&mut content)
        .map_err(ScenarioError::IoError)?;

    let scenario: MockScenario =
        serde_yaml::from_str(&content).map_err(|e| ScenarioError::ParseError(e.to_string()))?;

    // Validate iteration count
    if scenario.iterations.len() > MAX_ITERATIONS {
        return Err(ScenarioError::TooManyIterations);
    }

    Ok(scenario)
}

/// Middleware that validates the test secret token using constant-time comparison.
///
/// SECURITY: Uses constant-time comparison to prevent timing side-channel attacks.
pub async fn require_test_secret(request: Request, next: Next) -> Result<Response, StatusCode> {
    let expected = std::env::var("RALPH_WEB_TEST_SECRET").map_err(|_| {
        tracing::error!("RALPH_WEB_TEST_SECRET environment variable not set");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Minimum secret length requirement
    if expected.len() < MIN_SECRET_LENGTH {
        tracing::error!(
            "RALPH_WEB_TEST_SECRET must be at least {} characters",
            MIN_SECRET_LENGTH
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let provided = request
        .headers()
        .get("X-Ralph-Test-Secret")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::FORBIDDEN)?;

    // SECURITY: Constant-time comparison prevents timing attacks
    let expected_bytes = expected.as_bytes();
    let provided_bytes = provided.as_bytes();

    // Pad shorter string to prevent length oracle
    let max_len = std::cmp::max(expected_bytes.len(), provided_bytes.len());
    let mut expected_padded = vec![0u8; max_len];
    let mut provided_padded = vec![0u8; max_len];

    expected_padded[..expected_bytes.len()].copy_from_slice(expected_bytes);
    provided_padded[..provided_bytes.len()].copy_from_slice(provided_bytes);

    // Constant-time comparison of padded values AND length check
    let content_eq: bool = expected_padded.ct_eq(&provided_padded).into();
    let len_eq = expected_bytes.len() == provided_bytes.len();

    if !(content_eq && len_eq) {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(request).await)
}

// ==================== Request/Response Types ====================

/// Request body for injecting an event.
#[derive(Debug, Deserialize)]
pub struct InjectEventRequest {
    pub topic: String,
    pub payload: String,
}

/// Request body for completing a loop.
#[derive(Debug, Deserialize)]
pub struct CompleteLoopRequest {
    pub status: String, // "completed", "failed", or "cancelled"
}

/// Response for scenario loading.
#[derive(Debug, Serialize)]
pub struct ScenarioLoadedResponse {
    pub name: String,
    pub iterations: usize,
}

/// Response for raw session data.
#[derive(Debug, Serialize)]
pub struct RawSessionResponse {
    pub session_id: String,
    pub agent_output: Option<String>,
    pub orchestration: Option<String>,
}

// ==================== Endpoint Handlers ====================

/// Load a named scenario.
/// POST /api/__test__/scenarios/{name}
async fn load_scenario(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ScenarioLoadedResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Get the MockLoopManager from state
    let mock_manager = state
        .loop_manager
        .as_any()
        .downcast_ref::<MockLoopManager>()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Not running in test mode".to_string(),
                }),
            )
        })?;

    // Open scenario file with TOCTOU-safe validation
    let file = open_scenario_file(&name, &fixtures_dir()).map_err(|e| {
        let status = match &e {
            ScenarioError::NotFound(_) => StatusCode::NOT_FOUND,
            ScenarioError::InvalidName(_) => StatusCode::BAD_REQUEST,
            ScenarioError::TooLarge | ScenarioError::TooManyIterations => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Load and parse scenario
    let scenario = load_scenario_from_file(file).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let iterations = scenario.iterations.len();
    let scenario_name = scenario.name.clone();

    // Load into mock manager
    mock_manager.load_scenario(scenario).await;

    Ok(Json(ScenarioLoadedResponse {
        name: scenario_name,
        iterations,
    }))
}

/// Advance a loop to the next iteration.
/// POST /api/__test__/loops/{session_id}/advance
async fn advance_iteration(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mock_manager = state
        .loop_manager
        .as_any()
        .downcast_ref::<MockLoopManager>()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Not running in test mode".to_string(),
                }),
            )
        })?;

    mock_manager
        .advance_iteration(&session_id)
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Inject an event into a running loop.
/// POST /api/__test__/loops/{session_id}/inject
async fn inject_event(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<InjectEventRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mock_manager = state
        .loop_manager
        .as_any()
        .downcast_ref::<MockLoopManager>()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Not running in test mode".to_string(),
                }),
            )
        })?;

    let event = MockEvent {
        topic: req.topic,
        payload: req.payload,
    };

    mock_manager
        .inject_event(&session_id, event)
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Complete a loop with specified status.
/// POST /api/__test__/loops/{session_id}/complete
async fn complete_loop(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<CompleteLoopRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mock_manager = state
        .loop_manager
        .as_any()
        .downcast_ref::<MockLoopManager>()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Not running in test mode".to_string(),
                }),
            )
        })?;

    let status = match req.status.as_str() {
        "completed" => MockLoopStatus::Completed,
        "failed" => MockLoopStatus::Failed,
        "cancelled" => MockLoopStatus::Cancelled,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid status. Must be 'completed', 'failed', or 'cancelled'"
                        .to_string(),
                }),
            ));
        }
    };

    mock_manager
        .complete(&session_id, status)
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Reset all test state.
/// POST /api/__test__/reset
async fn reset(
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mock_manager = state
        .loop_manager
        .as_any()
        .downcast_ref::<MockLoopManager>()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Not running in test mode".to_string(),
                }),
            )
        })?;

    mock_manager.reset().await;

    Ok(StatusCode::NO_CONTENT)
}

/// Get raw JSONL file contents for a session.
/// GET /api/__test__/sessions/{id}/raw
async fn get_raw_session(
    State(_state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<RawSessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate session ID format (UUID v4)
    if uuid::Uuid::parse_str(&session_id).is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid session ID format".to_string(),
            }),
        ));
    }

    let diagnostics_dir = PathBuf::from(".ralph/diagnostics");
    let session_dir = diagnostics_dir.join(&session_id);

    // Check if session directory exists
    if !session_dir.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Session not found".to_string(),
            }),
        ));
    }

    // Read agent-output.jsonl if it exists
    let agent_output_path = session_dir.join("agent-output.jsonl");
    let agent_output = if agent_output_path.exists() {
        fs::read_to_string(&agent_output_path).ok()
    } else {
        None
    };

    // Read orchestration.jsonl if it exists
    let orchestration_path = session_dir.join("orchestration.jsonl");
    let orchestration = if orchestration_path.exists() {
        fs::read_to_string(&orchestration_path).ok()
    } else {
        None
    };

    Ok(Json(RawSessionResponse {
        session_id,
        agent_output,
        orchestration,
    }))
}

/// Create test API routes.
///
/// All routes are protected by the `require_test_secret` middleware.
pub fn test_api_routes(state: AppState) -> Router {
    Router::new()
        .route("/api/__test__/scenarios/{name}", post(load_scenario))
        .route(
            "/api/__test__/loops/{session_id}/advance",
            post(advance_iteration),
        )
        .route(
            "/api/__test__/loops/{session_id}/inject",
            post(inject_event),
        )
        .route(
            "/api/__test__/loops/{session_id}/complete",
            post(complete_loop),
        )
        .route("/api/__test__/reset", post(reset))
        .route("/api/__test__/sessions/{id}/raw", get(get_raw_session))
        .route_layer(middleware::from_fn(require_test_secret))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Scenario Name Validation Tests ====================

    #[test]
    fn test_validate_scenario_name_valid() {
        assert!(validate_scenario_name("basic-success").is_ok());
        assert!(validate_scenario_name("multi-iteration").is_ok());
        assert!(validate_scenario_name("error-scenario").is_ok());
        assert!(validate_scenario_name("ab").is_ok());
        assert!(validate_scenario_name("a1").is_ok());
        assert!(validate_scenario_name("1a").is_ok());
        assert!(validate_scenario_name("test123").is_ok());
    }

    #[test]
    fn test_validate_scenario_name_too_short() {
        assert!(matches!(
            validate_scenario_name("a"),
            Err(ScenarioError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_scenario_name_too_long() {
        let long_name = "a".repeat(65);
        assert!(matches!(
            validate_scenario_name(&long_name),
            Err(ScenarioError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_scenario_name_starts_with_hyphen() {
        assert!(matches!(
            validate_scenario_name("-invalid"),
            Err(ScenarioError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_scenario_name_ends_with_hyphen() {
        assert!(matches!(
            validate_scenario_name("invalid-"),
            Err(ScenarioError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_scenario_name_uppercase() {
        assert!(matches!(
            validate_scenario_name("InvalidName"),
            Err(ScenarioError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_scenario_name_special_chars() {
        assert!(matches!(
            validate_scenario_name("invalid_name"),
            Err(ScenarioError::InvalidName(_))
        ));
        assert!(matches!(
            validate_scenario_name("invalid.name"),
            Err(ScenarioError::InvalidName(_))
        ));
        assert!(matches!(
            validate_scenario_name("invalid/name"),
            Err(ScenarioError::InvalidName(_))
        ));
        assert!(matches!(
            validate_scenario_name("invalid\\name"),
            Err(ScenarioError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_scenario_name_path_traversal() {
        assert!(matches!(
            validate_scenario_name("../etc/passwd"),
            Err(ScenarioError::InvalidName(_))
        ));
        assert!(matches!(
            validate_scenario_name(".."),
            Err(ScenarioError::InvalidName(_))
        ));
    }

    /// VULN-3: ReDoS prevention test
    /// Verify validation completes in O(n) time, not exponential
    #[test]
    fn test_scenario_name_validation_performance() {
        // Generate worst-case regex input: "a-a-a-a-...-a!"
        // This would cause exponential backtracking with naive regex
        let worst_case = format!("a{}", "-a".repeat(31));
        let start = std::time::Instant::now();
        let _ = validate_scenario_name(&worst_case);
        assert!(
            start.elapsed() < std::time::Duration::from_millis(1),
            "Validation should be O(n), not exponential"
        );
    }

    // ==================== Scenario Loading Tests ====================

    #[test]
    fn test_load_scenario_basic_success() {
        let fixtures = fixtures_dir();
        if !fixtures.exists() {
            // Skip if fixtures don't exist in test environment
            return;
        }

        let file = open_scenario_file("basic-success", &fixtures);
        assert!(file.is_ok(), "Should open basic-success.yml");

        let scenario = load_scenario_from_file(file.unwrap());
        assert!(scenario.is_ok(), "Should parse scenario");

        let scenario = scenario.unwrap();
        assert_eq!(scenario.name, "basic-success");
        assert!(!scenario.iterations.is_empty());
    }

    #[test]
    fn test_load_scenario_not_found() {
        let fixtures = fixtures_dir();
        let result = open_scenario_file("nonexistent-scenario", &fixtures);
        assert!(matches!(result, Err(ScenarioError::NotFound(_))));
    }

    #[test]
    fn test_load_scenario_invalid_name() {
        let fixtures = fixtures_dir();
        let result = open_scenario_file("../../../etc/passwd", &fixtures);
        assert!(matches!(result, Err(ScenarioError::InvalidName(_))));
    }
}
