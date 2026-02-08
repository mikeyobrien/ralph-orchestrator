//! Skills API endpoints for exposing ralph-core SkillRegistry to mobile clients.
//!
//! - GET /api/skills - List all available skills
//! - GET /api/skills/{name} - Get skill metadata
//! - POST /api/skills/{name}/load - Get full skill content wrapped in XML

use actix_web::{web, HttpResponse, Responder};
use ralph_core::skill::{SkillEntry, SkillSource};
use ralph_core::skill_registry::SkillRegistry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Skill list item for API response (excludes full content).
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillListItem {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub hats: Vec<String>,
    pub backends: Vec<String>,
    pub auto_inject: bool,
    pub source: String, // "built-in" | "file"
}

impl From<&SkillEntry> for SkillListItem {
    fn from(entry: &SkillEntry) -> Self {
        Self {
            name: entry.name.clone(),
            description: entry.description.clone(),
            tags: entry.tags.clone(),
            hats: entry.hats.clone(),
            backends: entry.backends.clone(),
            auto_inject: entry.auto_inject,
            source: match &entry.source {
                SkillSource::BuiltIn => "built-in".to_string(),
                SkillSource::File(_) => "file".to_string(),
            },
        }
    }
}

/// Response for GET /api/skills.
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillsListResponse {
    pub skills: Vec<SkillListItem>,
    pub count: usize,
}

/// Response for GET /api/skills/{name}.
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillMetadataResponse {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub hats: Vec<String>,
    pub backends: Vec<String>,
    pub auto_inject: bool,
    pub source: String,
}

/// Response for POST /api/skills/{name}/load.
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillContentResponse {
    pub name: String,
    pub content: String, // XML-wrapped content
}

/// Error response for skill endpoints.
#[derive(Debug, Serialize)]
pub struct SkillErrorResponse {
    pub error: String,
}

/// GET /api/skills - List all available skills.
pub async fn list_skills(
    skill_registry: web::Data<Arc<RwLock<SkillRegistry>>>,
) -> impl Responder {
    let registry = skill_registry.read().await;
    let skills: Vec<SkillListItem> = registry
        .skills_for_hat(None)
        .into_iter()
        .map(SkillListItem::from)
        .collect();
    let count = skills.len();

    HttpResponse::Ok().json(SkillsListResponse { skills, count })
}

/// GET /api/skills/{name} - Get skill metadata.
pub async fn get_skill(
    skill_registry: web::Data<Arc<RwLock<SkillRegistry>>>,
    path: web::Path<String>,
) -> impl Responder {
    let name = path.into_inner();
    let registry = skill_registry.read().await;

    match registry.get(&name) {
        Some(skill) => {
            let response = SkillMetadataResponse {
                name: skill.name.clone(),
                description: skill.description.clone(),
                tags: skill.tags.clone(),
                hats: skill.hats.clone(),
                backends: skill.backends.clone(),
                auto_inject: skill.auto_inject,
                source: match &skill.source {
                    SkillSource::BuiltIn => "built-in".to_string(),
                    SkillSource::File(_) => "file".to_string(),
                },
            };
            HttpResponse::Ok().json(response)
        }
        None => HttpResponse::NotFound().json(SkillErrorResponse {
            error: format!("Skill '{}' not found", name),
        }),
    }
}

/// POST /api/skills/{name}/load - Get full skill content wrapped in XML.
pub async fn load_skill(
    skill_registry: web::Data<Arc<RwLock<SkillRegistry>>>,
    path: web::Path<String>,
) -> impl Responder {
    let name = path.into_inner();
    let registry = skill_registry.read().await;

    match registry.load_skill(&name) {
        Some(content) => HttpResponse::Ok().json(SkillContentResponse { name, content }),
        None => HttpResponse::NotFound().json(SkillErrorResponse {
            error: format!("Skill '{}' not found", name),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use ralph_core::SkillsConfig;
    use std::collections::HashMap;

    async fn create_test_registry() -> Arc<RwLock<SkillRegistry>> {
        let config = SkillsConfig {
            enabled: true,
            dirs: vec![],
            overrides: HashMap::new(),
        };
        let registry =
            SkillRegistry::from_config(&config, std::path::Path::new("."), None).unwrap();
        Arc::new(RwLock::new(registry))
    }

    #[actix_web::test]
    async fn test_list_skills_returns_builtins() {
        let registry = create_test_registry().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(registry))
                .route("/skills", web::get().to(list_skills)),
        )
        .await;

        let req = test::TestRequest::get().uri("/skills").to_request();
        let resp: SkillsListResponse = test::call_and_read_body_json(&app, req).await;

        // Should have at least the built-in skills
        assert!(resp.count >= 2);
        assert!(resp.skills.iter().any(|s| s.name == "ralph-tools"));
        assert!(resp.skills.iter().any(|s| s.name == "robot-interaction"));
    }

    #[actix_web::test]
    async fn test_get_skill_found() {
        let registry = create_test_registry().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(registry))
                .route("/skills/{name}", web::get().to(get_skill)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/skills/ralph-tools")
            .to_request();
        let resp: SkillMetadataResponse = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.name, "ralph-tools");
        assert_eq!(resp.source, "built-in");
    }

    #[actix_web::test]
    async fn test_get_skill_not_found() {
        let registry = create_test_registry().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(registry))
                .route("/skills/{name}", web::get().to(get_skill)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/skills/nonexistent-skill")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_load_skill_returns_xml_wrapped() {
        let registry = create_test_registry().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(registry))
                .route("/skills/{name}/load", web::post().to(load_skill)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/skills/ralph-tools/load")
            .to_request();
        let resp: SkillContentResponse = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.name, "ralph-tools");
        assert!(resp.content.starts_with("<ralph-tools-skill>"));
        assert!(resp.content.ends_with("</ralph-tools-skill>"));
    }

    #[actix_web::test]
    async fn test_load_skill_not_found() {
        let registry = create_test_registry().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(registry))
                .route("/skills/{name}/load", web::post().to(load_skill)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/skills/nonexistent/load")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);
    }
}
