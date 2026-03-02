use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::errors::ApiError;

use super::{
    CollectionRecord, GraphData, GraphEdge, GraphNode, HatNodeData, NodePosition, Viewport, now_ts,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct ExportPreset {
    event_loop: ExportEventLoop,
    cli: ExportCli,
    hats: BTreeMap<String, ExportHat>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    events: BTreeMap<String, ExportEventMetadata>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct ExportEventLoop {
    completion_promise: String,
    starting_event: String,
    max_iterations: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct ExportCli {
    backend: String,
    prompt_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct ExportHat {
    name: String,
    description: String,
    triggers: Vec<String>,
    publishes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_publishes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct ExportEventMetadata {
    description: String,
}

pub(super) fn graph_from_yaml(content: &str) -> Result<GraphData, ApiError> {
    let root: serde_yaml::Value = serde_yaml::from_str(content)
        .map_err(|error| ApiError::invalid_params(format!("invalid YAML payload: {error}")))?;

    let mapping = root
        .as_mapping()
        .ok_or_else(|| ApiError::invalid_params("collection.import yaml must be a mapping"))?;

    let hats_value = mapping_get(mapping, "hats")
        .ok_or_else(|| ApiError::invalid_params("collection.import yaml must define hats"))?;

    let hats_mapping = hats_value
        .as_mapping()
        .ok_or_else(|| ApiError::invalid_params("collection.import hats must be a mapping"))?;

    let mut hat_entries: Vec<(String, &serde_yaml::Mapping)> = hats_mapping
        .iter()
        .map(|(key, value)| {
            let key = key.as_str().ok_or_else(|| {
                ApiError::invalid_params("collection.import hat keys must be strings")
            })?;
            let value = value.as_mapping().ok_or_else(|| {
                ApiError::invalid_params(format!("collection.import hat '{key}' must be an object"))
            })?;
            Ok((key.to_string(), value))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    hat_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut nodes = Vec::new();
    let mut event_publishers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut event_subscribers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut y_position = 50.0;

    for (hat_key, config) in hat_entries {
        let node_id = hat_key.clone();
        let name = yaml_string_field(config, "name").unwrap_or_else(|| hat_key.clone());
        let description = yaml_string_field(config, "description").unwrap_or_default();
        let triggers = yaml_string_list(config, "triggers");
        let publishes = yaml_string_list(config, "publishes");
        let instructions = yaml_string_field(config, "instructions");

        for event_name in &publishes {
            event_publishers
                .entry(event_name.clone())
                .or_default()
                .push(node_id.clone());
        }

        for event_name in &triggers {
            event_subscribers
                .entry(event_name.clone())
                .or_default()
                .push(node_id.clone());
        }

        nodes.push(GraphNode {
            id: node_id,
            node_type: "hatNode".to_string(),
            position: NodePosition {
                x: 250.0,
                y: y_position,
            },
            data: HatNodeData {
                key: hat_key,
                name,
                description,
                triggers_on: triggers,
                publishes,
                instructions,
            },
        });

        y_position += 200.0;
    }

    let mut edges = Vec::new();
    let mut seen_edges = BTreeSet::new();
    let mut edge_index = 0_u64;

    for (event_name, publishers) in event_publishers {
        let subscribers = event_subscribers
            .get(&event_name)
            .cloned()
            .unwrap_or_default();

        for publisher in &publishers {
            for subscriber in &subscribers {
                if publisher == subscriber {
                    continue;
                }

                let edge_key = (publisher.clone(), subscriber.clone(), event_name.clone());
                if !seen_edges.insert(edge_key.clone()) {
                    continue;
                }

                edges.push(GraphEdge {
                    id: format!("edge-{edge_index}"),
                    source: edge_key.0.clone(),
                    target: edge_key.1.clone(),
                    source_handle: Some(edge_key.2.clone()),
                    target_handle: Some(edge_key.2.clone()),
                    label: Some(edge_key.2.clone()),
                });

                edge_index = edge_index.saturating_add(1);
            }
        }
    }

    Ok(GraphData {
        nodes,
        edges,
        viewport: Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 0.8,
        },
    })
}

pub(super) fn export_collection_yaml(collection: &CollectionRecord) -> Result<String, ApiError> {
    let mut hat_triggers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut hat_publishes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut all_events: BTreeSet<String> = BTreeSet::new();

    for node in &collection.graph.nodes {
        hat_triggers.insert(
            node.id.clone(),
            node.data.triggers_on.iter().cloned().collect(),
        );
        hat_publishes.insert(
            node.id.clone(),
            node.data.publishes.iter().cloned().collect(),
        );
    }

    for edge in &collection.graph.edges {
        let event_name = edge
            .label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| format!("{}_to_{}", edge.source, edge.target));

        all_events.insert(event_name.clone());

        if let Some(publishes) = hat_publishes.get_mut(&edge.source) {
            publishes.insert(event_name.clone());
        }

        if let Some(triggers) = hat_triggers.get_mut(&edge.target) {
            triggers.insert(event_name);
        }
    }

    let mut ordered_nodes = collection.graph.nodes.clone();
    ordered_nodes.sort_by(|a, b| a.data.key.cmp(&b.data.key).then(a.id.cmp(&b.id)));

    let mut hats = BTreeMap::new();
    for node in ordered_nodes {
        let triggers: Vec<String> = hat_triggers
            .get(&node.id)
            .map(|events| events.iter().cloned().collect())
            .unwrap_or_default();

        let publishes: Vec<String> = hat_publishes
            .get(&node.id)
            .map(|events| events.iter().cloned().collect())
            .unwrap_or_default();

        let default_publishes = publishes.first().cloned();

        hats.insert(
            node.data.key.clone(),
            ExportHat {
                name: node.data.name,
                description: node.data.description,
                triggers,
                publishes,
                instructions: node.data.instructions,
                default_publishes,
            },
        );
    }

    let events = all_events
        .into_iter()
        .map(|event_name| {
            (
                event_name.clone(),
                ExportEventMetadata {
                    description: format!("Event: {event_name}"),
                },
            )
        })
        .collect();

    let preset = ExportPreset {
        event_loop: ExportEventLoop {
            completion_promise: "LOOP_COMPLETE".to_string(),
            starting_event: "task.start".to_string(),
            max_iterations: 50,
        },
        cli: ExportCli {
            backend: "claude".to_string(),
            prompt_mode: "arg".to_string(),
        },
        hats,
        events,
    };

    let yaml_body = serde_yaml::to_string(&preset).map_err(|error| {
        ApiError::internal(format!("failed serializing collection yaml: {error}"))
    })?;

    let header = format!(
        "# {}\n# {}\n# Generated at: {}\n\n",
        collection.name,
        collection
            .description
            .clone()
            .unwrap_or_else(|| "Generated by Ralph Hat Collection Builder".to_string()),
        now_ts()
    );

    Ok(format!("{header}{yaml_body}"))
}

fn yaml_string_field(mapping: &serde_yaml::Mapping, key: &str) -> Option<String> {
    mapping_get(mapping, key)
        .and_then(serde_yaml::Value::as_str)
        .map(std::string::ToString::to_string)
}

fn yaml_string_list(mapping: &serde_yaml::Mapping, key: &str) -> Vec<String> {
    mapping_get(mapping, key)
        .and_then(serde_yaml::Value::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_yaml::Value::as_str)
                .map(std::string::ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn mapping_get<'a>(mapping: &'a serde_yaml::Mapping, key: &str) -> Option<&'a serde_yaml::Value> {
    mapping.get(serde_yaml::Value::String(key.to_string()))
}
