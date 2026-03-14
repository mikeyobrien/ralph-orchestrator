use anyhow::Result;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, PaginatedRequestParams};
use serde_json::{Map, Value, json};

use ralph_api::{ApiConfig, mcp::RalphMcpServer};

fn test_config(workspace_root: std::path::PathBuf) -> ApiConfig {
    ApiConfig {
        workspace_root,
        ..ApiConfig::default()
    }
}

fn obj(value: Value) -> Map<String, Value> {
    value
        .as_object()
        .cloned()
        .expect("tool arguments must be a JSON object")
}

#[tokio::test]
async fn mcp_lists_expected_tools() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
    let server_config = test_config(workspace.path().to_path_buf());

    let server_handle = tokio::spawn(async move {
        let server = RalphMcpServer::new(server_config)?;
        let running = server.serve(server_transport).await?;
        let _ = running.waiting().await?;
        anyhow::Ok(())
    });

    let client = ().serve(client_transport).await?;
    let tools = client.peer().list_all_tools().await?;
    let names: Vec<_> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert!(names.contains(&"task_list"));
    assert!(names.contains(&"loop_status"));
    assert!(names.contains(&"planning_start"));
    assert!(names.contains(&"config_get"));
    assert!(names.contains(&"stream_next"));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn mcp_call_tool_round_trips_structured_results() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
    let server_config = test_config(workspace.path().to_path_buf());

    let server_handle = tokio::spawn(async move {
        let server = RalphMcpServer::new(server_config)?;
        let running = server.serve(server_transport).await?;
        let _ = running.waiting().await?;
        anyhow::Ok(())
    });

    let client = ().serve(client_transport).await?;
    let created = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("task_create").with_arguments(obj(json!({
                "id": "task-mcp-1",
                "title": "Created from MCP",
                "autoExecute": false,
            }))),
        )
        .await?;
    assert_eq!(created.is_error, Some(false));
    assert_eq!(
        created.structured_content.as_ref().unwrap()["task"]["id"],
        "task-mcp-1"
    );

    let listed = client
        .peer()
        .call_tool(CallToolRequestParams::new("task_list").with_arguments(obj(json!({}))))
        .await?;
    assert_eq!(listed.is_error, Some(false));
    assert_eq!(
        listed.structured_content.as_ref().unwrap()["tasks"][0]["id"],
        "task-mcp-1"
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn mcp_returns_tool_errors_for_invalid_arguments() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
    let server_config = test_config(workspace.path().to_path_buf());

    let server_handle = tokio::spawn(async move {
        let server = RalphMcpServer::new(server_config)?;
        let running = server.serve(server_transport).await?;
        let _ = running.waiting().await?;
        anyhow::Ok(())
    });

    let client = ().serve(client_transport).await?;
    let result = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("task_create")
                .with_arguments(obj(json!({ "id": "task-bad" }))),
        )
        .await?;
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.as_ref().unwrap()["code"],
        "INVALID_PARAMS"
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn mcp_stream_tools_support_polling() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
    let server_config = test_config(workspace.path().to_path_buf());

    let server_handle = tokio::spawn(async move {
        let server = RalphMcpServer::new(server_config)?;
        let running = server.serve(server_transport).await?;
        let _ = running.waiting().await?;
        anyhow::Ok(())
    });

    let client = ().serve(client_transport).await?;
    let subscribed = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("stream_subscribe").with_arguments(obj(json!({
                "topics": ["task.status.changed"],
            }))),
        )
        .await?;
    let subscription_id = subscribed.structured_content.as_ref().unwrap()["subscriptionId"]
        .as_str()
        .expect("subscription id")
        .to_string();

    let created = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("task_create").with_arguments(obj(json!({
                "id": "task-stream-1",
                "title": "Stream task",
                "autoExecute": false,
            }))),
        )
        .await?;
    assert_eq!(created.is_error, Some(false));

    let updated = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("task_update").with_arguments(obj(json!({
                "id": "task-stream-1",
                "status": "running",
            }))),
        )
        .await?;
    assert_eq!(updated.is_error, Some(false));

    let next = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("stream_next").with_arguments(obj(json!({
                "subscriptionId": subscription_id,
                "waitMs": 100,
            }))),
        )
        .await?;
    assert_eq!(next.is_error, Some(false));
    let events = next.structured_content.as_ref().unwrap()["events"]
        .as_array()
        .unwrap();
    assert!(!events.is_empty());
    let cursor = events[0]["cursor"]
        .as_str()
        .expect("stream event cursor")
        .to_string();

    let ack = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("stream_ack").with_arguments(obj(json!({
                "subscriptionId": subscribed.structured_content.as_ref().unwrap()["subscriptionId"].as_str().unwrap(),
                "cursor": cursor,
            }))),
        )
        .await?;
    assert_eq!(ack.is_error, Some(false));

    let unsubscribe = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("stream_unsubscribe").with_arguments(obj(json!({
                "subscriptionId": subscribed.structured_content.as_ref().unwrap()["subscriptionId"].as_str().unwrap(),
            }))),
        )
        .await?;
    assert_eq!(unsubscribe.is_error, Some(false));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn mcp_accepts_paginated_tools_requests() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
    let server_config = test_config(workspace.path().to_path_buf());

    let server_handle = tokio::spawn(async move {
        let server = RalphMcpServer::new(server_config)?;
        let running = server.serve(server_transport).await?;
        let _ = running.waiting().await?;
        anyhow::Ok(())
    });

    let client = ().serve(client_transport).await?;
    let result = client
        .peer()
        .list_tools(Some(
            PaginatedRequestParams::default().with_cursor(Some("0".to_string())),
        ))
        .await?;
    assert!(!result.tools.is_empty());

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
