//! Prompt discovery API for Ralph Mobile.
//!
//! Provides:
//! - GET /api/prompts - List available prompt files
//! - GET /api/prompts/{path:.*} - Get raw content of a prompt file

use actix_web::{HttpResponse, web};
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::AppState;
use super::sessions::ErrorResponse;

/// Maximum characters for preview text.
const MAX_PREVIEW_LENGTH: usize = 50;

/// A prompt file item.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct PromptItem {
    /// Path to the prompt file relative to project root.
    pub path: String,
    /// Name derived from filename without extension.
    pub name: String,
    /// Preview extracted from first line, truncated to 50 characters.
    pub preview: String,
}

/// Response for GET /api/prompts.
#[derive(Debug, Serialize)]
pub struct PromptsResponse {
    pub prompts: Vec<PromptItem>,
}

/// Response for GET /api/prompts/{path}.
#[derive(Debug, Serialize)]
pub struct PromptContentResponse {
    pub path: String,
    pub content: String,
    pub content_type: String,
}

/// Discover prompt files in the prompts directory.
///
/// Recursively scans `prompts/` and subdirectories for `.md` files and extracts metadata.
pub fn discover_prompts(base_path: &Path) -> Vec<PromptItem> {
    let prompts_dir = base_path.join("prompts");

    if !prompts_dir.exists() || !prompts_dir.is_dir() {
        return Vec::new();
    }

    let mut prompts = Vec::new();
    discover_prompts_recursive(&prompts_dir, base_path, &mut prompts);

    // Sort by path for consistent ordering (includes subdirectory structure)
    prompts.sort_by(|a, b| a.path.cmp(&b.path));
    prompts
}

/// Recursively discover prompt files in a directory and its subdirectories.
fn discover_prompts_recursive(dir: &Path, base_path: &Path, prompts: &mut Vec<PromptItem>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Recurse into subdirectories
                discover_prompts_recursive(&path, base_path, prompts);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(prompt) = parse_prompt_file(&path, base_path) {
                    prompts.push(prompt);
                }
            }
        }
    }
}

/// Parse a prompt file to extract metadata.
fn parse_prompt_file(path: &Path, base_path: &Path) -> Option<PromptItem> {
    let name = path.file_stem()?.to_string_lossy().to_string();
    let relative_path = path.strip_prefix(base_path).ok()?;
    let path_str = relative_path.to_string_lossy().to_string();

    let preview = parse_preview(path);

    Some(PromptItem {
        path: path_str,
        name,
        preview,
    })
}

/// Parse the first line from a file as preview.
///
/// Returns empty string if file is empty or unreadable.
/// Truncates to 50 characters with "..." suffix if longer.
pub fn parse_preview(path: &Path) -> String {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };

    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return truncate_preview(trimmed);
        }
    }

    String::new()
}

/// Truncate text to MAX_PREVIEW_LENGTH characters, adding "..." if truncated.
fn truncate_preview(text: &str) -> String {
    if text.len() <= MAX_PREVIEW_LENGTH {
        text.to_string()
    } else {
        // Find a safe truncation point (don't cut in middle of multi-byte char)
        let mut end = MAX_PREVIEW_LENGTH - 3; // Reserve space for "..."
        while !text.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &text[..end])
    }
}

/// Handler for GET /api/prompts.
pub async fn list_prompts(_state: web::Data<AppState>) -> HttpResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let prompts = discover_prompts(&cwd);

    HttpResponse::Ok().json(PromptsResponse { prompts })
}

/// Handler for GET /api/prompts/{path:.*}.
///
/// Returns the raw content of a prompt file.
/// Path traversal is blocked for security.
pub async fn get_prompt_content(path: web::Path<String>) -> HttpResponse {
    let file_path = path.into_inner();

    // Path traversal protection
    if file_path.contains("..") {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "invalid_path: path traversal not allowed".to_string(),
        });
    }

    // Empty path check
    if file_path.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "invalid_path: path cannot be empty".to_string(),
        });
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let full_path = cwd.join(&file_path);

    // Read file content
    match fs::read_to_string(&full_path) {
        Ok(content) => HttpResponse::Ok().json(PromptContentResponse {
            path: file_path,
            content,
            content_type: "markdown".to_string(),
        }),
        Err(_) => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("prompt_not_found: {}", file_path),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{web, App};
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_prompts(dir: &TempDir, files: &[(&str, &str)]) {
        let prompts_dir = dir.path().join("prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        for (name, content) in files {
            let file_path = prompts_dir.join(name);
            let mut file = fs::File::create(&file_path).unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }
    }

    #[test]
    fn test_discover_prompts_directory() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(&temp_dir, &[
            ("add-auth.md", "Add user authentication\nMore details here"),
            ("fix-bug.md", "Fix the login bug\nSteps to reproduce"),
            ("refactor.md", "Refactor the database layer"),
        ]);

        let prompts = discover_prompts(temp_dir.path());

        assert_eq!(prompts.len(), 3);

        // Should be sorted by name
        assert_eq!(prompts[0].name, "add-auth");
        assert_eq!(prompts[1].name, "fix-bug");
        assert_eq!(prompts[2].name, "refactor");

        // Check paths are relative
        assert!(prompts[0].path.starts_with("prompts/"));
        assert!(prompts[0].path.ends_with(".md"));
    }

    #[test]
    fn test_parse_prompt_preview() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(&temp_dir, &[
            ("test.md", "Add user authentication\nMore details"),
        ]);

        let prompts = discover_prompts(temp_dir.path());

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].preview, "Add user authentication");
    }

    #[test]
    fn test_parse_prompt_preview_truncation() {
        let temp_dir = TempDir::new().unwrap();
        // Create a first line longer than 50 characters
        let long_line = "Add user authentication with OAuth2 and JWT tokens for the API";
        create_test_prompts(&temp_dir, &[
            ("test.md", &format!("{}\nMore details", long_line)),
        ]);

        let prompts = discover_prompts(temp_dir.path());

        assert_eq!(prompts.len(), 1);
        // Should be truncated to 50 chars with "..."
        assert_eq!(prompts[0].preview, "Add user authentication with OAuth2 and JWT tok...");
        assert!(prompts[0].preview.len() <= MAX_PREVIEW_LENGTH);
    }

    #[test]
    fn test_parse_prompt_preview_short() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(&temp_dir, &[
            ("test.md", "Fix bug\nMore details"),
        ]);

        let prompts = discover_prompts(temp_dir.path());

        assert_eq!(prompts.len(), 1);
        // Short line should not be truncated
        assert_eq!(prompts[0].preview, "Fix bug");
    }

    #[test]
    fn test_parse_prompt_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(&temp_dir, &[
            ("empty.md", ""),
        ]);

        let prompts = discover_prompts(temp_dir.path());

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].preview, "");
    }

    #[test]
    fn test_parse_prompt_whitespace_first_line() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(&temp_dir, &[
            ("test.md", "   \n  \nActual content here"),
        ]);

        let prompts = discover_prompts(temp_dir.path());

        assert_eq!(prompts.len(), 1);
        // Should skip empty/whitespace lines and get first real content
        assert_eq!(prompts[0].preview, "Actual content here");
    }

    #[test]
    fn test_prompt_item_serialization() {
        let prompt = PromptItem {
            path: "prompts/add-auth.md".to_string(),
            name: "add-auth".to_string(),
            preview: "Add user authentication".to_string(),
        };

        let json = serde_json::to_value(&prompt).unwrap();

        assert_eq!(json["path"], "prompts/add-auth.md");
        assert_eq!(json["name"], "add-auth");
        assert_eq!(json["preview"], "Add user authentication");
    }

    #[test]
    fn test_prompts_response_serialization() {
        let response = PromptsResponse {
            prompts: vec![
                PromptItem {
                    path: "prompts/add-auth.md".to_string(),
                    name: "add-auth".to_string(),
                    preview: "Add user authentication".to_string(),
                },
            ],
        };

        let json = serde_json::to_value(&response).unwrap();

        assert!(json["prompts"].is_array());
        assert_eq!(json["prompts"][0]["name"], "add-auth");
    }

    #[test]
    fn test_discover_empty_prompts_dir() {
        let temp_dir = TempDir::new().unwrap();
        let prompts_dir = temp_dir.path().join("prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        let prompts = discover_prompts(temp_dir.path());

        assert!(prompts.is_empty());
    }

    #[test]
    fn test_discover_no_prompts_dir() {
        let temp_dir = TempDir::new().unwrap();

        let prompts = discover_prompts(temp_dir.path());

        assert!(prompts.is_empty());
    }

    #[test]
    fn test_ignores_non_md_files() {
        let temp_dir = TempDir::new().unwrap();
        let prompts_dir = temp_dir.path().join("prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        // Create mixed file types
        fs::write(prompts_dir.join("prompt1.md"), "Valid prompt\n").unwrap();
        fs::write(prompts_dir.join("notes.txt"), "Not a prompt\n").unwrap();
        fs::write(prompts_dir.join("config.yml"), "Also not a prompt\n").unwrap();

        let prompts = discover_prompts(temp_dir.path());

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "prompt1");
    }

    #[test]
    fn test_truncate_preview_exact_boundary() {
        // Test exactly 50 characters - should not truncate
        let text = "A".repeat(50);
        assert_eq!(truncate_preview(&text), text);
        assert_eq!(truncate_preview(&text).len(), 50);

        // Test 51 characters - should truncate
        let text = "A".repeat(51);
        let result = truncate_preview(&text);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 50);
    }

    // Functional API tests

    fn create_test_app_state() -> AppState {
        use std::sync::Arc;
        use tokio::sync::RwLock;
        use std::collections::HashMap;

        AppState {
            sessions: vec![],
            watchers: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[actix_web::test]
    async fn test_list_prompts_returns_json() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/prompts", web::get().to(list_prompts))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/prompts").to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));
    }

    #[actix_web::test]
    async fn test_list_prompts_has_prompts_array() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/prompts", web::get().to(list_prompts))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/prompts").to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["prompts"].is_array());
    }

    #[actix_web::test]
    async fn test_get_prompt_content_path_traversal_blocked() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/prompts/{path:.*}", web::get().to(get_prompt_content)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/prompts/../../../etc/passwd")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 400);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("path traversal"));
    }

    #[actix_web::test]
    async fn test_get_prompt_content_not_found() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/prompts/{path:.*}", web::get().to(get_prompt_content)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/prompts/nonexistent.md")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("prompt_not_found"));
    }
}
