//! Instruction builder for Ralph agent prompts.
//!
//! Philosophy: Let Ralph Ralph. Two modes for one agent wearing different hats:
//! - Coordinator (Planner hat): Plans work, owns scratchpad, validates completion
//! - Ralph (Builder hat): Implements tasks, runs backpressure, commits
//!
//! This maps directly to ghuntley's PROMPT_plan.md / PROMPT_build.md split.

use crate::config::CoreConfig;
use ralph_proto::Hat;

/// Builds the prepended instructions for agent prompts.
///
/// One agent, two hats: Coordinator (planner) and Ralph (builder).
/// The orchestrator routes events to trigger hat changes.
///
/// Per spec: "Core behaviors are always present—hats add to them, never replace."
/// The builder injects core behaviors (scratchpad, specs, guardrails) into every prompt.
#[derive(Debug)]
pub struct InstructionBuilder {
    completion_promise: String,
    core: CoreConfig,
}

impl InstructionBuilder {
    /// Creates a new instruction builder with core configuration.
    ///
    /// The core config provides paths and guardrails that are injected
    /// into every prompt, per the spec's "Core Behaviors" requirement.
    pub fn new(completion_promise: impl Into<String>, core: CoreConfig) -> Self {
        Self {
            completion_promise: completion_promise.into(),
            core,
        }
    }

    /// Builds the core behaviors section injected into all prompts.
    ///
    /// Per spec: "Every Ralph invocation includes these behaviors, regardless of which hat is active."
    fn build_core_behaviors(&self) -> String {
        let guardrails = self
            .core
            .guardrails
            .iter()
            .map(|g| format!("- {g}"))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r"## CORE BEHAVIORS (Always Active)

**Scratchpad:** `{scratchpad}` is shared state. Read it. Update it.
**Specs:** `{specs_dir}` is the source of truth. Implementations must match.

### Guardrails
{guardrails}
",
            scratchpad = self.core.scratchpad,
            specs_dir = self.core.specs_dir,
            guardrails = guardrails,
        )
    }

    /// Builds Coordinator instructions (the Planner hat).
    ///
    /// Coordinator owns the scratchpad and decides what work needs doing.
    /// It does NOT implement—that's Ralph's job (Builder hat).
    pub fn build_coordinator(&self, prompt_content: &str) -> String {
        let core_behaviors = self.build_core_behaviors();

        format!(
            "You are Coordinator Ralph. You plan work and validate completion. You do NOT implement.

{core_behaviors}

## YOUR JOB

1. **Gap analysis.** Study `{specs_dir}` and compare against the codebase. What's missing? What's broken?

2. **Own the scratchpad.** Create or update `{scratchpad}` with prioritized tasks.
   - `[ ]` pending
   - `[x]` done
   - `[~]` cancelled (with reason)

3. **Dispatch work.** Publish `<event topic=\"build.task\">` ONE AT A TIME with clear acceptance criteria.

4. **Validate completion.** When Ralph reports `build.done`, verify the work actually satisfies the spec.

## WHAT YOU DON'T DO

- ❌ Write implementation code
- ❌ Run tests (Ralph does that)
- ❌ Make commits (Ralph does that)
- ❌ Pick tasks to implement yourself

## COMPLETION

When ALL tasks are `[x]` or `[~]` and ALL specs are satisfied, output: {promise}

---
{prompt}",
            core_behaviors = core_behaviors,
            specs_dir = self.core.specs_dir,
            scratchpad = self.core.scratchpad,
            promise = self.completion_promise,
            prompt = prompt_content
        )
    }

    /// Builds Ralph instructions (the Builder hat).
    ///
    /// Ralph implements tasks. It does NOT plan or manage the scratchpad.
    pub fn build_ralph(&self, prompt_content: &str) -> String {
        let core_behaviors = self.build_core_behaviors();

        format!(
            r#"You are Ralph. You implement. One task, then done.

{core_behaviors}

## YOUR JOB

1. **Pick ONE task.** Read `{scratchpad}`, pick the highest priority `[ ]` task.

2. **Implement it.** Write the code. Follow existing patterns.

3. **Validate.** Run backpressure. Tests, typecheck, lint must pass.

4. **Commit and exit.** One task, one commit. Mark `[x]` in scratchpad. Publish `<event topic="build.done">` with changes summary.

## WHAT YOU DON'T DO

- ❌ Create the scratchpad (Coordinator does that)
- ❌ Decide what tasks to add (Coordinator does that)
- ❌ Output the completion promise (Coordinator does that)

## WHEN DONE

Mark your task `[x]` in the scratchpad. Publish `<event topic="build.done">`. Exit. Coordinator will verify.

## STUCK?

Can't finish? Publish `<event topic="build.blocked">` with:
- What you tried
- Why it failed
- What would unblock you

---
{prompt}"#,
            core_behaviors = core_behaviors,
            scratchpad = self.core.scratchpad,
            prompt = prompt_content
        )
    }

    /// Builds custom hat instructions for extended multi-agent configurations.
    ///
    /// Use this for teams beyond the default Coordinator + Ralph.
    pub fn build_custom_hat(&self, hat: &Hat, events_context: &str) -> String {
        let core_behaviors = self.build_core_behaviors();

        let role_instructions = if hat.instructions.is_empty() {
            "Follow the incoming event instructions.".to_string()
        } else {
            hat.instructions.clone()
        };

        let publish_topics = if hat.publishes.is_empty() {
            String::new()
        } else {
            format!(
                "You publish to: {}",
                hat.publishes
                    .iter()
                    .map(|t| t.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        format!(
            r#"You are {name}. Fresh context each iteration.

{core_behaviors}

## YOUR ROLE

{role_instructions}

## THE RULES

1. **One task, then exit.** The loop continues.

## EVENTS

Communicate via: `<event topic="...">message</event>`
{publish_topics}

## COMPLETION

Only Coordinator outputs: {promise}

---
INCOMING:
{events}"#,
            name = hat.name,
            core_behaviors = core_behaviors,
            role_instructions = role_instructions,
            publish_topics = publish_topics,
            promise = self.completion_promise,
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
    fn test_coordinator_plans_not_implements() {
        let builder = default_builder("LOOP_COMPLETE");
        let instructions = builder.build_coordinator("Build a CLI tool");

        // Identity
        assert!(instructions.contains("Coordinator Ralph"));
        assert!(instructions.contains("Build a CLI tool"));

        // Coordinator's job (per spec lines 236-263)
        assert!(instructions.contains("Gap analysis"));
        assert!(instructions.contains("Own the scratchpad"));
        assert!(instructions.contains("Dispatch work")); // New: dispatches build.task events
        assert!(instructions.contains("build.task")); // New: publishes build.task
        assert!(instructions.contains("ONE AT A TIME")); // New: per spec
        assert!(instructions.contains("Validate completion"));
        assert!(instructions.contains("./specs/"));

        // Task markers per spec
        assert!(instructions.contains("[ ]")); // pending
        assert!(instructions.contains("[x]")); // done
        assert!(instructions.contains("[~]")); // cancelled

        // Completion promise (Coordinator outputs it)
        assert!(instructions.contains("LOOP_COMPLETE"));

        // What Coordinator doesn't do
        assert!(instructions.contains("❌ Write implementation code"));
        assert!(instructions.contains("❌ Run tests"));
        assert!(instructions.contains("❌ Make commits"));
        assert!(instructions.contains("❌ Pick tasks to implement yourself")); // New
    }

    #[test]
    fn test_ralph_implements_not_plans() {
        let builder = default_builder("LOOP_COMPLETE");
        let instructions = builder.build_ralph("Build a CLI tool");

        // Identity - should be "Ralph" not "Ralph Ralph"
        assert!(instructions.contains("You are Ralph."));
        assert!(instructions.contains("Build a CLI tool"));

        // Ralph's job (per spec lines 266-294)
        assert!(instructions.contains("Pick ONE task"));
        assert!(instructions.contains("Implement it"));
        assert!(instructions.contains("Validate")); // New: step 3 per spec
        assert!(instructions.contains("backpressure")); // New: must run backpressure
        assert!(instructions.contains("Commit and exit"));
        assert!(instructions.contains("build.done")); // New: publishes build.done

        // What Ralph doesn't do
        assert!(instructions.contains("❌ Create the scratchpad"));
        assert!(instructions.contains("❌ Decide what tasks"));
        assert!(instructions.contains("❌ Output the completion promise"));

        // STUCK section for build.blocked events
        assert!(instructions.contains("## STUCK?"));
        assert!(instructions.contains("build.blocked"));

        // Should NOT contain completion promise in output
        assert!(!instructions.contains("LOOP_COMPLETE"));
    }

    #[test]
    fn test_coordinator_and_ralph_share_guardrails() {
        let builder = default_builder("DONE");
        let coordinator = builder.build_coordinator("test");
        let ralph = builder.build_ralph("test");

        // Both reference the scratchpad (from CoreConfig)
        assert!(coordinator.contains(".agent/scratchpad.md"));
        assert!(ralph.contains(".agent/scratchpad.md"));

        // Both include default guardrails
        assert!(coordinator.contains("search first"));
        assert!(ralph.contains("search first"));
        assert!(coordinator.contains("Backpressure"));
        assert!(ralph.contains("Backpressure"));

        // Both use task markers
        assert!(coordinator.contains("[x]"));
        assert!(ralph.contains("[x]"));
        assert!(coordinator.contains("[~]"));
    }

    #[test]
    fn test_separation_of_concerns() {
        let builder = default_builder("DONE");
        let coordinator = builder.build_coordinator("test");
        let ralph = builder.build_ralph("test");

        // Coordinator does planning, not implementation
        assert!(coordinator.contains("Gap analysis"));
        assert!(!coordinator.contains("Commit and exit"));

        // Ralph does implementation, not planning
        assert!(ralph.contains("Commit and exit"));
        assert!(!ralph.contains("Gap analysis"));

        // Only Coordinator outputs completion promise
        assert!(coordinator.contains("output: DONE"));
        assert!(!ralph.contains("output: DONE"));
    }

    #[test]
    fn test_custom_hat_for_extended_teams() {
        let builder = default_builder("DONE");
        let hat = Hat::new("reviewer", "Code Reviewer")
            .with_instructions("Review PRs for quality and correctness.");

        let instructions = builder.build_custom_hat(&hat, "PR #123 ready for review");

        // Custom role
        assert!(instructions.contains("Code Reviewer"));
        assert!(instructions.contains("Review PRs for quality"));

        // Events
        assert!(instructions.contains("PR #123 ready for review"));
        assert!(instructions.contains("<event topic="));

        // Core behaviors are injected
        assert!(instructions.contains("CORE BEHAVIORS"));
        assert!(instructions.contains(".agent/scratchpad.md"));
    }

    #[test]
    fn test_custom_guardrails_injected() {
        let custom_core = CoreConfig {
            scratchpad: ".workspace/plan.md".to_string(),
            specs_dir: "./specifications/".to_string(),
            guardrails: vec![
                "Custom rule one".to_string(),
                "Custom rule two".to_string(),
            ],
        };
        let builder = InstructionBuilder::new("DONE", custom_core);

        let coordinator = builder.build_coordinator("test");
        let ralph = builder.build_ralph("test");

        // Custom scratchpad path is used
        assert!(coordinator.contains(".workspace/plan.md"));
        assert!(ralph.contains(".workspace/plan.md"));

        // Custom specs dir is used
        assert!(coordinator.contains("./specifications/"));

        // Custom guardrails are injected
        assert!(coordinator.contains("Custom rule one"));
        assert!(coordinator.contains("Custom rule two"));
        assert!(ralph.contains("Custom rule one"));
        assert!(ralph.contains("Custom rule two"));
    }
}
