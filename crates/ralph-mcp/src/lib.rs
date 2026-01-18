//! # ralph-mcp
//!
//! MCP (Model Context Protocol) server for Ralph Orchestrator.
//!
//! This crate provides an MCP server that exposes Ralph's orchestration
//! capabilities to MCP clients like Claude Desktop. Tools available:
//!
//! - `ralph_run` - Start a new Ralph orchestration session
//! - `ralph_status` - Get status of a running/completed session
//! - `ralph_stop` - Stop a running session
//! - `ralph_list_sessions` - List all sessions
//! - `ralph_list_hats` - List available hats from config

mod server;
mod tools;

pub use server::{RalphMcpServer, serve_stdio};
