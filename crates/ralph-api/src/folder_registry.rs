// ABOUTME: Registry of folder domains, each backed by its own RpcRuntime.
// ABOUTME: Routes RPC requests to the correct runtime based on folder slug.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

use crate::config::ApiConfig;
use crate::errors::ApiError;
use crate::runtime::RpcRuntime;
use crate::supervisor::ProcessSupervisor;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDomain {
    pub slug: String,
    pub path: PathBuf,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedFolders {
    folders: Vec<PersistedFolder>,
    default_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedFolder {
    slug: String,
    path: PathBuf,
}

pub struct FolderRegistry {
    folders: HashMap<String, FolderDomain>,
    runtimes: HashMap<String, RpcRuntime>,
    default_slug: Option<String>,
    persist_path: PathBuf,
    api_port: u16,
}

impl FolderRegistry {
    pub fn new(persist_path: PathBuf, api_port: u16) -> Self {
        Self {
            folders: HashMap::new(),
            runtimes: HashMap::new(),
            default_slug: None,
            persist_path,
            api_port,
        }
    }

    pub fn load(persist_path: &Path, api_port: u16) -> anyhow::Result<Self> {
        let mut registry = Self::new(persist_path.to_path_buf(), api_port);

        if persist_path.exists() {
            let content = std::fs::read_to_string(persist_path)?;
            let persisted: PersistedFolders = serde_json::from_str(&content)?;

            for folder in &persisted.folders {
                if folder.path.exists() {
                    if let Err(e) = registry.register(folder.path.clone()) {
                        tracing::warn!(
                            path = %folder.path.display(),
                            error = %e,
                            "skipping persisted folder that failed to register"
                        );
                    }
                }
            }

            if let Some(ref default) = persisted.default_slug {
                if registry.folders.contains_key(default) {
                    registry.default_slug = Some(default.clone());
                }
            }
        }

        Ok(registry)
    }

    pub fn register(&mut self, path: PathBuf) -> anyhow::Result<FolderDomain> {
        let path = path.canonicalize().map_err(|e| {
            anyhow::anyhow!("folder path '{}' does not exist: {e}", path.display())
        })?;

        // Check if already registered by path
        for folder in self.folders.values() {
            if folder.path == path {
                return Ok(folder.clone());
            }
        }

        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let slug = derive_slug(&display_name, &self.folders);

        let config = ApiConfig {
            port: self.api_port,
            workspace_root: path.clone(),
            ..ApiConfig::default()
        };

        let supervisor = ProcessSupervisor::new(path.clone(), self.api_port);
        let runtime = RpcRuntime::with_supervisor(config, supervisor)?;

        let folder = FolderDomain {
            slug: slug.clone(),
            path,
            display_name,
        };

        info!(slug = %slug, path = %folder.path.display(), "registered folder domain");

        self.folders.insert(slug.clone(), folder.clone());
        self.runtimes.insert(slug.clone(), runtime);

        if self.default_slug.is_none() {
            self.default_slug = Some(slug);
        }

        self.save()?;
        Ok(folder)
    }

    pub fn unregister(&mut self, slug: &str) -> Result<(), ApiError> {
        if !self.folders.contains_key(slug) {
            return Err(ApiError::not_found(format!("folder '{slug}' not found")));
        }

        self.folders.remove(slug);
        self.runtimes.remove(slug);

        if self.default_slug.as_deref() == Some(slug) {
            self.default_slug = self.folders.keys().next().cloned();
        }

        info!(slug = %slug, "unregistered folder domain");

        if let Err(e) = self.save() {
            tracing::warn!(error = %e, "failed to persist folder registry after unregister");
        }

        Ok(())
    }

    pub fn list(&self) -> Vec<FolderDomain> {
        let mut folders: Vec<FolderDomain> = self.folders.values().cloned().collect();
        folders.sort_by(|a, b| a.slug.cmp(&b.slug));
        folders
    }

    pub fn resolve(&self, slug: Option<&str>) -> Result<&RpcRuntime, ApiError> {
        let effective_slug = match slug {
            Some(s) => s,
            None => self
                .default_slug
                .as_deref()
                .ok_or_else(|| ApiError::not_found("no folders registered"))?,
        };

        self.runtimes
            .get(effective_slug)
            .ok_or_else(|| ApiError::not_found(format!("folder '{effective_slug}' not found")))
    }

    pub fn default_slug(&self) -> Option<&str> {
        self.default_slug.as_deref()
    }

    pub fn set_default(&mut self, slug: &str) -> Result<(), ApiError> {
        if !self.folders.contains_key(slug) {
            return Err(ApiError::not_found(format!("folder '{slug}' not found")));
        }
        self.default_slug = Some(slug.to_string());
        if let Err(e) = self.save() {
            tracing::warn!(error = %e, "failed to persist folder registry after set_default");
        }
        Ok(())
    }

    pub fn dispatch_folder_method(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, ApiError> {
        match method {
            "folder.list" => {
                let folders = self.list();
                Ok(json!({
                    "folders": folders,
                    "defaultSlug": self.default_slug,
                }))
            }
            "folder.register" => {
                let path = params
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ApiError::invalid_params("folder.register requires 'path' string")
                    })?;
                let folder = self
                    .register(PathBuf::from(path))
                    .map_err(|e| ApiError::invalid_params(format!("failed to register: {e}")))?;
                Ok(json!({ "folder": folder }))
            }
            "folder.unregister" => {
                let slug = params
                    .get("slug")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ApiError::invalid_params("folder.unregister requires 'slug' string")
                    })?;
                self.unregister(slug)?;
                Ok(json!({ "success": true }))
            }
            _ => Err(ApiError::method_not_found(method.to_string())),
        }
    }

    fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.persist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let persisted = PersistedFolders {
            folders: self
                .folders
                .values()
                .map(|f| PersistedFolder {
                    slug: f.slug.clone(),
                    path: f.path.clone(),
                })
                .collect(),
            default_slug: self.default_slug.clone(),
        };

        let content = serde_json::to_string_pretty(&persisted)?;
        std::fs::write(&self.persist_path, content)?;
        Ok(())
    }

    /// Get a supervisor reference for a specific folder (used for reaper management).
    pub fn supervisors(&self) -> Vec<&ProcessSupervisor> {
        self.runtimes
            .values()
            .filter_map(|rt| rt.supervisor_ref())
            .collect()
    }
}

fn derive_slug(name: &str, existing: &HashMap<String, FolderDomain>) -> String {
    let base: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    let base = if base.is_empty() {
        "folder".to_string()
    } else {
        // Collapse consecutive dashes
        let mut result = String::new();
        let mut prev_dash = false;
        for c in base.chars() {
            if c == '-' {
                if !prev_dash {
                    result.push(c);
                }
                prev_dash = true;
            } else {
                result.push(c);
                prev_dash = false;
            }
        }
        result
    };

    if !existing.contains_key(&base) {
        return base;
    }

    for i in 2.. {
        let candidate = format!("{base}-{i}");
        if !existing.contains_key(&candidate) {
            return candidate;
        }
    }

    unreachable!("slug collision loop should always find a free slot")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn derive_slug_basic() {
        let existing = HashMap::new();
        assert_eq!(derive_slug("ralph-orchestrator", &existing), "ralph-orchestrator");
    }

    #[test]
    fn derive_slug_special_chars() {
        let existing = HashMap::new();
        assert_eq!(derive_slug("My Project (v2)", &existing), "my-project-v2");
    }

    #[test]
    fn derive_slug_collision() {
        let mut existing = HashMap::new();
        existing.insert(
            "test".to_string(),
            FolderDomain {
                slug: "test".to_string(),
                path: PathBuf::from("/tmp/test"),
                display_name: "test".to_string(),
            },
        );
        assert_eq!(derive_slug("test", &existing), "test-2");
    }

    #[test]
    fn derive_slug_double_collision() {
        let mut existing = HashMap::new();
        for slug in &["test", "test-2"] {
            existing.insert(
                slug.to_string(),
                FolderDomain {
                    slug: slug.to_string(),
                    path: PathBuf::from(format!("/tmp/{slug}")),
                    display_name: slug.to_string(),
                },
            );
        }
        assert_eq!(derive_slug("test", &existing), "test-3");
    }

    #[test]
    fn register_and_list() {
        let temp = TempDir::new().unwrap();
        let persist = temp.path().join("folders.json");
        let mut registry = FolderRegistry::new(persist, 3000);

        let folder = registry.register(temp.path().to_path_buf()).unwrap();
        assert!(!folder.slug.is_empty());

        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].slug, folder.slug);
    }

    #[test]
    fn register_same_path_twice_returns_existing() {
        let temp = TempDir::new().unwrap();
        let persist = temp.path().join("folders.json");
        let mut registry = FolderRegistry::new(persist, 3000);

        let f1 = registry.register(temp.path().to_path_buf()).unwrap();
        let f2 = registry.register(temp.path().to_path_buf()).unwrap();
        assert_eq!(f1.slug, f2.slug);
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn unregister_removes_folder() {
        let temp = TempDir::new().unwrap();
        let persist = temp.path().join("folders.json");
        let mut registry = FolderRegistry::new(persist, 3000);

        let folder = registry.register(temp.path().to_path_buf()).unwrap();
        registry.unregister(&folder.slug).unwrap();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn resolve_default() {
        let temp = TempDir::new().unwrap();
        let persist = temp.path().join("folders.json");
        let mut registry = FolderRegistry::new(persist, 3000);

        registry.register(temp.path().to_path_buf()).unwrap();
        assert!(registry.resolve(None).is_ok());
    }

    #[test]
    fn resolve_nonexistent_returns_error() {
        let temp = TempDir::new().unwrap();
        let persist = temp.path().join("folders.json");
        let registry = FolderRegistry::new(persist, 3000);

        assert!(registry.resolve(Some("nonexistent")).is_err());
    }

    #[test]
    fn persistence_round_trip() {
        let temp = TempDir::new().unwrap();
        let persist = temp.path().join("folders.json");

        let slug = {
            let mut registry = FolderRegistry::new(persist.clone(), 3000);
            let folder = registry.register(temp.path().to_path_buf()).unwrap();
            folder.slug
        };

        let registry = FolderRegistry::load(&persist, 3000).unwrap();
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.list()[0].slug, slug);
    }

    #[test]
    fn dispatch_folder_list() {
        let temp = TempDir::new().unwrap();
        let persist = temp.path().join("folders.json");
        let mut registry = FolderRegistry::new(persist, 3000);
        registry.register(temp.path().to_path_buf()).unwrap();

        let result = registry
            .dispatch_folder_method("folder.list", json!({}))
            .unwrap();
        let folders = result["folders"].as_array().unwrap();
        assert_eq!(folders.len(), 1);
    }
}
