//! Embedded presets for ralph init command.
//!
//! This module embeds all preset YAML files at compile time, making the
//! binary self-contained. Users can initialize projects with presets
//! without needing access to the source repository.
//!
//! Canonical presets live in the shared `presets/` directory at the repo root.
//! The sync script (`scripts/sync-embedded-files.sh`) mirrors them into
//! `crates/ralph-cli/presets/` for `include_str!` to work with crates.io publishing.

/// An embedded preset with its name, description, and full content.
#[derive(Debug, Clone)]
pub struct EmbeddedPreset {
    /// The preset name (e.g., "feature")
    pub name: &'static str,
    /// Short description extracted from the preset's header comment
    pub description: &'static str,
    /// Full YAML content of the preset
    pub content: &'static str,
}

/// All embedded presets, compiled into the binary.
const PRESETS: &[EmbeddedPreset] = &[
    EmbeddedPreset {
        name: "bugfix",
        description: "Systematic bug reproduction, fix, and verification",
        content: include_str!("../presets/bugfix.yml"),
    },
    EmbeddedPreset {
        name: "code-assist",
        description: "TDD implementation from specs, tasks, or descriptions",
        content: include_str!("../presets/code-assist.yml"),
    },
    EmbeddedPreset {
        name: "debug",
        description: "Bug investigation and root cause analysis",
        content: include_str!("../presets/debug.yml"),
    },
    EmbeddedPreset {
        name: "deploy",
        description: "Deployment and Release Workflow",
        content: include_str!("../presets/deploy.yml"),
    },
    EmbeddedPreset {
        name: "docs",
        description: "Documentation Generation Workflow",
        content: include_str!("../presets/docs.yml"),
    },
    EmbeddedPreset {
        name: "feature",
        description: "Feature Development with integrated code review",
        content: include_str!("../presets/feature.yml"),
    },
    EmbeddedPreset {
        name: "fresh-eyes",
        description: "Implementation workflow with enforced repeated fresh-eyes self-review passes",
        content: include_str!("../presets/fresh-eyes.yml"),
    },
    EmbeddedPreset {
        name: "gap-analysis",
        description: "Gap Analysis and Planning Workflow",
        content: include_str!("../presets/gap-analysis.yml"),
    },
    EmbeddedPreset {
        name: "hatless-baseline",
        description: "Baseline hatless mode for comparison",
        content: include_str!("../presets/hatless-baseline.yml"),
    },
    EmbeddedPreset {
        name: "merge-loop",
        description: "Merges completed parallel loop from worktree back to main branch",
        content: include_str!("../presets/merge-loop.yml"),
    },
    EmbeddedPreset {
        name: "pdd-to-code-assist",
        description: "Full autonomous idea-to-code pipeline",
        content: include_str!("../presets/pdd-to-code-assist.yml"),
    },
    EmbeddedPreset {
        name: "pr-review",
        description: "Multi-perspective PR code review",
        content: include_str!("../presets/pr-review.yml"),
    },
    EmbeddedPreset {
        name: "refactor",
        description: "Code Refactoring Workflow",
        content: include_str!("../presets/refactor.yml"),
    },
    EmbeddedPreset {
        name: "research",
        description: "Deep exploration and analysis tasks",
        content: include_str!("../presets/research.yml"),
    },
    EmbeddedPreset {
        name: "review",
        description: "Code Review Workflow",
        content: include_str!("../presets/review.yml"),
    },
    EmbeddedPreset {
        name: "spec-driven",
        description: "Specification-Driven Development",
        content: include_str!("../presets/spec-driven.yml"),
    },
];

/// Returns all embedded presets.
pub fn list_presets() -> &'static [EmbeddedPreset] {
    PRESETS
}

/// Looks up a preset by name.
///
/// Returns `None` if the preset doesn't exist.
pub fn get_preset(name: &str) -> Option<&'static EmbeddedPreset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// Returns a formatted list of preset names for error messages.
pub fn preset_names() -> Vec<&'static str> {
    PRESETS.iter().map(|p| p.name).collect()
}

/// Shared hat files embedded at compile time.
///
/// These are standalone hat definitions that presets can reference via
/// `import: ./shared-hats/<name>.yml`. Keys match the relative path
/// from the presets directory (e.g., `shared-hats/builder.yml`).
const SHARED_HATS: &[(&str, &str)] = &[
    (
        "shared-hats/builder.yml",
        include_str!("../presets/shared-hats/builder.yml"),
    ),
    (
        "shared-hats/builder-tdd.yml",
        include_str!("../presets/shared-hats/builder-tdd.yml"),
    ),
    (
        "shared-hats/committer.yml",
        include_str!("../presets/shared-hats/committer.yml"),
    ),
];

/// Looks up an embedded shared hat by its import path.
///
/// The path should be relative to the presets directory, e.g.,
/// `shared-hats/committer.yml` or `./shared-hats/committer.yml`.
pub fn get_shared_hat(path: &str) -> Option<&'static str> {
    let normalized = path.strip_prefix("./").unwrap_or(path);
    SHARED_HATS
        .iter()
        .find(|(key, _)| *key == normalized)
        .map(|(_, content)| *content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_presets_returns_all() {
        let presets = list_presets();
        assert_eq!(presets.len(), 16, "Expected 16 presets");
    }

    #[test]
    fn test_get_preset_by_name() {
        let preset = get_preset("feature");
        assert!(preset.is_some(), "feature preset should exist");
        let preset = preset.unwrap();
        assert_eq!(preset.name, "feature");
        assert!(!preset.description.is_empty());
        assert!(!preset.content.is_empty());
    }

    #[test]
    fn test_merge_loop_preset_is_embedded() {
        let preset = get_preset("merge-loop").expect("merge-loop preset should exist");
        assert_eq!(
            preset.description,
            "Merges completed parallel loop from worktree back to main branch"
        );
        // Verify key merge-related content
        assert!(preset.content.contains("RALPH_MERGE_LOOP_ID"));
        assert!(preset.content.contains("merge.start"));
        assert!(preset.content.contains("MERGE_COMPLETE"));
        assert!(preset.content.contains("conflict.detected"));
        assert!(preset.content.contains("conflict.resolved"));
        assert!(preset.content.contains("git merge"));
        assert!(preset.content.contains("git worktree remove"));
    }

    #[test]
    fn test_get_preset_invalid_name() {
        let preset = get_preset("nonexistent-preset");
        assert!(preset.is_none(), "Nonexistent preset should return None");
    }

    #[test]
    fn test_all_presets_have_description() {
        for preset in list_presets() {
            assert!(
                !preset.description.is_empty(),
                "Preset '{}' should have a description",
                preset.name
            );
        }
    }

    #[test]
    fn test_all_presets_have_content() {
        for preset in list_presets() {
            assert!(
                !preset.content.is_empty(),
                "Preset '{}' should have content",
                preset.name
            );
        }
    }

    #[test]
    fn test_preset_content_is_valid_yaml() {
        for preset in list_presets() {
            let result: Result<serde_yaml::Value, _> = serde_yaml::from_str(preset.content);
            assert!(
                result.is_ok(),
                "Preset '{}' should be valid YAML: {:?}",
                preset.name,
                result.err()
            );
        }
    }

    #[test]
    fn test_preset_names_returns_all_names() {
        let names = preset_names();
        assert_eq!(names.len(), 16);
        assert!(names.contains(&"feature"));
        assert!(names.contains(&"debug"));
        assert!(names.contains(&"merge-loop"));
        assert!(names.contains(&"code-assist"));
        assert!(names.contains(&"fresh-eyes"));
    }

    #[test]
    fn test_get_shared_hat_by_path() {
        let hat = get_shared_hat("shared-hats/committer.yml");
        assert!(hat.is_some(), "committer shared hat should exist");
        assert!(hat.unwrap().contains("Committer"));
    }

    #[test]
    fn test_get_shared_hat_strips_dot_slash() {
        let hat = get_shared_hat("./shared-hats/builder.yml");
        assert!(
            hat.is_some(),
            "builder shared hat should resolve with ./ prefix"
        );
    }

    #[test]
    fn test_get_shared_hat_returns_none_for_unknown() {
        assert!(get_shared_hat("shared-hats/nonexistent.yml").is_none());
    }

    #[test]
    fn test_shared_hats_match_disk_files() {
        // Ensure every .yml file in presets/shared-hats/ is registered in SHARED_HATS.
        // This catches forgotten additions after creating a new shared hat file.
        let shared_hats_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("presets")
            .join("shared-hats");
        if shared_hats_dir.exists() {
            let on_disk: Vec<String> = std::fs::read_dir(&shared_hats_dir)
                .unwrap()
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    std::path::Path::new(&name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("yml"))
                        .then_some(name)
                })
                .collect();
            assert_eq!(
                on_disk.len(),
                SHARED_HATS.len(),
                "Shared hat files on disk ({:?}) don't match SHARED_HATS entries ({:?})",
                on_disk,
                SHARED_HATS.iter().map(|(k, _)| *k).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_all_shared_hats_are_valid_yaml() {
        for (path, content) in SHARED_HATS {
            let result: Result<serde_yaml::Value, _> = serde_yaml::from_str(content);
            assert!(
                result.is_ok(),
                "Shared hat '{}' should be valid YAML: {:?}",
                path,
                result.err()
            );
        }
    }
}
