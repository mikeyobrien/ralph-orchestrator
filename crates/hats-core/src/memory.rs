//! Memory types for persistent learning across Hats sessions.
//!
//! This module provides core data structures for the memories feature:
//! - `Memory`: A single stored learning/insight
//! - `MemoryType`: Classification of memory (pattern, decision, fix, context)
//!
//! Memories are stored in `.hats/agent/memories.md` using a structured markdown format
//! that is both human-readable and machine-parseable.

use serde::{Deserialize, Serialize};

/// Classification of a memory.
///
/// Memories are grouped by type in the markdown storage file,
/// each with its own section header.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    /// How this codebase does things (section: "## Patterns")
    #[default]
    Pattern,
    /// Why something was chosen (section: "## Decisions")
    Decision,
    /// Solution to a recurring problem (section: "## Fixes")
    Fix,
    /// Project-specific knowledge (section: "## Context")
    Context,
}

impl MemoryType {
    /// Returns the markdown section header name for this memory type.
    ///
    /// Used when writing memories to `.hats/agent/memories.md`.
    #[must_use]
    pub fn section_name(&self) -> &'static str {
        match self {
            Self::Pattern => "Patterns",
            Self::Decision => "Decisions",
            Self::Fix => "Fixes",
            Self::Context => "Context",
        }
    }

    /// Parses a section header name into a memory type.
    ///
    /// Returns `None` if the section name doesn't match any memory type.
    #[must_use]
    pub fn from_section(s: &str) -> Option<Self> {
        match s {
            "Patterns" => Some(Self::Pattern),
            "Decisions" => Some(Self::Decision),
            "Fixes" => Some(Self::Fix),
            "Context" => Some(Self::Context),
            _ => None,
        }
    }

    /// Returns the emoji associated with this memory type.
    ///
    /// Used for CLI output formatting.
    #[must_use]
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Pattern => "🔄",
            Self::Decision => "⚖️",
            Self::Fix => "🔧",
            Self::Context => "📍",
        }
    }

    /// Returns all memory types in display order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[Self::Pattern, Self::Decision, Self::Fix, Self::Context]
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pattern => write!(f, "pattern"),
            Self::Decision => write!(f, "decision"),
            Self::Fix => write!(f, "fix"),
            Self::Context => write!(f, "context"),
        }
    }
}

impl std::str::FromStr for MemoryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pattern" => Ok(Self::Pattern),
            "decision" => Ok(Self::Decision),
            "fix" => Ok(Self::Fix),
            "context" => Ok(Self::Context),
            _ => Err(format!(
                "Invalid memory type: '{}'. Valid types: pattern, decision, fix, context",
                s
            )),
        }
    }
}

/// A single memory entry.
///
/// Memories are stored in `.hats/agent/memories.md` with the following format:
/// ```markdown
/// ### mem-1737372000-a1b2
/// > The actual memory content
/// > Can span multiple lines
/// <!-- tags: tag1, tag2 | created: 2025-01-20 -->
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique identifier (format: `mem-{unix_timestamp}-{4_hex_chars}`)
    pub id: String,

    /// Classification of this memory
    pub memory_type: MemoryType,

    /// The actual memory content (may contain newlines)
    pub content: String,

    /// Tags for categorization and search
    pub tags: Vec<String>,

    /// Creation date (format: YYYY-MM-DD)
    pub created: String,
}

impl Memory {
    /// Creates a new memory with a generated ID.
    ///
    /// The ID is generated using the current Unix timestamp and random hex characters.
    #[must_use]
    pub fn new(memory_type: MemoryType, content: String, tags: Vec<String>) -> Self {
        Self {
            id: Self::generate_id(),
            memory_type,
            content,
            tags,
            created: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        }
    }

    /// Generates a unique memory ID.
    ///
    /// Format: `mem-{unix_timestamp}-{4_hex_chars}`
    /// Example: `mem-1737372000-a1b2`
    ///
    /// The hex suffix is derived from the microsecond component of the timestamp,
    /// providing sufficient uniqueness for typical usage without external dependencies.
    #[must_use]
    pub fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let timestamp = duration.as_secs();
        // Use microseconds as the hex suffix for uniqueness
        let micros = duration.subsec_micros();
        let hex_suffix = format!("{:04x}", micros % 0x10000);

        format!("mem-{}-{}", timestamp, hex_suffix)
    }

    /// Returns true if this memory matches the given search query.
    ///
    /// Matches against content and tags (case-insensitive).
    #[must_use]
    pub fn matches_query(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.content.to_lowercase().contains(&query_lower)
            || self
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&query_lower))
    }

    /// Returns true if this memory has any of the specified tags.
    #[must_use]
    pub fn has_any_tag(&self, tags: &[String]) -> bool {
        let tags_lower: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
        self.tags
            .iter()
            .any(|t| tags_lower.contains(&t.to_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_type_section_names() {
        assert_eq!(MemoryType::Pattern.section_name(), "Patterns");
        assert_eq!(MemoryType::Decision.section_name(), "Decisions");
        assert_eq!(MemoryType::Fix.section_name(), "Fixes");
        assert_eq!(MemoryType::Context.section_name(), "Context");
    }

    #[test]
    fn test_memory_type_from_section() {
        assert_eq!(
            MemoryType::from_section("Patterns"),
            Some(MemoryType::Pattern)
        );
        assert_eq!(
            MemoryType::from_section("Decisions"),
            Some(MemoryType::Decision)
        );
        assert_eq!(MemoryType::from_section("Fixes"), Some(MemoryType::Fix));
        assert_eq!(
            MemoryType::from_section("Context"),
            Some(MemoryType::Context)
        );
        assert_eq!(MemoryType::from_section("Unknown"), None);
    }

    #[test]
    fn test_memory_type_emojis() {
        assert_eq!(MemoryType::Pattern.emoji(), "🔄");
        assert_eq!(MemoryType::Decision.emoji(), "⚖️");
        assert_eq!(MemoryType::Fix.emoji(), "🔧");
        assert_eq!(MemoryType::Context.emoji(), "📍");
    }

    #[test]
    fn test_memory_type_from_str() {
        assert_eq!(
            "pattern".parse::<MemoryType>().unwrap(),
            MemoryType::Pattern
        );
        assert_eq!(
            "DECISION".parse::<MemoryType>().unwrap(),
            MemoryType::Decision
        );
        assert_eq!("Fix".parse::<MemoryType>().unwrap(), MemoryType::Fix);
        assert_eq!(
            "context".parse::<MemoryType>().unwrap(),
            MemoryType::Context
        );
        assert!("invalid".parse::<MemoryType>().is_err());
    }

    #[test]
    fn test_memory_type_display() {
        assert_eq!(format!("{}", MemoryType::Pattern), "pattern");
        assert_eq!(format!("{}", MemoryType::Decision), "decision");
        assert_eq!(format!("{}", MemoryType::Fix), "fix");
        assert_eq!(format!("{}", MemoryType::Context), "context");
    }

    #[test]
    fn test_memory_new() {
        let memory = Memory::new(
            MemoryType::Pattern,
            "Uses barrel exports".to_string(),
            vec!["imports".to_string(), "structure".to_string()],
        );

        assert!(memory.id.starts_with("mem-"));
        assert_eq!(memory.memory_type, MemoryType::Pattern);
        assert_eq!(memory.content, "Uses barrel exports");
        assert_eq!(memory.tags, vec!["imports", "structure"]);
        // Created date should be today
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert_eq!(memory.created, today);
    }

    #[test]
    fn test_memory_id_format() {
        let id = Memory::generate_id();
        assert!(id.starts_with("mem-"));

        // Should have format mem-{timestamp}-{4hex}
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "mem");
        assert!(parts[1].parse::<u64>().is_ok()); // timestamp
        assert_eq!(parts[2].len(), 4); // 4 hex chars
    }

    #[test]
    fn test_memory_matches_query() {
        let memory = Memory {
            id: "mem-123-abcd".to_string(),
            memory_type: MemoryType::Pattern,
            content: "Uses barrel exports for modules".to_string(),
            tags: vec!["imports".to_string(), "structure".to_string()],
            created: "2025-01-20".to_string(),
        };

        // Match in content
        assert!(memory.matches_query("barrel"));
        assert!(memory.matches_query("BARREL")); // case-insensitive

        // Match in tags
        assert!(memory.matches_query("imports"));
        assert!(memory.matches_query("STRUCTURE"));

        // No match
        assert!(!memory.matches_query("authentication"));
    }

    #[test]
    fn test_memory_has_any_tag() {
        let memory = Memory {
            id: "mem-123-abcd".to_string(),
            memory_type: MemoryType::Fix,
            content: "Docker fix".to_string(),
            tags: vec!["docker".to_string(), "debugging".to_string()],
            created: "2025-01-20".to_string(),
        };

        assert!(memory.has_any_tag(&["docker".to_string()]));
        assert!(memory.has_any_tag(&["DEBUGGING".to_string()])); // case-insensitive
        assert!(memory.has_any_tag(&["other".to_string(), "docker".to_string()]));
        assert!(!memory.has_any_tag(&["unrelated".to_string()]));
    }

    #[test]
    fn test_memory_type_all() {
        let all = MemoryType::all();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0], MemoryType::Pattern);
        assert_eq!(all[1], MemoryType::Decision);
        assert_eq!(all[2], MemoryType::Fix);
        assert_eq!(all[3], MemoryType::Context);
    }

    #[test]
    fn test_memory_type_default() {
        assert_eq!(MemoryType::default(), MemoryType::Pattern);
    }

    #[test]
    fn test_memory_serde_roundtrip() {
        let memory = Memory {
            id: "mem-123-abcd".to_string(),
            memory_type: MemoryType::Decision,
            content: "Chose Postgres".to_string(),
            tags: vec!["database".to_string()],
            created: "2025-01-20".to_string(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        let deserialized: Memory = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, memory.id);
        assert_eq!(deserialized.memory_type, memory.memory_type);
        assert_eq!(deserialized.content, memory.content);
        assert_eq!(deserialized.tags, memory.tags);
        assert_eq!(deserialized.created, memory.created);
    }

    #[test]
    fn test_memory_type_serde() {
        // Test that memory type serializes as lowercase
        let mt = MemoryType::Decision;
        let json = serde_json::to_string(&mt).unwrap();
        assert_eq!(json, "\"decision\"");

        // Test deserialization
        let deserialized: MemoryType = serde_json::from_str("\"fix\"").unwrap();
        assert_eq!(deserialized, MemoryType::Fix);
    }
}
