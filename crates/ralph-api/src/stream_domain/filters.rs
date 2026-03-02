use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::errors::ApiError;

use super::StreamEventEnvelope;

#[derive(Debug, Clone, Default)]
pub(super) struct SubscriptionFilters {
    resource_ids: HashSet<String>,
    resource_types: HashSet<String>,
}

impl SubscriptionFilters {
    pub(super) fn from_json(raw: Option<Value>) -> Result<Self, ApiError> {
        let Some(raw) = raw else {
            return Ok(Self::default());
        };

        let object = raw.as_object().ok_or_else(|| {
            ApiError::invalid_params("stream.subscribe filters must be an object")
                .with_details(json!({ "filters": raw }))
        })?;

        let mut resource_ids = HashSet::new();
        let mut resource_types = HashSet::new();

        parse_filter_set(object, "resourceId", &mut resource_ids)?;
        parse_filter_set(object, "resourceIds", &mut resource_ids)?;
        parse_filter_set(object, "taskId", &mut resource_ids)?;
        parse_filter_set(object, "taskIds", &mut resource_ids)?;
        parse_filter_set(object, "resourceType", &mut resource_types)?;
        parse_filter_set(object, "resourceTypes", &mut resource_types)?;

        Ok(Self {
            resource_ids,
            resource_types,
        })
    }

    pub(super) fn matches(&self, event: &StreamEventEnvelope) -> bool {
        if !self.resource_ids.is_empty() && !self.resource_ids.contains(&event.resource.id) {
            return false;
        }

        if !self.resource_types.is_empty() && !self.resource_types.contains(&event.resource.kind) {
            return false;
        }

        true
    }
}

pub(super) fn normalize_topics(
    topics: &[String],
    known_topics: &[&str],
) -> Result<Vec<String>, ApiError> {
    if topics.is_empty() {
        return Err(ApiError::invalid_params(
            "stream.subscribe requires at least one topic",
        ));
    }

    let known_topics = known_topics.iter().copied().collect::<HashSet<_>>();
    let mut accepted_topics = Vec::new();
    let mut dedupe = HashSet::new();

    for topic in topics {
        if !known_topics.contains(topic.as_str()) {
            return Err(
                ApiError::invalid_params(format!("unknown stream topic '{topic}'"))
                    .with_details(json!({ "topic": topic, "knownTopics": known_topics })),
            );
        }

        if dedupe.insert(topic.clone()) {
            accepted_topics.push(topic.clone());
        }
    }

    Ok(accepted_topics)
}

pub(super) fn validate_cursor(cursor: &str) -> Result<(), ApiError> {
    let _ = cursor_sequence(cursor)?;
    Ok(())
}

pub(super) fn cursor_sequence(cursor: &str) -> Result<u64, ApiError> {
    cursor
        .rsplit_once('-')
        .and_then(|(_, sequence)| sequence.parse::<u64>().ok())
        .ok_or_else(|| {
            ApiError::invalid_params("cursor must match '<epochMillis>-<sequence>' format")
                .with_details(json!({ "cursor": cursor }))
        })
}

pub(super) fn cursor_is_older(candidate: &str, current: &str) -> Result<bool, ApiError> {
    let candidate_sequence = cursor_sequence(candidate)?;
    let current_sequence = cursor_sequence(current)?;
    Ok(candidate_sequence < current_sequence)
}

fn parse_filter_set(
    object: &Map<String, Value>,
    key: &str,
    target: &mut HashSet<String>,
) -> Result<(), ApiError> {
    let Some(value) = object.get(key) else {
        return Ok(());
    };

    if let Some(single) = value.as_str() {
        if !single.trim().is_empty() {
            target.insert(single.to_string());
        }
        return Ok(());
    }

    if let Some(values) = value.as_array() {
        for item in values {
            let Some(value) = item.as_str() else {
                return Err(ApiError::invalid_params(format!(
                    "filters.{key} entries must be strings"
                )));
            };
            if !value.trim().is_empty() {
                target.insert(value.to_string());
            }
        }
        return Ok(());
    }

    Err(ApiError::invalid_params(format!(
        "filters.{key} must be a string or string array"
    )))
}
