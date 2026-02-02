//! CLI commands for the `ralph tools skill` namespace.
//!
//! Provides subcommands for interacting with skills:
//! - `load`: Load a skill by name and output its content
//! - `list`: List available skills

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ralph_core::{RalphConfig, SkillRegistry};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Output format for skill list command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table format
    #[default]
    Table,
    /// JSON format for programmatic access
    Json,
    /// Name-only output for scripting
    Quiet,
}

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

    /// List available skills
    List(ListArgs),
}

#[derive(Parser, Debug)]
pub struct LoadArgs {
    /// Name of the skill to load
    pub name: String,
}

/// Arguments for the `skill list` command.
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Execute a skill command.
pub fn execute(args: SkillArgs) -> Result<()> {
    let root = resolve_root(args.root)?;

    match args.command {
        SkillCommands::Load(load_args) => execute_load(&root, &load_args.name),
        SkillCommands::List(list_args) => execute_list(&root, list_args),
    }
}

fn execute_load(root: &Path, name: &str) -> Result<()> {
    let registry = build_registry(root)?;

    match registry.load_skill(name) {
        Some(content) => {
            print!("{content}");
            Ok(())
        }
        None => {
            eprintln!("Error: skill '{}' not found", name);
            let mut names: Vec<String> = registry
                .skills_for_hat(None)
                .into_iter()
                .map(|skill| skill.name.clone())
                .collect();
            names.sort();
            if names.is_empty() {
                eprintln!("No skills discovered. Check skills.dirs in ralph.yml or use --root.");
            } else {
                eprintln!("Available skills: {}", names.join(", "));
            }
            std::process::exit(1);
        }
    }
}

fn execute_list(root: &Path, args: ListArgs) -> Result<()> {
    let registry = build_registry(root)?;
    let mut skills = registry.skills_for_hat(None);
    skills.sort_by_key(|skill| skill.name.clone());

    match args.format {
        OutputFormat::Table => {
            if skills.is_empty() {
                println!("No skills found");
                return Ok(());
            }

            println!("{:<24} {:<28} {:<60}", "Name", "Source", "Description");
            println!("{}", "-".repeat(112));

            for skill in skills {
                let name = crate::display::truncate(&skill.name, 24);
                let source = format_source(skill);
                let source_truncated = crate::display::truncate(&source, 28);
                let description = if skill.description.is_empty() {
                    "(no description)".to_string()
                } else {
                    skill.description.clone()
                };
                let description_truncated = crate::display::truncate(&description, 60);

                println!(
                    "{:<24} {:<28} {:<60}",
                    name, source_truncated, description_truncated
                );
            }
        }
        OutputFormat::Json => {
            let items: Vec<SkillListItem> = skills.into_iter().map(SkillListItem::from).collect();
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
        OutputFormat::Quiet => {
            for skill in skills {
                println!("{}", skill.name);
            }
        }
    }

    Ok(())
}

fn build_registry(root: &Path) -> Result<SkillRegistry> {
    let config = load_config(root);
    let active_backend = Some(config.cli.backend.as_str());
    SkillRegistry::from_config(&config.skills, root, active_backend)
        .context("Failed to build skill registry")
}

fn format_source(skill: &ralph_core::SkillEntry) -> String {
    match &skill.source {
        ralph_core::SkillSource::BuiltIn => "built-in".to_string(),
        ralph_core::SkillSource::File(path) => path.display().to_string(),
    }
}

#[derive(Debug, Serialize)]
struct SkillListItem {
    name: String,
    description: String,
    source: String,
    path: Option<String>,
    hats: Vec<String>,
    backends: Vec<String>,
    tags: Vec<String>,
    auto_inject: bool,
}

impl From<&ralph_core::SkillEntry> for SkillListItem {
    fn from(skill: &ralph_core::SkillEntry) -> Self {
        let (source, path) = match &skill.source {
            ralph_core::SkillSource::BuiltIn => ("built-in".to_string(), None),
            ralph_core::SkillSource::File(path) => {
                ("file".to_string(), Some(path.display().to_string()))
            }
        };

        Self {
            name: skill.name.clone(),
            description: skill.description.clone(),
            source,
            path,
            hats: skill.hats.clone(),
            backends: skill.backends.clone(),
            tags: skill.tags.clone(),
            auto_inject: skill.auto_inject,
        }
    }
}

fn resolve_root(explicit_root: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = explicit_root {
        return Ok(root);
    }

    let cwd = std::env::current_dir().context("failed to get current directory")?;
    if let Some(found) = find_workspace_root(&cwd) {
        return Ok(found);
    }

    Ok(cwd)
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if dir.join("ralph.yml").exists() || dir.join("ralph.yaml").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn find_default_skills_dir(root: &Path) -> Option<PathBuf> {
    let default_dir = root.join(".claude/skills");
    if default_dir.is_dir() {
        return Some(default_dir);
    }

    let cwd = std::env::current_dir().ok()?;
    if !cwd.starts_with(root) {
        return None;
    }

    let mut current = Some(cwd.as_path());
    while let Some(dir) = current {
        let candidate = dir.join(".claude/skills");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if dir == root {
            break;
        }
        current = dir.parent();
    }

    // Fallback: if the workspace root is nested (ralph.yml inside a subdir),
    // allow discovering a parent-level .claude/skills directory.
    let mut current = root.parent();
    while let Some(dir) = current {
        let candidate = dir.join(".claude/skills");
        if candidate.is_dir() {
            return Some(candidate);
        }
        current = dir.parent();
    }

    None
}

fn resolve_configured_skills_dir(root: &Path, dir: &Path) -> PathBuf {
    if dir.is_absolute() {
        return dir.to_path_buf();
    }

    let candidate = root.join(dir);
    if candidate.is_dir() {
        return candidate;
    }

    let mut current = root.parent();
    while let Some(parent) = current {
        let candidate = parent.join(dir);
        if candidate.is_dir() {
            return candidate;
        }
        current = parent.parent();
    }

    candidate
}

/// Load config from workspace root, falling back to defaults.
fn load_config(root: &Path) -> RalphConfig {
    // Try standard config file names
    let candidates = ["ralph.yml", "ralph.yaml"];
    let mut config = None;
    for candidate in &candidates {
        let path = root.join(candidate);
        if path.exists()
            && let Ok(loaded) = RalphConfig::from_file(&path)
        {
            config = Some(loaded);
            break;
        }
    }

    let mut config = config.unwrap_or_default();
    config.normalize();

    if config.skills.dirs.is_empty() {
        if let Some(default_dir) = find_default_skills_dir(root) {
            config.skills.dirs.push(default_dir);
        }
    } else {
        config.skills.dirs = config
            .skills
            .dirs
            .iter()
            .map(|dir| resolve_configured_skills_dir(root, dir))
            .collect();
    }

    config
}
