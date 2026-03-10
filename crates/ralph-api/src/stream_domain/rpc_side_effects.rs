use serde_json::{Value, json};

use super::StreamDomain;

pub(super) fn publish_rpc_side_effect(
    streams: &StreamDomain,
    method: &str,
    params: &Value,
    result: &Value,
    before_state: Option<&Value>,
) {
    match method {
        "task.create" => {
            if let Some((task_id, task_status)) = task_id_and_status(result) {
                let task = result.get("task");
                let title = task.and_then(|t| t.get("title")).and_then(Value::as_str);
                let loop_id = task.and_then(|t| t.get("loopId")).and_then(Value::as_str);
                streams.publish(
                    "task.created",
                    "task",
                    task_id,
                    json!({
                        "taskId": task_id,
                        "status": task_status,
                        "title": title,
                        "loopId": loop_id,
                    }),
                );
                streams.publish(
                    "task.status.changed",
                    "task",
                    task_id,
                    json!({
                        "from": "none",
                        "to": task_status,
                        "taskTitle": title,
                        "loopId": loop_id,
                    }),
                );
            }
        }
        "task.update" | "task.close" | "task.cancel" | "task.retry" | "task.run" => {
            if let Some((task_id, task_status)) = task_id_and_status(result) {
                let from = before_state
                    .and_then(|bs| bs.get("task"))
                    .and_then(|t| t.get("status"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let task = result.get("task");
                let title = task.and_then(|t| t.get("title")).and_then(Value::as_str);
                let hat = task.and_then(|t| t.get("lastHat")).and_then(Value::as_str);
                let loop_id = task.and_then(|t| t.get("loopId")).and_then(Value::as_str);
                streams.publish(
                    "task.status.changed",
                    "task",
                    task_id,
                    json!({
                        "from": from,
                        "to": task_status,
                        "hat": hat,
                        "loopId": loop_id,
                        "taskTitle": title,
                    }),
                );
            }
        }
        "task.delete" => {
            if let Some(id) = params.get("id").and_then(Value::as_str) {
                streams.publish(
                    "task.deleted",
                    "task",
                    id,
                    json!({ "taskId": id, "action": "deleted" }),
                );
            }
        }
        "task.clear" => {
            streams.publish("task.deleted", "task", "*", json!({ "action": "cleared" }));
        }
        "loop.merge" => {
            if let Some(loop_id) = params.get("id").and_then(Value::as_str) {
                streams.publish(
                    "loop.merge.progress",
                    "loop",
                    loop_id,
                    json!({ "loopId": loop_id, "stage": "merged" }),
                );
            }
        }
        "loop.retry" => {
            if let Some(loop_id) = params.get("id").and_then(Value::as_str) {
                streams.publish(
                    "loop.merge.progress",
                    "loop",
                    loop_id,
                    json!({ "loopId": loop_id, "stage": "queued" }),
                );
            }
        }
        "loop.discard" => {
            if let Some(loop_id) = params.get("id").and_then(Value::as_str) {
                streams.publish(
                    "loop.merge.progress",
                    "loop",
                    loop_id,
                    json!({ "loopId": loop_id, "stage": "discarded" }),
                );
            }
        }
        "loop.stop" => {
            if let Some(loop_id) = params.get("id").and_then(Value::as_str) {
                streams.publish(
                    "loop.completed",
                    "loop",
                    loop_id,
                    json!({ "loopId": loop_id, "reason": "stopped" }),
                );
            }
        }
        "planning.start" => {
            if let Some(session) = result.get("session") {
                let session_id = session.get("id").and_then(Value::as_str);
                let prompt = session.get("prompt").and_then(Value::as_str);
                if let (Some(session_id), Some(prompt)) = (session_id, prompt) {
                    streams.publish(
                        "planning.prompt.issued",
                        "planning",
                        session_id,
                        json!({
                            "sessionId": session_id,
                            "promptId": "initial",
                            "prompt": prompt
                        }),
                    );
                }
            }
        }
        "planning.respond" => {
            let session_id = params.get("sessionId").and_then(Value::as_str);
            let prompt_id = params.get("promptId").and_then(Value::as_str);
            if let (Some(session_id), Some(prompt_id)) = (session_id, prompt_id) {
                streams.publish(
                    "planning.response.recorded",
                    "planning",
                    session_id,
                    json!({ "sessionId": session_id, "promptId": prompt_id }),
                );
            }
        }
        "config.update" => {
            streams.publish(
                "config.updated",
                "config",
                "ralph.yml",
                json!({ "path": "ralph.yml", "updatedBy": "rpc-v1" }),
            );
        }
        "collection.create" => {
            if let Some(collection_id) = result
                .get("collection")
                .and_then(|collection| collection.get("id"))
                .and_then(Value::as_str)
            {
                streams.publish(
                    "collection.updated",
                    "collection",
                    collection_id,
                    json!({
                        "collectionId": collection_id,
                        "action": "created"
                    }),
                );
            }
        }
        "collection.update" => {
            if let Some(collection_id) = result
                .get("collection")
                .and_then(|collection| collection.get("id"))
                .and_then(Value::as_str)
            {
                streams.publish(
                    "collection.updated",
                    "collection",
                    collection_id,
                    json!({
                        "collectionId": collection_id,
                        "action": "updated"
                    }),
                );
            }
        }
        "collection.delete" => {
            if let Some(collection_id) = params.get("id").and_then(Value::as_str) {
                streams.publish(
                    "collection.updated",
                    "collection",
                    collection_id,
                    json!({
                        "collectionId": collection_id,
                        "action": "deleted"
                    }),
                );
            }
        }
        "collection.import" => {
            if let Some(collection_id) = result
                .get("collection")
                .and_then(|collection| collection.get("id"))
                .and_then(Value::as_str)
            {
                streams.publish(
                    "collection.updated",
                    "collection",
                    collection_id,
                    json!({
                        "collectionId": collection_id,
                        "action": "imported"
                    }),
                );
            }
        }
        _ => {}
    }
}

fn task_id_and_status(result: &Value) -> Option<(&str, &str)> {
    let task = result.get("task")?;
    let task_id = task.get("id")?.as_str()?;
    let task_status = task.get("status")?.as_str()?;
    Some((task_id, task_status))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::stream_domain::{StreamDomain, StreamSubscribeParams};

    fn publish_and_collect(
        method: &str,
        params: serde_json::Value,
        result: serde_json::Value,
        before_state: Option<serde_json::Value>,
        topics: Vec<&str>,
    ) -> Vec<crate::stream_domain::StreamEventEnvelope> {
        let streams = StreamDomain::new();
        streams.publish_rpc_side_effect(method, &params, &result, before_state.as_ref());
        let sub = streams
            .subscribe(
                StreamSubscribeParams {
                    topics: topics.into_iter().map(String::from).collect(),
                    cursor: Some("0-0".to_string()),
                    replay_limit: Some(100),
                    filters: None,
                },
                "test",
            )
            .unwrap();
        streams
            .replay_for_subscription(&sub.subscription_id)
            .unwrap()
            .events
    }

    #[test]
    fn task_create_emits_created_and_status_changed() {
        let events = publish_and_collect(
            "task.create",
            json!({}),
            json!({"task": {"id": "t1", "status": "open", "title": "Test task", "loopId": "loop-1"}}),
            None,
            vec!["task.created", "task.status.changed"],
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].topic, "task.created");
        assert_eq!(events[0].payload["taskId"], "t1");
        assert_eq!(events[0].payload["status"], "open");
        assert_eq!(events[0].payload["title"], "Test task");
        assert_eq!(events[0].payload["loopId"], "loop-1");
        assert_eq!(events[1].topic, "task.status.changed");
        assert_eq!(events[1].payload["from"], "none");
        assert_eq!(events[1].payload["to"], "open");
        assert_eq!(events[1].payload["taskTitle"], "Test task");
        assert_eq!(events[1].payload["loopId"], "loop-1");
    }

    #[test]
    fn task_close_uses_before_state_for_from_status() {
        let events = publish_and_collect(
            "task.close",
            json!({}),
            json!({"task": {"id": "t1", "status": "closed", "title": "Test", "lastHat": "Builder", "loopId": "loop-1"}}),
            Some(json!({"task": {"status": "in_progress"}})),
            vec!["task.status.changed"],
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["from"], "in_progress");
        assert_eq!(events[0].payload["to"], "closed");
        assert_eq!(events[0].payload["hat"], "Builder");
        assert_eq!(events[0].payload["loopId"], "loop-1");
        assert_eq!(events[0].payload["taskTitle"], "Test");
    }

    #[test]
    fn task_close_falls_back_to_unknown_without_before_state() {
        let events = publish_and_collect(
            "task.close",
            json!({}),
            json!({"task": {"id": "t1", "status": "closed", "title": "T", "loopId": "l1"}}),
            None,
            vec!["task.status.changed"],
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["from"], "unknown");
    }

    #[test]
    fn task_delete_emits_deleted() {
        let events = publish_and_collect(
            "task.delete",
            json!({"id": "t1"}),
            json!({}),
            None,
            vec!["task.deleted"],
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["taskId"], "t1");
        assert_eq!(events[0].payload["action"], "deleted");
    }

    #[test]
    fn task_clear_emits_deleted_with_cleared_action() {
        let events = publish_and_collect(
            "task.clear",
            json!({}),
            json!({}),
            None,
            vec!["task.deleted"],
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["action"], "cleared");
    }

    #[test]
    fn loop_stop_emits_completed() {
        let events = publish_and_collect(
            "loop.stop",
            json!({"id": "loop-1"}),
            json!({}),
            None,
            vec!["loop.completed"],
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["loopId"], "loop-1");
        assert_eq!(events[0].payload["reason"], "stopped");
    }

    #[test]
    fn new_stream_topics_are_subscribable() {
        let streams = StreamDomain::new();
        let result = streams.subscribe(
            StreamSubscribeParams {
                topics: vec![
                    "task.created".into(),
                    "task.deleted".into(),
                    "loop.started".into(),
                    "loop.completed".into(),
                    "event.published".into(),
                ],
                cursor: None,
                replay_limit: None,
                filters: None,
            },
            "test",
        );
        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.accepted_topics.len(), 5);
    }
}
