//! End-to-end tests for the Ralph MCP server.
//!
//! These tests spawn `ralph mcp serve` as a subprocess and communicate
//! with it using the MCP JSON-RPC protocol over stdio.

use anyhow::Result;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;
use wait_timeout::ChildExt;

/// Gets the path to the ralph binary from the target directory.
fn ralph_binary() -> PathBuf {
    // The binary is in the workspace target directory
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // from ralph-mcp to crates
    path.pop(); // from crates to workspace root
    path.push("target");
    path.push(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    });
    path.push("ralph");
    path
}

/// Sends a JSON-RPC request and reads the response.
/// Skips any non-JSON lines (like logging output) before the actual response.
fn send_request(
    stdin: &mut impl Write,
    stdout: &mut BufReader<impl std::io::Read>,
    request: Value,
) -> Result<Value> {
    let request_str = serde_json::to_string(&request)?;
    writeln!(stdin, "{}", request_str)?;
    stdin.flush()?;

    // Read lines until we get valid JSON (skip log output)
    loop {
        let mut response_line = String::new();
        let bytes_read = stdout.read_line(&mut response_line)?;
        if bytes_read == 0 {
            anyhow::bail!("EOF while waiting for response");
        }

        // Try to parse as JSON - if it fails, it's probably a log line
        let trimmed = response_line.trim();
        if trimmed.starts_with('{') {
            let response: Value = serde_json::from_str(trimmed)?;
            return Ok(response);
        }
        // Skip non-JSON lines (logs, etc.)
    }
}

/// Helper to cleanly shut down a child process.
fn shutdown_child(mut child: Child) {
    let _ = child.wait_timeout(Duration::from_secs(2));
    child.kill().ok();
    child.wait().ok();
}

#[test]
fn test_mcp_server_initialization() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create a minimal ralph.yml config
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
"#;
    std::fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Spawn ralph mcp serve
    let mut child = Command::new(ralph_binary())
        .arg("mcp")
        .arg("serve")
        .current_dir(temp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);

    // Send MCP initialize request
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });

    let response = send_request(&mut stdin, &mut reader, init_request)?;

    // Verify initialization response
    assert!(response.get("result").is_some(), "Should have result");
    let result = response.get("result").unwrap();

    // Verify server info
    assert!(result.get("serverInfo").is_some(), "Should have serverInfo");
    assert!(
        result.get("capabilities").is_some(),
        "Should have capabilities"
    );
    assert!(
        result
            .get("capabilities")
            .and_then(|c| c.get("tools"))
            .is_some(),
        "Should have tools capability"
    );

    // Send initialized notification
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let init_str = serde_json::to_string(&initialized)?;
    writeln!(stdin, "{}", init_str)?;
    stdin.flush()?;

    // Clean shutdown
    drop(stdin);
    shutdown_child(child);

    Ok(())
}

#[test]
fn test_mcp_tools_list() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config
    std::fs::write(
        temp_path.join("ralph.yml"),
        "core:\n  scratchpad: \".agent/scratchpad.md\"\n",
    )?;

    // Spawn server
    let mut child = Command::new(ralph_binary())
        .arg("mcp")
        .arg("serve")
        .current_dir(temp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);

    // Initialize first
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request)?;

    // Send initialized notification
    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    writeln!(stdin, "{}", serde_json::to_string(&initialized)?)?;
    stdin.flush()?;

    // List tools
    let list_tools = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    let response = send_request(&mut stdin, &mut reader, list_tools)?;

    // Verify tools list
    let result = response.get("result").expect("Should have result");
    let tools = result.get("tools").expect("Should have tools array");
    let tools_arr = tools.as_array().expect("Tools should be array");

    // Verify expected tools exist
    let tool_names: Vec<&str> = tools_arr
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(
        tool_names.contains(&"ralph_run"),
        "Should have ralph_run tool"
    );
    assert!(
        tool_names.contains(&"ralph_status"),
        "Should have ralph_status tool"
    );
    assert!(
        tool_names.contains(&"ralph_stop"),
        "Should have ralph_stop tool"
    );
    assert!(
        tool_names.contains(&"ralph_list_sessions"),
        "Should have ralph_list_sessions tool"
    );
    assert!(
        tool_names.contains(&"ralph_list_hats"),
        "Should have ralph_list_hats tool"
    );

    // Clean shutdown
    drop(stdin);
    shutdown_child(child);

    Ok(())
}

#[test]
fn test_mcp_call_ralph_list_hats() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config with hats
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
hats:
  builder:
    name: "Builder"
    description: "Builds features"
    triggers: ["build.task"]
    publishes: ["build.done"]
  reviewer:
    name: "Reviewer"
    description: "Reviews code"
    triggers: ["review.request"]
    publishes: ["review.done"]
"#;
    std::fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Spawn server
    let mut child = Command::new(ralph_binary())
        .arg("mcp")
        .arg("serve")
        .current_dir(temp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request)?;

    // Send initialized notification
    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    writeln!(stdin, "{}", serde_json::to_string(&initialized)?)?;
    stdin.flush()?;

    // Call ralph_list_hats tool
    let call_tool = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "ralph_list_hats",
            "arguments": {}
        }
    });
    let response = send_request(&mut stdin, &mut reader, call_tool)?;

    // Verify response contains hat information
    let result = response.get("result").expect("Should have result");
    let content = result.get("content").expect("Should have content");
    let content_arr = content.as_array().expect("Content should be array");

    assert!(!content_arr.is_empty(), "Should have content");
    let text = content_arr[0]
        .get("text")
        .and_then(|t| t.as_str())
        .expect("Should have text content");

    assert!(text.contains("builder"), "Should list builder hat");
    assert!(text.contains("reviewer"), "Should list reviewer hat");
    assert!(text.contains("Builder"), "Should have Builder name");
    assert!(text.contains("Reviewer"), "Should have Reviewer name");

    // Clean shutdown
    drop(stdin);
    shutdown_child(child);

    Ok(())
}

#[test]
fn test_mcp_call_ralph_run() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config
    std::fs::write(
        temp_path.join("ralph.yml"),
        "core:\n  scratchpad: \".agent/scratchpad.md\"\n",
    )?;

    // Spawn server
    let mut child = Command::new(ralph_binary())
        .arg("mcp")
        .arg("serve")
        .current_dir(temp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    send_request(&mut stdin, &mut reader, init_request)?;

    // Send initialized notification
    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    writeln!(stdin, "{}", serde_json::to_string(&initialized)?)?;
    stdin.flush()?;

    // Call ralph_run tool
    let call_tool = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "ralph_run",
            "arguments": {
                "prompt": "Test prompt for MCP"
            }
        }
    });
    let response = send_request(&mut stdin, &mut reader, call_tool)?;

    // Verify response (stub implementation returns info about what would run)
    let result = response.get("result").expect("Should have result");
    let content = result.get("content").expect("Should have content");
    let content_arr = content.as_array().expect("Content should be array");

    assert!(!content_arr.is_empty(), "Should have content");
    let text = content_arr[0]
        .get("text")
        .and_then(|t| t.as_str())
        .expect("Should have text content");

    assert!(
        text.contains("Test prompt for MCP"),
        "Should echo the prompt"
    );
    assert!(
        text.contains("ralph.yml") || text.contains("Config"),
        "Should mention config"
    );

    // Clean shutdown
    drop(stdin);
    shutdown_child(child);

    Ok(())
}
