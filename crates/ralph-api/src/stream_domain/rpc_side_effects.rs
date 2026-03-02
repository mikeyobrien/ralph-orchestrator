use serde_json::{Value, json};

use super::StreamDomain;

pub(super) fn publish_rpc_side_effect(
    streams: &StreamDomain,
    method: &str,
    params: &Value,
    result: &Value,
) {
    match method {
        "task.create" => {
            if let Some((task_id, task_status)) = task_id_and_status(result) {
                streams.publish(
                    "task.status.changed",
                    "task",
                    task_id,
                    json!({ "from": "none", "to": task_status }),
                );
            }
        }
        "task.update" | "task.close" | "task.cancel" | "task.retry" | "task.run" => {
            if let Some((task_id, task_status)) = task_id_and_status(result) {
                streams.publish(
                    "task.status.changed",
                    "task",
                    task_id,
                    json!({ "from": "unknown", "to": task_status }),
                );
            }
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
