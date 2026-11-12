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
    /// Whether this preset should be shown in normal user-facing listings.
    pub public: bool,
}

/// All embedded presets, compiled into the binary.
const PRESETS: &[EmbeddedPreset] = &[
    EmbeddedPreset {
        name: "autoresearch",
        description: "Autonomous experiment loop: try ideas, measure, keep what works, discard what doesn't",
        content: include_str!("../presets/autoresearch.yml"),
        public: true,
    },
    EmbeddedPreset {
        name: "code-assist",
        description: "Default implementation workflow with TDD and adversarial validation",
        content: include_str!("../presets/code-assist.yml"),
        public: true,
    },
    EmbeddedPreset {
        name: "debug",
        description: "Bug investigation, root-cause analysis, and adversarial fix verification",
        content: include_str!("../presets/debug.yml"),
        public: true,
    },
    EmbeddedPreset {
        name: "hatless-baseline",
        description: "Baseline hatless mode for comparison",
        content: include_str!("../presets/hatless-baseline.yml"),
        public: false,
    },
    EmbeddedPreset {
        name: "merge-loop",
        description: "Merges completed parallel loop from worktree back to main branch",
        content: include_str!("../presets/merge-loop.yml"),
        public: false,
    },
    EmbeddedPreset {
        name: "pdd-to-code-assist",
        description: "Advanced end-to-end idea-to-code workflow; powerful, slower, and best treated as a fun example",
        content: include_str!("../presets/pdd-to-code-assist.yml"),
        public: true,
    },
    EmbeddedPreset {
        name: "research",
        description: "Read-only codebase and architecture exploration with evidence-first synthesis",
        content: include_str!("../presets/research.yml"),
        public: true,
    },
    EmbeddedPreset {
        name: "review",
        description: "Adversarial code review without making modifications",
        content: include_str!("../presets/review.yml"),
        public: true,
    },
];

/// Returns all embedded presets.
pub fn list_presets() -> Vec<&'static EmbeddedPreset> {
    PRESETS.iter().filter(|preset| preset.public).collect()
}

/// Looks up a preset by name.
///
/// Returns `None` if the preset doesn't exist.
pub fn get_preset(name: &str) -> Option<&'static EmbeddedPreset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// Returns a formatted list of preset names for error messages.
pub fn preset_names() -> Vec<&'static str> {
    PRESETS
        .iter()
        .filter(|preset| preset.public)
        .map(|preset| preset.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_core::RalphConfig;

    fn assert_public_preset_has_completion_path(preset: &EmbeddedPreset) {
        let config =
            RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");
        let promise = config.event_loop.completion_promise.trim();
        assert!(
            !promise.is_empty(),
            "Preset '{}' must define a non-empty completion promise",
            preset.name
        );

        let has_completion_path = config.hats.values().any(|hat| {
            hat.publishes.iter().any(|topic| topic == promise)
                || hat.default_publishes.as_deref() == Some(promise)
        });

        assert!(
            has_completion_path,
            "Preset '{}' must expose its completion promise '{}' via publishes/default_publishes",
            preset.name, promise
        );
    }

    fn assert_public_preset_has_required_events(preset: &EmbeddedPreset) {
        let config =
            RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");
        assert!(
            !config.event_loop.required_events.is_empty(),
            "Preset '{}' should define required_events to block premature completion",
            preset.name
        );
    }

    #[test]
    fn test_list_presets_returns_all() {
        let presets = list_presets();
        assert_eq!(presets.len(), 6, "Expected 6 public presets");
    }

    #[test]
    fn test_get_preset_by_name() {
        let preset = get_preset("code-assist");
        assert!(preset.is_some(), "code-assist preset should exist");
        let preset = preset.unwrap();
        assert_eq!(preset.name, "code-assist");
        assert_ne!(preset.description, "");
        assert_ne!(preset.content, "");
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
        for preset in PRESETS {
            assert!(
                !preset.description.is_empty(),
                "Preset '{}' should have a description",
                preset.name
            );
        }
    }

    #[test]
    fn test_all_presets_have_content() {
        for preset in PRESETS {
            assert!(
                !preset.content.is_empty(),
                "Preset '{}' should have content",
                preset.name
            );
        }
    }

    #[test]
    fn test_preset_content_is_valid_yaml() {
        for preset in PRESETS {
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
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"autoresearch"));
        assert!(names.contains(&"debug"));
        assert!(names.contains(&"code-assist"));
        assert!(names.contains(&"research"));
        assert!(names.contains(&"review"));
        assert!(names.contains(&"pdd-to-code-assist"));
    }

    #[test]
    fn test_public_presets_have_completion_path() {
        for preset in PRESETS.iter().filter(|preset| preset.public) {
            assert_public_preset_has_completion_path(preset);
        }
    }

    #[test]
    fn test_public_presets_have_required_events() {
        for preset in PRESETS.iter().filter(|preset| preset.public) {
            assert_public_preset_has_required_events(preset);
        }
    }

    #[test]
    fn test_embedded_presets_use_canonical_artifact_boundary() {
        for preset in PRESETS {
            let config =
                RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");

            assert_eq!(
                config.core.specs_dir, ".ralph/specs/",
                "Preset '{}' should use the canonical committed specs boundary",
                preset.name
            );
            assert!(
                !preset.content.contains(".agents/scratchpad"),
                "Preset '{}' should not direct artifacts to the legacy agents scratchpad",
                preset.name
            );
            assert!(
                !preset.content.contains("./specs/"),
                "Preset '{}' should not direct artifacts to the legacy top-level specs directory",
                preset.name
            );
        }
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn test_pdd_to_code_assist_uses_reviewed_increment_loop() {
        let preset = get_preset("pdd-to-code-assist").expect("pdd-to-code-assist should exist");
        let config =
            RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");

        assert_eq!(config.core.specs_dir, ".ralph/specs/");
        assert!(preset.content.contains(".ralph/specs/{spec_name}/"));
        assert!(preset.content.contains("idea-honing.md"));
        assert!(!preset.content.contains("requirements-interview.md"));

        assert_eq!(
            config.event_loop.required_events,
            strings(&[
                "design.approved",
                "plan.ready",
                "tasks.ready",
                "implementation.ready",
                "validation.passed",
            ])
        );

        let builder = config
            .hats
            .get("builder")
            .expect("builder hat should exist");
        for trigger in [
            "tasks.ready",
            "review.rejected",
            "finalization.failed",
            "validation.failed",
        ] {
            assert!(builder.triggers.contains(&trigger.to_string()));
        }
        assert_eq!(
            builder.publishes,
            strings(&["review.ready", "build.blocked"])
        );
        assert_eq!(builder.default_publishes.as_deref(), Some("build.blocked"));
        assert!(builder.instructions.contains("`task_id`"));
        assert!(builder.instructions.contains("`task_key`"));
        assert!(
            builder
                .instructions
                .contains("ralph tools task show <task_id> --format json")
        );
        assert!(
            builder
                .instructions
                .contains("ralph tools task start <task_id>")
        );

        let critic = config.hats.get("critic").expect("critic hat should exist");
        assert_eq!(critic.triggers, strings(&["review.ready"]));
        assert_eq!(
            critic.publishes,
            strings(&["review.passed", "review.rejected"])
        );
        assert_eq!(critic.default_publishes.as_deref(), Some("review.rejected"));
        assert!(critic.instructions.contains("`task_id`"));
        assert!(critic.instructions.contains("`task_key`"));
        assert!(
            critic
                .instructions
                .contains("ralph tools task show <task_id> --format json")
        );
        assert!(critic.instructions.contains("ralph tools memory add"));

        let finalizer = config
            .hats
            .get("finalizer")
            .expect("finalizer hat should exist");
        assert_eq!(finalizer.triggers, strings(&["review.passed"]));
        assert_eq!(
            finalizer.publishes,
            strings(&[
                "queue.advance",
                "implementation.ready",
                "finalization.failed"
            ])
        );
        assert_eq!(
            finalizer.default_publishes.as_deref(),
            Some("finalization.failed")
        );
        assert!(
            finalizer
                .instructions
                .contains("ralph tools task close <task_id>")
        );
        assert!(
            finalizer
                .instructions
                .contains("ralph tools task reopen <task_id>")
        );

        let task_writer = config
            .hats
            .get("task_writer")
            .expect("task_writer hat should exist");
        assert_eq!(
            task_writer.triggers,
            strings(&["plan.ready", "queue.advance"])
        );
        assert!(
            task_writer
                .instructions
                .contains("`pdd:{spec_name}:step-01:{task_slug}`")
        );

        let validator = config
            .hats
            .get("validator")
            .expect("validator hat should exist");
        assert_eq!(
            validator.default_publishes.as_deref(),
            Some("validation.failed")
        );
        assert!(
            validator
                .instructions
                .contains("`pdd:{spec_name}:validation`")
        );

        let committer = config
            .hats
            .get("committer")
            .expect("committer hat should exist");
        assert_eq!(committer.default_publishes, None);
        assert_eq!(committer.publishes, strings(&["LOOP_COMPLETE"]));
        assert!(committer.instructions.contains("`pdd:{spec_name}:commit`"));
    }

    #[test]
    fn test_code_assist_uses_canonical_artifact_layout_and_builder_workflow() {
        let preset = get_preset("code-assist").expect("code-assist should exist");
        let config =
            RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");

        assert_eq!(config.core.specs_dir, ".ralph/specs/");
        assert_eq!(
            config.event_loop.required_events,
            strings(&["review.passed"])
        );

        let planner = config
            .hats
            .get("planner")
            .expect("planner hat should exist");
        assert_eq!(planner.triggers, strings(&["build.start", "queue.advance"]));
        assert!(planner.instructions.contains(".ralph/specs/{task_name}/"));
        assert!(planner.instructions.contains("ralph tools task ensure"));
        assert!(
            planner
                .instructions
                .contains("`code-assist:{task_name}:step-NN:{slug}`")
        );
        assert!(planner.instructions.contains("`task_id`"));
        assert!(planner.instructions.contains("`task_key`"));
        assert!(planner.instructions.contains("context.md"));
        assert!(planner.instructions.contains("plan.md"));
        assert!(planner.instructions.contains("progress.md"));
        assert!(!planner.instructions.contains("rough-idea.md"));
        assert!(planner.instructions.contains("`## Current Step`"));
        assert!(planner.instructions.contains("`## Active Wave`"));

        let builder = config
            .hats
            .get("builder")
            .expect("builder hat should exist");
        assert!(builder.instructions.contains("CODEASSIST.md"));
        assert!(
            builder
                .instructions
                .contains("ralph tools task show <task_id> --format json")
        );
        assert!(
            builder
                .instructions
                .contains("ralph tools task start <task_id>")
        );

        let finalizer = config
            .hats
            .get("finalizer")
            .expect("finalizer hat should exist");
        assert_eq!(
            finalizer.publishes,
            strings(&["queue.advance", "finalization.failed", "LOOP_COMPLETE"])
        );
        assert!(
            finalizer
                .instructions
                .contains("ralph tools task close <task_id>")
        );
        assert!(
            finalizer
                .instructions
                .contains("ralph tools task reopen <task_id>")
        );
        // Hats only see their own instructions, so the Finalizer needs the
        // shared docs path spelled out to find context.md / plan.md / progress.md.
        assert!(finalizer.instructions.contains(".ralph/specs/{task_name}/"));
        assert!(!finalizer.instructions.contains("task.complete"));
        // Planner owns wave creation; the Finalizer must not ensure tasks.
        assert!(!finalizer.instructions.contains("ralph tools task ensure"));
    }

    #[test]
    fn test_review_uses_staged_adversarial_completion_contract() {
        let preset = get_preset("review").expect("review preset should exist");
        let config =
            RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");

        assert_eq!(
            config.event_loop.required_events,
            strings(&["review.section", "analysis.complete"])
        );

        let reviewer = config
            .hats
            .get("reviewer")
            .expect("reviewer hat should exist");
        assert_eq!(
            reviewer.triggers,
            strings(&["review.start", "review.followup"])
        );
        assert_eq!(reviewer.publishes, strings(&["review.section"]));
        assert_eq!(
            reviewer.default_publishes.as_deref(),
            Some("review.section")
        );
        assert!(reviewer.instructions.contains("On `review.start`:"));
        assert!(reviewer.instructions.contains("On `review.followup`:"));
        assert!(reviewer.instructions.contains("`review:step-01:primary`"));
        assert!(reviewer.instructions.contains("`review:step-02:{slug}`"));
        assert!(reviewer.instructions.contains("`task_id` and `task_key`"));
        assert!(reviewer.instructions.contains(".ralph/review/"));
        assert!(reviewer.instructions.contains("findings.md"));

        let analyzer = config
            .hats
            .get("analyzer")
            .expect("analyzer hat should exist");
        assert_eq!(analyzer.triggers, strings(&["review.section"]));
        assert_eq!(analyzer.publishes, strings(&["analysis.complete"]));
        assert_eq!(
            analyzer.default_publishes.as_deref(),
            Some("analysis.complete")
        );
        assert!(
            analyzer
                .instructions
                .contains("ralph tools task start <analysis_task_id>")
        );
        assert!(
            analyzer
                .instructions
                .contains("ralph tools task close <analysis_task_id>")
        );
        assert!(analyzer.instructions.contains("adversarial"));
        // The Closer reads primary_task_id from the analyzer payload.
        assert!(analyzer.instructions.contains("`primary_task_id`"));

        let closer = config.hats.get("closer").expect("closer hat should exist");
        assert_eq!(closer.triggers, strings(&["analysis.complete"]));
        assert_eq!(
            closer.publishes,
            strings(&["review.followup", "REVIEW_COMPLETE"])
        );
        assert_eq!(closer.default_publishes, None);
        assert!(
            closer
                .instructions
                .contains("\"$RALPH_BIN\" tools task close <primary_task_id>")
        );
        // The Reviewer opens each wave's task; the Closer must not create tasks.
        assert!(!closer.instructions.contains("tools task ensure"));
    }

    #[test]
    fn test_research_uses_runtime_tasks_and_memory_discipline() {
        let preset = get_preset("research").expect("research preset should exist");
        let config =
            RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");

        assert_eq!(config.core.specs_dir, ".ralph/specs/");
        assert_eq!(
            config.event_loop.required_events,
            strings(&["research.finding"])
        );

        let researcher = config
            .hats
            .get("researcher")
            .expect("researcher hat should exist");
        assert_eq!(researcher.publishes, strings(&["research.finding"]));
        assert_eq!(
            researcher.default_publishes.as_deref(),
            Some("research.finding")
        );
        assert!(researcher.instructions.contains(".ralph/research/"));
        assert!(researcher.instructions.contains("plan.md"));
        assert!(researcher.instructions.contains("summary.md"));
        assert!(
            researcher
                .instructions
                .contains("`research:step-01:primary`")
        );
        assert!(
            researcher
                .instructions
                .contains("`research:step-02:{slug}`")
        );
        assert!(
            researcher
                .instructions
                .contains("ralph tools task start <task_id>")
        );
        assert!(researcher.instructions.contains("ralph tools memory add"));
        assert!(researcher.instructions.contains("`task_id`"));
        assert!(researcher.instructions.contains("`task_key`"));

        let synthesizer = config
            .hats
            .get("synthesizer")
            .expect("synthesizer hat should exist");
        assert!(
            synthesizer
                .instructions
                .contains("ralph tools task show <task_id> --format json")
        );
        assert!(
            synthesizer
                .instructions
                .contains("ralph tools task close <task_id>")
        );
        assert!(
            synthesizer
                .instructions
                .contains("`research:step-02:{slug}`")
        );
    }

    #[test]
    fn test_debug_uses_staged_adversarial_fix_contract() {
        let preset = get_preset("debug").expect("debug preset should exist");
        let config =
            RalphConfig::parse_yaml(preset.content).expect("embedded preset YAML should parse");

        assert_eq!(
            config.event_loop.required_events,
            strings(&[
                "hypothesis.test",
                "hypothesis.confirmed",
                "fix.applied",
                "fix.verified",
            ])
        );

        let investigator = config
            .hats
            .get("investigator")
            .expect("investigator hat should exist");
        assert_eq!(
            investigator.triggers,
            strings(&[
                "debug.start",
                "hypothesis.rejected",
                "hypothesis.confirmed",
                "fix.verified",
            ])
        );
        assert_eq!(
            investigator.publishes,
            strings(&["hypothesis.test", "fix.propose", "DEBUG_COMPLETE"])
        );
        assert!(
            investigator
                .instructions
                .contains("On `debug.start` or `hypothesis.rejected`:")
        );
        assert!(
            investigator
                .instructions
                .contains("ralph emit \"hypothesis.test\"")
        );
        assert!(
            investigator
                .instructions
                .contains("ralph tools task start <task_id>")
        );
        assert!(investigator.instructions.contains("`task_id`, `task_key`"));
        assert!(
            investigator
                .instructions
                .contains("On `hypothesis.confirmed`:")
        );
        assert!(investigator.instructions.contains("emit `fix.propose`"));
        assert!(investigator.instructions.contains("On `fix.verified`:"));

        let tester = config.hats.get("tester").expect("tester hat should exist");
        assert_eq!(tester.triggers, strings(&["hypothesis.test"]));
        assert_eq!(
            tester.publishes,
            strings(&["hypothesis.confirmed", "hypothesis.rejected"])
        );
        assert!(
            tester
                .instructions
                .contains("ralph tools task start <task_id>")
        );
        assert!(tester.instructions.contains("adversarial"));

        let fixer = config.hats.get("fixer").expect("fixer hat should exist");
        assert_eq!(fixer.publishes, strings(&["fix.applied", "fix.blocked"]));
        assert_eq!(fixer.default_publishes.as_deref(), Some("fix.blocked"));
        assert!(!fixer.instructions.contains("git commit"));
        assert!(
            fixer
                .instructions
                .contains("ralph tools task start <task_id>")
        );
        assert!(fixer.instructions.contains("ralph tools memory add"));

        let verifier = config
            .hats
            .get("verifier")
            .expect("verifier hat should exist");
        assert_eq!(verifier.publishes, strings(&["fix.verified", "fix.failed"]));
        assert_eq!(verifier.default_publishes.as_deref(), Some("fix.failed"));
        assert!(verifier.instructions.contains("adversarial"));
        assert!(
            verifier
                .instructions
                .contains("ralph tools task start <task_id>")
        );
        assert!(verifier.instructions.contains("`task_id`/`task_key`"));
    }
}
