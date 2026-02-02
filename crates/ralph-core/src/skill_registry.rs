//! Skill registry for discovering, storing, and providing access to skills.
//!
//! The registry manages both built-in skills (compiled into the binary) and
//! user-defined skills (discovered from configured directories).

use crate::config::{SkillOverride, SkillsConfig};
use crate::skill::{SkillEntry, SkillSource, parse_frontmatter};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Built-in ralph-tools skill content (tasks + memories).
const RALPH_TOOLS_SKILL_RAW: &str = include_str!("../data/ralph-tools.md");

/// Built-in RObot interaction skill content.
const ROBOT_INTERACTION_SKILL_RAW: &str = include_str!("../data/robot-interaction-skill.md");

/// Registry of all available skills for the current loop.
pub struct SkillRegistry {
    /// All skills indexed by name.
    skills: HashMap<String, SkillEntry>,
    /// The active backend name (for filtering).
    active_backend: Option<String>,
}

impl SkillRegistry {
    /// Create a new empty registry.
    pub fn new(active_backend: Option<&str>) -> Self {
        Self {
            skills: HashMap::new(),
            active_backend: active_backend.map(String::from),
        }
    }

    /// Register a built-in skill from raw content (with frontmatter).
    pub fn register_builtin(&mut self, fallback_name: &str, raw_content: &str) -> Result<()> {
        let (fm, content) = parse_frontmatter(raw_content);
        let fm = fm.unwrap_or_default();

        let name = fm.name.unwrap_or_else(|| fallback_name.to_string());
        let description = fm.description.unwrap_or_default();

        self.skills.insert(
            name.clone(),
            SkillEntry {
                name,
                description,
                content,
                source: SkillSource::BuiltIn,
                hats: fm.hats,
                backends: fm.backends,
                tags: fm.tags,
                auto_inject: false, // Built-ins default to false; overridden by config
            },
        );

        Ok(())
    }

    /// Register built-in skills (ralph-tools and robot-interaction).
    fn register_builtins(&mut self) -> Result<()> {
        self.register_builtin("ralph-tools", RALPH_TOOLS_SKILL_RAW)?;
        self.register_builtin("robot-interaction", ROBOT_INTERACTION_SKILL_RAW)?;
        Ok(())
    }

    /// Scan a directory for skill files and register them.
    ///
    /// Discovers two patterns:
    /// - `dir/*.md` — single-file skills (name from filename stem)
    /// - `dir/*/SKILL.md` — directory-based skills (name from parent dir)
    ///
    /// User skills with the same name as built-in skills replace them.
    pub fn scan_directory(&mut self, dir: &Path) -> Result<()> {
        if !dir.exists() {
            warn!("Skills directory does not exist: {}", dir.display());
            return Ok(());
        }

        if !dir.is_dir() {
            warn!("Skills path is not a directory: {}", dir.display());
            return Ok(());
        }

        // Scan for *.md files directly in the directory
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                    let fallback_name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    self.register_from_file(&path, &fallback_name)?;
                } else if path.is_dir() {
                    // Check for SKILL.md inside subdirectory
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        let fallback_name = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        self.register_from_file(&skill_file, &fallback_name)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Register a skill from a file path.
    fn register_from_file(&mut self, path: &Path, fallback_name: &str) -> Result<()> {
        let raw = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read skill file {}: {}", path.display(), e);
                return Ok(());
            }
        };

        let (fm, content) = parse_frontmatter(&raw);
        let fm = fm.unwrap_or_default();

        let name = fm.name.unwrap_or_else(|| fallback_name.to_string());
        let description = fm.description.unwrap_or_default();

        self.skills.insert(
            name.clone(),
            SkillEntry {
                name,
                description,
                content,
                source: SkillSource::File(path.to_path_buf()),
                hats: fm.hats,
                backends: fm.backends,
                tags: fm.tags,
                auto_inject: false,
            },
        );

        Ok(())
    }

    /// Apply config overrides to registered skills.
    fn apply_overrides(&mut self, overrides: &HashMap<String, SkillOverride>) {
        // Collect names to remove first (to avoid borrow conflicts)
        let to_remove: Vec<String> = overrides
            .iter()
            .filter(|(_, o)| o.enabled == Some(false))
            .map(|(name, _)| name.clone())
            .collect();

        for name in to_remove {
            self.skills.remove(&name);
        }

        // Apply remaining overrides
        for (name, override_) in overrides {
            if override_.enabled == Some(false) {
                continue; // Already removed
            }
            if let Some(skill) = self.skills.get_mut(name) {
                if !override_.hats.is_empty() {
                    skill.hats = override_.hats.clone();
                }
                if !override_.backends.is_empty() {
                    skill.backends = override_.backends.clone();
                }
                if !override_.tags.is_empty() {
                    skill.tags = override_.tags.clone();
                }
                if let Some(auto_inject) = override_.auto_inject {
                    skill.auto_inject = auto_inject;
                }
            }
        }
    }

    /// Construct a fully-populated registry from config.
    pub fn from_config(
        config: &SkillsConfig,
        workspace_root: &Path,
        active_backend: Option<&str>,
    ) -> Result<Self> {
        let mut registry = Self::new(active_backend);

        // 1. Register built-in skills
        registry.register_builtins()?;

        // 2. Scan configured directories
        for dir in &config.dirs {
            let resolved = Self::resolve_skill_dir(workspace_root, dir);
            registry.scan_directory(&resolved)?;
        }

        // 3. Apply config overrides
        registry.apply_overrides(&config.overrides);

        Ok(registry)
    }

    fn resolve_skill_dir(workspace_root: &Path, dir: &Path) -> PathBuf {
        if dir.is_absolute() {
            return dir.to_path_buf();
        }

        let candidate = workspace_root.join(dir);
        if candidate.is_dir() {
            return candidate;
        }

        let mut current = workspace_root.parent();
        while let Some(parent) = current {
            let candidate = parent.join(dir);
            if candidate.is_dir() {
                return candidate;
            }
            current = parent.parent();
        }

        candidate
    }

    /// Get a skill by name.
    pub fn get(&self, name: &str) -> Option<&SkillEntry> {
        self.skills.get(name)
    }

    /// Get all skills visible to a specific hat (filtered by hat + backend).
    pub fn skills_for_hat(&self, hat_id: Option<&str>) -> Vec<&SkillEntry> {
        self.skills
            .values()
            .filter(|s| self.is_visible(s, hat_id))
            .collect()
    }

    /// Get all auto-inject skills (filtered by hat + backend).
    pub fn auto_inject_skills(&self, hat_id: Option<&str>) -> Vec<&SkillEntry> {
        self.skills
            .values()
            .filter(|s| s.auto_inject && self.is_visible(s, hat_id))
            .collect()
    }

    /// Check if a skill is visible given the current hat and backend.
    fn is_visible(&self, skill: &SkillEntry, hat_id: Option<&str>) -> bool {
        // Backend filtering
        if !skill.backends.is_empty()
            && let Some(ref backend) = self.active_backend
            && !skill.backends.iter().any(|b| b == backend)
        {
            return false;
        }

        // Hat filtering: if skill is restricted to specific hats, filter by hat.
        // If no hat specified but skill has hat restriction, still show it
        // (solo mode has no explicit hat).
        if !skill.hats.is_empty()
            && let Some(hat) = hat_id
            && !skill.hats.iter().any(|h| h == hat)
        {
            return false;
        }

        true
    }

    /// Build the compact skill index for prompt injection.
    pub fn build_index(&self, hat_id: Option<&str>) -> String {
        let visible: Vec<&SkillEntry> = self.skills_for_hat(hat_id);

        if visible.is_empty() {
            return String::new();
        }

        let mut index = String::from("## SKILLS\n\nAvailable skills you can load on demand:\n\n");
        index.push_str("| Skill | Description | Load Command |\n");
        index.push_str("|-------|-------------|-------------|\n");

        let mut sorted: Vec<&&SkillEntry> = visible.iter().collect();
        sorted.sort_by_key(|s| &s.name);

        for skill in sorted {
            index.push_str(&format!(
                "| {} | {} | `ralph tools skill load {}` |\n",
                skill.name, skill.description, skill.name
            ));
        }

        index.push_str(
            "\nTo load a skill, run the load command. The skill content will guide you.\n",
        );
        index
    }

    /// Get skill content wrapped in XML tags for CLI output.
    pub fn load_skill(&self, name: &str) -> Option<String> {
        self.skills.get(name).map(|skill| {
            format!(
                "<{name}-skill>\n{content}\n</{name}-skill>",
                name = skill.name,
                content = skill.content
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_register_builtin_skill() {
        let mut registry = SkillRegistry::new(None);
        registry
            .register_builtin("ralph-tools", RALPH_TOOLS_SKILL_RAW)
            .unwrap();

        // The ralph-tools.md has name: ralph-tools in frontmatter
        let skill = registry
            .get("ralph-tools")
            .expect("should find built-in skill");
        assert!(matches!(skill.source, SkillSource::BuiltIn));
        assert!(!skill.description.is_empty());
        assert!(skill.content.contains("# Ralph Tools"));
        // Frontmatter fields should not be in content
        assert!(!skill.content.contains("name: ralph-tools"));
    }

    #[test]
    fn test_register_builtins() {
        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();

        // All built-in skills should be registered
        assert!(registry.get("ralph-tools").is_some());
        assert!(registry.get("robot-interaction").is_some());
    }

    #[test]
    fn test_get_returns_none_for_unknown() {
        let registry = SkillRegistry::new(None);
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_scan_directory_discovers_md_files() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir(&skill_dir).unwrap();

        fs::write(
            skill_dir.join("test-skill.md"),
            "---\nname: test-skill\ndescription: A test skill\n---\n\nTest content.\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new(None);
        registry.scan_directory(&skill_dir).unwrap();

        let skill = registry
            .get("test-skill")
            .expect("should find scanned skill");
        assert!(matches!(skill.source, SkillSource::File(_)));
        assert_eq!(skill.description, "A test skill");
        assert!(skill.content.contains("Test content."));
    }

    #[test]
    fn test_scan_directory_discovers_skill_md_subdirs() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        let sub_dir = skill_dir.join("my-complex-skill");
        fs::create_dir_all(&sub_dir).unwrap();

        fs::write(
            sub_dir.join("SKILL.md"),
            "---\nname: my-complex-skill\ndescription: Complex skill\n---\n\nComplex content.\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new(None);
        registry.scan_directory(&skill_dir).unwrap();

        let skill = registry
            .get("my-complex-skill")
            .expect("should find subdir skill");
        assert_eq!(skill.description, "Complex skill");
    }

    #[test]
    fn test_user_skill_overrides_builtin() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir(&skill_dir).unwrap();

        // User skill with same name as built-in
        fs::write(
            skill_dir.join("ralph-tools.md"),
            "---\nname: ralph-tools\ndescription: Custom tools skill\n---\n\nCustom content.\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();
        registry.scan_directory(&skill_dir).unwrap();

        let skill = registry.get("ralph-tools").unwrap();
        assert!(matches!(skill.source, SkillSource::File(_)));
        assert_eq!(skill.description, "Custom tools skill");
    }

    #[test]
    fn test_missing_directory_warns_but_no_error() {
        let mut registry = SkillRegistry::new(None);
        let result = registry.scan_directory(Path::new("/nonexistent/path"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_skill_name_from_frontmatter_takes_precedence() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir(&skill_dir).unwrap();

        // Filename is "file-name.md" but frontmatter says name is "frontmatter-name"
        fs::write(
            skill_dir.join("file-name.md"),
            "---\nname: frontmatter-name\ndescription: Test\n---\n\nContent.\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new(None);
        registry.scan_directory(&skill_dir).unwrap();

        assert!(registry.get("file-name").is_none());
        assert!(registry.get("frontmatter-name").is_some());
    }

    #[test]
    fn test_override_disables_skill() {
        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();
        assert!(registry.get("ralph-tools").is_some());

        let mut overrides = HashMap::new();
        overrides.insert(
            "ralph-tools".to_string(),
            SkillOverride {
                enabled: Some(false),
                ..Default::default()
            },
        );
        registry.apply_overrides(&overrides);

        assert!(registry.get("ralph-tools").is_none());
    }

    #[test]
    fn test_override_adds_hat_restriction() {
        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();

        let mut overrides = HashMap::new();
        overrides.insert(
            "ralph-tools".to_string(),
            SkillOverride {
                hats: vec!["builder".to_string()],
                ..Default::default()
            },
        );
        registry.apply_overrides(&overrides);

        let skill = registry.get("ralph-tools").unwrap();
        assert_eq!(skill.hats, vec!["builder"]);
    }

    #[test]
    fn test_override_sets_auto_inject() {
        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();

        let mut overrides = HashMap::new();
        overrides.insert(
            "ralph-tools".to_string(),
            SkillOverride {
                auto_inject: Some(true),
                ..Default::default()
            },
        );
        registry.apply_overrides(&overrides);

        let skill = registry.get("ralph-tools").unwrap();
        assert!(skill.auto_inject);
    }

    #[test]
    fn test_backend_filtering() {
        let mut registry = SkillRegistry::new(Some("claude"));
        registry
            .register_builtin(
                "claude-only",
                "---\nname: claude-only\ndescription: Claude\nbackends: [claude]\n---\nContent.\n",
            )
            .unwrap();
        registry
            .register_builtin(
                "gemini-only",
                "---\nname: gemini-only\ndescription: Gemini\nbackends: [gemini]\n---\nContent.\n",
            )
            .unwrap();
        registry
            .register_builtin(
                "any-backend",
                "---\nname: any-backend\ndescription: Any\n---\nContent.\n",
            )
            .unwrap();

        let visible = registry.skills_for_hat(None);
        let names: Vec<&str> = visible.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"claude-only"));
        assert!(!names.contains(&"gemini-only"));
        assert!(names.contains(&"any-backend"));
    }

    #[test]
    fn test_hat_filtering() {
        let mut registry = SkillRegistry::new(None);
        registry
            .register_builtin(
                "builder-only",
                "---\nname: builder-only\ndescription: Builder\nhats: [builder]\n---\nContent.\n",
            )
            .unwrap();
        registry
            .register_builtin(
                "all-hats",
                "---\nname: all-hats\ndescription: All\n---\nContent.\n",
            )
            .unwrap();

        let builder_skills = registry.skills_for_hat(Some("builder"));
        let builder_names: Vec<&str> = builder_skills.iter().map(|s| s.name.as_str()).collect();
        assert!(builder_names.contains(&"builder-only"));
        assert!(builder_names.contains(&"all-hats"));

        let reviewer_skills = registry.skills_for_hat(Some("reviewer"));
        let reviewer_names: Vec<&str> = reviewer_skills.iter().map(|s| s.name.as_str()).collect();
        assert!(!reviewer_names.contains(&"builder-only"));
        assert!(reviewer_names.contains(&"all-hats"));
    }

    #[test]
    fn test_auto_inject_skills_only_returns_auto_inject() {
        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();

        // No auto-inject skills by default
        let auto = registry.auto_inject_skills(None);
        assert!(auto.is_empty());

        // Set ralph-tools to auto-inject
        let mut overrides = HashMap::new();
        overrides.insert(
            "ralph-tools".to_string(),
            SkillOverride {
                auto_inject: Some(true),
                ..Default::default()
            },
        );
        registry.apply_overrides(&overrides);

        let auto = registry.auto_inject_skills(None);
        assert_eq!(auto.len(), 1);
        assert_eq!(auto[0].name, "ralph-tools");
    }

    #[test]
    fn test_build_index_generates_table() {
        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();

        let index = registry.build_index(None);
        assert!(index.contains("## SKILLS"));
        assert!(index.contains("| Skill | Description | Load Command |"));
        assert!(index.contains("ralph-tools"));
        assert!(index.contains("robot-interaction"));
        assert!(index.contains("`ralph tools skill load"));
    }

    #[test]
    fn test_build_index_empty_registry() {
        let registry = SkillRegistry::new(None);
        let index = registry.build_index(None);
        assert!(index.is_empty());
    }

    #[test]
    fn test_build_index_hat_filtering() {
        let mut registry = SkillRegistry::new(None);
        registry
            .register_builtin(
                "builder-only",
                "---\nname: builder-only\ndescription: Builder\nhats: [builder]\n---\nContent.\n",
            )
            .unwrap();
        registry
            .register_builtin(
                "all-hats",
                "---\nname: all-hats\ndescription: All\n---\nContent.\n",
            )
            .unwrap();

        let builder_index = registry.build_index(Some("builder"));
        assert!(builder_index.contains("builder-only"));
        assert!(builder_index.contains("all-hats"));

        let reviewer_index = registry.build_index(Some("reviewer"));
        assert!(!reviewer_index.contains("builder-only"));
        assert!(reviewer_index.contains("all-hats"));
    }

    #[test]
    fn test_load_skill_xml_wrapping() {
        let mut registry = SkillRegistry::new(None);
        registry.register_builtins().unwrap();

        let loaded = registry
            .load_skill("ralph-tools")
            .expect("should load skill");
        assert!(loaded.starts_with("<ralph-tools-skill>"));
        assert!(loaded.ends_with("</ralph-tools-skill>"));
        assert!(loaded.contains("# Ralph Tools"));
        // Frontmatter should not be in the output
        assert!(!loaded.contains("name: ralph-tools"));
    }

    #[test]
    fn test_load_skill_unknown() {
        let registry = SkillRegistry::new(None);
        assert!(registry.load_skill("nonexistent").is_none());
    }

    #[test]
    fn test_from_config_full_pipeline() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir(&skill_dir).unwrap();

        fs::write(
            skill_dir.join("custom.md"),
            "---\nname: custom\ndescription: Custom skill\n---\nCustom content.\n",
        )
        .unwrap();

        let config = SkillsConfig {
            enabled: true,
            dirs: vec![skill_dir.clone()],
            overrides: {
                let mut m = HashMap::new();
                m.insert(
                    "ralph-tools".to_string(),
                    SkillOverride {
                        auto_inject: Some(true),
                        ..Default::default()
                    },
                );
                m
            },
        };

        let registry = SkillRegistry::from_config(&config, tmp.path(), Some("claude")).unwrap();

        // Built-ins present
        assert!(registry.get("ralph-tools").is_some());
        // User skill present
        assert!(registry.get("custom").is_some());
        // Override applied
        assert!(registry.get("ralph-tools").unwrap().auto_inject);
    }

    #[test]
    fn test_from_config_resolves_parent_skills_dir_for_relative_path() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("repo");
        let workspace_dir = repo_dir.join("ralph-orchestrator");
        fs::create_dir_all(&workspace_dir).unwrap();

        let skill_dir = repo_dir
            .join(".claude")
            .join("skills")
            .join("test-driven-development");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-driven-development\ndescription: Test generation skill\n---\n\nSkill content.\n",
        )
        .unwrap();

        let config = SkillsConfig {
            enabled: true,
            dirs: vec![std::path::PathBuf::from(".claude/skills")],
            overrides: HashMap::new(),
        };

        let registry = SkillRegistry::from_config(&config, &workspace_dir, None).unwrap();
        assert!(registry.get("test-driven-development").is_some());
    }
}
