//! Memories API endpoints.
//!
//! GET /api/memories - Get memories content from `.ralph/agent/memories.md`
//! PUT /api/memories - Update memories content
//! POST /api/memories/export - Export memories snapshot with timestamp

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::sessions::ErrorResponse;

/// Response format for GET /api/memories.
#[derive(Debug, Serialize)]
pub struct MemoriesContent {
    pub content: String,
    pub last_modified: Option<String>, // ISO 8601 format
}

/// Request format for PUT /api/memories.
#[derive(Debug, Deserialize)]
pub struct UpdateMemoriesRequest {
    pub content: String,
}

/// Response format for POST /api/memories/export.
#[derive(Debug, Serialize)]
pub struct MemoriesExport {
    pub content: String,
    pub exported_at: String,
    pub filename: String,
}

/// GET /api/memories - Get memories content.
///
/// Returns the content of `.ralph/agent/memories.md` from the current working directory.
pub async fn get_memories() -> impl Responder {
    let memories_path = get_memories_path();

    match fs::read_to_string(&memories_path) {
        Ok(content) => {
            let last_modified = fs::metadata(&memories_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

            HttpResponse::Ok().json(MemoriesContent {
                content,
                last_modified,
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Return empty content if file doesn't exist
            HttpResponse::Ok().json(MemoriesContent {
                content: String::new(),
                last_modified: None,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to read memories: {}", e),
        }),
    }
}

/// PUT /api/memories - Update memories content.
///
/// Writes the provided content to `.ralph/agent/memories.md`.
pub async fn update_memories(body: web::Json<UpdateMemoriesRequest>) -> impl Responder {
    let memories_path = get_memories_path();

    // Ensure parent directory exists
    if let Some(parent) = memories_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to create memories directory: {}", e),
            });
        }
    }

    match fs::write(&memories_path, &body.content) {
        Ok(()) => {
            let last_modified = fs::metadata(&memories_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

            HttpResponse::Ok().json(MemoriesContent {
                content: body.content.clone(),
                last_modified,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to write memories: {}", e),
        }),
    }
}

/// POST /api/memories/export - Export memories snapshot.
///
/// Returns the current memories content with an export timestamp and filename.
pub async fn export_memories() -> impl Responder {
    let memories_path = get_memories_path();

    match fs::read_to_string(&memories_path) {
        Ok(content) => {
            let now = Utc::now();
            let exported_at = now.to_rfc3339();
            let filename = format!("memories-{}.md", now.format("%Y%m%d-%H%M%S"));

            HttpResponse::Ok().json(MemoriesExport {
                content,
                exported_at,
                filename,
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            HttpResponse::NotFound().json(ErrorResponse {
                error: "memories_not_found".to_string(),
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to read memories: {}", e),
        }),
    }
}

/// Returns the path to the memories file.
fn get_memories_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".ralph")
        .join("agent")
        .join("memories.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use serial_test::serial;
    use std::path::Path;
    use tempfile::TempDir;

    // Helper to create memories file in a temp directory
    fn setup_memories_file(temp_dir: &Path, content: &str) {
        let memories_dir = temp_dir.join(".ralph").join("agent");
        fs::create_dir_all(&memories_dir).unwrap();
        let memories_file = memories_dir.join("memories.md");
        fs::write(&memories_file, content).unwrap();
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_get_memories_returns_content() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        setup_memories_file(temp_dir.path(), "# Test Memory Content");
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/memories", web::get().to(get_memories)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/memories")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["content"], "# Test Memory Content");
        assert!(json["last_modified"].is_string());
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_get_memories_empty_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/memories", web::get().to(get_memories)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/memories")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["content"], "");
        assert!(json["last_modified"].is_null());
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_update_memories_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/memories", web::put().to(update_memories)),
            ),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/api/memories")
            .insert_header(("Content-Type", "application/json"))
            .set_json(serde_json::json!({
                "content": "# New Memory\n\n## Patterns\n\n- Test pattern"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["content"].as_str().unwrap().contains("# New Memory"));
        assert!(json["last_modified"].is_string());

        // Verify file was actually written
        let memories_path = temp_dir.path().join(".ralph").join("agent").join("memories.md");
        assert!(memories_path.exists());
        let content = fs::read_to_string(&memories_path).unwrap();
        assert!(content.contains("# New Memory"));
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_update_memories_overwrites_existing() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        setup_memories_file(temp_dir.path(), "Old content");
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/memories", web::put().to(update_memories)),
            ),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/api/memories")
            .insert_header(("Content-Type", "application/json"))
            .set_json(serde_json::json!({
                "content": "New content"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let memories_file = temp_dir.path().join(".ralph").join("agent").join("memories.md");
        let content = fs::read_to_string(&memories_file).unwrap();
        assert_eq!(content, "New content");
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_export_memories_returns_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        setup_memories_file(temp_dir.path(), "# Memories to Export");
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/memories/export", web::post().to(export_memories)),
            ),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/api/memories/export")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["content"], "# Memories to Export");
        assert!(json["exported_at"].is_string());
        assert!(json["filename"].as_str().unwrap().starts_with("memories-"));
        assert!(json["filename"].as_str().unwrap().ends_with(".md"));
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_export_memories_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/memories/export", web::post().to(export_memories)),
            ),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/api/memories/export")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "memories_not_found");
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_export_filename_format() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        setup_memories_file(temp_dir.path(), "Content");
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/memories/export", web::post().to(export_memories)),
            ),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/api/memories/export")
            .to_request();

        let resp = test::call_service(&app, req).await;
        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let filename = json["filename"].as_str().unwrap();
        // Should match format: memories-YYYYMMDD-HHMMSS.md
        assert!(filename.len() >= 25); // memories-20250101-000000.md
        assert!(filename.starts_with("memories-"));
        assert!(filename.ends_with(".md"));

        // Parse the date portion
        let date_part = &filename[9..17]; // YYYYMMDD
        assert!(date_part.chars().all(|c| c.is_ascii_digit()));
    }
}
