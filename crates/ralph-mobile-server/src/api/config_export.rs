//! Configuration export/import API endpoints.
//!
//! POST /api/config/export - Export current ralph configuration as YAML
//! POST /api/config/import - Import and validate YAML configuration

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::fs;

use super::sessions::ErrorResponse;

/// Response for POST /api/config/export.
#[derive(Debug, Serialize)]
pub struct ExportConfigResponse {
    /// The YAML configuration content.
    pub content: String,
    /// The filename for download.
    pub filename: String,
}

/// Request body for POST /api/config/import.
#[derive(Debug, Deserialize)]
pub struct ImportConfigRequest {
    /// YAML configuration content to import.
    pub content: String,
}

/// Response for POST /api/config/import.
#[derive(Debug, Serialize)]
pub struct ImportConfigResponse {
    /// Status message.
    pub status: String,
    /// Path where the config was saved.
    pub path: String,
}

/// Find the ralph configuration file in the current directory.
///
/// Looks for ralph.yml or ralph.yaml in order of preference.
fn find_config_file() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;

    // Check ralph.yml first, then ralph.yaml
    for filename in &["ralph.yml", "ralph.yaml"] {
        let path = cwd.join(filename);
        if path.exists() {
            return Some(filename.to_string());
        }
    }

    None
}

/// Validate YAML content by attempting to parse it.
///
/// Returns Ok(()) if valid, Err(message) if invalid.
fn validate_yaml(content: &str) -> Result<(), String> {
    serde_yaml::from_str::<serde_yaml::Value>(content)
        .map_err(|e| format!("Invalid YAML: {}", e))?;
    Ok(())
}

/// Handler for POST /api/config/export.
///
/// Returns the current ralph configuration as downloadable YAML.
pub async fn export_config() -> impl Responder {
    let cwd = std::env::current_dir().unwrap_or_default();

    // Find existing config file
    let config_filename = match find_config_file() {
        Some(name) => name,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "No ralph configuration file found (ralph.yml or ralph.yaml)".to_string(),
            });
        }
    };

    let config_path = cwd.join(&config_filename);

    // Read config content
    match fs::read_to_string(&config_path) {
        Ok(content) => {
            // Return as JSON with content and filename
            HttpResponse::Ok().json(ExportConfigResponse {
                content,
                filename: config_filename,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to read configuration: {}", e),
        }),
    }
}

/// Handler for POST /api/config/import.
///
/// Accepts YAML configuration upload, validates it, and saves to ralph.yml.
pub async fn import_config(req: web::Json<ImportConfigRequest>) -> impl Responder {
    let content = &req.content;

    // Validate YAML structure
    if let Err(err) = validate_yaml(content) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: err,
        });
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let config_path = cwd.join("ralph.yml");

    // Save to ralph.yml
    match fs::write(&config_path, content) {
        Ok(_) => HttpResponse::Ok().json(ImportConfigResponse {
            status: "Configuration imported successfully".to_string(),
            path: "ralph.yml".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("Failed to write configuration: {}", e),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::App;
    use tempfile::TempDir;

    #[test]
    fn test_validate_yaml_valid() {
        let yaml = r#"
model: claude-3-opus
max_iterations: 10
        "#;
        assert!(validate_yaml(yaml).is_ok());
    }

    #[test]
    fn test_validate_yaml_invalid() {
        let yaml = r#"
invalid: [unclosed
        "#;
        assert!(validate_yaml(yaml).is_err());
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_find_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });

        // Change to temp directory
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // No config file exists
        assert!(find_config_file().is_none());

        // Create ralph.yml
        fs::write(temp_dir.path().join("ralph.yml"), "model: test").unwrap();
        assert_eq!(find_config_file(), Some("ralph.yml".to_string()));
    }

    #[actix_web::test]
    #[serial_test::serial(cwd)]
    async fn test_export_config_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Ensure cleanup happens even if test panics
        let _guard = scopeguard::guard(original_dir.clone(), |dir| {
            let _ = std::env::set_current_dir(dir);
        });

        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/config/export", web::post().to(export_config)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri("/api/config/export")
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    #[serial_test::serial(cwd)]
    async fn test_export_config_success() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Ensure cleanup happens even if test panics
        let _guard = scopeguard::guard(original_dir.clone(), |dir| {
            let _ = std::env::set_current_dir(dir);
        });

        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create a test config file
        let config_content = "model: claude-3-opus\nmax_iterations: 10\n";
        fs::write(temp_dir.path().join("ralph.yml"), config_content).unwrap();

        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/config/export", web::post().to(export_config)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::post()
            .uri("/api/config/export")
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["content"], config_content);
        assert_eq!(json["filename"], "ralph.yml");
    }

    #[actix_web::test]
    async fn test_import_config_invalid_yaml() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/config/import", web::post().to(import_config)),
            ),
        )
        .await;

        let invalid_yaml = r#"{"content": "invalid: [unclosed"}"#;

        let req = actix_web::test::TestRequest::post()
            .uri("/api/config/import")
            .insert_header(("content-type", "application/json"))
            .set_payload(invalid_yaml)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("Invalid YAML"));
    }

    #[actix_web::test]
    #[serial_test::serial(cwd)]
    async fn test_import_config_success() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Ensure cleanup happens even if test panics
        let _guard = scopeguard::guard(original_dir.clone(), |dir| {
            let _ = std::env::set_current_dir(dir);
        });

        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/config/import", web::post().to(import_config)),
            ),
        )
        .await;

        let valid_yaml = r#"{"content": "model: claude-3-opus\nmax_iterations: 10\n"}"#;

        let req = actix_web::test::TestRequest::post()
            .uri("/api/config/import")
            .insert_header(("content-type", "application/json"))
            .set_payload(valid_yaml)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "Configuration imported successfully");
        assert_eq!(json["path"], "ralph.yml");

        // Verify file was created
        let config_path = temp_dir.path().join("ralph.yml");
        assert!(config_path.exists());
        let content = fs::read_to_string(config_path).unwrap();
        assert_eq!(content, "model: claude-3-opus\nmax_iterations: 10\n");
    }

}
