//! CLI commands for the `ralph memory` namespace.
//!
//! Provides subcommands for managing persistent memories:
//! - `add`: Store a new memory
//! - `list`: List all memories
//! - `show`: Show a single memory by ID
//! - `delete`: Delete a memory by ID
//! - `search`: Find memories by query
//! - `prime`: Output memories for context injection
//! - `init`: Initialize memories file

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ralph_core::{MarkdownMemoryStore, Memory, MemoryType};
use std::path::PathBuf;

/// ANSI color codes for terminal output.
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const GREEN: &str = "\x1b[32m";
}

/// Output format for memory commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table format
    #[default]
    Table,
    /// JSON format for programmatic access
    Json,
    /// Markdown format (for prime command)
    Markdown,
    /// ID-only output for scripting
    Quiet,
}

/// Memory management commands for persistent learning across sessions.
#[derive(Parser, Debug)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommands,

    /// Working directory (default: current directory)
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum MemoryCommands {
    /// Store a new memory
    Add(AddArgs),

    /// List all memories
    List(ListArgs),

    /// Show a single memory by ID
    Show(ShowArgs),

    /// Delete a memory by ID
    Delete(DeleteArgs),

    /// Find memories by query
    Search(SearchArgs),

    /// Output memories for context injection
    Prime(PrimeArgs),

    /// Initialize memories file
    Init(InitArgs),
}

/// Arguments for the `memory add` command.
#[derive(Parser, Debug)]
pub struct AddArgs {
    /// The memory content to store
    pub content: String,

    /// Memory type
    #[arg(short = 't', long, default_value = "pattern")]
    pub r#type: MemoryType,

    /// Comma-separated tags
    #[arg(long)]
    pub tags: Option<String>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `memory list` command.
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Filter by memory type
    #[arg(short = 't', long)]
    pub r#type: Option<MemoryType>,

    /// Show only last N memories
    #[arg(long)]
    pub last: Option<usize>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `memory show` command.
#[derive(Parser, Debug)]
pub struct ShowArgs {
    /// Memory ID (e.g., mem-1737372000-a1b2)
    pub id: String,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `memory delete` command.
#[derive(Parser, Debug)]
pub struct DeleteArgs {
    /// Memory ID to delete
    pub id: String,
}

/// Arguments for the `memory search` command.
#[derive(Parser, Debug)]
pub struct SearchArgs {
    /// Search query (fuzzy match on content/tags)
    pub query: Option<String>,

    /// Filter by memory type
    #[arg(short = 't', long)]
    pub r#type: Option<MemoryType>,

    /// Filter by tags (comma-separated, OR logic)
    #[arg(long)]
    pub tags: Option<String>,

    /// Show all results (no limit)
    #[arg(long)]
    pub all: bool,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `memory prime` command.
#[derive(Parser, Debug)]
pub struct PrimeArgs {
    /// Maximum tokens to include (0 = unlimited)
    #[arg(long)]
    pub budget: Option<usize>,

    /// Filter by types (comma-separated)
    #[arg(short = 't', long)]
    pub r#type: Option<String>,

    /// Filter by tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,

    /// Only memories from last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
    pub format: OutputFormat,
}

/// Arguments for the `memory init` command.
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Overwrite existing file
    #[arg(long)]
    pub force: bool,
}

/// Execute a memory command.
pub fn execute(args: MemoryArgs, use_colors: bool) -> Result<()> {
    let root = args.root.unwrap_or_else(|| PathBuf::from("."));
    let store = MarkdownMemoryStore::with_default_path(&root);

    match args.command {
        MemoryCommands::Add(add_args) => add_command(&store, add_args, use_colors),
        MemoryCommands::List(list_args) => list_command(&store, list_args, use_colors),
        MemoryCommands::Show(show_args) => show_command(&store, show_args, use_colors),
        MemoryCommands::Delete(delete_args) => delete_command(&store, delete_args, use_colors),
        MemoryCommands::Search(search_args) => search_command(&store, search_args, use_colors),
        MemoryCommands::Prime(prime_args) => prime_command(&store, prime_args),
        MemoryCommands::Init(init_args) => init_command(&store, init_args, use_colors),
    }
}

fn add_command(store: &MarkdownMemoryStore, args: AddArgs, use_colors: bool) -> Result<()> {
    // Parse tags
    let tags: Vec<String> = args
        .tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Create and store the memory
    let memory = Memory::new(args.r#type, args.content, tags);
    let id = memory.id.clone();

    store.append(&memory).context("Failed to store memory")?;

    // Output based on format
    match args.format {
        OutputFormat::Quiet => {
            println!("{}", id);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&memory)?;
            println!("{}", json);
        }
        _ => {
            if use_colors {
                println!("{}📝 Memory stored:{} {}", colors::GREEN, colors::RESET, id);
            } else {
                println!("Memory stored: {}", id);
            }
        }
    }

    Ok(())
}

fn list_command(store: &MarkdownMemoryStore, args: ListArgs, use_colors: bool) -> Result<()> {
    let mut memories = store.load().context("Failed to load memories")?;

    // Filter by type if specified
    if let Some(memory_type) = args.r#type {
        memories.retain(|m| m.memory_type == memory_type);
    }

    // Apply last N filter
    if let Some(n) = args.last
        && memories.len() > n
    {
        memories = memories.into_iter().rev().take(n).rev().collect();
    }

    if memories.is_empty() {
        if use_colors {
            println!(
                "{}No memories found.{} Use `ralph memory add` to create one.",
                colors::DIM,
                colors::RESET
            );
        } else {
            println!("No memories found. Use `ralph memory add` to create one.");
        }
        return Ok(());
    }

    output_memories(&memories, args.format, use_colors);
    Ok(())
}

fn show_command(store: &MarkdownMemoryStore, args: ShowArgs, use_colors: bool) -> Result<()> {
    let memory = store
        .get(&args.id)
        .context("Failed to read memories")?
        .ok_or_else(|| anyhow::anyhow!("Memory not found: {}", args.id))?;

    output_memory(&memory, args.format, use_colors);
    Ok(())
}

fn delete_command(store: &MarkdownMemoryStore, args: DeleteArgs, use_colors: bool) -> Result<()> {
    let deleted = store.delete(&args.id).context("Failed to delete memory")?;

    if deleted {
        if use_colors {
            println!(
                "{}🗑️  Memory deleted:{} {}",
                colors::GREEN,
                colors::RESET,
                args.id
            );
        } else {
            println!("Memory deleted: {}", args.id);
        }
        Ok(())
    } else {
        anyhow::bail!("Memory not found: {}", args.id)
    }
}

fn search_command(store: &MarkdownMemoryStore, args: SearchArgs, use_colors: bool) -> Result<()> {
    let mut memories = store.load().context("Failed to load memories")?;

    // Filter by query if provided
    if let Some(ref query) = args.query {
        memories.retain(|m| m.matches_query(query));
    }

    // Filter by type if specified
    if let Some(memory_type) = args.r#type {
        memories.retain(|m| m.memory_type == memory_type);
    }

    // Filter by tags if specified
    if let Some(ref tags_str) = args.tags {
        let tags: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        memories.retain(|m| m.has_any_tag(&tags));
    }

    // Limit results unless --all is specified
    if !args.all && memories.len() > 10 {
        memories.truncate(10);
    }

    if memories.is_empty() {
        if use_colors {
            println!(
                "{}No matching memories found.{}",
                colors::DIM,
                colors::RESET
            );
        } else {
            println!("No matching memories found.");
        }
        return Ok(());
    }

    output_memories(&memories, args.format, use_colors);
    Ok(())
}

fn prime_command(store: &MarkdownMemoryStore, args: PrimeArgs) -> Result<()> {
    let mut memories = store.load().context("Failed to load memories")?;

    // Filter by types if specified
    if let Some(ref types_str) = args.r#type {
        let types: Vec<MemoryType> = types_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if !types.is_empty() {
            memories.retain(|m| types.contains(&m.memory_type));
        }
    }

    // Filter by tags if specified
    if let Some(ref tags_str) = args.tags {
        let tags: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        memories.retain(|m| m.has_any_tag(&tags));
    }

    // Filter by recent days if specified
    if let Some(days) = args.recent {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        let cutoff_str = cutoff.format("%Y-%m-%d").to_string();
        memories.retain(|m| m.created >= cutoff_str);
    }

    if memories.is_empty() {
        return Ok(());
    }

    // Generate output
    let output = match args.format {
        OutputFormat::Json => serde_json::to_string_pretty(&memories)?,
        OutputFormat::Markdown | OutputFormat::Table | OutputFormat::Quiet => {
            format_memories_as_markdown(&memories)
        }
    };

    // Apply budget if specified
    let final_output = if let Some(budget) = args.budget {
        if budget > 0 {
            truncate_to_budget(&output, budget)
        } else {
            output
        }
    } else {
        output
    };

    print!("{}", final_output);
    Ok(())
}

fn init_command(store: &MarkdownMemoryStore, args: InitArgs, use_colors: bool) -> Result<()> {
    store.init(args.force).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            anyhow::anyhow!(
                "Memories file already exists at {}. Use --force to overwrite.",
                store.path().display()
            )
        } else {
            anyhow::anyhow!("Failed to initialize memories: {}", e)
        }
    })?;

    if use_colors {
        println!(
            "{}✓{} Initialized memories file at {}",
            colors::GREEN,
            colors::RESET,
            store.path().display()
        );
    } else {
        println!("Initialized memories file at {}", store.path().display());
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Output Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn output_memories(memories: &[Memory], format: OutputFormat, use_colors: bool) {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(memories).unwrap_or_default();
            println!("{}", json);
        }
        OutputFormat::Markdown => {
            print!("{}", format_memories_as_markdown(memories));
        }
        OutputFormat::Quiet => {
            for memory in memories {
                println!("{}", memory.id);
            }
        }
        OutputFormat::Table => {
            print_memories_table(memories, use_colors);
        }
    }
}

fn output_memory(memory: &Memory, format: OutputFormat, use_colors: bool) {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(memory).unwrap_or_default();
            println!("{}", json);
        }
        OutputFormat::Markdown => {
            println!(
                "### {}\n> {}\n<!-- tags: {} | created: {} -->",
                memory.id,
                memory.content.replace('\n', "\n> "),
                memory.tags.join(", "),
                memory.created
            );
        }
        OutputFormat::Quiet => {
            println!("{}", memory.id);
        }
        OutputFormat::Table => {
            print_memory_detail(memory, use_colors);
        }
    }
}

fn print_memories_table(memories: &[Memory], use_colors: bool) {
    use colors::*;

    // Header
    if use_colors {
        println!(
            "{BOLD}{DIM}  # │ Type     │ ID                      │ Tags             │ Content{RESET}"
        );
        println!(
            "{DIM}────┼──────────┼─────────────────────────┼──────────────────┼─────────────────────────{RESET}"
        );
    } else {
        println!("  # | Type     | ID                      | Tags             | Content");
        println!(
            "----|----------|-------------------------|------------------|-------------------------"
        );
    }

    for (i, memory) in memories.iter().enumerate() {
        let emoji = memory.memory_type.emoji();
        let type_name = memory.memory_type.to_string();
        let tags = if memory.tags.is_empty() {
            "-".to_string()
        } else {
            memory.tags.join(", ")
        };
        let content_preview = if memory.content.len() > 30 {
            format!("{}...", &memory.content[..30].replace('\n', " "))
        } else {
            memory.content.replace('\n', " ")
        };

        if use_colors {
            println!(
                "{DIM}{:>3}{RESET} │ {} {:<6} │ {:<23} │ {:<16} │ {}",
                i + 1,
                emoji,
                type_name,
                truncate_str(&memory.id, 23),
                truncate_str(&tags, 16),
                content_preview
            );
        } else {
            println!(
                "{:>3} | {} {:<6} | {:<23} | {:<16} | {}",
                i + 1,
                emoji,
                type_name,
                truncate_str(&memory.id, 23),
                truncate_str(&tags, 16),
                content_preview
            );
        }
    }

    // Footer
    if use_colors {
        println!("\n{DIM}Total: {} memories{RESET}", memories.len());
    } else {
        println!("\nTotal: {} memories", memories.len());
    }
}

fn print_memory_detail(memory: &Memory, use_colors: bool) {
    use colors::*;

    if use_colors {
        println!("{BOLD}ID:{RESET}      {}", memory.id);
        println!(
            "{BOLD}Type:{RESET}    {} {}",
            memory.memory_type.emoji(),
            memory.memory_type
        );
        println!("{BOLD}Created:{RESET} {}", memory.created);
        println!(
            "{BOLD}Tags:{RESET}    {}",
            if memory.tags.is_empty() {
                "-".to_string()
            } else {
                memory.tags.join(", ")
            }
        );
        println!("{BOLD}Content:{RESET}");
        for line in memory.content.lines() {
            println!("  {}", line);
        }
    } else {
        println!("ID:      {}", memory.id);
        println!(
            "Type:    {} {}",
            memory.memory_type.emoji(),
            memory.memory_type
        );
        println!("Created: {}", memory.created);
        println!(
            "Tags:    {}",
            if memory.tags.is_empty() {
                "-".to_string()
            } else {
                memory.tags.join(", ")
            }
        );
        println!("Content:");
        for line in memory.content.lines() {
            println!("  {}", line);
        }
    }
}

fn format_memories_as_markdown(memories: &[Memory]) -> String {
    let mut output = String::from("# Memories\n");

    // Group by type
    for memory_type in MemoryType::all() {
        let type_memories: Vec<_> = memories
            .iter()
            .filter(|m| m.memory_type == *memory_type)
            .collect();

        if type_memories.is_empty() {
            continue;
        }

        output.push_str(&format!("\n## {}\n", memory_type.section_name()));

        for memory in type_memories {
            output.push_str(&format!(
                "\n### {}\n> {}\n<!-- tags: {} | created: {} -->\n",
                memory.id,
                memory.content.replace('\n', "\n> "),
                memory.tags.join(", "),
                memory.created
            ));
        }
    }

    output
}

/// Truncate content to approximately fit within a token budget.
///
/// Uses a simple heuristic of ~4 characters per token.
fn truncate_to_budget(content: &str, budget: usize) -> String {
    // Rough estimate: 4 chars per token
    let char_budget = budget * 4;

    if content.len() <= char_budget {
        return content.to_string();
    }

    // Find a good break point (end of a memory block)
    let truncated = &content[..char_budget];

    // Try to find the last complete memory block (ends with -->)
    if let Some(last_complete) = truncated.rfind("-->") {
        let end = last_complete + 3;
        // Find the next newline after -->
        let final_end = truncated[end..].find('\n').map_or(end, |n| end + n + 1);
        format!(
            "{}\n\n<!-- truncated: budget {} tokens exceeded -->",
            &content[..final_end],
            budget
        )
    } else {
        format!(
            "{}\n\n<!-- truncated: budget {} tokens exceeded -->",
            truncated, budget
        )
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}
