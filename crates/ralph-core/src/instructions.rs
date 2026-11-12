//! Instruction builder for Ralph agent prompts.
//!
//! Builds ghuntley-style prompts with numbered phases:
//! - 0a, 0b: Orientation (study specs, study context)
//! - 1, 2, 3: Workflow phases
//! - 999+: Guardrails (higher = more important)

use crate::config::{CoreConfig, EventMetadata, ScratchpadConfig};
use ralph_proto::Hat;
use std::collections::HashMap;

const DEFAULT_SCRATCHPAD_GUARDRAIL: &str = "Fresh context each iteration - scratchpad is memory";
const DISABLED_SCRATCHPAD_GUARDRAIL: &str =
    "Fresh context each iteration - runtime tasks and events are memory";

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
    pub fn new(core: CoreConfig) -> Self {
        Self {
            core,
            events: HashMap::new(),
        }
    }

    /// Creates a new instruction builder with event metadata for custom hats.
    pub fn with_events(core: CoreConfig, events: HashMap<String, EventMetadata>) -> Self {
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
    fn task_planning_behavior(scratchpad: &ScratchpadConfig) -> String {
        if !scratchpad.enabled {
            return "Analyze the task and create a plan.".to_string();
        }

        if scratchpad.path == ScratchpadConfig::default().path {
            "Analyze the task and create a plan in the scratchpad.".to_string()
        } else {
            format!(
                "Analyze the task and create a plan in the scratchpad at `{}`.",
                scratchpad.path
            )
        }
    }

    fn changes_requested_behavior(scratchpad: &ScratchpadConfig) -> String {
        if !scratchpad.enabled {
            return "Add fix tasks with `ralph tools task` and dispatch.".to_string();
        }

        if scratchpad.path == ScratchpadConfig::default().path {
            "Add fix tasks to scratchpad and dispatch.".to_string()
        } else {
            format!(
                "Add fix tasks to the scratchpad at `{}` and dispatch.",
                scratchpad.path
            )
        }
    }

    fn guardrails(&self, scratchpad: &ScratchpadConfig) -> String {
        self.core
            .guardrails
            .iter()
            .enumerate()
            .map(|(i, guardrail)| {
                let guardrail = if !scratchpad.enabled && guardrail == DEFAULT_SCRATCHPAD_GUARDRAIL
                {
                    DISABLED_SCRATCHPAD_GUARDRAIL
                } else {
                    guardrail
                };
                format!("{}. {guardrail}", 999 + i)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn derive_instructions_from_contract(
        &self,
        hat: &Hat,
        scratchpad: &ScratchpadConfig,
    ) -> String {
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
                "task.start" | "task.resume" => Some(Self::task_planning_behavior(scratchpad)),
                "build.done" => {
                    Some("Review the completed work and decide next steps.".to_string())
                }
                "build.blocked" => Some(
                    "Analyze the blocker and decide how to unblock (simplify task, gather info, or escalate)."
                        .to_string(),
                ),
                "build.task" => Some(
                    "Implement the assigned task. Follow existing patterns. Run backpressure (tests/lint/typecheck/audit/coverage/specs; mutation testing when configured). Commit atomically when tests pass."
                        .to_string(),
                ),
                "review.request" => Some(
                    "Review the recent changes for correctness, tests, patterns, errors, and security."
                        .to_string(),
                ),
                "review.approved" => {
                    Some("Mark the task complete `[x]` and proceed to next task.".to_string())
                }
                "review.changes_requested" => Some(Self::changes_requested_behavior(scratchpad)),
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
                "build.task" => Some("Dispatch ONE AT A TIME for pending `[ ]` tasks.".to_string()),
                "build.done" => Some("When implementation is finished and tests pass.".to_string()),
                "build.blocked" => {
                    Some("When stuck - include what you tried and why it failed.".to_string())
                }
                "review.request" => {
                    Some("After build completion, before marking done.".to_string())
                }
                "review.approved" => {
                    Some("If changes look good and meet requirements.".to_string())
                }
                "review.changes_requested" => {
                    Some("If issues found - include specific feedback.".to_string())
                }
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
                "Publish one of `{}` every iteration; an iteration without one terminates the loop.",
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
    pub fn build_custom_hat(
        &self,
        hat: &Hat,
        events_context: &str,
        scratchpad: &ScratchpadConfig,
    ) -> String {
        let guardrails = self.guardrails(scratchpad);
        let role_instructions = if hat.instructions.is_empty() {
            self.derive_instructions_from_contract(hat, scratchpad)
        } else {
            hat.instructions.clone()
        };

        let (publish_topics, must_publish) = if hat.publishes.is_empty() {
            (String::new(), String::new())
        } else {
            let topics: Vec<&str> = hat.publishes.iter().map(|t| t.as_str()).collect();
            let topics_list = topics.join(", ");
            let topics_backticked = format!("`{}`", topics.join("`, `"));
            let example_topic = topics.first().copied().unwrap_or("event.name");

            (
                format!("You publish to: {}", topics_list),
                format!(
                    "\n\nEmit exactly one of these events with `ralph emit \"<topic>\" \"<summary>\"`: {}\nUse `ralph emit \"{}\" \"<summary>\"` as the pattern; a plain-language summary does not publish anything.\nStop after emitting. An iteration that ends without publishing terminates the loop.",
                    topics_backticked, example_topic
                ),
            )
        };

        format!(
            r"You are {name}. You have fresh context each iteration.

### 0. ORIENTATION
Start from the incoming event context, and check what already exists before assuming work is missing.

### 0b. TOOL DISCIPLINE
Runtime work state lives in `ralph tools task`, not in ad hoc markdown checklists.
Check `<ready-tasks>` before creating more tasks, and track durable work with `ralph tools task ensure`, `start`, `close`, `reopen`, or `fail`.
Search memories with `ralph tools memory search` before acting in an unfamiliar area, and save durable learnings with `ralph tools memory add`.
The workflow commands are available while the loop runs; `$RALPH_BIN` points at the current Ralph executable (`$RALPH_BIN emit ...`, `$RALPH_BIN tools ...`).
Many of them print little or nothing on success, so check the side effect (task/event state, or the files the command should have changed) before concluding one failed.
Keep temporary artifacts where later steps can still inspect them, such as a repo-local `logs/` directory or `/var/tmp`.
If a blocker stops the work and you cannot work through it this iteration, fail or reopen the relevant runtime task with the reason.
Make reversible decisions yourself and record consequential ones in `.ralph/agent/decisions.md`.
If this turn will run longer than a few minutes, send a progress update with `ralph tools interact progress`.

### 1. EXECUTE
{role_instructions}
Run builds and tests in one place at a time, because parallel runs contend for the same build output.

### 2. VERIFY
Run the tests and verify the implementation before reporting done, and include the evidence (test output, build result).
Close a task only when the implementation is complete, the tests pass, and the build succeeds where there is one.

### 3. REPORT
Publish a result event with that evidence using `ralph emit`.
{publish_topics}{must_publish}

### GUARDRAILS
{guardrails}

---
Handle these events:
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
    use crate::config::ScratchpadConfig;

    fn default_builder() -> InstructionBuilder {
        InstructionBuilder::new(CoreConfig::default())
    }

    #[test]
    fn test_custom_hat_with_rfc2119_patterns() {
        let builder = default_builder();
        let hat = Hat::new("reviewer", "Code Reviewer")
            .with_instructions("Review PRs for quality and correctness.");

        let instructions = builder.build_custom_hat(
            &hat,
            "PR #123 ready for review",
            &ScratchpadConfig::default(),
        );

        // Custom role with RFC2119 style identity
        assert!(instructions.contains("Code Reviewer"));
        assert!(instructions.contains("You have fresh context each iteration"));

        // Numbered orientation phase with RFC2119
        assert!(instructions.contains("### 0. ORIENTATION"));
        assert!(instructions.contains("Start from the incoming event context"));
        assert!(instructions.contains("check what already exists before assuming work is missing"));

        assert!(instructions.contains("### 0b. TOOL DISCIPLINE"));
        assert!(instructions.contains("Check `<ready-tasks>` before creating more tasks"));
        assert!(
            instructions
                .contains("`ralph tools task ensure`, `start`, `close`, `reopen`, or `fail`")
        );
        assert!(instructions.contains("`ralph tools memory search`"));
        assert!(instructions.contains("`ralph tools memory add`"));
        assert!(instructions.contains(".ralph/agent/decisions.md"));
        assert!(instructions.contains("ralph tools interact progress"));

        // Numbered execute phase with RFC2119
        assert!(instructions.contains("### 1. EXECUTE"));
        assert!(instructions.contains("Review PRs for quality"));
        assert!(instructions.contains("Run builds and tests in one place at a time"));

        // Verify phase with RFC2119 (task completion verification)
        assert!(instructions.contains("### 2. VERIFY"));
        assert!(instructions.contains("Run the tests and verify the implementation"));
        assert!(instructions.contains("Close a task only when the implementation is complete"));

        // Report phase with RFC2119
        assert!(instructions.contains("### 3. REPORT"));
        assert!(
            instructions.contains("Publish a result event with that evidence using `ralph emit`")
        );

        // Guardrails section with high numbers
        assert!(instructions.contains("### GUARDRAILS"));
        assert!(instructions.contains("999."));

        // Event context is included with RFC2119 directive
        assert!(instructions.contains("Handle these events"));
        assert!(instructions.contains("PR #123 ready for review"));
    }

    #[test]
    fn test_custom_guardrails_injected() {
        let scratchpad = ScratchpadConfig {
            enabled: true,
            path: ".workspace/plan.md".to_string(),
        };
        let custom_core = CoreConfig {
            scratchpad: scratchpad.clone(),
            specs_dir: "./specifications/".to_string(),
            guardrails: vec!["Custom rule one".to_string(), "Custom rule two".to_string()],
            workspace_root: std::path::PathBuf::from("."),
        };
        let builder = InstructionBuilder::new(custom_core);

        let hat = Hat::new("worker", "Worker").with_instructions("Do the work.");
        let instructions = builder.build_custom_hat(&hat, "context", &scratchpad);

        // Custom guardrails are injected with 999+ numbering
        assert!(instructions.contains("999. Custom rule one"));
        assert!(instructions.contains("1000. Custom rule two"));
    }

    #[test]
    fn test_must_publish_injected_for_explicit_instructions() {
        use ralph_proto::Topic;

        let builder = default_builder();
        let hat = Hat::new("reviewer", "Code Reviewer")
            .with_instructions("Review PRs for quality and correctness.")
            .with_publishes(vec![
                Topic::new("review.approved"),
                Topic::new("review.changes_requested"),
            ]);

        let instructions =
            builder.build_custom_hat(&hat, "PR #123 ready", &ScratchpadConfig::default());

        // Must-publish rule should be injected even with explicit instructions (RFC2119)
        assert!(
            instructions.contains("Emit exactly one of these events with `ralph emit"),
            "Must-publish rule should be injected for custom hats with publishes"
        );
        assert!(instructions.contains("`review.approved`"));
        assert!(instructions.contains("`review.changes_requested`"));
        assert!(instructions.contains("a plain-language summary does not publish anything"));
        assert!(instructions.contains("Stop after emitting"));
        assert!(
            instructions.contains("An iteration that ends without publishing terminates the loop")
        );
    }

    #[test]
    fn test_must_publish_not_injected_when_no_publishes() {
        let builder = default_builder();
        let hat = Hat::new("observer", "Silent Observer")
            .with_instructions("Observe and log, but do not emit events.");

        let instructions =
            builder.build_custom_hat(&hat, "Observe this", &ScratchpadConfig::default());

        // No must-publish rule when hat has no publishes
        // Note: The prompt says "You MUST publish a result event" in the REPORT section,
        // but the specific emitted-topics list should not appear
        assert!(
            !instructions.contains("Emit exactly one of these events with `ralph emit"),
            "Specific must-publish list should NOT be injected when hat has no publishes"
        );
    }

    #[test]
    fn test_derived_behaviors_when_no_explicit_instructions() {
        use ralph_proto::Topic;

        let builder = default_builder();
        let hat = Hat::new("builder", "Builder")
            .subscribe("build.task")
            .with_publishes(vec![Topic::new("build.done"), Topic::new("build.blocked")]);

        let instructions =
            builder.build_custom_hat(&hat, "Implement feature X", &ScratchpadConfig::default());

        // Should derive behaviors from pub/sub contract
        assert!(instructions.contains("Derived Behaviors"));
        assert!(instructions.contains("build.task"));
    }

    #[test]
    fn test_derived_behaviors_keep_default_scratchpad_wording() {
        let builder = default_builder();
        let hat = Hat::new("planner", "Planner")
            .subscribe("task.start")
            .subscribe("review.changes_requested");

        let instructions =
            builder.build_custom_hat(&hat, "Plan the work", &ScratchpadConfig::default());

        assert!(instructions.contains("Analyze the task and create a plan in the scratchpad."));
        assert!(instructions.contains("Add fix tasks to scratchpad and dispatch."));
        assert!(
            !instructions.contains(".ralph/agent/scratchpad.md"),
            "Default scratchpad path should not be newly injected into inherited defaults"
        );
    }

    #[test]
    fn test_derived_behaviors_omit_scratchpad_when_disabled() {
        let builder = default_builder();
        let scratchpad = ScratchpadConfig {
            enabled: false,
            path: ".ralph/agent/scratchpad.md".to_string(),
        };
        let hat = Hat::new("planner", "Planner")
            .subscribe("task.start")
            .subscribe("review.changes_requested");

        let instructions = builder.build_custom_hat(&hat, "Plan the work", &scratchpad);

        assert!(instructions.contains("Analyze the task and create a plan."));
        assert!(instructions.contains("Add fix tasks with `ralph tools task` and dispatch."));
        assert!(
            !instructions.to_ascii_lowercase().contains("scratchpad"),
            "Disabled hat prompts must not instruct agents to use scratchpad"
        );
    }

    #[test]
    fn test_derived_behaviors_name_custom_scratchpad_path() {
        let builder = default_builder();
        let scratchpad = ScratchpadConfig {
            enabled: true,
            path: ".ralph/agent/planner.md".to_string(),
        };
        let hat = Hat::new("planner", "Planner")
            .subscribe("task.start")
            .subscribe("review.changes_requested");

        let instructions = builder.build_custom_hat(&hat, "Plan the work", &scratchpad);

        assert!(instructions.contains(
            "Analyze the task and create a plan in the scratchpad at `.ralph/agent/planner.md`."
        ));
        assert!(instructions.contains(
            "Add fix tasks to the scratchpad at `.ralph/agent/planner.md` and dispatch."
        ));
    }
}
