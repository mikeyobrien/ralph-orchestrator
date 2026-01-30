//! Integration tests for `ralph memory` CLI commands.
//!
//! Tests the memory subcommands per the ralph-tools skill specification.

use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Run ralph tools memory command with given args in the temp directory.
fn ralph_memory(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("tools")
        .arg("memory")
        .args(args)
        .arg("--root")
        .arg(temp_path)
        .current_dir(temp_path)
        .output()
        .expect("Failed to execute ralph command")
}

/// Run ralph tools memory command and assert success.
fn ralph_memory_ok(temp_path: &std::path::Path, args: &[&str]) -> String {
    let output = ralph_memory(temp_path, args);
    assert!(
        output.status.success(),
        "Command 'ralph tools memory {}' failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Init Command Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_init_creates_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Ensure .ralph/agent directory and memories.md don't exist
    let memories_path = temp_path.join(".ralph/agent/memories.md");
    assert!(!memories_path.exists());

    // Run init
    let stdout = ralph_memory_ok(temp_path, &["init"]);

    // File should be created
    assert!(memories_path.exists(), "memories.md should be created");

    // Should contain template structure
    let content = fs::read_to_string(&memories_path)?;
    assert!(content.contains("# Memories"));
    assert!(content.contains("## Patterns"));
    assert!(content.contains("## Decisions"));
    assert!(content.contains("## Fixes"));
    assert!(content.contains("## Context"));

    // Output should confirm initialization
    assert!(
        stdout.contains("Initialized") || stdout.contains("✓"),
        "Output should confirm initialization: {}",
        stdout
    );

    Ok(())
}

#[test]
fn test_memory_init_fails_without_force() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create existing memories file
    let agent_dir = temp_path.join(".ralph/agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(agent_dir.join("memories.md"), "# Existing content")?;

    // Run init without --force
    let output = ralph_memory(temp_path, &["init"]);

    // Should fail
    assert!(
        !output.status.success(),
        "Init should fail when file exists without --force"
    );

    // Original content should be preserved
    let content = fs::read_to_string(agent_dir.join("memories.md"))?;
    assert!(content.contains("Existing content"));

    Ok(())
}

#[test]
fn test_memory_init_force_overwrites() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create existing memories file
    let agent_dir = temp_path.join(".ralph/agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(agent_dir.join("memories.md"), "# Existing content")?;

    // Run init with --force
    ralph_memory_ok(temp_path, &["init", "--force"]);

    // Should have template content, not original
    let content = fs::read_to_string(agent_dir.join("memories.md"))?;
    assert!(content.contains("## Patterns"));
    assert!(!content.contains("Existing content"));

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Add Command Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_add_creates_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add a memory (should create file automatically)
    let stdout = ralph_memory_ok(temp_path, &["add", "test memory content"]);

    // File should exist
    let memories_path = temp_path.join(".ralph/agent/memories.md");
    assert!(memories_path.exists(), "memories.md should be created");

    // Should contain the memory
    let content = fs::read_to_string(&memories_path)?;
    assert!(content.contains("test memory content"));

    // Output should confirm storage
    assert!(
        stdout.contains("Memory stored") || stdout.contains("📝"),
        "Output should confirm storage: {}",
        stdout
    );

    Ok(())
}

#[test]
fn test_memory_add_with_type() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add a fix type memory
    ralph_memory_ok(
        temp_path,
        &["add", "ECONNREFUSED means start docker", "-t", "fix"],
    );

    let content = fs::read_to_string(temp_path.join(".ralph/agent/memories.md"))?;
    assert!(content.contains("## Fixes"));
    assert!(content.contains("ECONNREFUSED means start docker"));

    Ok(())
}

#[test]
fn test_memory_add_with_tags() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add a memory with tags
    ralph_memory_ok(
        temp_path,
        &["add", "uses barrel exports", "--tags", "imports,structure"],
    );

    let content = fs::read_to_string(temp_path.join(".ralph/agent/memories.md"))?;
    assert!(content.contains("uses barrel exports"));
    assert!(content.contains("tags: imports, structure"));

    Ok(())
}

#[test]
fn test_memory_add_quiet_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add with quiet format
    let stdout = ralph_memory_ok(temp_path, &["add", "quiet test", "--format", "quiet"]);

    // Should output only the ID
    assert!(stdout.starts_with("mem-"), "Should output ID: {}", stdout);
    assert!(
        !stdout.contains("Memory stored"),
        "Should not have verbose output"
    );

    Ok(())
}

#[test]
fn test_memory_add_json_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add with JSON format
    let stdout = ralph_memory_ok(temp_path, &["add", "json test", "--format", "json"]);

    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(parsed["content"], "json test");
    assert!(parsed["id"].as_str().unwrap().starts_with("mem-"));

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// List Command Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_list_shows_all() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add multiple memories
    ralph_memory_ok(temp_path, &["add", "first memory"]);
    ralph_memory_ok(temp_path, &["add", "second memory", "-t", "fix"]);
    ralph_memory_ok(temp_path, &["add", "third memory", "-t", "decision"]);

    // List all
    let stdout = ralph_memory_ok(temp_path, &["list"]);

    // Should show all three
    assert!(stdout.contains("first memory") || stdout.contains("first mem..."));
    assert!(stdout.contains("second memory") || stdout.contains("second me..."));
    assert!(stdout.contains("third memory") || stdout.contains("third mem..."));
    assert!(stdout.contains("Showing 3 memories"));

    Ok(())
}

#[test]
fn test_memory_list_filter_by_type() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add memories of different types
    ralph_memory_ok(temp_path, &["add", "pattern one"]);
    ralph_memory_ok(temp_path, &["add", "fix one", "-t", "fix"]);
    ralph_memory_ok(temp_path, &["add", "fix two", "-t", "fix"]);

    // List only fixes
    let stdout = ralph_memory_ok(temp_path, &["list", "-t", "fix"]);

    // Should show only fixes
    assert!(stdout.contains("fix one") || stdout.contains("fix o..."));
    assert!(stdout.contains("fix two") || stdout.contains("fix t..."));
    assert!(!stdout.contains("pattern one"));
    assert!(stdout.contains("Showing 2 memories"));

    Ok(())
}

#[test]
fn test_memory_list_last_n() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add multiple memories
    ralph_memory_ok(temp_path, &["add", "mem1"]);
    ralph_memory_ok(temp_path, &["add", "mem2"]);
    ralph_memory_ok(temp_path, &["add", "mem3"]);
    ralph_memory_ok(temp_path, &["add", "mem4"]);

    // List only last 2
    let stdout = ralph_memory_ok(temp_path, &["list", "--last", "2"]);

    assert!(stdout.contains("Showing 2 memories"));

    Ok(())
}

#[test]
fn test_memory_list_empty() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Initialize empty file
    ralph_memory_ok(temp_path, &["init"]);

    // List should show no memories
    let stdout = ralph_memory_ok(temp_path, &["list"]);

    assert!(
        stdout.contains("No memories yet"),
        "Should indicate empty list: {}",
        stdout
    );

    Ok(())
}

#[test]
fn test_memory_list_json_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "test memory"]);

    let stdout = ralph_memory_ok(temp_path, &["list", "--format", "json"]);

    // Should be valid JSON array
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["content"], "test memory");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Show Command Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_show_by_id() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add and capture ID
    let add_stdout = ralph_memory_ok(temp_path, &["add", "show test memory", "--format", "quiet"]);
    let memory_id = add_stdout.trim();

    // Show by ID
    let stdout = ralph_memory_ok(temp_path, &["show", memory_id]);

    assert!(stdout.contains(memory_id));
    assert!(stdout.contains("show test memory"));

    Ok(())
}

#[test]
fn test_memory_show_not_found() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["init"]);

    // Try to show non-existent ID
    let output = ralph_memory(temp_path, &["show", "mem-0000000000-xxxx"]);

    assert!(!output.status.success(), "Should fail for non-existent ID");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("Not found"),
        "Should indicate not found: {}",
        stderr
    );

    Ok(())
}

#[test]
fn test_memory_show_json_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    let add_stdout = ralph_memory_ok(temp_path, &["add", "json show test", "--format", "quiet"]);
    let memory_id = add_stdout.trim();

    let stdout = ralph_memory_ok(temp_path, &["show", memory_id, "--format", "json"]);

    let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(parsed["content"], "json show test");
    assert_eq!(parsed["id"], memory_id);

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Delete Command Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_delete_removes_entry() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add a memory
    let add_stdout = ralph_memory_ok(temp_path, &["add", "to be deleted", "--format", "quiet"]);
    let memory_id = add_stdout.trim();

    // Verify it exists
    let content_before = fs::read_to_string(temp_path.join(".ralph/agent/memories.md"))?;
    assert!(content_before.contains("to be deleted"));

    // Delete it
    let stdout = ralph_memory_ok(temp_path, &["delete", memory_id]);
    assert!(
        stdout.contains("deleted") || stdout.contains("🗑"),
        "Should confirm deletion: {}",
        stdout
    );

    // Verify it's gone
    let content_after = fs::read_to_string(temp_path.join(".ralph/agent/memories.md"))?;
    assert!(!content_after.contains("to be deleted"));
    assert!(!content_after.contains(memory_id));

    Ok(())
}

#[test]
fn test_memory_delete_not_found() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["init"]);

    // Try to delete non-existent ID
    let output = ralph_memory(temp_path, &["delete", "mem-0000000000-xxxx"]);

    assert!(!output.status.success(), "Should fail for non-existent ID");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("Not found"),
        "Should indicate not found: {}",
        stderr
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Search Command Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_search_finds_by_content() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "uses barrel exports everywhere"]);
    ralph_memory_ok(temp_path, &["add", "API routes use kebab-case"]);
    ralph_memory_ok(temp_path, &["add", "chose Postgres over SQLite"]);

    // Search for "barrel"
    let stdout = ralph_memory_ok(temp_path, &["search", "barrel"]);

    assert!(
        stdout.contains("barrel exports") || stdout.contains("barrel e..."),
        "Should find barrel memory: {}",
        stdout
    );
    // Should not show unrelated memories
    assert!(!stdout.contains("Postgres"));

    Ok(())
}

#[test]
fn test_memory_search_finds_by_tags() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(
        temp_path,
        &["add", "docker is slow", "--tags", "docker,perf"],
    );
    ralph_memory_ok(
        temp_path,
        &["add", "nginx config here", "--tags", "nginx,config"],
    );
    ralph_memory_ok(temp_path, &["add", "docker compose up", "--tags", "docker"]);

    // Search by tag
    let stdout = ralph_memory_ok(temp_path, &["search", "--tags", "docker"]);

    assert!(
        stdout.contains("docker is slow") || stdout.contains("docker i..."),
        "Should find first docker memory: {}",
        stdout
    );
    assert!(
        stdout.contains("docker compose") || stdout.contains("docker c..."),
        "Should find second docker memory: {}",
        stdout
    );
    assert!(!stdout.contains("nginx"));

    Ok(())
}

#[test]
fn test_memory_search_filter_by_type() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "a pattern", "-t", "pattern"]);
    ralph_memory_ok(temp_path, &["add", "a fix", "-t", "fix"]);
    ralph_memory_ok(temp_path, &["add", "another fix", "-t", "fix"]);

    // Search only fixes
    let stdout = ralph_memory_ok(temp_path, &["search", "-t", "fix"]);

    assert!(stdout.contains("a fix") || stdout.contains("a f..."));
    assert!(stdout.contains("another fix") || stdout.contains("another ..."));
    assert!(!stdout.contains("a pattern"));

    Ok(())
}

#[test]
fn test_memory_search_no_results() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "something else entirely"]);

    let stdout = ralph_memory_ok(temp_path, &["search", "nonexistent_xyz"]);

    assert!(
        stdout.contains("No matching"),
        "Should indicate no results: {}",
        stdout
    );

    Ok(())
}

#[test]
fn test_memory_search_json_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "searchable memory"]);

    let stdout = ralph_memory_ok(temp_path, &["search", "searchable", "--format", "json"]);

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["content"], "searchable memory");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Prime Command Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_prime_outputs_markdown() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "prime test one"]);
    ralph_memory_ok(temp_path, &["add", "prime test two", "-t", "fix"]);

    let stdout = ralph_memory_ok(temp_path, &["prime"]);

    // Should output markdown structure
    assert!(stdout.contains("# Memories"));
    assert!(stdout.contains("## Patterns"));
    assert!(stdout.contains("prime test one"));
    assert!(stdout.contains("## Fixes"));
    assert!(stdout.contains("prime test two"));

    Ok(())
}

#[test]
fn test_memory_prime_respects_budget() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Add several memories to create some content
    for i in 0..10 {
        ralph_memory_ok(
            temp_path,
            &[
                "add",
                &format!(
                    "This is memory number {} with some longer content to fill space",
                    i
                ),
            ],
        );
    }

    // Prime with a small budget (100 tokens ≈ 400 chars)
    let stdout = ralph_memory_ok(temp_path, &["prime", "--budget", "100"]);

    // Should be truncated
    assert!(
        stdout.contains("truncated") || stdout.len() < 600,
        "Should be truncated: len={}, content={}",
        stdout.len(),
        &stdout[..stdout.len().min(200)]
    );

    Ok(())
}

#[test]
fn test_memory_prime_filter_by_type() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "pattern memory"]);
    ralph_memory_ok(temp_path, &["add", "fix memory", "-t", "fix"]);

    // Prime only patterns
    let stdout = ralph_memory_ok(temp_path, &["prime", "-t", "pattern"]);

    assert!(stdout.contains("pattern memory"));
    assert!(!stdout.contains("fix memory"));

    Ok(())
}

#[test]
fn test_memory_prime_filter_by_tags() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "docker memory", "--tags", "docker"]);
    ralph_memory_ok(temp_path, &["add", "nginx memory", "--tags", "nginx"]);

    // Prime only docker tag
    let stdout = ralph_memory_ok(temp_path, &["prime", "--tags", "docker"]);

    assert!(stdout.contains("docker memory"));
    assert!(!stdout.contains("nginx memory"));

    Ok(())
}

#[test]
fn test_memory_prime_json_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "json prime test"]);

    let stdout = ralph_memory_ok(temp_path, &["prime", "--format", "json"]);

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["content"], "json prime test");

    Ok(())
}

#[test]
fn test_memory_prime_empty() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["init"]);

    // Prime should produce no output for empty memories
    let stdout = ralph_memory_ok(temp_path, &["prime"]);

    assert!(
        stdout.is_empty() || stdout.trim().is_empty(),
        "Should be empty for no memories: '{}'",
        stdout
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Color Output Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_memory_list_color_never() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    ralph_memory_ok(temp_path, &["add", "color test"]);

    // Run with --color never via the main CLI
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("--color")
        .arg("never")
        .arg("tools")
        .arg("memory")
        .arg("list")
        .arg("--root")
        .arg(temp_path)
        .current_dir(temp_path)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "Should not contain ANSI codes with --color never"
    );

    Ok(())
}
