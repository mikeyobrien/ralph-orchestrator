//! MCP server implementation for Ralph.

use crate::tools::{ListHatsParams, RunParams, StatusParams, StopParams};
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServiceExt;
use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParam, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::tool;
use rmcp::tool_router;
use tracing::info;

/// Ralph MCP Server that exposes orchestration tools.
#[derive(Clone)]
pub struct RalphMcpServer {
    tool_router: ToolRouter<Self>,
}

impl RalphMcpServer {
    /// Creates a new Ralph MCP server.
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for RalphMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a successful text result.
fn text_result(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

/// Creates an error text result.
fn error_result(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![Content::text(text)]))
}

#[tool_router]
impl RalphMcpServer {
    /// Start a new Ralph orchestration session.
    #[tool(description = "Start a new Ralph orchestration session. \
        The prompt should describe the task to accomplish (e.g., 'Implement the UserService class'). \
        Returns a session_id for tracking. Sessions run asynchronously—use ralph_status to monitor \
        progress and ralph_stop to cancel. Each session uses a 'hat' persona from the config to \
        guide behavior.")]
    async fn ralph_run(
        &self,
        Parameters(params): Parameters<RunParams>,
    ) -> Result<CallToolResult, McpError> {
        let config_info = params.config.as_deref().unwrap_or("ralph.yml");
        let dir_info = params.working_dir.as_deref().unwrap_or(".");

        text_result(format!(
            "Ralph session would start with:\n\
             - Prompt: {}\n\
             - Config: {}\n\
             - Working dir: {}\n\
             \n\
             Note: Full implementation pending - this is a stub.",
            params.prompt, config_info, dir_info
        ))
    }

    /// Get status of a Ralph session.
    #[tool(description = "Get the status of a Ralph orchestration session. \
        Returns: status (running|completed|failed|not_found), iteration count, elapsed time, \
        current hat, and recent activity. Poll periodically to monitor long-running sessions.")]
    async fn ralph_status(
        &self,
        Parameters(params): Parameters<StatusParams>,
    ) -> Result<CallToolResult, McpError> {
        text_result(format!(
            "Session {} status: Not implemented yet (stub)",
            params.session_id
        ))
    }

    /// Stop a running Ralph session.
    #[tool(
        description = "Stop a running Ralph orchestration session gracefully. \
        Idempotent: safe to call multiple times or on already-stopped sessions. \
        Returns confirmation with final status. Use when a task is no longer needed \
        or to free resources."
    )]
    async fn ralph_stop(
        &self,
        Parameters(params): Parameters<StopParams>,
    ) -> Result<CallToolResult, McpError> {
        text_result(format!("Would stop session: {} (stub)", params.session_id))
    }

    /// List all Ralph sessions.
    #[tool(
        description = "List all Ralph orchestration sessions (running, completed, and failed). \
        Returns session_id, status, start time, and prompt summary for each. \
        Use to find session IDs for ralph_status or ralph_stop calls."
    )]
    async fn ralph_list_sessions(&self) -> Result<CallToolResult, McpError> {
        text_result("No sessions found (stub implementation)")
    }

    /// List available hats from config.
    #[tool(
        description = "List available hats (agent personas) from Ralph configuration. \
        Returns hat ID, name, description, trigger events, and published events for each. \
        Hats define specialized behaviors—use to understand what personas are available \
        before starting a session. Read-only: does not modify any state."
    )]
    async fn ralph_list_hats(
        &self,
        Parameters(params): Parameters<ListHatsParams>,
    ) -> Result<CallToolResult, McpError> {
        let config_path = params.config.as_deref().unwrap_or("ralph.yml");

        // Try to load config and list hats
        match std::fs::read_to_string(config_path) {
            Ok(content) => match serde_yaml::from_str::<ralph_core::RalphConfig>(&content) {
                Ok(config) => {
                    if config.hats.is_empty() {
                        text_result("No custom hats defined. Running in solo mode (Ralph only).")
                    } else {
                        let mut output = String::from("Available hats:\n\n");
                        for (id, hat) in &config.hats {
                            output.push_str(&format!("- **{}** ({})\n", id, hat.name));
                            if let Some(desc) = &hat.description {
                                output.push_str(&format!("  {}\n", desc));
                            }
                            output.push_str(&format!("  Triggers: {}\n", hat.triggers.join(", ")));
                            if !hat.publishes.is_empty() {
                                output.push_str(&format!(
                                    "  Publishes: {}\n",
                                    hat.publishes.join(", ")
                                ));
                            }
                            output.push('\n');
                        }
                        text_result(output)
                    }
                }
                Err(e) => error_result(format!("Failed to parse config: {}", e)),
            },
            Err(e) => error_result(format!("Failed to read config '{}': {}", config_path, e)),
        }
    }
}

impl ServerHandler for RalphMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Ralph Orchestrator manages AI agent sessions that execute coding tasks autonomously. \
                 Workflow: (1) ralph_list_hats to see available personas, (2) ralph_run to start a session, \
                 (3) ralph_status to monitor progress, (4) ralph_stop if cancellation needed. \
                 Sessions run asynchronously and persist across tool calls. Each session uses 'hats' \
                 (specialized personas) that switch based on events during execution."
                    .to_string(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = ToolCallContext::new(self, request, context);
        self.tool_router.call(ctx).await
    }
}

/// Serves the MCP server over stdio.
///
/// This is the main entry point for `ralph mcp serve`.
pub async fn serve_stdio() -> anyhow::Result<()> {
    info!("Starting Ralph MCP server on stdio");

    let server = RalphMcpServer::new();

    // Create stdio transport and serve
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = server.serve(transport).await?;
    service.waiting().await?;

    info!("Ralph MCP server stopped");
    Ok(())
}
