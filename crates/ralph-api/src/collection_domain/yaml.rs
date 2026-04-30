use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::errors::ApiError;

use super::{
    CollectionRecord, GraphData, GraphEdge, GraphNode, HatNodeData, NodePosition, Viewport, now_ts,
};

/// Returns the hat key that will activate first when the collection runs.
///
/// Derives the `starting_event` from the graph topology (same logic as the
/// YAML exporter), then finds which hat triggers on that event. The frontend
/// uses this to highlight the entry node immediately after Run, avoiding the
/// timing race between the RPC response and the first WebSocket event.
pub fn starting_hat_for_collection(collection: &CollectionRecord) -> Option<String> {
    let graph = &collection.graph;

    // Build triggers and publishes from node data + edges (same as the
    // YAML exporter: node.data.triggers_on seeds the set, edges add more).
    let mut hat_triggers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut hat_publishes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for node in &graph.nodes {
        hat_triggers.insert(
            node.id.clone(),
            node.data.triggers_on.iter().cloned().collect(),
        );
        hat_publishes.insert(
            node.id.clone(),
            node.data.publishes.iter().cloned().collect(),
        );
    }

    for edge in &graph.edges {
        let event_name = edge
            .label
            .clone()
            .or_else(|| edge.source_handle.clone())
            .unwrap_or_default();
        if event_name.is_empty() {
            continue;
        }
        if let Some(publishes) = hat_publishes.get_mut(&edge.source) {
            publishes.insert(event_name.clone());
        }
        if let Some(triggers) = hat_triggers.get_mut(&edge.target) {
            triggers.insert(event_name);
        }
    }

    // Collect all published topics.
    let all_published: std::collections::HashSet<&str> = hat_publishes
        .values()
        .flat_map(|s| s.iter().map(String::as_str))
        .collect();

    // Find the starting_event (first external trigger).
    let starting_event = hat_triggers
        .values()
        .flat_map(|t| t.iter())
        .find(|t| !all_published.contains(t.as_str()))
        .cloned()
        .or_else(|| hat_triggers.values().find_map(|t| t.iter().next()).cloned())?;

    // Find which hat triggers on the starting_event.
    for (node_id, triggers) in &hat_triggers {
        if triggers.contains(&starting_event) {
            // Return the hat key (from node data), not the node id.
            if let Some(node) = graph.nodes.iter().find(|n| n.id == *node_id) {
                return Some(node.data.key.clone());
            }
        }
    }

    None
}

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

    // Derive starting_event from the graph topology: find the first
    // trigger that no hat publishes (an "external" entry point). This
    // ensures the starting event actually reaches a hat instead of
    // falling through to Ralph's coordinator fallback as an orphan.
    let all_published: std::collections::HashSet<&str> = hats
        .values()
        .flat_map(|h| h.publishes.iter().map(String::as_str))
        .collect();

    let starting_event = hats
        .values()
        .flat_map(|h| h.triggers.iter())
        .find(|t| !all_published.contains(t.as_str()))
        .cloned()
        // Fallback for pure cycles (all triggers are internal): use the
        // first trigger of the first hat. It's internal, but at least it
        // matches a real hat and will activate the cycle.
        .or_else(|| hats.values().find_map(|h| h.triggers.first()).cloned())
        .unwrap_or_else(|| "work.start".to_string());

    let preset = ExportPreset {
        event_loop: ExportEventLoop {
            completion_promise: "LOOP_COMPLETE".to_string(),
            starting_event,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, key: &str, triggers: &[&str], publishes: &[&str]) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            node_type: "hatNode".to_string(),
            position: NodePosition { x: 0.0, y: 0.0 },
            data: HatNodeData {
                key: key.to_string(),
                name: key.to_string(),
                description: format!("Test hat {key}"),
                triggers_on: triggers.iter().map(|s| s.to_string()).collect(),
                publishes: publishes.iter().map(|s| s.to_string()).collect(),
                instructions: None,
            },
        }
    }

    fn make_edge(source: &str, target: &str, label: &str) -> GraphEdge {
        GraphEdge {
            id: format!("{source}-{target}"),
            source: source.to_string(),
            target: target.to_string(),
            source_handle: Some(label.to_string()),
            target_handle: Some(label.to_string()),
            label: Some(label.to_string()),
        }
    }

    fn make_collection(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> CollectionRecord {
        CollectionRecord {
            id: "test-collection".to_string(),
            name: "Test".to_string(),
            description: None,
            graph: GraphData {
                nodes,
                edges,
                viewport: Viewport {
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                },
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn export_derives_starting_event_from_entry_hat() {
        // planner triggers on build.start (external) + queue.advance (internal from finalizer)
        // builder triggers on tasks.ready (internal from planner)
        // -> starting_event should be build.start
        let collection = make_collection(
            vec![
                make_node(
                    "p",
                    "planner",
                    &["build.start", "queue.advance"],
                    &["tasks.ready"],
                ),
                make_node("b", "builder", &["tasks.ready"], &["review.ready"]),
                make_node(
                    "f",
                    "finalizer",
                    &["review.ready"],
                    &["LOOP_COMPLETE", "queue.advance"],
                ),
            ],
            vec![
                make_edge("p", "b", "tasks.ready"),
                make_edge("b", "f", "review.ready"),
                make_edge("f", "p", "queue.advance"),
            ],
        );

        let yaml = export_collection_yaml(&collection).unwrap();
        assert!(
            yaml.contains("starting_event: build.start"),
            "expected starting_event: build.start, got:\n{yaml}"
        );
    }

    #[test]
    fn export_falls_back_to_first_trigger_when_all_triggers_are_internal() {
        // Pure cycle: a -> b -> a. All triggers are internal.
        // Fallback: use the first trigger of the first hat (event.b for hat-a).
        let collection = make_collection(
            vec![
                make_node("a", "hat-a", &["event.b"], &["event.a"]),
                make_node("b", "hat-b", &["event.a"], &["event.b"]),
            ],
            vec![
                make_edge("a", "b", "event.a"),
                make_edge("b", "a", "event.b"),
            ],
        );

        let yaml = export_collection_yaml(&collection).unwrap();
        // Should use the first hat's first trigger (event.b) not work.start
        assert!(
            yaml.contains("starting_event: event.b"),
            "expected fallback starting_event: event.b (first hat's first trigger), got:\n{yaml}"
        );
    }

    #[test]
    fn export_does_not_use_task_start_when_no_hat_triggers_on_it() {
        let collection = make_collection(
            vec![
                make_node("p", "planner", &["go.start"], &["build.task"]),
                make_node("b", "builder", &["build.task"], &["LOOP_COMPLETE"]),
            ],
            vec![make_edge("p", "b", "build.task")],
        );

        let yaml = export_collection_yaml(&collection).unwrap();
        assert!(
            !yaml.contains("starting_event: task.start"),
            "should not hardcode task.start when no hat triggers on it, got:\n{yaml}"
        );
        assert!(
            yaml.contains("starting_event: go.start"),
            "expected starting_event: go.start, got:\n{yaml}"
        );
    }

    #[test]
    fn export_empty_collection_produces_valid_yaml() {
        let collection = make_collection(vec![], vec![]);
        let yaml = export_collection_yaml(&collection).unwrap();
        // Should still produce valid YAML with the fallback starting_event.
        assert!(yaml.contains("starting_event:"));
        assert!(yaml.contains("completion_promise: LOOP_COMPLETE"));
        assert!(yaml.contains("hats:"));
    }
}

#[cfg(test)]
mod starting_hat_tests {
    use super::*;

    fn make_node(id: &str, key: &str, triggers: &[&str], publishes: &[&str]) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            node_type: "hatNode".to_string(),
            position: NodePosition { x: 0.0, y: 0.0 },
            data: HatNodeData {
                key: key.to_string(),
                name: key.to_string(),
                description: format!("Test hat {key}"),
                triggers_on: triggers.iter().map(|s| s.to_string()).collect(),
                publishes: publishes.iter().map(|s| s.to_string()).collect(),
                instructions: None,
            },
        }
    }

    fn make_edge(source: &str, target: &str, label: &str) -> GraphEdge {
        GraphEdge {
            id: format!("{source}-{target}"),
            source: source.to_string(),
            target: target.to_string(),
            source_handle: Some(label.to_string()),
            target_handle: Some(label.to_string()),
            label: Some(label.to_string()),
        }
    }

    fn make_collection(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> CollectionRecord {
        CollectionRecord {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            graph: GraphData {
                nodes,
                edges,
                viewport: Viewport {
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                },
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn finds_planner_via_node_triggers_not_just_edges() {
        // planner has triggersOn: ["work.start", "subtask.done"]
        // "work.start" is external (no hat publishes it)
        // The function must find planner, not builder.
        let collection = make_collection(
            vec![
                make_node(
                    "p",
                    "planner",
                    &["work.start", "subtask.done"],
                    &["subtask.ready"],
                ),
                make_node(
                    "b",
                    "builder",
                    &["subtask.ready"],
                    &["subtask.done", "impl.done"],
                ),
                make_node("r", "reviewer", &["impl.done"], &["review.approved"]),
                make_node("f", "finalizer", &["review.approved"], &["LOOP_COMPLETE"]),
            ],
            vec![
                make_edge("p", "b", "subtask.ready"),
                make_edge("b", "p", "subtask.done"),
                make_edge("b", "r", "impl.done"),
                make_edge("r", "f", "review.approved"),
            ],
        );

        assert_eq!(
            starting_hat_for_collection(&collection).as_deref(),
            Some("planner"),
            "should find planner via its triggersOn, not just edges"
        );
    }

    #[test]
    fn returns_none_for_empty_collection() {
        let collection = make_collection(vec![], vec![]);
        assert_eq!(starting_hat_for_collection(&collection), None);
    }

    #[test]
    fn single_hat_returns_its_key() {
        let collection = make_collection(
            vec![make_node("n1", "solo-hat", &["kick.off"], &["done"])],
            vec![],
        );
        assert_eq!(
            starting_hat_for_collection(&collection).as_deref(),
            Some("solo-hat")
        );
    }

    #[test]
    fn linear_chain_returns_entry_hat() {
        // A -> B -> C; only A has an external trigger
        let collection = make_collection(
            vec![
                make_node("a", "hat-a", &["external.start"], &["evt.ab"]),
                make_node("b", "hat-b", &["evt.ab"], &["evt.bc"]),
                make_node("c", "hat-c", &["evt.bc"], &["done"]),
            ],
            vec![make_edge("a", "b", "evt.ab"), make_edge("b", "c", "evt.bc")],
        );
        assert_eq!(
            starting_hat_for_collection(&collection).as_deref(),
            Some("hat-a")
        );
    }

    #[test]
    fn diamond_fork_returns_entry_hat() {
        // A -> B and A -> C; A has external trigger
        let collection = make_collection(
            vec![
                make_node("a", "hat-a", &["external.go"], &["fork.b", "fork.c"]),
                make_node("b", "hat-b", &["fork.b"], &["done.b"]),
                make_node("c", "hat-c", &["fork.c"], &["done.c"]),
            ],
            vec![make_edge("a", "b", "fork.b"), make_edge("a", "c", "fork.c")],
        );
        assert_eq!(
            starting_hat_for_collection(&collection).as_deref(),
            Some("hat-a")
        );
    }

    #[test]
    fn all_internal_triggers_uses_fallback() {
        // Pure cycle: every trigger is published by another hat.
        // Fallback should return the hat whose first trigger is chosen.
        let collection = make_collection(
            vec![
                make_node("a", "hat-a", &["evt.ba"], &["evt.ab"]),
                make_node("b", "hat-b", &["evt.ab"], &["evt.ba"]),
            ],
            vec![make_edge("a", "b", "evt.ab"), make_edge("b", "a", "evt.ba")],
        );
        let result = starting_hat_for_collection(&collection);
        // Fallback picks first trigger of first hat (evt.ba), which hat-a triggers on.
        assert_eq!(result.as_deref(), Some("hat-a"));
    }

    #[test]
    fn hat_with_no_triggers_is_skipped() {
        // One hat has no triggers, the other has an external trigger.
        let collection = make_collection(
            vec![
                make_node("empty", "no-trigger-hat", &[], &["some.event"]),
                make_node("real", "real-hat", &["external.evt"], &["done"]),
            ],
            vec![],
        );
        assert_eq!(
            starting_hat_for_collection(&collection).as_deref(),
            Some("real-hat")
        );
    }

    #[test]
    fn returns_key_not_node_id() {
        // node id and data key differ; function must return the key.
        let collection = make_collection(
            vec![make_node("node-1", "my-hat-key", &["kick.off"], &["done"])],
            vec![],
        );
        let result = starting_hat_for_collection(&collection).unwrap();
        assert_eq!(result, "my-hat-key", "should return data.key, not node id");
        assert_ne!(result, "node-1");
    }
}
