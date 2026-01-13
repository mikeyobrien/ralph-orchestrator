//! Event loop orchestration.
//!
//! The event loop coordinates the execution of hats via pub/sub messaging.

use crate::config::RalphConfig;
use crate::event_parser::EventParser;
use crate::hat_registry::HatRegistry;
use crate::instructions::InstructionBuilder;
use ralph_proto::{Event, EventBus, HatId};
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Reason the event loop terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationReason {
    /// Completion promise was detected in output.
    CompletionPromise,
    /// Maximum iterations reached.
    MaxIterations,
    /// Maximum runtime exceeded.
    MaxRuntime,
    /// Maximum cost exceeded.
    MaxCost,
    /// Too many consecutive failures.
    ConsecutiveFailures,
    /// Manually stopped.
    Stopped,
    /// Interrupted by signal (SIGINT/SIGTERM).
    Interrupted,
}

impl TerminationReason {
    /// Returns the exit code for this termination reason per spec.
    ///
    /// Per spec "Loop Termination" section:
    /// - 0: Completion promise detected (success)
    /// - 1: Consecutive failures or unrecoverable error (failure)
    /// - 2: Max iterations, max runtime, or max cost exceeded (limit)
    /// - 130: User interrupt (SIGINT = 128 + 2)
    pub fn exit_code(&self) -> i32 {
        match self {
            TerminationReason::CompletionPromise => 0,
            TerminationReason::ConsecutiveFailures | TerminationReason::Stopped => 1,
            TerminationReason::MaxIterations
            | TerminationReason::MaxRuntime
            | TerminationReason::MaxCost => 2,
            TerminationReason::Interrupted => 130,
        }
    }

    /// Returns the reason string for use in loop.terminate event payload.
    ///
    /// Per spec event payload format:
    /// `completed | max_iterations | max_runtime | consecutive_failures | interrupted | error`
    pub fn as_str(&self) -> &'static str {
        match self {
            TerminationReason::CompletionPromise => "completed",
            TerminationReason::MaxIterations => "max_iterations",
            TerminationReason::MaxRuntime => "max_runtime",
            TerminationReason::MaxCost => "max_cost",
            TerminationReason::ConsecutiveFailures => "consecutive_failures",
            TerminationReason::Stopped => "stopped",
            TerminationReason::Interrupted => "interrupted",
        }
    }
}

/// Current state of the event loop.
#[derive(Debug)]
pub struct LoopState {
    /// Current iteration number (1-indexed).
    pub iteration: u32,
    /// Number of consecutive failures.
    pub consecutive_failures: u32,
    /// Cumulative cost in USD (if tracked).
    pub cumulative_cost: f64,
    /// When the loop started.
    pub started_at: Instant,
    /// The last hat that executed.
    pub last_hat: Option<HatId>,
    /// Number of git checkpoints created.
    pub checkpoint_count: u32,
}

impl Default for LoopState {
    fn default() -> Self {
        Self {
            iteration: 0,
            consecutive_failures: 0,
            cumulative_cost: 0.0,
            started_at: Instant::now(),
            last_hat: None,
            checkpoint_count: 0,
        }
    }
}

impl LoopState {
    /// Creates a new loop state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the elapsed time since the loop started.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

/// The main event loop orchestrator.
pub struct EventLoop {
    config: RalphConfig,
    registry: HatRegistry,
    bus: EventBus,
    state: LoopState,
    instruction_builder: InstructionBuilder,
}

impl EventLoop {
    /// Creates a new event loop from configuration.
    pub fn new(config: RalphConfig) -> Self {
        let registry = HatRegistry::from_config(&config);
        let instruction_builder = InstructionBuilder::new(
            &config.event_loop.completion_promise,
            config.core.clone(),
        );

        let mut bus = EventBus::new();
        for hat in registry.all() {
            bus.register(hat.clone());
        }

        Self {
            config,
            registry,
            bus,
            state: LoopState::new(),
            instruction_builder,
        }
    }

    /// Returns the current loop state.
    pub fn state(&self) -> &LoopState {
        &self.state
    }

    /// Returns the configuration.
    pub fn config(&self) -> &RalphConfig {
        &self.config
    }

    /// Returns the hat registry.
    pub fn registry(&self) -> &HatRegistry {
        &self.registry
    }

    /// Checks if any termination condition is met.
    pub fn check_termination(&self) -> Option<TerminationReason> {
        let cfg = &self.config.event_loop;

        if self.state.iteration >= cfg.max_iterations {
            return Some(TerminationReason::MaxIterations);
        }

        if self.state.elapsed().as_secs() >= cfg.max_runtime_seconds {
            return Some(TerminationReason::MaxRuntime);
        }

        if let Some(max_cost) = cfg.max_cost_usd {
            if self.state.cumulative_cost >= max_cost {
                return Some(TerminationReason::MaxCost);
            }
        }

        if self.state.consecutive_failures >= cfg.max_consecutive_failures {
            return Some(TerminationReason::ConsecutiveFailures);
        }

        None
    }

    /// Initializes the loop by publishing the start event.
    pub fn initialize(&mut self, prompt_content: &str) {
        self.initialize_with_topic("task.start", prompt_content);
    }

    /// Initializes the loop for resume mode by publishing task.resume.
    ///
    /// Per spec: "User can run `ralph resume` to restart reading existing scratchpad."
    /// The planner should read the existing scratchpad rather than doing fresh gap analysis.
    pub fn initialize_resume(&mut self, prompt_content: &str) {
        self.initialize_with_topic("task.resume", prompt_content);
    }

    /// Common initialization logic with configurable topic.
    fn initialize_with_topic(&mut self, topic: &str, prompt_content: &str) {
        // Per spec: Log hat list, not "mode" terminology
        // ✅ "Ralph ready with hats: planner, builder"
        // ❌ "Starting in multi-hat mode"
        let hat_names: Vec<_> = self.registry.all().map(|h| h.id.as_str()).collect();
        let action = if topic == "task.resume" { "Resuming" } else { "Let's do this" };
        info!(
            hats = ?hat_names,
            max_iterations = %self.config.event_loop.max_iterations,
            "I'm Ralph. Got my hats ready: {}. {}.",
            hat_names.join(", "),
            action
        );

        let start_event = Event::new(topic, prompt_content);
        self.bus.publish(start_event);
        debug!(topic = topic, "Published {} event", topic);
    }

    /// Gets the next hat to execute (if any have pending events).
    pub fn next_hat(&self) -> Option<&HatId> {
        self.bus.next_hat_with_pending()
    }

    /// Builds the prompt for a hat's execution.
    ///
    /// Per spec: Default hats (planner/builder) use specialized rich prompts
    /// from `InstructionBuilder`. Custom hats use `build_custom_hat()` with
    /// their configured instructions.
    pub fn build_prompt(&mut self, hat_id: &HatId) -> Option<String> {
        let hat = self.registry.get(hat_id)?;

        let events = self.bus.take_pending(&hat_id.clone());
        let events_context = events
            .iter()
            .map(|e| format!("Event: {} - {}", e.topic, e.payload))
            .collect::<Vec<_>>()
            .join("\n");

        // Default planner and builder hats use specialized prompts per spec
        // Custom hats (or defaults with custom instructions) use build_custom_hat
        match hat_id.as_str() {
            "planner" if hat.instructions.is_empty() => {
                Some(self.instruction_builder.build_coordinator(&events_context))
            }
            "builder" if hat.instructions.is_empty() => {
                Some(self.instruction_builder.build_ralph(&events_context))
            }
            _ => Some(self.instruction_builder.build_custom_hat(hat, &events_context)),
        }
    }

    /// Builds the Coordinator prompt (planning mode).
    pub fn build_coordinator_prompt(&self, prompt_content: &str) -> String {
        self.instruction_builder.build_coordinator(prompt_content)
    }

    /// Builds the Ralph prompt (build mode).
    pub fn build_ralph_prompt(&self, prompt_content: &str) -> String {
        self.instruction_builder.build_ralph(prompt_content)
    }

    /// Builds prompt for single-hat mode.
    ///
    /// In single mode, Ralph acts as a unified agent handling both planning
    /// and implementation. Uses the Ralph (builder) prompt since single
    /// mode is typically used for direct implementation workflows.
    pub fn build_single_prompt(&self, prompt_content: &str) -> String {
        self.instruction_builder.build_ralph(prompt_content)
    }

    /// Processes output from a hat execution.
    ///
    /// Returns the termination reason if the loop should stop.
    pub fn process_output(
        &mut self,
        hat_id: &HatId,
        output: &str,
        success: bool,
    ) -> Option<TerminationReason> {
        self.state.iteration += 1;
        self.state.last_hat = Some(hat_id.clone());

        // Track failures
        if success {
            self.state.consecutive_failures = 0;
        } else {
            self.state.consecutive_failures += 1;
        }

        // Check for completion promise - only valid from planner hat
        // Per spec: "Builder hat outputs LOOP_COMPLETE → completion promise is ignored (only Planner can terminate)"
        if hat_id.as_str() == "planner"
            && EventParser::contains_promise(output, &self.config.event_loop.completion_promise)
        {
            return Some(TerminationReason::CompletionPromise);
        }

        // Parse and publish events from output
        let parser = EventParser::new().with_source(hat_id.clone());
        let events = parser.parse(output);

        for event in events {
            debug!(
                topic = %event.topic,
                source = ?event.source,
                target = ?event.target,
                "Publishing event from output"
            );
            self.bus.publish(event);
        }

        // If single-hat mode and no completion, publish continue event
        if self.config.is_single_mode() && self.bus.next_hat_with_pending().is_none() {
            let continue_event = Event::new("task.continue", "Continue with the task");
            self.bus.publish(continue_event);
        }

        // Check termination conditions
        self.check_termination()
    }

    /// Returns true if a checkpoint should be created at this iteration.
    pub fn should_checkpoint(&self) -> bool {
        let interval = self.config.event_loop.checkpoint_interval;
        interval > 0 && self.state.iteration % interval == 0
    }

    /// Adds cost to the cumulative total.
    pub fn add_cost(&mut self, cost: f64) {
        self.state.cumulative_cost += cost;
    }

    /// Records that a checkpoint was created.
    pub fn record_checkpoint(&mut self) {
        self.state.checkpoint_count += 1;
        debug!(
            checkpoint_count = self.state.checkpoint_count,
            iteration = self.state.iteration,
            "Checkpoint recorded"
        );
    }

    /// Publishes the loop.terminate system event to observers.
    ///
    /// Per spec: "Published by the orchestrator (not agents) when the loop exits."
    /// This is an observer-only event—hats cannot trigger on it.
    ///
    /// Returns the event for logging purposes.
    pub fn publish_terminate_event(&mut self, reason: &TerminationReason) -> Event {
        let elapsed = self.state.elapsed();
        let duration_str = format_duration(elapsed);

        let payload = format!(
            "## Reason\n{}\n\n## Status\n{}\n\n## Summary\n- Iterations: {}\n- Duration: {}\n- Exit code: {}",
            reason.as_str(),
            termination_status_text(reason),
            self.state.iteration,
            duration_str,
            reason.exit_code()
        );

        let event = Event::new("loop.terminate", &payload);

        // Publish to bus for observers (but no hat can trigger on this)
        self.bus.publish(event.clone());

        info!(
            reason = %reason.as_str(),
            iterations = self.state.iteration,
            duration = %duration_str,
            "Wrapping up: {}. {} iterations in {}.",
            reason.as_str(),
            self.state.iteration,
            duration_str
        );

        event
    }
}

/// Formats a duration as human-readable string.
fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Returns a human-readable status based on termination reason.
fn termination_status_text(reason: &TerminationReason) -> &'static str {
    match reason {
        TerminationReason::CompletionPromise => "All tasks completed successfully.",
        TerminationReason::MaxIterations => "Stopped at iteration limit.",
        TerminationReason::MaxRuntime => "Stopped at runtime limit.",
        TerminationReason::MaxCost => "Stopped at cost limit.",
        TerminationReason::ConsecutiveFailures => "Too many consecutive failures.",
        TerminationReason::Stopped => "Manually stopped.",
        TerminationReason::Interrupted => "Interrupted by signal.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_triggers_planner() {
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);

        event_loop.initialize("Test prompt");

        // Per spec: task.start triggers planner hat
        let next = event_loop.next_hat();
        assert!(next.is_some());
        assert_eq!(next.unwrap().as_str(), "planner");
    }

    #[test]
    fn test_termination_max_iterations() {
        let yaml = r#"
event_loop:
  max_iterations: 2
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);
        event_loop.state.iteration = 2;

        assert_eq!(
            event_loop.check_termination(),
            Some(TerminationReason::MaxIterations)
        );
    }

    #[test]
    fn test_completion_promise_detection() {
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test");

        // Use planner hat since it's the one that outputs completion promise per spec
        let hat_id = HatId::new("planner");
        let reason = event_loop.process_output(&hat_id, "Done! LOOP_COMPLETE", true);

        assert_eq!(reason, Some(TerminationReason::CompletionPromise));
    }

    #[test]
    fn test_builder_cannot_terminate_loop() {
        // Per spec: "Builder hat outputs LOOP_COMPLETE → completion promise is ignored (only Planner can terminate)"
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test");

        // Builder hat outputs completion promise - should be IGNORED
        let hat_id = HatId::new("builder");
        let reason = event_loop.process_output(&hat_id, "Done! LOOP_COMPLETE", true);

        // Builder cannot terminate, so no termination reason
        assert_eq!(reason, None);
    }

    #[test]
    fn test_checkpoint_interval() {
        let yaml = r#"
event_loop:
  checkpoint_interval: 5
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        event_loop.state.iteration = 4;
        assert!(!event_loop.should_checkpoint());

        event_loop.state.iteration = 5;
        assert!(event_loop.should_checkpoint());

        event_loop.state.iteration = 10;
        assert!(event_loop.should_checkpoint());
    }

    #[test]
    fn test_build_prompt_uses_specialized_prompts_for_default_hats() {
        // Per spec: Default planner and builder hats use specialized rich prompts
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test task");

        // Planner hat should get specialized planner prompt
        let planner_id = HatId::new("planner");
        let planner_prompt = event_loop.build_prompt(&planner_id).unwrap();

        // Verify it's the Coordinator/Planner prompt (has PLANNER MODE header)
        assert!(
            planner_prompt.contains("PLANNER MODE"),
            "Planner should use specialized planner prompt"
        );
        assert!(
            planner_prompt.contains("planning, not building"),
            "Planner prompt should have planning instructions"
        );

        // Now trigger builder hat by publishing build.task event
        let hat_id = HatId::new("builder");
        // We need to trigger the builder to have pending events
        event_loop.bus.publish(Event::new("build.task", "Build something"));

        let builder_prompt = event_loop.build_prompt(&hat_id).unwrap();

        // Verify it's the Builder/Ralph prompt (has BUILDER MODE header)
        assert!(
            builder_prompt.contains("BUILDER MODE"),
            "Builder should use specialized builder prompt"
        );
        assert!(
            builder_prompt.contains("building, not planning"),
            "Builder prompt should have building instructions"
        );
    }

    #[test]
    fn test_build_prompt_uses_custom_hat_for_non_defaults() {
        // Per spec: Custom hats use build_custom_hat with their instructions
        let yaml = r#"
mode: "multi"
hats:
  reviewer:
    name: "Code Reviewer"
    triggers: ["review.request"]
    instructions: "Review code quality."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        // Publish event to trigger reviewer
        event_loop.bus.publish(Event::new("review.request", "Review PR #123"));

        let reviewer_id = HatId::new("reviewer");
        let prompt = event_loop.build_prompt(&reviewer_id).unwrap();

        // Should be custom hat prompt (contains custom instructions)
        assert!(
            prompt.contains("Code Reviewer"),
            "Custom hat should use its name"
        );
        assert!(
            prompt.contains("Review code quality"),
            "Custom hat should include its instructions"
        );
        // Should NOT be planner or builder prompt
        assert!(
            !prompt.contains("PLANNER MODE"),
            "Custom hat should not use planner prompt"
        );
        assert!(
            !prompt.contains("BUILDER MODE"),
            "Custom hat should not use builder prompt"
        );
    }

    #[test]
    fn test_exit_codes_per_spec() {
        // Per spec "Loop Termination" section:
        // - 0: Completion promise detected (success)
        // - 1: Consecutive failures or unrecoverable error (failure)
        // - 2: Max iterations, max runtime, or max cost exceeded (limit)
        // - 130: User interrupt (SIGINT = 128 + 2)
        assert_eq!(TerminationReason::CompletionPromise.exit_code(), 0);
        assert_eq!(TerminationReason::ConsecutiveFailures.exit_code(), 1);
        assert_eq!(TerminationReason::Stopped.exit_code(), 1);
        assert_eq!(TerminationReason::MaxIterations.exit_code(), 2);
        assert_eq!(TerminationReason::MaxRuntime.exit_code(), 2);
        assert_eq!(TerminationReason::MaxCost.exit_code(), 2);
        assert_eq!(TerminationReason::Interrupted.exit_code(), 130);
    }
}
