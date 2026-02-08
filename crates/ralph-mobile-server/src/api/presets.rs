//! Presets API for Ralph Mobile.
//!
//! Provides:
//! - GET /api/presets - List available preset files

use actix_web::{HttpResponse, web};
use serde::Serialize;
use std::fs;
use std::path::Path;

use super::AppState;

/// A preset file item.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct PresetItem {
    /// Name derived from filename without extension.
    pub name: String,
    /// Path to the preset file relative to project root.
    pub path: String,
    /// Description extracted from first comment line, empty if none.
    pub description: String,
}

/// Response for GET /api/presets.
#[derive(Debug, Serialize)]
pub struct PresetsResponse {
    pub presets: Vec<PresetItem>,
}

/// Discover preset files in the presets directory.
///
/// Scans `presets/` for `.yml` and `.yaml` files and extracts metadata.
pub fn discover_presets(base_path: &Path) -> Vec<PresetItem> {
    let presets_dir = base_path.join("presets");

    if !presets_dir.exists() || !presets_dir.is_dir() {
        return Vec::new();
    }

    let mut presets = Vec::new();

    if let Ok(entries) = fs::read_dir(&presets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "yml" || e == "yaml").unwrap_or(false) {
                if let Some(preset) = parse_preset_file(&path, base_path) {
                    presets.push(preset);
                }
            }
        }
    }

    // Sort by name for consistent ordering
    presets.sort_by(|a, b| a.name.cmp(&b.name));
    presets
}

/// Parse a preset file to extract metadata.
fn parse_preset_file(path: &Path, base_path: &Path) -> Option<PresetItem> {
    let name = path.file_stem()?.to_string_lossy().to_string();
    let relative_path = path.strip_prefix(base_path).ok()?;
    let path_str = relative_path.to_string_lossy().to_string();

    let description = super::configs::parse_description(path);

    Some(PresetItem {
        name,
        path: path_str,
        description,
    })
}

/// Handler for GET /api/presets.
pub async fn list_presets(_state: web::Data<AppState>) -> HttpResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let presets = discover_presets(&cwd);

    HttpResponse::Ok().json(PresetsResponse { presets })
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

        let presets = discover_presets(temp_dir.path());

        assert_eq!(presets.len(), 3);

        // Should be sorted by name
        assert_eq!(presets[0].name, "debug");
        assert_eq!(presets[1].name, "feature");
        assert_eq!(presets[2].name, "minimal");

        // Check paths are relative
        assert!(presets[0].path.starts_with("presets/"));
        assert!(presets[0].path.ends_with(".yml"));
    }

    #[test]
    fn test_parse_preset_description() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("test.yml", "# Some description\nmodel: claude-3"),
        ]);

        let presets = discover_presets(temp_dir.path());

        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].description, "Some description");
    }

    #[test]
    fn test_parse_preset_no_description() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("test.yml", "model: claude-3\n# This is not a description"),
        ]);

        let presets = discover_presets(temp_dir.path());

        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].description, "");
    }

    #[test]
    fn test_preset_item_serialization() {
        let preset = PresetItem {
            name: "feature".to_string(),
            path: "presets/feature.yml".to_string(),
            description: "Feature Development".to_string(),
        };

        let json = serde_json::to_value(&preset).unwrap();

        assert_eq!(json["name"], "feature");
        assert_eq!(json["path"], "presets/feature.yml");
        assert_eq!(json["description"], "Feature Development");
    }

    #[test]
    fn test_presets_response_serialization() {
        let response = PresetsResponse {
            presets: vec![
                PresetItem {
                    name: "feature".to_string(),
                    path: "presets/feature.yml".to_string(),
                    description: "Feature Development".to_string(),
                },
            ],
        };

        let json = serde_json::to_value(&response).unwrap();

        assert!(json["presets"].is_array());
        assert_eq!(json["presets"][0]["name"], "feature");
    }

    #[test]
    fn test_discover_empty_presets_dir() {
        let temp_dir = TempDir::new().unwrap();
        let presets_dir = temp_dir.path().join("presets");
        fs::create_dir_all(&presets_dir).unwrap();

        let presets = discover_presets(temp_dir.path());

        assert!(presets.is_empty());
    }

    #[test]
    fn test_discover_no_presets_dir() {
        let temp_dir = TempDir::new().unwrap();

        let presets = discover_presets(temp_dir.path());

        assert!(presets.is_empty());
    }

    #[test]
    fn test_parse_description_with_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("test.yml", "#    Spaced description   \nmodel: claude-3"),
        ]);

        let presets = discover_presets(temp_dir.path());

        assert_eq!(presets[0].description, "Spaced description");
    }

    #[test]
    fn test_yaml_extension_variants() {
        let temp_dir = TempDir::new().unwrap();
        let presets_dir = temp_dir.path().join("presets");
        fs::create_dir_all(&presets_dir).unwrap();

        // Create both .yml and .yaml files
        fs::write(presets_dir.join("preset1.yml"), "# YML file\n").unwrap();
        fs::write(presets_dir.join("preset2.yaml"), "# YAML file\n").unwrap();
        fs::write(presets_dir.join("readme.txt"), "Not a preset\n").unwrap();

        let presets = discover_presets(temp_dir.path());

        assert_eq!(presets.len(), 2);
        assert!(presets.iter().any(|p| p.name == "preset1"));
        assert!(presets.iter().any(|p| p.name == "preset2"));
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
    async fn test_list_presets_returns_json() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/presets", web::get().to(list_presets))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/presets").to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));
    }

    #[actix_web::test]
    async fn test_list_presets_has_presets_array() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/presets", web::get().to(list_presets))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/presets").to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["presets"].is_array());
    }

    #[actix_web::test]
    async fn test_list_presets_items_have_required_fields() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/presets", web::get().to(list_presets))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/presets").to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // If there are presets, they should have name, path, and description
        if let Some(presets) = json["presets"].as_array() {
            for preset in presets {
                assert!(preset.get("name").is_some());
                assert!(preset.get("path").is_some());
                assert!(preset.get("description").is_some());
            }
        }
    }
}
