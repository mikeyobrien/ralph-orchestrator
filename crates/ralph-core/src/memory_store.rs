//! Markdown-based memory storage.
//!
//! Provides `MarkdownMemoryStore` for reading, writing, and managing
//! memories in the `.ralph/agent/memories.md` file format.
//!
//! # Multi-loop Safety
//!
//! When multiple Ralph loops run concurrently (in worktrees), this store uses
//! file locking to ensure safe concurrent access:
//!
//! - **Shared locks** for reading: Multiple loops can read simultaneously
//! - **Exclusive locks** for writing: Only one loop can write at a time
//!
//! The `MarkdownMemoryStore` is Clone because it doesn't hold the lock;
//! locks are acquired for each operation.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::text::floor_char_boundary;

use crate::file_lock::FileLock;
use crate::memory::{Memory, MemoryType};
use crate::memory_parser::parse_memories;

/// Default path for the memories file relative to the workspace root.
pub const DEFAULT_MEMORIES_PATH: &str = ".ralph/agent/memories.md";

/// A store for managing memories in markdown format.
///
/// This store uses a single markdown file (`.ralph/agent/memories.md`) to persist
/// memories. The file format is human-readable and version-control friendly.
///
/// # Multi-loop Safety
///
/// All read operations use shared locks, and all write operations use
/// exclusive locks. This ensures safe concurrent access from multiple
/// Ralph loops running in worktrees.
#[derive(Debug, Clone)]
pub struct MarkdownMemoryStore {
    path: PathBuf,
}

impl MarkdownMemoryStore {
    /// Creates a new store at the given path.
    ///
    /// The path should point to a `.md` file (typically `.ralph/agent/memories.md`).
    /// The file does not need to exist - it will be created when first written to.
    #[must_use]
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Creates a store with the default path (`.ralph/agent/memories.md`) under the given root.
    #[must_use]
    pub fn with_default_path(root: impl AsRef<Path>) -> Self {
        Self::new(root.as_ref().join(DEFAULT_MEMORIES_PATH))
    }

    /// Returns the path to the memories file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns true if the memories file exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Initializes the memories file with an empty template.
    ///
    /// If `force` is false and the file already exists, this returns an error.
    /// Uses an exclusive lock to prevent concurrent writes.
    pub fn init(&self, force: bool) -> io::Result<()> {
        let lock = FileLock::new(&self.path)?;
        let _guard = lock.exclusive()?;

        if self.exists() && !force {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Memories file already exists: {}", self.path.display()),
            ));
        }

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.path, self.template())
    }

    /// Reads all memories from the file.
    ///
    /// Returns an empty vector if the file doesn't exist.
    /// Uses a shared lock to allow concurrent reads from multiple loops.
    pub fn load(&self) -> io::Result<Vec<Memory>> {
        if !self.exists() {
            return Ok(Vec::new());
        }

        let lock = FileLock::new(&self.path)?;
        let _guard = lock.shared()?;

        let content = fs::read_to_string(&self.path)?;
        Ok(parse_memories(&content))
    }

    /// Appends a new memory to the file.
    ///
    /// The memory is inserted into its appropriate section (based on type).
    /// If the file doesn't exist, it's created with the template first.
    /// Uses an exclusive lock to prevent concurrent writes.
    pub fn append(&self, memory: &Memory) -> io::Result<()> {
        let lock = FileLock::new(&self.path)?;
        let _guard = lock.exclusive()?;

        let content = if self.exists() {
            fs::read_to_string(&self.path)?
        } else {
            // Ensure parent directory exists
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            self.template()
        };

        let section = format!("## {}", memory.memory_type.section_name());
        let memory_block = self.format_memory(memory);

        let new_content = if let Some(pos) = self.find_section_insert_point(&content, &section) {
            format!("{}{}{}", &content[..pos], memory_block, &content[pos..])
        } else {
            // Section doesn't exist, append section + memory at end
            format!("{}\n{}\n{}", content.trim_end(), section, memory_block)
        };

        fs::write(&self.path, new_content)
    }

    /// Deletes a memory by ID.
    ///
    /// Returns `Ok(true)` if the memory was found and deleted,
    /// `Ok(false)` if the memory was not found.
    /// Uses an exclusive lock to prevent concurrent writes.
    pub fn delete(&self, id: &str) -> io::Result<bool> {
        if !self.exists() {
            return Ok(false);
        }

        let lock = FileLock::new(&self.path)?;
        let _guard = lock.exclusive()?;

        let content = fs::read_to_string(&self.path)?;
        let memories = parse_memories(&content);

        if !memories.iter().any(|m| m.id == id) {
            return Ok(false);
        }

        // Rebuild the file without the deleted memory
        let remaining: Vec<_> = memories.into_iter().filter(|m| m.id != id).collect();
        self.write_all_internal(&remaining)?;

        Ok(true)
    }

    /// Returns the memory with the given ID, if it exists.
    pub fn get(&self, id: &str) -> io::Result<Option<Memory>> {
        let memories = self.load()?;
        Ok(memories.into_iter().find(|m| m.id == id))
    }

    /// Searches memories by query string.
    ///
    /// Matches against content and tags (case-insensitive).
    pub fn search(&self, query: &str) -> io::Result<Vec<Memory>> {
        let memories = self.load()?;
        Ok(memories
            .into_iter()
            .filter(|m| m.matches_query(query))
            .collect())
    }

    /// Filters memories by type.
    pub fn filter_by_type(&self, memory_type: MemoryType) -> io::Result<Vec<Memory>> {
        let memories = self.load()?;
        Ok(memories
            .into_iter()
            .filter(|m| m.memory_type == memory_type)
            .collect())
    }

    /// Filters memories by tags (OR logic - matches if any tag matches).
    pub fn filter_by_tags(&self, tags: &[String]) -> io::Result<Vec<Memory>> {
        let memories = self.load()?;
        Ok(memories
            .into_iter()
            .filter(|m| m.has_any_tag(tags))
            .collect())
    }

    /// Writes all memories to the file, replacing existing content.
    ///
    /// This is used internally for operations like delete that need
    /// to rewrite the entire file. The caller must hold the exclusive lock.
    fn write_all_internal(&self, memories: &[Memory]) -> io::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut content = String::from("# Memories\n");

        // Group memories by type
        for memory_type in MemoryType::all() {
            let type_memories: Vec<_> = memories
                .iter()
                .filter(|m| m.memory_type == *memory_type)
                .collect();

            content.push_str(&format!("\n## {}\n", memory_type.section_name()));

            for memory in type_memories {
                content.push_str(&self.format_memory(memory));
            }
        }

        fs::write(&self.path, content)
    }

    /// Formats a memory as a markdown block.
    fn format_memory(&self, memory: &Memory) -> String {
        // Escape newlines in content by prefixing each line with `> `
        let content_lines: Vec<_> = memory
            .content
            .lines()
            .map(|line| format!("> {}", line))
            .collect();

        format!(
            "\n### {}\n{}\n<!-- tags: {} | created: {} -->\n",
            memory.id,
            content_lines.join("\n"),
            memory.tags.join(", "),
            memory.created,
        )
    }

    /// Finds the insertion point for a new memory in the given section.
    ///
    /// Returns the byte offset where the new memory block should be inserted,
    /// which is right after the section header line.
    fn find_section_insert_point(&self, content: &str, section: &str) -> Option<usize> {
        let section_start = content.find(section)?;
        // Find the end of the section header line
        let after_section = section_start + section.len();
        // Skip to end of line (including the newline)
        let newline_pos = content[after_section..].find('\n')?;
        Some(after_section + newline_pos + 1)
    }

    /// Returns the empty template for a new memories file.
    fn template(&self) -> String {
        "# Memories\n\n## Patterns\n\n## Decisions\n\n## Fixes\n\n## Context\n".to_string()
    }
}

/// Formats memories as markdown for context injection.
///
/// This produces a markdown document suitable for including in agent prompts:
/// ```markdown
/// # Memories
///
/// ## Patterns
/// ### mem-xxx-xxxx
/// > Memory content
/// <!-- tags: tag1, tag2 | created: 2025-01-20 -->
/// ```
///
/// Used by `ralph memory prime` and the event loop's auto-injection feature.
#[must_use]
pub fn format_memories_as_markdown(memories: &[Memory]) -> String {
    if memories.is_empty() {
        return String::new();
    }

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

/// Truncates memory content to approximately fit within a token budget.
///
/// Uses a simple heuristic of ~4 characters per token. Tries to end
/// at a natural break point (end of a memory block).
///
/// # Arguments
/// * `content` - The markdown content to truncate
/// * `budget` - Maximum tokens (0 = unlimited)
///
/// # Returns
/// The truncated content with a truncation notice if applicable.
#[must_use]
pub fn truncate_to_budget(content: &str, budget: usize) -> String {
    if budget == 0 || content.is_empty() {
        return content.to_string();
    }

    // Rough estimate: 4 chars per token
    let char_budget = budget * 4;

    if content.len() <= char_budget {
        return content.to_string();
    }

    // Ensure we truncate at a valid UTF-8 character boundary
    let safe_budget = floor_char_boundary(content, char_budget);

    // Find a good break point (end of a memory block)
    let truncated = &content[..safe_budget];

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_temp_store() -> (TempDir, MarkdownMemoryStore) {
        let temp_dir = TempDir::new().unwrap();
        let store = MarkdownMemoryStore::with_default_path(temp_dir.path());
        (temp_dir, store)
    }

    #[test]
    fn test_init_creates_file() {
        let (_temp_dir, store) = create_temp_store();

        assert!(!store.exists());
        store.init(false).unwrap();
        assert!(store.exists());

        let content = fs::read_to_string(store.path()).unwrap();
        assert!(content.contains("# Memories"));
        assert!(content.contains("## Patterns"));
        assert!(content.contains("## Decisions"));
        assert!(content.contains("## Fixes"));
        assert!(content.contains("## Context"));
    }

    #[test]
    fn test_init_fails_if_exists_without_force() {
        let (_temp_dir, store) = create_temp_store();

        store.init(false).unwrap();
        let result = store.init(false);
        assert!(result.is_err());
        assert!(result.unwrap_err().kind() == io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn test_init_with_force_overwrites() {
        let (_temp_dir, store) = create_temp_store();

        store.init(false).unwrap();

        // Add a memory
        let memory = Memory::new(
            MemoryType::Pattern,
            "Test content".to_string(),
            vec!["test".to_string()],
        );
        store.append(&memory).unwrap();

        // Force reinit
        store.init(true).unwrap();

        // Should be empty again
        let memories = store.load().unwrap();
        assert!(memories.is_empty());
    }

    #[test]
    fn test_append_creates_file_if_missing() {
        let (_temp_dir, store) = create_temp_store();

        let memory = Memory::new(
            MemoryType::Pattern,
            "Uses barrel exports".to_string(),
            vec!["imports".to_string()],
        );

        assert!(!store.exists());
        store.append(&memory).unwrap();
        assert!(store.exists());

        let memories = store.load().unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].content, "Uses barrel exports");
    }

    #[test]
    fn test_append_to_existing_section() {
        let (_temp_dir, store) = create_temp_store();
        store.init(false).unwrap();

        let memory1 = Memory::new(
            MemoryType::Pattern,
            "First pattern".to_string(),
            vec!["first".to_string()],
        );
        let memory2 = Memory::new(
            MemoryType::Pattern,
            "Second pattern".to_string(),
            vec!["second".to_string()],
        );

        store.append(&memory1).unwrap();
        store.append(&memory2).unwrap();

        let memories = store.load().unwrap();
        assert_eq!(memories.len(), 2);
        // Both should be in the Patterns section
        assert!(
            memories
                .iter()
                .all(|m| m.memory_type == MemoryType::Pattern)
        );
    }

    #[test]
    fn test_append_to_different_sections() {
        let (_temp_dir, store) = create_temp_store();
        store.init(false).unwrap();

        let pattern = Memory::new(MemoryType::Pattern, "A pattern".to_string(), vec![]);
        let decision = Memory::new(MemoryType::Decision, "A decision".to_string(), vec![]);
        let fix = Memory::new(MemoryType::Fix, "A fix".to_string(), vec![]);

        store.append(&pattern).unwrap();
        store.append(&decision).unwrap();
        store.append(&fix).unwrap();

        let memories = store.load().unwrap();
        assert_eq!(memories.len(), 3);

        // Verify each type is present
        assert!(
            memories
                .iter()
                .any(|m| m.memory_type == MemoryType::Pattern)
        );
        assert!(
            memories
                .iter()
                .any(|m| m.memory_type == MemoryType::Decision)
        );
        assert!(memories.iter().any(|m| m.memory_type == MemoryType::Fix));
    }

    #[test]
    fn test_delete_removes_memory() {
        let (_temp_dir, store) = create_temp_store();
        store.init(false).unwrap();

        let memory = Memory::new(MemoryType::Pattern, "To be deleted".to_string(), vec![]);
        let id = memory.id.clone();

        store.append(&memory).unwrap();
        assert_eq!(store.load().unwrap().len(), 1);

        let deleted = store.delete(&id).unwrap();
        assert!(deleted);
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn test_delete_returns_false_for_nonexistent() {
        let (_temp_dir, store) = create_temp_store();
        store.init(false).unwrap();

        let deleted = store.delete("mem-nonexistent-0000").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_get_finds_memory() {
        let (_temp_dir, store) = create_temp_store();

        let memory = Memory::new(
            MemoryType::Decision,
            "Important decision".to_string(),
            vec!["important".to_string()],
        );
        let id = memory.id.clone();

        store.append(&memory).unwrap();

        let found = store.get(&id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().content, "Important decision");
    }

    #[test]
    fn test_get_returns_none_for_nonexistent() {
        let (_temp_dir, store) = create_temp_store();
        store.init(false).unwrap();

        let found = store.get("mem-nonexistent-0000").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_search_matches_content() {
        let (_temp_dir, store) = create_temp_store();

        let memory1 = Memory::new(
            MemoryType::Pattern,
            "Uses barrel exports".to_string(),
            vec![],
        );
        let memory2 = Memory::new(
            MemoryType::Pattern,
            "Uses named exports".to_string(),
            vec![],
        );

        store.append(&memory1).unwrap();
        store.append(&memory2).unwrap();

        let results = store.search("barrel").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("barrel"));
    }

    #[test]
    fn test_search_matches_tags() {
        let (_temp_dir, store) = create_temp_store();

        let memory = Memory::new(
            MemoryType::Fix,
            "Docker fix".to_string(),
            vec!["docker".to_string(), "debugging".to_string()],
        );

        store.append(&memory).unwrap();

        let results = store.search("docker").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_filter_by_type() {
        let (_temp_dir, store) = create_temp_store();

        store
            .append(&Memory::new(MemoryType::Pattern, "P1".to_string(), vec![]))
            .unwrap();
        store
            .append(&Memory::new(MemoryType::Decision, "D1".to_string(), vec![]))
            .unwrap();
        store
            .append(&Memory::new(MemoryType::Pattern, "P2".to_string(), vec![]))
            .unwrap();

        let patterns = store.filter_by_type(MemoryType::Pattern).unwrap();
        assert_eq!(patterns.len(), 2);

        let decisions = store.filter_by_type(MemoryType::Decision).unwrap();
        assert_eq!(decisions.len(), 1);
    }

    #[test]
    fn test_filter_by_tags() {
        let (_temp_dir, store) = create_temp_store();

        store
            .append(&Memory::new(
                MemoryType::Pattern,
                "M1".to_string(),
                vec!["rust".to_string(), "async".to_string()],
            ))
            .unwrap();
        store
            .append(&Memory::new(
                MemoryType::Pattern,
                "M2".to_string(),
                vec!["python".to_string()],
            ))
            .unwrap();
        store
            .append(&Memory::new(
                MemoryType::Pattern,
                "M3".to_string(),
                vec!["rust".to_string()],
            ))
            .unwrap();

        let rust_memories = store.filter_by_tags(&["rust".to_string()]).unwrap();
        assert_eq!(rust_memories.len(), 2);

        let python_or_async = store
            .filter_by_tags(&["python".to_string(), "async".to_string()])
            .unwrap();
        assert_eq!(python_or_async.len(), 2);
    }

    #[test]
    fn test_load_empty_file() {
        let (_temp_dir, store) = create_temp_store();

        // File doesn't exist
        let memories = store.load().unwrap();
        assert!(memories.is_empty());
    }

    #[test]
    fn test_multiline_content_roundtrip() {
        let (_temp_dir, store) = create_temp_store();

        let memory = Memory::new(
            MemoryType::Pattern,
            "Line 1\nLine 2\nLine 3".to_string(),
            vec!["multiline".to_string()],
        );
        let id = memory.id.clone();

        store.append(&memory).unwrap();

        let loaded = store.get(&id).unwrap().unwrap();
        assert_eq!(loaded.content, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_format_memories_as_markdown_empty() {
        let output = format_memories_as_markdown(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn test_format_memories_as_markdown_single() {
        let memory = Memory {
            id: "mem-123-abcd".to_string(),
            memory_type: MemoryType::Pattern,
            content: "Use barrel exports".to_string(),
            tags: vec!["imports".to_string()],
            created: "2025-01-20".to_string(),
        };

        let output = format_memories_as_markdown(&[memory]);

        assert!(output.contains("# Memories"));
        assert!(output.contains("## Patterns"));
        assert!(output.contains("### mem-123-abcd"));
        assert!(output.contains("> Use barrel exports"));
        assert!(output.contains("tags: imports"));
    }

    #[test]
    fn test_format_memories_as_markdown_grouped_by_type() {
        let pattern = Memory {
            id: "mem-1-p".to_string(),
            memory_type: MemoryType::Pattern,
            content: "A pattern".to_string(),
            tags: vec![],
            created: "2025-01-20".to_string(),
        };
        let decision = Memory {
            id: "mem-2-d".to_string(),
            memory_type: MemoryType::Decision,
            content: "A decision".to_string(),
            tags: vec![],
            created: "2025-01-20".to_string(),
        };

        let output = format_memories_as_markdown(&[pattern, decision]);

        // Both sections should be present
        assert!(output.contains("## Patterns"));
        assert!(output.contains("## Decisions"));

        // Patterns section should come before Decisions
        let patterns_pos = output.find("## Patterns").unwrap();
        let decisions_pos = output.find("## Decisions").unwrap();
        assert!(patterns_pos < decisions_pos);
    }

    #[test]
    fn test_truncate_to_budget_no_truncation_needed() {
        let content = "Short content";
        let result = truncate_to_budget(content, 100);
        assert_eq!(result, content);
    }

    #[test]
    fn test_truncate_to_budget_zero_means_unlimited() {
        let content = "This is some long content that would normally be truncated";
        let result = truncate_to_budget(content, 0);
        assert_eq!(result, content);
    }

    #[test]
    fn test_truncate_to_budget_adds_notice() {
        let content = "x".repeat(1000); // 1000 chars = ~250 tokens
        let result = truncate_to_budget(&content, 10); // 10 tokens = 40 chars

        assert!(result.len() < content.len());
        assert!(result.contains("<!-- truncated:"));
    }
}
