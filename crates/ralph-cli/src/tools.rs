//! CLI commands for the `ralph tools` namespace.
//!
//! Ralph's runtime tools - things Ralph uses during orchestration.
//! This namespace contains agent-facing tools, while top-level commands
//! are user-facing.
//!
//! Subcommands:
//! - `memory`: Persistent memories for accumulated learning
//! - `task`: Work item tracking (beads-lite)
//! - `skill`: Load skill content on demand

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::memory;
use crate::skill_cli;
use crate::task_cli;

/// Ralph's runtime tools (agent-facing).
#[derive(Parser, Debug)]
pub struct ToolsArgs {
    #[command(subcommand)]
    pub command: ToolsCommands,
}

#[derive(Subcommand, Debug)]
pub enum ToolsCommands {
    /// Manage persistent memories for accumulated learning
    Memory(memory::MemoryArgs),

    /// Manage work items (task tracking)
    Task(task_cli::TaskArgs),

    /// Load and manage skills
    Skill(skill_cli::SkillArgs),
}

/// Execute a tools command.
pub fn execute(args: ToolsArgs, use_colors: bool) -> Result<()> {
    match args.command {
        ToolsCommands::Memory(memory_args) => memory::execute(memory_args, use_colors),
        ToolsCommands::Task(task_args) => task_cli::execute(task_args, use_colors),
        ToolsCommands::Skill(skill_args) => skill_cli::execute(skill_args),
    }
}
