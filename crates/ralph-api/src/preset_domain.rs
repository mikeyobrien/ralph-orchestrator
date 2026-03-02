use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_yaml::Value;
use tracing::warn;

use crate::collection_domain::CollectionSummary;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetRecord {
    pub id: String,
    pub name: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PresetDomain {
    workspace_root: PathBuf,
}

impl PresetDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    pub fn list(&self, collections: &[CollectionSummary]) -> Vec<PresetRecord> {
        let builtin_dir = self.workspace_root.join("presets");
        let hats_dir = self.workspace_root.join(".ralph/hats");

        let mut builtin = read_presets_from_dir(&builtin_dir, "builtin", false);
        let mut directory = read_presets_from_dir(&hats_dir, "directory", true);
        let mut collection_presets: Vec<_> = collections
            .iter()
            .map(|collection| PresetRecord {
                id: collection.id.clone(),
                name: collection.name.clone(),
                source: "collection".to_string(),
                description: collection.description.clone(),
                path: None,
            })
            .collect();

        builtin.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        directory.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        collection_presets.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));

        let mut presets =
            Vec::with_capacity(builtin.len() + directory.len() + collection_presets.len());
        presets.extend(builtin);
        presets.extend(directory);
        presets.extend(collection_presets);
        presets
    }
}

fn read_presets_from_dir(dir: &Path, source: &str, include_path: bool) -> Vec<PresetRecord> {
    if !dir.exists() {
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().is_some_and(|extension| extension == "yml"))
        .collect();

    files.sort();

    files
        .into_iter()
        .filter_map(|path| {
            let file_stem = path.file_stem()?.to_str()?.to_string();
            let description = read_preset_description(&path);

            Some(PresetRecord {
                id: format!("{source}:{file_stem}"),
                name: file_stem,
                source: source.to_string(),
                description,
                path: include_path.then(|| path.display().to_string()),
            })
        })
        .collect()
}

fn read_preset_description(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let parsed: Value = match serde_yaml::from_str(&content) {
        Ok(parsed) => parsed,
        Err(error) => {
            warn!(path = %path.display(), %error, "failed parsing preset yaml");
            return None;
        }
    };

    parsed
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("description".to_string())))
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string)
}
