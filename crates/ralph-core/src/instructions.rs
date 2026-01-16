//! Instruction builder for Ralph agent prompts.
//!
//! Builds ghuntley-style prompts with numbered phases:
//! - 0a, 0b: Orientation (study specs, study context)
//! - 1, 2, 3: Workflow phases
//! - 999+: Guardrails (higher = more important)

use crate::config::{CoreConfig, EventMetadata};
use ralph_proto::Hat;
use std::collections::HashMap;

/// Builds instructions for custom hats.
///
/// Uses ghuntley methodology: numbered phases, specific verbs ("study"),
/// subagent limits (parallel for reads, single for builds).
#[derive(Debug)]
pub struct InstructionBuilder {
    core: CoreConfig,
    /// Event metadata for deriving instructions from pub/sub contracts.
    events: HashMap<String, EventMetadata>,
}

impl InstructionBuilder {
    /// Creates a new instruction builder with core configuration.
    #[allow(unused_variables)]
    pub fn new(completion_promise: impl Into<String>, core: CoreConfig) -> Self {
        Self {
            core,
            events: HashMap::new(),
        }
    }

    /// Creates a new instruction builder with event metadata for custom hats.
    #[allow(unused_variables)]
    pub fn with_events(
        completion_promise: impl Into<String>,
        core: CoreConfig,
        events: HashMap<String, EventMetadata>,
    ) -> Self {
        Self { core, events }
    }

    /// Derives instructions from a hat's pub/sub contract and event metadata.
    ///
    /// For each event the hat triggers on or publishes:
    /// 1. Check event metadata for on_trigger/on_publish instructions
    /// 2. Fall back to built-in defaults for well-known events
    ///
    /// This allows users to define custom events with custom behaviors,
    /// while still getting sensible defaults for standard events.
    fn derive_instructions_from_contract(&self, hat: &Hat) -> String {
        let mut behaviors: Vec<String> = Vec::new();

        // Derive behaviors from triggers (what this hat responds to)
        for trigger in &hat.subscriptions {
            let trigger_str = trigger.as_str();

            // First, check event metadata
            if let Some(meta) = self.events.get(trigger_str)
                && !meta.on_trigger.is_empty()
            {
                behaviors.push(format!("**On `{}`:** {}", trigger_str, meta.on_trigger));
                continue;
            }

            // Fall back to built-in defaults for well-known events
            let default_behavior = match trigger_str {
                "task.start" | "task.resume" => {
                    Some("Analyze the task and create a plan in the scratchpad.")
                }
                "build.done" => Some("Review the completed work and decide next steps."),
                "build.blocked" => Some(
                    "Analyze the blocker and decide how to unblock (simplify task, gather info, or escalate).",
                ),
                "build.task" => Some(
                    "Implement the assigned task. Follow existing patterns. Run backpressure (tests/checks). Commit when done.",
                ),
                "review.request" => Some(
                    "Review the recent changes for correctness, tests, patterns, errors, and security.",
                ),
                "review.approved" => Some("Mark the task complete `[x]` and proceed to next task."),
                "review.changes_requested" => Some("Add fix tasks to scratchpad and dispatch."),
                _ => None,
            };

            if let Some(behavior) = default_behavior {
                behaviors.push(format!("**On `{}`:** {}", trigger_str, behavior));
            }
        }

        // Derive behaviors from publishes (what this hat outputs)
        for publish in &hat.publishes {
            let publish_str = publish.as_str();

            // First, check event metadata
            if let Some(meta) = self.events.get(publish_str)
                && !meta.on_publish.is_empty()
            {
                behaviors.push(format!(
                    "**Publish `{}`:** {}",
                    publish_str, meta.on_publish
                ));
                continue;
            }

            // Fall back to built-in defaults for well-known events
            let default_behavior = match publish_str {
                "build.task" => Some("Dispatch ONE AT A TIME for pending `[ ]` tasks."),
                "build.done" => Some("When implementation is finished and tests pass."),
                "build.blocked" => Some("When stuck - include what you tried and why it failed."),
                "review.request" => Some("After build completion, before marking done."),
                "review.approved" => Some("If changes look good and meet requirements."),
                "review.changes_requested" => Some("If issues found - include specific feedback."),
                _ => None,
            };

            if let Some(behavior) = default_behavior {
                behaviors.push(format!("**Publish `{}`:** {}", publish_str, behavior));
            }
        }

        // Add must-publish rule if hat has publishable events
        if !hat.publishes.is_empty() {
            let topics: Vec<&str> = hat.publishes.iter().map(|t| t.as_str()).collect();
            behaviors.push(format!(
                "**IMPORTANT:** Every iteration MUST publish one of: `{}` or the loop will terminate.",
                topics.join("`, `")
            ));
        }

        if behaviors.is_empty() {
            "Follow the incoming event instructions.".to_string()
        } else {
            format!("### Derived Behaviors\n\n{}", behaviors.join("\n\n"))
        }
    }

    /// Builds custom hat instructions for extended multi-agent configurations.
    ///
    /// Use this for hats beyond the default Ralph.
    /// When instructions are empty, derives them from the pub/sub contract.
    pub fn build_custom_hat(&self, hat: &Hat, events_context: &str) -> String {
        let guardrails = self
            .core
            .guardrails
            .iter()
            .enumerate()
            .map(|(i, g)| format!("{}. {g}", 999 + i))
            .collect::<Vec<_>>()
            .join("\n");

        let role_instructions = if hat.instructions.is_empty() {
            self.derive_instructions_from_contract(hat)
        } else {
            hat.instructions.clone()
        };

        let (publish_topics, must_publish) = if hat.publishes.is_empty() {
            (String::new(), String::new())
        } else {
            let topics: Vec<&str> = hat.publishes.iter().map(|t| t.as_str()).collect();
            let topics_list = topics.join(", ");
            let topics_backticked = format!("`{}`", topics.join("`, `"));

            (
                format!("You publish to: {}", topics_list),
                format!(
                    "\n\n**You MUST publish one of these events:** {}\nFailure to publish will terminate the loop.",
                    topics_backticked
                ),
            )
        };

        format!(
            r"You are {name}. Fresh context each iteration.

### 0. ORIENTATION
Study the incoming event context.
Don't assume work isn't done—verify first.

### 1. EXECUTE
{role_instructions}
Only 1 subagent for build/tests.

### 2. REPORT
Publish result event with evidence.
{publish_topics}{must_publish}

### GUARDRAILS
{guardrails}

---
INCOMING:
{events}",
            name = hat.name,
            role_instructions = role_instructions,
            publish_topics = publish_topics,
            must_publish = must_publish,
            guardrails = guardrails,
            events = events_context,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_builder(promise: &str) -> InstructionBuilder {
        InstructionBuilder::new(promise, CoreConfig::default())
    }

    #[test]
    fn test_custom_hat_with_ghuntley_patterns() {
        let builder = default_builder("DONE");
        let hat = Hat::new("reviewer", "Code Reviewer")
            .with_instructions("Review PRs for quality and correctness.");

        let instructions = builder.build_custom_hat(&hat, "PR #123 ready for review");

        // Custom role with ghuntley style identity
        assert!(instructions.contains("Code Reviewer"));
        assert!(instructions.contains("Fresh context each iteration"));

        // Numbered orientation phase
        assert!(instructions.contains("### 0. ORIENTATION"));
        assert!(instructions.contains("Study the incoming event context"));
        assert!(instructions.contains("Don't assume work isn't done"));

        // Numbered execute phase
        assert!(instructions.contains("### 1. EXECUTE"));
        assert!(instructions.contains("Review PRs for quality"));
        assert!(instructions.contains("Only 1 subagent for build/tests"));

        // Report phase
        assert!(instructions.contains("### 2. REPORT"));

        // Guardrails section with high numbers
        assert!(instructions.contains("### GUARDRAILS"));
        assert!(instructions.contains("999."));

        // Event context is included
        assert!(instructions.contains("PR #123 ready for review"));
    }

    #[test]
    fn test_custom_guardrails_injected() {
        let custom_core = CoreConfig {
            scratchpad: ".workspace/plan.md".to_string(),
            specs_dir: "./specifications/".to_string(),
            guardrails: vec!["Custom rule one".to_string(), "Custom rule two".to_string()],
        };
        let builder = InstructionBuilder::new("DONE", custom_core);

        let hat = Hat::new("worker", "Worker").with_instructions("Do the work.");
        let instructions = builder.build_custom_hat(&hat, "context");

        // Custom guardrails are injected with 999+ numbering
        assert!(instructions.contains("999. Custom rule one"));
        assert!(instructions.contains("1000. Custom rule two"));
    }

    #[test]
    fn test_must_publish_injected_for_explicit_instructions() {
        use ralph_proto::Topic;

        let builder = default_builder("DONE");
        let hat = Hat::new("reviewer", "Code Reviewer")
            .with_instructions("Review PRs for quality and correctness.")
            .with_publishes(vec![
                Topic::new("review.approved"),
                Topic::new("review.changes_requested"),
            ]);

        let instructions = builder.build_custom_hat(&hat, "PR #123 ready");

        // Must-publish rule should be injected even with explicit instructions
        assert!(
            instructions.contains("You MUST publish one of these events"),
            "Must-publish rule should be injected for custom hats with publishes"
        );
        assert!(instructions.contains("`review.approved`"));
        assert!(instructions.contains("`review.changes_requested`"));
        assert!(instructions.contains("Failure to publish will terminate the loop"));
    }

    #[test]
    fn test_must_publish_not_injected_when_no_publishes() {
        let builder = default_builder("DONE");
        let hat = Hat::new("observer", "Silent Observer")
            .with_instructions("Observe and log, but do not emit events.");

        let instructions = builder.build_custom_hat(&hat, "Observe this");

        // No must-publish rule when hat has no publishes
        assert!(
            !instructions.contains("You MUST publish"),
            "Must-publish rule should NOT be injected when hat has no publishes"
        );
    }

    #[test]
    fn test_derived_behaviors_when_no_explicit_instructions() {
        use ralph_proto::Topic;

        let builder = default_builder("DONE");
        let hat = Hat::new("builder", "Builder")
            .subscribe("build.task")
            .with_publishes(vec![Topic::new("build.done"), Topic::new("build.blocked")]);

        let instructions = builder.build_custom_hat(&hat, "Implement feature X");

        // Should derive behaviors from pub/sub contract
        assert!(instructions.contains("Derived Behaviors"));
        assert!(instructions.contains("build.task"));
    }
}
