//! CLI commands for the `ralph tools skill` namespace.
//!
//! Provides subcommands for interacting with skills:
//! - `load`: Load a skill by name and output its content

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ralph_core::{RalphConfig, SkillRegistry};
use std::path::{Path, PathBuf};

/// Skill management commands.
#[derive(Parser, Debug)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommands,

    /// Working directory (default: current directory)
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum SkillCommands {
    /// Load a skill by name and output its content
    Load(LoadArgs),
}

#[derive(Parser, Debug)]
pub struct LoadArgs {
    /// Name of the skill to load
    pub name: String,
}

/// Execute a skill command.
pub fn execute(args: SkillArgs) -> Result<()> {
    let root = args
        .root
        .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

    match args.command {
        SkillCommands::Load(load_args) => execute_load(&root, &load_args.name),
    }
}

fn execute_load(root: &Path, name: &str) -> Result<()> {
    // Load config from workspace root, fall back to defaults
    let config = load_config(root);
    let registry = SkillRegistry::from_config(&config.skills, root, None)
        .context("Failed to build skill registry")?;

    match registry.load_skill(name) {
        Some(content) => {
            print!("{content}");
            Ok(())
        }
        None => {
            eprintln!("Error: skill '{}' not found", name);
            std::process::exit(1);
        }
    }
}

/// Load config from workspace root, falling back to defaults.
fn load_config(root: &Path) -> RalphConfig {
    // Try standard config file names
    let candidates = ["ralph.yml", "ralph.yaml"];
    for candidate in &candidates {
        let path = root.join(candidate);
        if path.exists()
            && let Ok(config) = RalphConfig::from_file(&path)
        {
            return config;
        }
    }
    RalphConfig::default()
}
