//! Embedded presets for ralph init command.
//!
//! This module embeds all preset YAML files at compile time, making the
//! binary self-contained. Users can initialize projects with presets
//! without needing access to the source repository.

/// An embedded preset with its name, description, and full content.
#[derive(Debug, Clone)]
pub struct EmbeddedPreset {
    /// The preset name (e.g., "tdd-red-green")
    pub name: &'static str,
    /// Short description extracted from the preset's header comment
    pub description: &'static str,
    /// Full YAML content of the preset
    pub content: &'static str,
}

/// All embedded presets, compiled into the binary.
const PRESETS: &[EmbeddedPreset] = &[
    EmbeddedPreset {
        name: "adversarial-review",
        description: "Red Team / Blue Team Security Review",
        content: include_str!("../../../presets/adversarial-review.yml"),
    },
    EmbeddedPreset {
        name: "api-design",
        description: "API-First Design Workflow",
        content: include_str!("../../../presets/api-design.yml"),
    },
    EmbeddedPreset {
        name: "code-archaeology",
        description: "Legacy Code Understanding and Modernization",
        content: include_str!("../../../presets/code-archaeology.yml"),
    },
    EmbeddedPreset {
        name: "debug",
        description: "Bug investigation and root cause analysis",
        content: include_str!("../../../presets/debug.yml"),
    },
    EmbeddedPreset {
        name: "deploy",
        description: "Deployment and Release Workflow",
        content: include_str!("../../../presets/deploy.yml"),
    },
    EmbeddedPreset {
        name: "docs",
        description: "Documentation Generation Workflow",
        content: include_str!("../../../presets/docs.yml"),
    },
    EmbeddedPreset {
        name: "documentation-first",
        description: "Documentation-Driven Development",
        content: include_str!("../../../presets/documentation-first.yml"),
    },
    EmbeddedPreset {
        name: "feature",
        description: "Feature Development with integrated code review",
        content: include_str!("../../../presets/feature.yml"),
    },
    EmbeddedPreset {
        name: "feature-minimal",
        description: "Minimal feature development with auto-derived instructions",
        content: include_str!("../../../presets/feature-minimal.yml"),
    },
    EmbeddedPreset {
        name: "gap-analysis",
        description: "Gap Analysis and Planning Workflow",
        content: include_str!("../../../presets/gap-analysis.yml"),
    },
    EmbeddedPreset {
        name: "hatless-baseline",
        description: "Baseline hatless mode for comparison",
        content: include_str!("../../../presets/hatless-baseline.yml"),
    },
    EmbeddedPreset {
        name: "incident-response",
        description: "Production Incident Response Workflow",
        content: include_str!("../../../presets/incident-response.yml"),
    },
    EmbeddedPreset {
        name: "migration-safety",
        description: "Safe Database/API Migration Workflow",
        content: include_str!("../../../presets/migration-safety.yml"),
    },
    EmbeddedPreset {
        name: "mob-programming",
        description: "Mob Programming with rotating roles",
        content: include_str!("../../../presets/mob-programming.yml"),
    },
    EmbeddedPreset {
        name: "performance-optimization",
        description: "Performance Analysis and Optimization",
        content: include_str!("../../../presets/performance-optimization.yml"),
    },
    EmbeddedPreset {
        name: "pr-review",
        description: "Multi-perspective PR code review",
        content: include_str!("../../../presets/pr-review.yml"),
    },
    EmbeddedPreset {
        name: "refactor",
        description: "Code Refactoring Workflow",
        content: include_str!("../../../presets/refactor.yml"),
    },
    EmbeddedPreset {
        name: "research",
        description: "Deep exploration and analysis tasks",
        content: include_str!("../../../presets/research.yml"),
    },
    EmbeddedPreset {
        name: "review",
        description: "Code Review Workflow",
        content: include_str!("../../../presets/review.yml"),
    },
    EmbeddedPreset {
        name: "scientific-method",
        description: "Hypothesis-driven experimentation",
        content: include_str!("../../../presets/scientific-method.yml"),
    },
    EmbeddedPreset {
        name: "socratic-learning",
        description: "Learning through guided questioning",
        content: include_str!("../../../presets/socratic-learning.yml"),
    },
    EmbeddedPreset {
        name: "spec-driven",
        description: "Specification-Driven Development",
        content: include_str!("../../../presets/spec-driven.yml"),
    },
    EmbeddedPreset {
        name: "tdd-red-green",
        description: "Test-Driven Development with red-green-refactor cycle",
        content: include_str!("../../../presets/tdd-red-green.yml"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_presets_returns_all() {
        let presets = list_presets();
        assert_eq!(presets.len(), 23, "Expected 23 presets");
    }

    #[test]
    fn test_get_preset_by_name() {
        let preset = get_preset("tdd-red-green");
        assert!(preset.is_some(), "tdd-red-green preset should exist");
        let preset = preset.unwrap();
        assert_eq!(preset.name, "tdd-red-green");
        assert!(!preset.description.is_empty());
        assert!(!preset.content.is_empty());
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
        assert_eq!(names.len(), 23);
        assert!(names.contains(&"tdd-red-green"));
        assert!(names.contains(&"debug"));
    }
}
