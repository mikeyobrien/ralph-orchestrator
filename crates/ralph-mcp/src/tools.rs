//! MCP tool parameter definitions for Ralph orchestration.
//!
//! These structs define the JSON Schema for MCP tool parameters.
//! The `schemars` descriptions are what LLMs see when choosing tools.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for the ralph_run tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunParams {
    /// The prompt/task description to run
    #[schemars(
        description = "The task to accomplish. Should be clear and actionable. \
        Examples: 'Implement user authentication with JWT tokens', \
        'Fix the failing tests in src/api/', 'Refactor the database layer for async'"
    )]
    pub prompt: String,

    /// Optional path to config file (defaults to ralph.yml)
    #[schemars(
        description = "Path to Ralph config file. Defaults to 'ralph.yml' in working_dir. \
        The config defines available hats (personas) and their behaviors."
    )]
    #[serde(default)]
    pub config: Option<String>,

    /// Optional working directory
    #[schemars(
        description = "Working directory for the session. Defaults to current directory. \
        All file operations and git commands run relative to this path."
    )]
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// Parameters for the ralph_status tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusParams {
    /// Session ID to check status for
    #[schemars(description = "Session ID returned by ralph_run. \
        Use ralph_list_sessions to find session IDs if unknown.")]
    pub session_id: String,
}

/// Parameters for the ralph_stop tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopParams {
    /// Session ID to stop
    #[schemars(
        description = "Session ID to stop. Safe to call on already-stopped sessions (idempotent). \
        Use ralph_list_sessions to find session IDs if unknown."
    )]
    pub session_id: String,
}

/// Parameters for the ralph_list_hats tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListHatsParams {
    /// Optional path to config file (defaults to ralph.yml)
    #[schemars(
        description = "Path to Ralph config file. Defaults to 'ralph.yml' in current directory. \
        Specify to inspect hats from a different project's config."
    )]
    #[serde(default)]
    pub config: Option<String>,
}
