//! Markdown parser for the memories file format.
//!
//! Parses `.hats/agent/memories.md` into a vector of `Memory` structs.
//! The format uses:
//! - `## Section` headers to denote memory types
//! - `### mem-{id}` headers for individual memories
//! - `> content` blockquotes for memory content
//! - `<!-- tags: ... | created: ... -->` HTML comments for metadata

use regex::Regex;
use std::sync::LazyLock;

use crate::memory::{Memory, MemoryType};

/// Regex to match section headers like `## Patterns`
static SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^## (Patterns|Decisions|Fixes|Context)").unwrap());

/// Regex to match memory ID headers like `### mem-1737372000-a1b2`
static MEMORY_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^### (mem-\d+-[0-9a-f]{4})").unwrap());

/// Regex to match blockquote content lines like `> content`
static CONTENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^> (.+)$").unwrap());

/// Regex to match metadata HTML comments like `<!-- tags: a, b | created: 2025-01-20 -->`
static METADATA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<!-- tags: ([^|]*) \| created: (\d{4}-\d{2}-\d{2}) -->").unwrap()
});

/// Parse a memories markdown file into a vector of Memory structs.
///
/// # Arguments
/// * `markdown` - The contents of a `.hats/agent/memories.md` file
///
/// # Returns
/// A vector of parsed memories. Malformed memory blocks are skipped.
///
/// # Example
/// ```
/// use hats_core::memory_parser::parse_memories;
///
/// let markdown = "# Memories\n\n## Patterns\n\n### mem-1737372000-a1b2\n> Uses barrel exports\n<!-- tags: imports, structure | created: 2025-01-20 -->\n";
///
/// let memories = parse_memories(markdown);
/// assert_eq!(memories.len(), 1);
/// assert_eq!(memories[0].content, "Uses barrel exports");
/// ```
pub fn parse_memories(markdown: &str) -> Vec<Memory> {
    let mut memories = Vec::new();
    let mut current_type = MemoryType::Pattern;
    let mut current_id: Option<String> = None;
    let mut current_content: Vec<String> = Vec::new();
    let mut current_tags: Vec<String> = Vec::new();
    let mut current_created: Option<String> = None;

    for line in markdown.lines() {
        if let Some(caps) = SECTION_RE.captures(line) {
            // Flush any pending memory before switching sections
            flush_memory(
                &mut memories,
                &mut current_id,
                current_type,
                &mut current_content,
                &mut current_tags,
                &mut current_created,
            );
            current_type = MemoryType::from_section(&caps[1]).unwrap_or(MemoryType::Pattern);
        } else if let Some(caps) = MEMORY_ID_RE.captures(line) {
            // Flush any pending memory before starting a new one
            flush_memory(
                &mut memories,
                &mut current_id,
                current_type,
                &mut current_content,
                &mut current_tags,
                &mut current_created,
            );
            current_id = Some(caps[1].to_string());
        } else if let Some(caps) = CONTENT_RE.captures(line) {
            current_content.push(caps[1].to_string());
        } else if let Some(caps) = METADATA_RE.captures(line) {
            current_tags = caps[1]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            current_created = Some(caps[2].to_string());
        }
    }

    // Flush any remaining memory
    flush_memory(
        &mut memories,
        &mut current_id,
        current_type,
        &mut current_content,
        &mut current_tags,
        &mut current_created,
    );

    memories
}

/// Helper to finalize and push a memory if we have enough data.
fn flush_memory(
    memories: &mut Vec<Memory>,
    current_id: &mut Option<String>,
    current_type: MemoryType,
    current_content: &mut Vec<String>,
    current_tags: &mut Vec<String>,
    current_created: &mut Option<String>,
) {
    if let Some(id) = current_id.take()
        && !current_content.is_empty()
    {
        memories.push(Memory {
            id,
            memory_type: current_type,
            content: current_content.join("\n"),
            tags: std::mem::take(current_tags),
            created: current_created
                .take()
                .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string()),
        });
    }
    current_content.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_memory() {
        let markdown = r"# Memories

## Patterns

### mem-1737372000-a1b2
> Uses barrel exports for modules
<!-- tags: imports, structure | created: 2025-01-20 -->
";

        let memories = parse_memories(markdown);
        assert_eq!(memories.len(), 1);

        let mem = &memories[0];
        assert_eq!(mem.id, "mem-1737372000-a1b2");
        assert_eq!(mem.memory_type, MemoryType::Pattern);
        assert_eq!(mem.content, "Uses barrel exports for modules");
        assert_eq!(mem.tags, vec!["imports", "structure"]);
        assert_eq!(mem.created, "2025-01-20");
    }

    #[test]
    fn test_parse_multiple_sections() {
        let markdown = r"# Memories

## Patterns

### mem-1737372000-a1b2
> Uses barrel exports
<!-- tags: imports | created: 2025-01-20 -->

## Decisions

### mem-1737372100-c3d4
> Chose Postgres over SQLite
<!-- tags: database | created: 2025-01-20 -->

## Fixes

### mem-1737372200-e5f6
> ECONNREFUSED means start docker
<!-- tags: docker, debugging | created: 2025-01-21 -->

## Context

### mem-1737372300-a7b8
> hats-core is the shared library
<!-- tags: architecture | created: 2025-01-21 -->
";

        let memories = parse_memories(markdown);
        assert_eq!(memories.len(), 4);

        assert_eq!(memories[0].memory_type, MemoryType::Pattern);
        assert_eq!(memories[1].memory_type, MemoryType::Decision);
        assert_eq!(memories[2].memory_type, MemoryType::Fix);
        assert_eq!(memories[3].memory_type, MemoryType::Context);
    }

    #[test]
    fn test_parse_multiline_content() {
        let markdown = r"# Memories

## Patterns

### mem-1737372000-a1b2
> First line of content
> Second line of content
> Third line of content
<!-- tags: multiline | created: 2025-01-20 -->
";

        let memories = parse_memories(markdown);
        assert_eq!(memories.len(), 1);
        assert_eq!(
            memories[0].content,
            "First line of content\nSecond line of content\nThird line of content"
        );
    }

    #[test]
    fn test_parse_missing_metadata_uses_defaults() {
        let markdown = r"# Memories

## Patterns

### mem-1737372000-a1b2
> Some content without metadata
";

        let memories = parse_memories(markdown);
        assert_eq!(memories.len(), 1);

        let mem = &memories[0];
        assert_eq!(mem.content, "Some content without metadata");
        assert!(mem.tags.is_empty());
        // Created should default to today's date
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert_eq!(mem.created, today);
    }

    #[test]
    fn test_parse_ignores_malformed_blocks() {
        let markdown = r"# Memories

## Patterns

### mem-1737372000-a1b2
> Valid memory
<!-- tags: valid | created: 2025-01-20 -->

### mem-invalid-format
> This has an invalid ID format and will be skipped

### mem-1737372100-c3d4
> Another valid memory
<!-- tags: also-valid | created: 2025-01-21 -->
";

        let memories = parse_memories(markdown);
        // The invalid one should be skipped based on the regex not matching
        // Actually it should match since the regex looks for mem-\d+-[0-9a-f]{4}
        // "mem-invalid-format" won't match, so it won't create a new memory block
        assert_eq!(memories.len(), 2);
        assert_eq!(memories[0].id, "mem-1737372000-a1b2");
        assert_eq!(memories[1].id, "mem-1737372100-c3d4");
    }

    #[test]
    fn test_parse_empty_file() {
        let memories = parse_memories("");
        assert!(memories.is_empty());
    }

    #[test]
    fn test_parse_empty_tags() {
        let markdown = r"# Memories

## Patterns

### mem-1737372000-a1b2
> Content with empty tags
<!-- tags:  | created: 2025-01-20 -->
";

        let memories = parse_memories(markdown);
        assert_eq!(memories.len(), 1);
        assert!(memories[0].tags.is_empty());
    }

    #[test]
    fn test_parse_memory_without_content_is_skipped() {
        let markdown = r"# Memories

## Patterns

### mem-1737372000-a1b2
<!-- tags: no-content | created: 2025-01-20 -->

### mem-1737372100-c3d4
> This one has content
<!-- tags: valid | created: 2025-01-20 -->
";

        let memories = parse_memories(markdown);
        // Memory without content should be skipped
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].id, "mem-1737372100-c3d4");
    }
}
