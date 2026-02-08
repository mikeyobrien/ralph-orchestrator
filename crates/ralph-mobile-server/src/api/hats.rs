//! Hats discovery API for Ralph Mobile.
//!
//! Provides:
//! - GET /api/hats - List available hats from preset configurations

use actix_web::{HttpResponse, web};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::AppState;

/// A hat definition extracted from a preset file.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct HatItem {
    /// Name of the hat (e.g., "builder", "reviewer").
    pub name: String,
    /// Description of the hat's role.
    pub description: String,
    /// Emoji representing the hat (extracted or default).
    pub emoji: String,
}

/// Response for GET /api/hats.
#[derive(Debug, Serialize)]
pub struct HatsResponse {
    pub hats: Vec<HatItem>,
}

/// Partial YAML structure for parsing preset files.
#[derive(Debug, Deserialize)]
struct PresetConfig {
    #[serde(default)]
    hats: HashMap<String, HatDefinition>,
}

#[derive(Debug, Deserialize)]
struct HatDefinition {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

/// Discover hats from preset configuration files.
///
/// Scans `presets/` for `.yml` files and extracts hat definitions.
pub fn discover_hats(base_path: &Path) -> Vec<HatItem> {
    let presets_dir = base_path.join("presets");

    if !presets_dir.exists() || !presets_dir.is_dir() {
        return Vec::new();
    }

    let mut hats_map: HashMap<String, HatItem> = HashMap::new();

    if let Ok(entries) = fs::read_dir(&presets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "yml" || e == "yaml").unwrap_or(false) {
                if let Some(parsed_hats) = parse_hats_from_preset(&path) {
                    for hat in parsed_hats {
                        // Use entry API to avoid duplicates, keep first occurrence
                        hats_map.entry(hat.name.clone()).or_insert(hat);
                    }
                }
            }
        }
    }

    let mut hats: Vec<HatItem> = hats_map.into_values().collect();

    // Sort by name for consistent ordering
    hats.sort_by(|a, b| a.name.cmp(&b.name));
    hats
}

/// Parse hat definitions from a preset YAML file.
fn parse_hats_from_preset(path: &Path) -> Option<Vec<HatItem>> {
    let content = fs::read_to_string(path).ok()?;
    let config: PresetConfig = serde_yaml::from_str(&content).ok()?;

    let mut hats = Vec::new();

    for (hat_key, hat_def) in config.hats {
        let emoji = extract_emoji(&hat_key);

        hats.push(HatItem {
            name: if hat_def.name.is_empty() {
                hat_key.clone()
            } else {
                hat_def.name
            },
            description: hat_def.description,
            emoji,
        });
    }

    Some(hats)
}

/// Extract or generate emoji for a hat based on its name.
fn extract_emoji(hat_name: &str) -> String {
    // Map common hat names to emojis
    match hat_name.to_lowercase().as_str() {
        "builder" => "🏗️",
        "reviewer" => "👀",
        "investigator" => "🔍",
        "tester" => "🧪",
        "fixer" => "🔧",
        "verifier" => "✅",
        "planner" => "📋",
        "architect" => "🏛️",
        "deployer" => "🚀",
        "analyst" => "📊",
        "researcher" => "📚",
        "writer" => "✍️",
        "designer" => "🎨",
        "security" => "🔒",
        "coordinator" => "🎯",
        _ => "🎩", // Default hat emoji
    }
    .to_string()
}

/// Handler for GET /api/hats.
pub async fn list_hats(_state: web::Data<AppState>) -> HttpResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let hats = discover_hats(&cwd);

    HttpResponse::Ok().json(HatsResponse { hats })
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
    fn test_discover_hats_from_presets() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("feature.yml", r#"
hats:
  builder:
    name: "Builder"
    description: "Implements one task with quality gates"
  reviewer:
    name: "Reviewer"
    description: "Reviews implementation for quality"
"#),
            ("debug.yml", r#"
hats:
  investigator:
    name: "Investigator"
    description: "Finds root cause through systematic investigation"
  tester:
    name: "Hypothesis Tester"
    description: "Designs and runs experiments"
"#),
        ]);

        let hats = discover_hats(temp_dir.path());

        assert_eq!(hats.len(), 4);

        // Should be sorted by name
        let names: Vec<&str> = hats.iter().map(|h| h.name.as_str()).collect();
        assert!(names.contains(&"Builder"));
        assert!(names.contains(&"Reviewer"));
        assert!(names.contains(&"Investigator"));
        assert!(names.contains(&"Hypothesis Tester"));
    }

    #[test]
    fn test_discover_hats_with_duplicates() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("config1.yml", r#"
hats:
  builder:
    name: "Builder"
    description: "First definition"
"#),
            ("config2.yml", r#"
hats:
  builder:
    name: "Builder"
    description: "Second definition"
"#),
        ]);

        let hats = discover_hats(temp_dir.path());

        // Should only have one builder (first occurrence)
        assert_eq!(hats.len(), 1);
        assert_eq!(hats[0].name, "Builder");
    }

    #[test]
    fn test_extract_emoji_known_hats() {
        assert_eq!(extract_emoji("builder"), "🏗️");
        assert_eq!(extract_emoji("reviewer"), "👀");
        assert_eq!(extract_emoji("investigator"), "🔍");
        assert_eq!(extract_emoji("tester"), "🧪");
        assert_eq!(extract_emoji("fixer"), "🔧");
        assert_eq!(extract_emoji("verifier"), "✅");
    }

    #[test]
    fn test_extract_emoji_unknown_hat() {
        assert_eq!(extract_emoji("custom_hat"), "🎩");
        assert_eq!(extract_emoji("unknown"), "🎩");
    }

    #[test]
    fn test_hat_item_serialization() {
        let hat = HatItem {
            name: "Builder".to_string(),
            description: "Implements features".to_string(),
            emoji: "🏗️".to_string(),
        };

        let json = serde_json::to_value(&hat).unwrap();

        assert_eq!(json["name"], "Builder");
        assert_eq!(json["description"], "Implements features");
        assert_eq!(json["emoji"], "🏗️");
    }

    #[test]
    fn test_hats_response_serialization() {
        let response = HatsResponse {
            hats: vec![
                HatItem {
                    name: "Builder".to_string(),
                    description: "Implements features".to_string(),
                    emoji: "🏗️".to_string(),
                },
                HatItem {
                    name: "Reviewer".to_string(),
                    description: "Reviews code".to_string(),
                    emoji: "👀".to_string(),
                },
            ],
        };

        let json = serde_json::to_value(&response).unwrap();

        assert!(json["hats"].is_array());
        assert_eq!(json["hats"].as_array().unwrap().len(), 2);
        assert_eq!(json["hats"][0]["name"], "Builder");
        assert_eq!(json["hats"][1]["name"], "Reviewer");
    }

    #[test]
    fn test_discover_empty_presets_dir() {
        let temp_dir = TempDir::new().unwrap();
        let presets_dir = temp_dir.path().join("presets");
        fs::create_dir_all(&presets_dir).unwrap();

        let hats = discover_hats(temp_dir.path());

        assert!(hats.is_empty());
    }

    #[test]
    fn test_discover_no_presets_dir() {
        let temp_dir = TempDir::new().unwrap();

        let hats = discover_hats(temp_dir.path());

        assert!(hats.is_empty());
    }

    #[test]
    fn test_parse_preset_without_hats() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("no-hats.yml", r#"
event_loop:
  prompt_file: "PROMPT.md"
cli:
  backend: "claude"
"#),
        ]);

        let hats = discover_hats(temp_dir.path());

        assert!(hats.is_empty());
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("invalid.yml", "not: valid: yaml: content"),
        ]);

        let hats = discover_hats(temp_dir.path());

        // Should handle gracefully and return empty
        assert!(hats.is_empty());
    }

    #[test]
    fn test_hat_name_fallback() {
        let temp_dir = TempDir::new().unwrap();
        create_test_presets(&temp_dir, &[
            ("test.yml", r#"
hats:
  my_custom_hat:
    description: "A custom hat without explicit name"
"#),
        ]);

        let hats = discover_hats(temp_dir.path());

        assert_eq!(hats.len(), 1);
        assert_eq!(hats[0].name, "my_custom_hat");
        assert_eq!(hats[0].description, "A custom hat without explicit name");
    }

    #[test]
    fn test_yaml_extension_variants() {
        let temp_dir = TempDir::new().unwrap();
        let presets_dir = temp_dir.path().join("presets");
        fs::create_dir_all(&presets_dir).unwrap();

        // Create both .yml and .yaml files
        fs::write(presets_dir.join("config1.yml"), r#"
hats:
  builder:
    name: "Builder"
    description: "From yml file"
"#).unwrap();

        fs::write(presets_dir.join("config2.yaml"), r#"
hats:
  reviewer:
    name: "Reviewer"
    description: "From yaml file"
"#).unwrap();

        fs::write(presets_dir.join("readme.txt"), "Not a config\n").unwrap();

        let hats = discover_hats(temp_dir.path());

        assert_eq!(hats.len(), 2);
        assert!(hats.iter().any(|h| h.name == "Builder"));
        assert!(hats.iter().any(|h| h.name == "Reviewer"));
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
    async fn test_list_hats_returns_json() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/hats", web::get().to(list_hats))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/hats").to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));
    }

    #[actix_web::test]
    async fn test_list_hats_has_hats_array() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/hats", web::get().to(list_hats))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/hats").to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["hats"].is_array());
    }

    #[actix_web::test]
    async fn test_list_hats_items_have_required_fields() {
        let state = create_test_app_state();

        let app = actix_web::test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .service(web::scope("/api").route("/hats", web::get().to(list_hats))),
        )
        .await;

        let req = actix_web::test::TestRequest::get().uri("/api/hats").to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // If there are hats, they should have name, description, and emoji
        if let Some(hats) = json["hats"].as_array() {
            for hat in hats {
                assert!(hat.get("name").is_some());
                assert!(hat.get("description").is_some());
                assert!(hat.get("emoji").is_some());
            }
        }
    }
}
