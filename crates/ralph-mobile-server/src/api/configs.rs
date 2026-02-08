//! Config discovery API for Ralph Mobile.
//!
//! Provides:
//! - GET /api/configs - List available configuration files
//! - GET /api/configs/{path:.*} - Get raw content of a config file

use actix_web::{HttpResponse, web};
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::AppState;
use super::sessions::ErrorResponse;

/// A configuration file item.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ConfigItem {
    /// Path to the config file relative to project root.
    pub path: String,
    /// Name derived from filename without extension.
    pub name: String,
    /// Description extracted from first comment line, empty if none.
    pub description: String,
}

/// Response for GET /api/configs.
#[derive(Debug, Serialize)]
pub struct ConfigsResponse {
    pub configs: Vec<ConfigItem>,
}

/// Response for GET /api/configs/{path}.
#[derive(Debug, Serialize)]
pub struct ConfigContentResponse {
    pub path: String,
    pub content: String,
    pub content_type: String,
}

/// Discover configuration files in the presets directory.
///
/// Scans `presets/` for `.yml` files and extracts metadata.
pub fn discover_configs(base_path: &Path) -> Vec<ConfigItem> {
    let presets_dir = base_path.join("presets");

    if !presets_dir.exists() || !presets_dir.is_dir() {
        return Vec::new();
    }

    let mut configs = Vec::new();

    if let Ok(entries) = fs::read_dir(&presets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "yml" || e == "yaml").unwrap_or(false) {
                if let Some(config) = parse_config_file(&path, base_path) {
                    configs.push(config);
                }
            }
        }
    }

    // Sort by name for consistent ordering
    configs.sort_by(|a, b| a.name.cmp(&b.name));
    configs
}

/// Parse a config file to extract metadata.
fn parse_config_file(path: &Path, base_path: &Path) -> Option<ConfigItem> {
    let name = path.file_stem()?.to_string_lossy().to_string();
    let relative_path = path.strip_prefix(base_path).ok()?;
    let path_str = relative_path.to_string_lossy().to_string();

    let description = parse_description(path);

    Some(ConfigItem {
        path: path_str,
        name,
        description,
    })
}

/// Parse the first comment line from a file as description.
///
/// Returns empty string if no comment line found.
pub fn parse_description(path: &Path) -> String {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };

    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            // Remove the # and any leading whitespace after it
            return trimmed.trim_start_matches('#').trim().to_string();
        }
        // If first non-empty line is not a comment, no description
        if !trimmed.is_empty() {
            break;
        }
    }

    String::new()
}

/// Handler for GET /api/configs.
pub async fn list_configs(_state: web::Data<AppState>) -> HttpResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let configs = discover_configs(&cwd);

    HttpResponse::Ok().json(ConfigsResponse { configs })
}

/// Handler for GET /api/configs/{path:.*}.
///
/// Returns the raw content of a config file.
/// Path traversal is blocked for security.
pub async fn get_config_content(path: web::Path<String>) -> HttpResponse {
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
        Ok(content) => HttpResponse::Ok().json(ConfigContentResponse {
            path: file_path,
            content,
            content_type: "yaml".to_string(),
        }),
        Err(_) => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("config_not_found: {}", file_path),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{web, App};
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_presets(dir: &TempDir, files: &[(&str, &str)]) {
        let presets_dir = dir.path().join("presets");
        fs::create_dir_all(&presets_dir).unwrap();

        for (name, content) in files {
            let file_path = presets_dir.join(name);
            let mut file = fs::File::create(&file_path).unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }
    }

    #[test]
    fn test_discover_presets_directory() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("feature.yml", "# Feature Development\nmodel: claude-3"),
            ("debug.yml", "# Debug Mode\nmodel: claude-3"),
            ("minimal.yml", "model: claude-3"), // No description
        ]);

        let configs = discover_configs(temp_dir.path());

        assert_eq!(configs.len(), 3);

        // Should be sorted by name
        assert_eq!(configs[0].name, "debug");
        assert_eq!(configs[1].name, "feature");
        assert_eq!(configs[2].name, "minimal");

        // Check paths are relative
        assert!(configs[0].path.starts_with("presets/"));
        assert!(configs[0].path.ends_with(".yml"));
    }

    #[test]
    fn test_parse_config_description() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("test.yml", "# Some description\nmodel: claude-3"),
        ]);

        let configs = discover_configs(temp_dir.path());

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].description, "Some description");
    }

    #[test]
    fn test_parse_config_no_description() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("test.yml", "model: claude-3\n# This is not a description"),
        ]);

        let configs = discover_configs(temp_dir.path());

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].description, "");
    }

    #[test]
    fn test_config_item_serialization() {
        let config = ConfigItem {
            path: "presets/feature.yml".to_string(),
            name: "feature".to_string(),
            description: "Feature Development".to_string(),
        };

        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["path"], "presets/feature.yml");
        assert_eq!(json["name"], "feature");
        assert_eq!(json["description"], "Feature Development");
    }

    #[test]
    fn test_configs_response_serialization() {
        let response = ConfigsResponse {
            configs: vec![
                ConfigItem {
                    path: "presets/feature.yml".to_string(),
                    name: "feature".to_string(),
                    description: "Feature Development".to_string(),
                },
            ],
        };

        let json = serde_json::to_value(&response).unwrap();

        assert!(json["configs"].is_array());
        assert_eq!(json["configs"][0]["name"], "feature");
    }

    #[test]
    fn test_discover_empty_presets_dir() {
        let temp_dir = TempDir::new().unwrap();
        let presets_dir = temp_dir.path().join("presets");
        fs::create_dir_all(&presets_dir).unwrap();

        let configs = discover_configs(temp_dir.path());

        assert!(configs.is_empty());
    }

    #[test]
    fn test_discover_no_presets_dir() {
        let temp_dir = TempDir::new().unwrap();

        let configs = discover_configs(temp_dir.path());

        assert!(configs.is_empty());
    }

    #[test]
    fn test_parse_description_with_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("test.yml", "#    Spaced description   \nmodel: claude-3"),
        ]);

        let configs = discover_configs(temp_dir.path());

        assert_eq!(configs[0].description, "Spaced description");
    }

    #[test]
    fn test_yaml_extension_variants() {
        let temp_dir = TempDir::new().unwrap();
        let presets_dir = temp_dir.path().join("presets");
        fs::create_dir_all(&presets_dir).unwrap();

        // Create both .yml and .yaml files
        fs::write(presets_dir.join("config1.yml"), "# YML file\n").unwrap();
        fs::write(presets_dir.join("config2.yaml"), "# YAML file\n").unwrap();
        fs::write(presets_dir.join("readme.txt"), "Not a config\n").unwrap();

        let configs = discover_configs(temp_dir.path());

        assert_eq!(configs.len(), 2);
        assert!(configs.iter().any(|c| c.name == "config1"));
        assert!(configs.iter().any(|c| c.name == "config2"));
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
    async fn test_list_configs_returns_json() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/configs", web::get().to(list_configs))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/configs").to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));
    }

    #[actix_web::test]
    async fn test_list_configs_has_configs_array() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/configs", web::get().to(list_configs))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/configs").to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["configs"].is_array());
    }

    #[actix_web::test]
    async fn test_get_config_content_path_traversal_blocked() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/configs/{path:.*}", web::get().to(get_config_content)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/configs/../../../etc/passwd")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 400);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("path traversal"));
    }

    #[actix_web::test]
    async fn test_get_config_content_not_found() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/configs/{path:.*}", web::get().to(get_config_content)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/configs/nonexistent.yml")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("config_not_found"));
    }
}
