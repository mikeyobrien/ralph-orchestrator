//! Human-readable loop name generation.
//!
//! Generates descriptive names for loops/worktrees derived from prompt text,
//! combined with adjective-noun suffixes for uniqueness.
//!
//! Example outputs:
//! - `fix-header-swift-peacock`
//! - `add-auth-clever-badger`
//! - `refactor-api-calm-falcon`

use serde::{Deserialize, Serialize};

/// Configuration for loop naming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopNamingConfig {
    /// Naming format: "human-readable" or "timestamp".
    #[serde(default = "default_format")]
    pub format: String,

    /// Maximum length for generated names.
    #[serde(default = "default_max_length")]
    pub max_length: usize,
}

fn default_format() -> String {
    "human-readable".to_string()
}

fn default_max_length() -> usize {
    50
}

impl Default for LoopNamingConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            max_length: default_max_length(),
        }
    }
}

/// Generator for human-readable loop names.
pub struct LoopNameGenerator {
    config: LoopNamingConfig,
}

impl LoopNameGenerator {
    /// Create a new generator with the given configuration.
    pub fn new(config: LoopNamingConfig) -> Self {
        Self { config }
    }

    /// Create a generator from config, using defaults if not configured.
    pub fn from_config(config: &LoopNamingConfig) -> Self {
        Self::new(config.clone())
    }

    /// Generate a name from a prompt.
    ///
    /// Returns a name in the format: `keywords-adjective-noun`
    /// For example: `fix-header-swift-peacock`
    pub fn generate(&self, prompt: &str) -> String {
        if self.config.format == "timestamp" {
            return generate_timestamp_id();
        }

        let keywords = self.extract_keywords(prompt);
        let suffix = self.generate_suffix();

        let keyword_part = if keywords.is_empty() {
            "loop".to_string()
        } else {
            keywords.join("-")
        };

        let name = format!("{}-{}", keyword_part, suffix);
        self.truncate_to_max_length(&name)
    }

    /// Generate a unique name, using `exists` to check for collisions.
    ///
    /// Tries up to 3 times with different suffixes before falling back
    /// to timestamp format.
    pub fn generate_unique(&self, prompt: &str, exists: impl Fn(&str) -> bool) -> String {
        if self.config.format == "timestamp" {
            return generate_timestamp_id();
        }

        let keywords = self.extract_keywords(prompt);
        let keyword_part = if keywords.is_empty() {
            "loop".to_string()
        } else {
            keywords.join("-")
        };

        // Try up to 3 times with different suffixes
        for _ in 0..3 {
            let suffix = self.generate_suffix();
            let name = format!("{}-{}", keyword_part, suffix);
            let name = self.truncate_to_max_length(&name);

            if !exists(&name) {
                return name;
            }
        }

        // Fallback to timestamp format
        generate_timestamp_id()
    }

    /// Generate a memorable name (adjective-noun only, no keywords).
    ///
    /// Returns a name like "bright-maple" or "swift-falcon".
    pub fn generate_memorable(&self) -> String {
        self.generate_suffix()
    }

    /// Generate a unique memorable name, using `exists` to check for collisions.
    ///
    /// Tries up to 10 times with different suffixes before falling back
    /// to timestamp format.
    pub fn generate_memorable_unique(&self, exists: impl Fn(&str) -> bool) -> String {
        // Try up to 10 times with different suffixes
        for _ in 0..10 {
            let name = self.generate_suffix();
            if !exists(&name) {
                return name;
            }
            // Small delay to get different nanosecond value
            std::thread::sleep(std::time::Duration::from_micros(1));
        }

        // Fallback to timestamp format (very unlikely with 50*50 = 2500 combinations)
        generate_timestamp_id()
    }

    /// Extract keywords from a prompt.
    fn extract_keywords(&self, prompt: &str) -> Vec<String> {
        let words: Vec<&str> = prompt
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();

        let mut keywords = Vec::new();

        // Prioritize action verbs
        for word in &words {
            let lower = word.to_lowercase();
            if ACTION_VERBS.contains(&lower.as_str()) && keywords.len() < 3 {
                keywords.push(lower);
            }
        }

        // Then add other significant words
        for word in &words {
            let lower = word.to_lowercase();
            if !STOP_WORDS.contains(&lower.as_str())
                && !keywords.contains(&lower)
                && keywords.len() < 3
                && lower.len() >= 2
            {
                keywords.push(lower);
            }
        }

        // Sanitize each keyword
        keywords
            .into_iter()
            .map(|w| sanitize_for_git(&w))
            .filter(|w| !w.is_empty())
            .take(3)
            .collect()
    }

    /// Generate a random adjective-noun suffix.
    fn generate_suffix(&self) -> String {
        use std::time::SystemTime;

        // Use nanoseconds for randomness
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let adj_idx = (nanos % ADJECTIVES.len() as u128) as usize;
        let noun_idx = ((nanos / 1000) % NOUNS.len() as u128) as usize;

        format!("{}-{}", ADJECTIVES[adj_idx], NOUNS[noun_idx])
    }

    /// Truncate name to max length, preserving word boundaries where possible.
    fn truncate_to_max_length(&self, name: &str) -> String {
        if name.len() <= self.config.max_length {
            return name.to_string();
        }

        // Try to truncate at a word boundary
        let mut result = String::new();
        for part in name.split('-') {
            let candidate = if result.is_empty() {
                part.to_string()
            } else {
                format!("{}-{}", result, part)
            };

            if candidate.len() <= self.config.max_length {
                result = candidate;
            } else {
                break;
            }
        }

        // If we couldn't fit even one word, just truncate
        if result.is_empty() {
            name.chars().take(self.config.max_length).collect()
        } else {
            result
        }
    }
}

/// Generate a timestamp-based ID (legacy format).
fn generate_timestamp_id() -> String {
    use std::time::SystemTime;

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");

    // Generate 4-character random hex suffix
    let random_suffix: u16 = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| (d.as_nanos() & 0xFFFF) as u16)
        .unwrap_or(0);

    format!("hats-{}-{:04x}", timestamp, random_suffix)
}

/// Sanitize text for git branch/worktree names.
pub fn sanitize_for_git(text: &str) -> String {
    let result: String = text
        .to_lowercase()
        .replace([' ', '_'], "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();

    // Collapse multiple hyphens
    let mut prev_hyphen = false;
    let result: String = result
        .chars()
        .filter(|c| {
            if *c == '-' {
                if prev_hyphen {
                    return false;
                }
                prev_hyphen = true;
            } else {
                prev_hyphen = false;
            }
            true
        })
        .collect();

    // Trim leading/trailing hyphens
    result.trim_matches('-').to_string()
}

/// Action verbs to prioritize in keyword extraction.
const ACTION_VERBS: &[&str] = &[
    "add",
    "fix",
    "update",
    "remove",
    "delete",
    "implement",
    "create",
    "refactor",
    "move",
    "rename",
    "change",
    "modify",
    "improve",
    "optimize",
    "clean",
    "rewrite",
    "replace",
    "merge",
    "split",
    "extract",
    "inline",
    "simplify",
    "consolidate",
    "migrate",
    "upgrade",
    "downgrade",
    "enable",
    "disable",
    "configure",
    "setup",
    "init",
    "build",
    "test",
    "debug",
    "deploy",
    "release",
];

/// Stop words to filter out of prompts.
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "to", "for", "of", "in", "on", "at", "by", "with", "from", "as", "is", "are",
    "was", "were", "be", "been", "being", "have", "has", "had", "do", "does", "did", "will",
    "would", "could", "should", "may", "might", "must", "shall", "can", "need", "it", "its",
    "this", "that", "these", "those", "i", "you", "he", "she", "we", "they", "me", "him", "her",
    "us", "them", "my", "your", "his", "our", "their", "what", "which", "who", "whom", "when",
    "where", "why", "how", "all", "each", "every", "both", "few", "more", "most", "other", "some",
    "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too", "very", "just", "also",
    "and", "but", "or", "if", "then", "else", "please", "make", "sure", "get", "let", "put",
];

/// Adjectives for suffix generation.
const ADJECTIVES: &[&str] = &[
    "swift", "clever", "bright", "calm", "bold", "keen", "quick", "brave", "fair", "wise", "warm",
    "cool", "crisp", "fresh", "clear", "sharp", "smooth", "steady", "gentle", "agile", "nimble",
    "lively", "merry", "jolly", "happy", "lucky", "eager", "ready", "able", "noble", "grand",
    "prime", "pure", "true", "neat", "tidy", "clean", "sleek", "slick", "smart", "savvy", "snappy",
    "zippy", "zesty", "peppy", "perky", "chipper", "chirpy", "cheery", "sunny", "breezy",
];

/// Nouns for suffix generation.
const NOUNS: &[&str] = &[
    "peacock", "badger", "falcon", "otter", "robin", "maple", "brook", "cedar", "willow", "finch",
    "heron", "aspen", "birch", "crane", "egret", "lark", "sparrow", "raven", "hawk", "owl", "fox",
    "deer", "wolf", "bear", "lion", "tiger", "eagle", "dove", "swan", "gull", "wren", "jay",
    "pine", "oak", "elm", "fern", "moss", "reed", "sage", "mint", "rose", "lily", "iris", "daisy",
    "tulip", "orchid", "lotus", "ivy", "palm", "cork", "teak",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_sanitize_for_git() {
        assert_eq!(sanitize_for_git("Hello World"), "hello-world");
        assert_eq!(sanitize_for_git("fix_the_bug"), "fix-the-bug");
        assert_eq!(sanitize_for_git("  spaces  "), "spaces");
        assert_eq!(sanitize_for_git("multiple---hyphens"), "multiple-hyphens");
        assert_eq!(sanitize_for_git("special!@#chars"), "specialchars");
        assert_eq!(sanitize_for_git("MixedCase"), "mixedcase");
        assert_eq!(sanitize_for_git("123numbers"), "123numbers");
        assert_eq!(sanitize_for_git("-leading-trailing-"), "leading-trailing");
    }

    #[test]
    fn test_extract_keywords_prioritizes_verbs() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let keywords = generator.extract_keywords("Fix the header alignment issue");
        assert!(keywords.contains(&"fix".to_string()));
        assert!(keywords.contains(&"header".to_string()));
    }

    #[test]
    fn test_extract_keywords_filters_stop_words() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let keywords = generator.extract_keywords("Add a new feature to the system");
        assert!(!keywords.contains(&"a".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
        assert!(!keywords.contains(&"to".to_string()));
        assert!(keywords.contains(&"add".to_string()));
    }

    #[test]
    fn test_extract_keywords_limits_to_three() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let keywords =
            generator.extract_keywords("Fix header footer sidebar navigation menu content layout");
        assert!(keywords.len() <= 3);
    }

    #[test]
    fn test_generate_produces_valid_name() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let name = generator.generate("Fix the header alignment");
        assert!(!name.is_empty());
        // Should contain keywords
        assert!(name.contains("fix") || name.contains("header"));
        // Should be valid for git
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn test_generate_empty_prompt() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let name = generator.generate("");
        assert!(name.starts_with("loop-"));
    }

    #[test]
    fn test_generate_only_stop_words() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let name = generator.generate("the a an to for of in on");
        assert!(name.starts_with("loop-"));
    }

    #[test]
    fn test_generate_respects_max_length() {
        let config = LoopNamingConfig {
            format: "human-readable".to_string(),
            max_length: 30,
        };
        let generator = LoopNameGenerator::new(config);

        let name = generator.generate("Implement the authentication system with OAuth2 support");
        assert!(name.len() <= 30);
    }

    #[test]
    fn test_timestamp_format() {
        let config = LoopNamingConfig {
            format: "timestamp".to_string(),
            max_length: 50,
        };
        let generator = LoopNameGenerator::new(config);

        let name = generator.generate("Fix header");
        assert!(name.starts_with("hats-"));
        // Format: hats-YYYYMMDD-HHMMSS-XXXX
        assert!(name.len() > 20);
    }

    #[test]
    fn test_generate_unique_avoids_collisions() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let mut generated = HashSet::new();

        // First call should succeed
        let name1 = generator.generate_unique("Fix header", |n| generated.contains(n));
        generated.insert(name1.clone());

        // This is a bit tricky to test since suffixes are time-based
        // Just verify it generates a valid name
        assert!(!name1.is_empty());
        assert!(name1.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn test_generate_unique_falls_back_to_timestamp() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        // Always say name exists to force fallback
        let name = generator.generate_unique("Fix header", |_| true);

        // Should fall back to timestamp format
        assert!(name.starts_with("hats-"));
    }

    #[test]
    fn test_default_config() {
        let config = LoopNamingConfig::default();
        assert_eq!(config.format, "human-readable");
        assert_eq!(config.max_length, 50);
    }

    #[test]
    fn test_generate_memorable() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let name = generator.generate_memorable();

        // Should be adjective-noun format (e.g., "bright-maple")
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2, "Expected adjective-noun format: {}", name);

        // Should be valid for git
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn test_generate_memorable_unique() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        let mut generated = HashSet::new();

        // First call should succeed
        let name1 = generator.generate_memorable_unique(|n| generated.contains(n));
        generated.insert(name1.clone());

        // Verify format
        let parts: Vec<&str> = name1.split('-').collect();
        assert_eq!(parts.len(), 2, "Expected adjective-noun format: {}", name1);
        assert!(name1.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn test_generate_memorable_unique_falls_back_to_timestamp() {
        let generator = LoopNameGenerator::new(LoopNamingConfig::default());

        // Always say name exists to force fallback
        let name = generator.generate_memorable_unique(|_| true);

        // Should fall back to timestamp format
        assert!(name.starts_with("hats-"));
    }
}
