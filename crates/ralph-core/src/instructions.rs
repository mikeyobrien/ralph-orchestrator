//! Instruction builder for Ralph agent prompts.
//!
//! Philosophy: Let Ralph Ralph. Two specialized agents, each with one job:
//! - Coordinator: Plans work, owns scratchpad, validates completion
//! - Ralph Ralph: Implements tasks, runs backpressure, commits
//!
//! This maps directly to ghuntley's PROMPT_plan.md / PROMPT_build.md split.

use ralph_proto::Hat;

/// Builds the prepended instructions for agent prompts.
///
/// Two-agent architecture: Coordinator (planner) and Ralph Ralph (builder).
/// The orchestrator invokes the appropriate agent based on loop state.
#[derive(Debug)]
pub struct InstructionBuilder {
    completion_promise: String,
}

impl InstructionBuilder {
    /// Creates a new instruction builder.
    pub fn new(completion_promise: impl Into<String>) -> Self {
        Self {
            completion_promise: completion_promise.into(),
        }
    }

    /// Builds Coordinator instructions (the Planner).
    ///
    /// Coordinator owns the scratchpad and decides what work needs doing.
    /// It does NOT implement—that's Ralph Ralph's job.
    pub fn build_coordinator(&self, prompt_content: &str) -> String {
        format!(
            r#"You are Coordinator Ralph. You plan work and validate completion. You do NOT implement.

## YOUR JOB

1. **Gap analysis.** Study `./specs/` and compare against the codebase. What's missing? What's broken?

2. **Own the scratchpad.** Create or update `.agent/scratchpad.md` with prioritized tasks. Mark `[x]` done, `[~]` cancelled.

3. **Validate completion claims.** When Ralph Ralph reports done, verify the work actually satisfies the spec.

4. **Don't assume "not implemented."** Search before concluding something is missing.

## WHAT YOU DON'T DO

- ❌ Write implementation code
- ❌ Run tests (Ralph Ralph does that)
- ❌ Make commits (Ralph Ralph does that)

## COMPLETION

When ALL tasks are `[x]` or `[~]` and ALL specs are satisfied, output: {promise}

---
{prompt}"#,
            promise = self.completion_promise,
            prompt = prompt_content
        )
    }

    /// Builds Ralph Ralph instructions (the Builder).
    ///
    /// Ralph Ralph implements tasks. It does NOT plan or manage the scratchpad.
    pub fn build_ralph_ralph(&self, prompt_content: &str) -> String {
        format!(
            r#"You are Ralph Ralph. You implement. One task, then done.

## YOUR JOB

1. **Pick ONE task.** Read `.agent/scratchpad.md`, pick the highest priority `[ ]` task.

2. **Implement it.** Write the code. Follow existing patterns.

3. **Backpressure is law.** Tests, typecheck, lint must pass before you're done.

4. **Commit and exit.** One task, one commit, then you're done. The loop continues.

5. **Don't assume "not implemented."** Search before concluding something is missing.

## WHAT YOU DON'T DO

- ❌ Create the scratchpad (Coordinator does that)
- ❌ Decide what tasks to add (Coordinator does that)
- ❌ Output the completion promise (Coordinator does that)

## WHEN DONE

Mark your task `[x]` in the scratchpad. Exit. Coordinator will verify.

## STUCK?

Can't finish? Publish `<event topic="build.blocked">` with:
- What you tried
- Why it failed
- What would unblock you

---
{prompt}"#,
            prompt = prompt_content
        )
    }

    /// Builds custom hat instructions for extended multi-agent configurations.
    ///
    /// Use this for teams beyond the default Coordinator + Ralph Ralph.
    pub fn build_custom_hat(&self, hat: &Hat, events_context: &str) -> String {
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
            r#"You are {name}. Fresh context each iteration—`.agent/scratchpad.md` is shared memory.

## YOUR ROLE

{role_instructions}

## THE RULES

1. **One task, then exit.** The loop continues.
2. **Backpressure is law.** Tests, typecheck, lint must pass.
3. **Don't assume "not implemented."** Search first.

## EVENTS

Communicate via: `<event topic="...">message</event>`
{publish_topics}

## COMPLETION

Only Coordinator outputs: {promise}

---
INCOMING:
{events}"#,
            name = hat.name,
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

    #[test]
    fn test_coordinator_plans_not_implements() {
        let builder = InstructionBuilder::new("LOOP_COMPLETE");
        let instructions = builder.build_coordinator("Build a CLI tool");

        // Identity
        assert!(instructions.contains("Coordinator Ralph"));
        assert!(instructions.contains("Build a CLI tool"));

        // Coordinator's job
        assert!(instructions.contains("Gap analysis"));
        assert!(instructions.contains("Own the scratchpad"));
        assert!(instructions.contains("Validate completion"));
        assert!(instructions.contains("./specs/"));

        // Completion promise (Coordinator outputs it)
        assert!(instructions.contains("LOOP_COMPLETE"));

        // What Coordinator doesn't do
        assert!(instructions.contains("❌ Write implementation code"));
        assert!(instructions.contains("❌ Run tests"));
        assert!(instructions.contains("❌ Make commits"));
    }

    #[test]
    fn test_ralph_ralph_implements_not_plans() {
        let builder = InstructionBuilder::new("LOOP_COMPLETE");
        let instructions = builder.build_ralph_ralph("Build a CLI tool");

        // Identity
        assert!(instructions.contains("Ralph Ralph"));
        assert!(instructions.contains("Build a CLI tool"));

        // Ralph Ralph's job
        assert!(instructions.contains("Pick ONE task"));
        assert!(instructions.contains("Implement it"));
        assert!(instructions.contains("Backpressure is law"));
        assert!(instructions.contains("Commit and exit"));

        // What Ralph Ralph doesn't do
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
    fn test_coordinator_and_ralph_ralph_share_guardrails() {
        let builder = InstructionBuilder::new("DONE");
        let coordinator = builder.build_coordinator("test");
        let ralph_ralph = builder.build_ralph_ralph("test");

        // Both reference the scratchpad
        assert!(coordinator.contains(".agent/scratchpad.md"));
        assert!(ralph_ralph.contains(".agent/scratchpad.md"));

        // Both have "don't assume" guardrail
        assert!(coordinator.contains("Don't assume \"not implemented.\""));
        assert!(ralph_ralph.contains("Don't assume \"not implemented.\""));

        // Both use task markers
        assert!(coordinator.contains("[x]"));
        assert!(ralph_ralph.contains("[x]"));
        assert!(coordinator.contains("[~]"));
    }

    #[test]
    fn test_separation_of_concerns() {
        let builder = InstructionBuilder::new("DONE");
        let coordinator = builder.build_coordinator("test");
        let ralph_ralph = builder.build_ralph_ralph("test");

        // Coordinator does planning, not implementation
        assert!(coordinator.contains("Gap analysis"));
        assert!(!coordinator.contains("Commit and exit"));

        // Ralph Ralph does implementation, not planning
        assert!(ralph_ralph.contains("Commit and exit"));
        assert!(!ralph_ralph.contains("Gap analysis"));

        // Only Coordinator outputs completion promise
        assert!(coordinator.contains("output: DONE"));
        assert!(!ralph_ralph.contains("output: DONE"));
    }

    #[test]
    fn test_custom_hat_for_extended_teams() {
        let builder = InstructionBuilder::new("DONE");
        let hat = Hat::new("reviewer", "Code Reviewer")
            .with_instructions("Review PRs for quality and correctness.");

        let instructions = builder.build_custom_hat(&hat, "PR #123 ready for review");

        // Custom role
        assert!(instructions.contains("Code Reviewer"));
        assert!(instructions.contains("Review PRs for quality"));

        // Events
        assert!(instructions.contains("PR #123 ready for review"));
        assert!(instructions.contains("<event topic="));

        // Core rules
        assert!(instructions.contains("Backpressure is law"));
        assert!(instructions.contains("Don't assume \"not implemented.\""));
    }
}
