mod yaml;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

use crate::errors::ApiError;
use crate::loop_support::now_ts;

use self::yaml::{export_collection_yaml, graph_from_yaml};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCreateParams {
    pub name: String,
    pub description: Option<String>,
    pub graph: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub graph: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionImportParams {
    pub yaml: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRecord {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub graph: GraphData,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub viewport: Viewport,
}

impl Default for GraphData {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub position: NodePosition,
    pub data: HatNodeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HatNodeData {
    pub key: String,
    pub name: String,
    pub description: String,
    pub triggers_on: Vec<String>,
    pub publishes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CollectionSnapshot {
    collections: Vec<CollectionRecord>,
    id_counter: u64,
}

pub struct CollectionDomain {
    store_path: PathBuf,
    collections: BTreeMap<String, CollectionRecord>,
    id_counter: u64,
}

impl CollectionDomain {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let store_path = workspace_root
            .as_ref()
            .join(".ralph/api/collections-v1.json");
        let mut domain = Self {
            store_path,
            collections: BTreeMap::new(),
            id_counter: 0,
        };
        domain.load();
        domain
    }

    pub fn list(&self) -> Vec<CollectionSummary> {
        let mut entries: Vec<_> = self
            .collections
            .values()
            .map(|collection| CollectionSummary {
                id: collection.id.clone(),
                name: collection.name.clone(),
                description: collection.description.clone(),
                created_at: collection.created_at.clone(),
                updated_at: collection.updated_at.clone(),
            })
            .collect();

        entries.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        entries
    }

    pub fn get(&self, id: &str) -> Result<CollectionRecord, ApiError> {
        self.collections
            .get(id)
            .cloned()
            .ok_or_else(|| collection_not_found_error(id))
    }

    pub fn create(&mut self, params: CollectionCreateParams) -> Result<CollectionRecord, ApiError> {
        let graph = params
            .graph
            .map(parse_graph)
            .transpose()?
            .unwrap_or_default();

        let now = now_ts();
        let id = self.next_collection_id();

        let record = CollectionRecord {
            id: id.clone(),
            name: params.name,
            description: params.description,
            graph,
            created_at: now.clone(),
            updated_at: now,
        };

        self.collections.insert(id.clone(), record);
        self.persist()?;
        self.get(&id)
    }

    pub fn update(&mut self, params: CollectionUpdateParams) -> Result<CollectionRecord, ApiError> {
        let record = self
            .collections
            .get_mut(&params.id)
            .ok_or_else(|| collection_not_found_error(&params.id))?;

        if let Some(name) = params.name {
            record.name = name;
        }

        if let Some(description) = params.description {
            record.description = Some(description);
        }

        if let Some(graph) = params.graph {
            record.graph = parse_graph(graph)?;
        }

        record.updated_at = now_ts();
        self.persist()?;
        self.get(&params.id)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), ApiError> {
        if self.collections.remove(id).is_none() {
            return Err(collection_not_found_error(id));
        }

        self.persist()
    }

    pub fn import(&mut self, params: CollectionImportParams) -> Result<CollectionRecord, ApiError> {
        let graph = graph_from_yaml(&params.yaml)?;
        self.create(CollectionCreateParams {
            name: params.name,
            description: params.description,
            graph: Some(serde_json::to_value(graph).map_err(|error| {
                ApiError::internal(format!("failed serializing graph: {error}"))
            })?),
        })
    }

    pub fn export(&self, id: &str) -> Result<String, ApiError> {
        let collection = self.get(id)?;
        export_collection_yaml(&collection)
    }

    fn next_collection_id(&mut self) -> String {
        self.id_counter = self.id_counter.saturating_add(1);
        format!(
            "collection-{}-{:04x}",
            Utc::now().timestamp_millis(),
            self.id_counter
        )
    }

    fn load(&mut self) {
        if !self.store_path.exists() {
            return;
        }

        let content = match fs::read_to_string(&self.store_path) {
            Ok(content) => content,
            Err(error) => {
                warn!(
                    path = %self.store_path.display(),
                    %error,
                    "failed reading collection snapshot"
                );
                return;
            }
        };

        let snapshot: CollectionSnapshot = match serde_json::from_str(&content) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                warn!(
                    path = %self.store_path.display(),
                    %error,
                    "failed parsing collection snapshot"
                );
                return;
            }
        };

        self.collections = snapshot
            .collections
            .into_iter()
            .map(|collection| (collection.id.clone(), collection))
            .collect();
        self.id_counter = snapshot.id_counter;
    }

    fn persist(&self) -> Result<(), ApiError> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed creating collection snapshot directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let snapshot = CollectionSnapshot {
            collections: self.sorted_records(),
            id_counter: self.id_counter,
        };

        let payload = serde_json::to_string_pretty(&snapshot).map_err(|error| {
            ApiError::internal(format!("failed serializing collections snapshot: {error}"))
        })?;

        fs::write(&self.store_path, payload).map_err(|error| {
            ApiError::internal(format!(
                "failed writing collection snapshot '{}': {error}",
                self.store_path.display()
            ))
        })
    }

    fn sorted_records(&self) -> Vec<CollectionRecord> {
        let mut records: Vec<_> = self.collections.values().cloned().collect();
        records.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        records
    }
}

fn parse_graph(raw: Value) -> Result<GraphData, ApiError> {
    serde_json::from_value(raw)
        .map_err(|error| ApiError::invalid_params(format!("invalid collection graph: {error}")))
}

fn collection_not_found_error(collection_id: &str) -> ApiError {
    ApiError::collection_not_found(format!("Collection with id '{collection_id}' not found"))
        .with_details(serde_json::json!({ "collectionId": collection_id }))
}
