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
    pub const CYAN: &str = "\x1b[36m";
}

/// Format a date string as a human-readable relative time.
fn format_relative_date(date_str: &str) -> String {
    format_relative_date_with_today(date_str, chrono::Utc::now().date_naive())
}

fn format_relative_date_with_today(date_str: &str, today: chrono::NaiveDate) -> String {
    let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return date_str.to_string();
    };

    let days = (today - date).num_days();

    match days {
        0 => "today".to_string(),
        1 => "yesterday".to_string(),
        2..=6 => format!("{} days ago", days),
        7..=13 => "1 week ago".to_string(),
        14..=20 => "2 weeks ago".to_string(),
        21..=27 => "3 weeks ago".to_string(),
        28..=44 => "1 month ago".to_string(),
        45..=89 => "2 months ago".to_string(),
        _ => {
            let months = days / 30;
            if months < 12 {
                format!("{} months ago", months)
            } else {
                date_str.to_string()
            }
        }
    }
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
            println!("\n{}No memories yet.{}\n", colors::DIM, colors::RESET);
            println!("Create your first memory:");
            println!(
                "  {}ralph tools memory add \"<content>\" -t pattern --tags tag1,tag2{}\n",
                colors::CYAN,
                colors::RESET
            );
            println!("Memory types: pattern, decision, fix, context");
            println!();
        } else {
            println!("\nNo memories yet.\n");
            println!("Create your first memory:");
            println!("  ralph tools memory add \"<content>\" -t pattern --tags tag1,tag2\n");
            println!("Memory types: pattern, decision, fix, context");
            println!();
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
    let all_memories = store.load().context("Failed to load memories")?;
    let total_count = all_memories.len();
    let mut memories = all_memories;

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

    let match_count = memories.len();
    let truncated = !args.all && match_count > 10;

    // Limit results unless --all is specified
    if truncated {
        memories.truncate(10);
    }

    if memories.is_empty() {
        if use_colors {
            println!(
                "\n{}No matching memories found in {} total memories.{}",
                colors::DIM,
                total_count,
                colors::RESET
            );
            println!(
                "{}Try a different search term or use `ralph tools memory list` to see all.{}\n",
                colors::DIM,
                colors::RESET
            );
        } else {
            println!(
                "\nNo matching memories found in {} total memories.",
                total_count
            );
            println!("Try a different search term or use `ralph tools memory list` to see all.\n");
        }
        return Ok(());
    }

    // Print search header (only for table format)
    if args.format == OutputFormat::Table {
        if use_colors {
            if let Some(ref query) = args.query {
                println!(
                    "\n{}Search results for \"{}\"{} ({} of {} memories)",
                    colors::DIM,
                    query,
                    colors::RESET,
                    match_count,
                    total_count
                );
            }
        } else if let Some(ref query) = args.query {
            println!(
                "\nSearch results for \"{}\" ({} of {} memories)",
                query, match_count, total_count
            );
        }
    }

    output_memories(&memories, args.format, use_colors);

    // Show truncation hint (only for table format)
    if truncated && args.format == OutputFormat::Table {
        if use_colors {
            println!(
                "{}Showing 10 of {} matches • Use --all to see all results{}\n",
                colors::DIM,
                match_count,
                colors::RESET
            );
        } else {
            println!(
                "Showing 10 of {} matches • Use --all to see all results\n",
                match_count
            );
        }
    }

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

    // Header - simplified columns: Type, Age, Tags, Content
    if use_colors {
        println!("\n{BOLD}  # │ Type      │ Age          │ Tags             │ Content{RESET}");
        println!(
            "{DIM}────┼───────────┼──────────────┼──────────────────┼────────────────────────────────────────{RESET}"
        );
    } else {
        println!("\n  # | Type      | Age          | Tags             | Content");
        println!(
            "----|-----------|--------------|------------------|----------------------------------------"
        );
    }

    for (i, memory) in memories.iter().enumerate() {
        let emoji = memory.memory_type.emoji();
        let type_name = memory.memory_type.to_string();
        let age = format_relative_date(&memory.created);
        let tags = if memory.tags.is_empty() {
            "-".to_string()
        } else {
            memory.tags.join(", ")
        };
        // Longer content preview (50 chars) for better readability
        let content_preview = if memory.content.len() > 50 {
            format!("{}…", &memory.content[..50].replace('\n', " "))
        } else {
            memory.content.replace('\n', " ")
        };

        if use_colors {
            println!(
                "{DIM}{:>3}{RESET} │ {} {:<7} │ {:<12} │ {CYAN}{:<16}{RESET} │ {}",
                i + 1,
                emoji,
                type_name,
                age,
                truncate_str(&tags, 16),
                content_preview
            );
        } else {
            println!(
                "{:>3} | {} {:<7} | {:<12} | {:<16} | {}",
                i + 1,
                emoji,
                type_name,
                age,
                truncate_str(&tags, 16),
                content_preview
            );
        }
    }

    // Footer with hint
    if use_colors {
        println!(
            "\n{DIM}Showing {} memories • Use `ralph tools memory show <id>` for details{RESET}",
            memories.len()
        );
    } else {
        println!(
            "\nShowing {} memories • Use `ralph tools memory show <id>` for details",
            memories.len()
        );
    }
}

fn print_memory_detail(memory: &Memory, use_colors: bool) {
    use colors::*;

    let relative_date = format_relative_date(&memory.created);
    let tags_display = if memory.tags.is_empty() {
        "-".to_string()
    } else {
        memory.tags.join(", ")
    };

    if use_colors {
        println!();
        println!("{DIM}╭────────────────────────────────────────────────────────────────╮{RESET}");
        println!(
            "{DIM}│{RESET} {} {BOLD}{}{RESET}",
            memory.memory_type.emoji(),
            memory.memory_type.to_string().to_uppercase()
        );
        println!("{DIM}╰────────────────────────────────────────────────────────────────╯{RESET}");
        println!();
        println!("  {BOLD}ID:{RESET}      {DIM}{}{RESET}", memory.id);
        println!(
            "  {BOLD}Created:{RESET} {} {DIM}({}){RESET}",
            relative_date, memory.created
        );
        println!("  {BOLD}Tags:{RESET}    {CYAN}{}{RESET}", tags_display);
        println!();
        println!("  {BOLD}Content:{RESET}");
        println!("{DIM}  ─────────────────────────────────────────────────────────────{RESET}");
        for line in memory.content.lines() {
            println!("  {}", line);
        }
        println!();
    } else {
        println!();
        println!("┌────────────────────────────────────────────────────────────────┐");
        println!(
            "│ {} {}",
            memory.memory_type.emoji(),
            memory.memory_type.to_string().to_uppercase()
        );
        println!("└────────────────────────────────────────────────────────────────┘");
        println!();
        println!("  ID:      {}", memory.id);
        println!("  Created: {} ({})", relative_date, memory.created);
        println!("  Tags:    {}", tags_display);
        println!();
        println!("  Content:");
        println!("  ─────────────────────────────────────────────────────────────");
        for line in memory.content.lines() {
            println!("  {}", line);
        }
        println!();
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate};

    fn fixed_today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 31).expect("valid date")
    }

    fn date_days_ago(days: i64) -> String {
        (fixed_today() - Duration::days(days))
            .format("%Y-%m-%d")
            .to_string()
    }

    #[test]
    fn format_relative_date_with_today_handles_ranges() {
        let today = fixed_today();
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(0), today),
            "today"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(1), today),
            "yesterday"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(2), today),
            "2 days ago"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(7), today),
            "1 week ago"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(14), today),
            "2 weeks ago"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(21), today),
            "3 weeks ago"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(28), today),
            "1 month ago"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(45), today),
            "2 months ago"
        );
        assert_eq!(
            format_relative_date_with_today(&date_days_ago(90), today),
            "3 months ago"
        );
    }

    #[test]
    fn format_relative_date_with_today_returns_input_on_invalid() {
        let today = fixed_today();
        let value = "not-a-date";
        assert_eq!(format_relative_date_with_today(value, today), "not-a-date");
    }

    #[test]
    fn format_relative_date_with_today_returns_date_for_old_entries() {
        let today = fixed_today();
        let date_str = date_days_ago(400);
        assert_eq!(format_relative_date_with_today(&date_str, today), date_str);
    }

    #[test]
    fn truncate_to_budget_prefers_complete_memory_blocks() {
        let content = "### mem-1\n> hi\n<!-- tags: a | created: 2026-01-31 -->\n\n\
### mem-2\n> more\n<!-- tags: b | created: 2026-01-31 -->\n"
            .to_string();
        let first_end = content.find("-->").expect("marker") + 3;
        let budget = (first_end + 6).div_ceil(4);
        let truncated = truncate_to_budget(&content, budget);

        assert!(truncated.contains("mem-1"));
        assert!(!truncated.contains("mem-2"));
        assert!(truncated.contains("<!-- truncated: budget"));
    }

    #[test]
    fn truncate_to_budget_falls_back_without_marker() {
        let content = "abcdefghijklmnopqrstuvwxyz";
        let truncated = truncate_to_budget(content, 1);
        assert!(truncated.starts_with("abcd"));
        assert!(truncated.contains("truncated: budget 1 tokens exceeded"));
    }

    #[test]
    fn truncate_str_handles_short_and_long_values() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("1234567890", 5), "1234…");
    }

    #[test]
    fn format_memories_as_markdown_groups_by_type() {
        let memories = vec![
            Memory {
                id: "mem-1".to_string(),
                memory_type: MemoryType::Pattern,
                content: "alpha".to_string(),
                tags: vec!["tag1".to_string()],
                created: "2026-01-31".to_string(),
            },
            Memory {
                id: "mem-2".to_string(),
                memory_type: MemoryType::Fix,
                content: "beta".to_string(),
                tags: vec![],
                created: "2026-01-31".to_string(),
            },
        ];

        let output = format_memories_as_markdown(&memories);
        assert!(output.contains("# Memories"));
        assert!(output.contains("## Patterns"));
        assert!(output.contains("## Fixes"));
        assert!(!output.contains("## Decisions"));
        assert!(output.contains("mem-1"));
        assert!(output.contains("mem-2"));
    }
}
