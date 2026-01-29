//! Skill data types and frontmatter parser.
//!
//! Skills are markdown documents with YAML frontmatter that provide knowledge
//! and tool instructions to agents during orchestration loops.

use serde::Deserialize;
use std::path::PathBuf;

/// A discovered skill with parsed frontmatter and content.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    /// Unique identifier (derived from filename or frontmatter `name`).
    pub name: String,
    /// Human-readable description from frontmatter.
    pub description: String,
    /// Full markdown content (frontmatter stripped).
    pub content: String,
    /// Source: built-in or filesystem path.
    pub source: SkillSource,
    /// Optional: restrict to specific hats.
    pub hats: Vec<String>,
    /// Optional: restrict to specific backends.
    pub backends: Vec<String>,
    /// Optional: tags for categorization.
    pub tags: Vec<String>,
    /// Whether to inject full content into every prompt (not just index entry).
    pub auto_inject: bool,
}

/// Where a skill was loaded from.
#[derive(Debug, Clone)]
pub enum SkillSource {
    /// Compiled into the binary via include_str!
    BuiltIn,
    /// Loaded from a filesystem path.
    File(PathBuf),
}

/// Parsed YAML frontmatter from a skill file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SkillFrontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub hats: Vec<String>,
    #[serde(default)]
    pub backends: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Parse YAML frontmatter from a markdown document.
///
/// Returns the parsed frontmatter (if valid) and the body content
/// with frontmatter delimiters stripped.
///
/// Frontmatter format:
/// ```text
/// ---
/// name: my-skill
/// description: A useful skill
/// ---
/// Body content here...
/// ```
pub fn parse_frontmatter(raw: &str) -> (Option<SkillFrontmatter>, String) {
    let trimmed = raw.trim_start();

    // Must start with `---`
    if !trimmed.starts_with("---") {
        return (None, raw.to_string());
    }

    // Find the closing `---` (skip the opening one)
    let after_open = &trimmed[3..];
    let closing_pos = after_open.find("\n---");

    match closing_pos {
        Some(pos) => {
            let yaml_str = &after_open[..pos];
            let body_start = pos + 4; // skip \n---
            let body = after_open[body_start..].trim_start_matches('\n');

            match serde_yaml::from_str::<SkillFrontmatter>(yaml_str) {
                Ok(fm) => (Some(fm), body.to_string()),
                Err(_) => {
                    // Invalid YAML — return None frontmatter but still strip the block
                    (None, body.to_string())
                }
            }
        }
        None => {
            // No closing delimiter — treat entire content as body, no frontmatter
            (None, raw.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frontmatter_all_fields() {
        let raw = r"---
name: my-skill
description: A useful skill
hats: [builder, reviewer]
backends: [claude, gemini]
tags: [testing, tdd]
---

# My Skill

Body content here.
";
        let (fm, body) = parse_frontmatter(raw);
        let fm = fm.expect("should parse frontmatter");
        assert_eq!(fm.name.as_deref(), Some("my-skill"));
        assert_eq!(fm.description.as_deref(), Some("A useful skill"));
        assert_eq!(fm.hats, vec!["builder", "reviewer"]);
        assert_eq!(fm.backends, vec!["claude", "gemini"]);
        assert_eq!(fm.tags, vec!["testing", "tdd"]);
        assert!(body.contains("# My Skill"));
        assert!(body.contains("Body content here."));
        // Frontmatter delimiters should be stripped
        assert!(!body.contains("---"));
    }

    #[test]
    fn test_parse_frontmatter_name_and_description_only() {
        let raw = r"---
name: memories
description: Persistent learning across sessions
---

# Memories

Content.
";
        let (fm, body) = parse_frontmatter(raw);
        let fm = fm.expect("should parse frontmatter");
        assert_eq!(fm.name.as_deref(), Some("memories"));
        assert_eq!(
            fm.description.as_deref(),
            Some("Persistent learning across sessions")
        );
        assert!(fm.hats.is_empty());
        assert!(fm.backends.is_empty());
        assert!(fm.tags.is_empty());
        assert!(body.starts_with("# Memories"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let raw = "# Just Markdown\n\nNo frontmatter here.\n";
        let (fm, body) = parse_frontmatter(raw);
        assert!(fm.is_none());
        assert_eq!(body, raw);
    }

    #[test]
    fn test_parse_invalid_yaml_frontmatter() {
        let raw = r"---
this: is: not: valid: yaml: [[[
---

Body content.
";
        let (fm, body) = parse_frontmatter(raw);
        assert!(fm.is_none());
        assert!(body.contains("Body content."));
    }

    #[test]
    fn test_parse_no_closing_delimiter() {
        let raw = "---\nname: broken\nNo closing delimiter\n";
        let (fm, body) = parse_frontmatter(raw);
        assert!(fm.is_none());
        assert_eq!(body, raw);
    }

    #[test]
    fn test_content_body_strips_frontmatter_delimiters() {
        let raw = "---\nname: test\n---\nFirst line of body.\nSecond line.\n";
        let (fm, body) = parse_frontmatter(raw);
        assert!(fm.is_some());
        assert!(body.starts_with("First line of body."));
        assert!(body.contains("Second line."));
        assert!(!body.contains("---"));
        assert!(!body.contains("name: test"));
    }

    #[test]
    fn test_empty_frontmatter() {
        let raw = "---\n---\nBody only.\n";
        let (fm, body) = parse_frontmatter(raw);
        // Empty YAML is valid and parses to defaults
        let fm = fm.expect("empty frontmatter should parse");
        assert!(fm.name.is_none());
        assert!(fm.description.is_none());
        assert!(body.contains("Body only."));
    }
}
