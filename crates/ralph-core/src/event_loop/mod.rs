//! Event loop orchestration.
//!
//! The event loop coordinates the execution of hats via pub/sub messaging.

mod loop_state;
#[cfg(test)]
mod tests;

pub use loop_state::LoopState;

use crate::config::{HatBackend, InjectMode, RalphConfig};
use crate::event_parser::EventParser;
use crate::event_reader::EventReader;
use crate::hat_registry::HatRegistry;
use crate::hatless_ralph::HatlessRalph;
use crate::instructions::InstructionBuilder;
use crate::memory_store::{MarkdownMemoryStore, format_memories_as_markdown, truncate_to_budget};
use ralph_proto::{Event, EventBus, Hat, HatId};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Skill content injected when memories are enabled.
///
/// This teaches the agent how to read and create memories.
/// Skill injection is implicit when `memories.enabled: true`.
/// Embedded from `.claude/skills/ralph-memories/SKILL.md` at compile time.
const MEMORIES_SKILL: &str = include_str!("../../../../.claude/skills/ralph-memories/SKILL.md");

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
    /// Loop thrashing detected (repeated blocked events).
    LoopThrashing,
    /// Too many consecutive malformed JSONL lines in events file.
    ValidationFailure,
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
            TerminationReason::ConsecutiveFailures
            | TerminationReason::LoopThrashing
            | TerminationReason::ValidationFailure
            | TerminationReason::Stopped => 1,
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
            TerminationReason::LoopThrashing => "loop_thrashing",
            TerminationReason::ValidationFailure => "validation_failure",
            TerminationReason::Stopped => "stopped",
            TerminationReason::Interrupted => "interrupted",
        }
    }
}

/// The main event loop orchestrator.
pub struct EventLoop {
    config: RalphConfig,
    registry: HatRegistry,
    bus: EventBus,
    state: LoopState,
    instruction_builder: InstructionBuilder,
    ralph: HatlessRalph,
    event_reader: EventReader,
    diagnostics: crate::diagnostics::DiagnosticsCollector,
}

impl EventLoop {
    /// Creates a new event loop from configuration.
    pub fn new(config: RalphConfig) -> Self {
        // Try to create diagnostics collector, but fall back to disabled if it fails
        // (e.g., in tests without proper directory setup)
        let diagnostics = crate::diagnostics::DiagnosticsCollector::new(std::path::Path::new("."))
            .unwrap_or_else(|e| {
                debug!(
                    "Failed to initialize diagnostics: {}, using disabled collector",
                    e
                );
                crate::diagnostics::DiagnosticsCollector::disabled()
            });

        Self::with_diagnostics(config, diagnostics)
    }

    /// Creates a new event loop with explicit diagnostics collector (for testing).
    pub fn with_diagnostics(
        config: RalphConfig,
        diagnostics: crate::diagnostics::DiagnosticsCollector,
    ) -> Self {
        let registry = HatRegistry::from_config(&config);
        let instruction_builder = InstructionBuilder::with_events(
            &config.event_loop.completion_promise,
            config.core.clone(),
            config.events.clone(),
        );

        let mut bus = EventBus::new();

        // Per spec: "Hatless Ralph is constant — Cannot be replaced, overwritten, or configured away"
        // Ralph is ALWAYS registered as the universal fallback for orphaned events.
        // Custom hats are registered first (higher priority), Ralph catches everything else.
        for hat in registry.all() {
            bus.register(hat.clone());
        }

        // Always register Ralph as catch-all coordinator
        // Per spec: "Ralph runs when no hat triggered — Universal fallback for orphaned events"
        let ralph_hat = ralph_proto::Hat::new("ralph", "Ralph").subscribe("*"); // Subscribe to all events
        bus.register(ralph_hat);

        if registry.is_empty() {
            debug!("Solo mode: Ralph is the only coordinator");
        } else {
            debug!(
                "Multi-hat mode: {} custom hats + Ralph as fallback",
                registry.len()
            );
        }

        // When memories are enabled, scratchpad instructions are excluded (mutually exclusive)
        let ralph = HatlessRalph::new(
            config.event_loop.completion_promise.clone(),
            config.core.clone(),
            &registry,
            config.event_loop.starting_event.clone(),
        )
        .with_scratchpad(!config.memories.enabled);

        // Read events path from marker file, fall back to default if not present
        // The marker file is written by run_loop_impl() at run startup
        let events_path = std::fs::read_to_string(".ralph/current-events")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| ".ralph/events.jsonl".to_string());
        let event_reader = EventReader::new(&events_path);

        Self {
            config,
            registry,
            bus,
            state: LoopState::new(),
            instruction_builder,
            ralph,
            event_reader,
            diagnostics,
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

    /// Gets the backend configuration for a hat.
    ///
    /// If the hat has a backend configured, returns that.
    /// Otherwise, returns None (caller should use global backend).
    pub fn get_hat_backend(&self, hat_id: &HatId) -> Option<&HatBackend> {
        self.registry
            .get_config(hat_id)
            .and_then(|config| config.backend.as_ref())
    }

    /// Adds an observer that receives all published events.
    ///
    /// Multiple observers can be added (e.g., session recorder + TUI).
    /// Each observer is called before events are routed to subscribers.
    pub fn add_observer<F>(&mut self, observer: F)
    where
        F: Fn(&Event) + Send + 'static,
    {
        self.bus.add_observer(observer);
    }

    /// Sets a single observer, clearing any existing observers.
    ///
    /// Prefer `add_observer` when multiple observers are needed.
    #[deprecated(since = "2.0.0", note = "Use add_observer instead")]
    pub fn set_observer<F>(&mut self, observer: F)
    where
        F: Fn(&Event) + Send + 'static,
    {
        #[allow(deprecated)]
        self.bus.set_observer(observer);
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

        if let Some(max_cost) = cfg.max_cost_usd
            && self.state.cumulative_cost >= max_cost
        {
            return Some(TerminationReason::MaxCost);
        }

        if self.state.consecutive_failures >= cfg.max_consecutive_failures {
            return Some(TerminationReason::ConsecutiveFailures);
        }

        // Check for loop thrashing: planner keeps dispatching abandoned tasks
        if self.state.abandoned_task_redispatches >= 3 {
            return Some(TerminationReason::LoopThrashing);
        }

        // Check for validation failures: too many consecutive malformed JSONL lines
        if self.state.consecutive_malformed_events >= 3 {
            return Some(TerminationReason::ValidationFailure);
        }

        None
    }

    /// Initializes the loop by publishing the start event.
    pub fn initialize(&mut self, prompt_content: &str) {
        // Use configured starting_event or default to task.start for backward compatibility
        let topic = self.config.event_loop.starting_event
            .clone()
            .unwrap_or_else(|| "task.start".to_string());
        self.initialize_with_topic(&topic, prompt_content);
    }

    /// Initializes the loop for resume mode by publishing task.resume.
    ///
    /// Per spec: "User can run `ralph resume` to restart reading existing scratchpad."
    /// The planner should read the existing scratchpad rather than doing fresh gap analysis.
    pub fn initialize_resume(&mut self, prompt_content: &str) {
        // Resume always uses task.resume regardless of starting_event config
        self.initialize_with_topic("task.resume", prompt_content);
    }

    /// Common initialization logic with configurable topic.
    fn initialize_with_topic(&mut self, topic: &str, prompt_content: &str) {
        // Per spec: Log hat list, not "mode" terminology
        // ✅ "Ralph ready with hats: planner, builder"
        // ❌ "Starting in multi-hat mode"
        let start_event = Event::new(topic, prompt_content);
        self.bus.publish(start_event);
        debug!(topic = topic, "Published {} event", topic);
    }

    /// Gets the next hat to execute (if any have pending events).
    ///
    /// Per "Hatless Ralph" architecture: When custom hats are defined, Ralph is
    /// always the executor. Custom hats define topology (pub/sub contracts) that
    /// Ralph uses for coordination context, but Ralph handles all iterations.
    ///
    /// - Solo mode (no custom hats): Returns "ralph" if Ralph has pending events
    /// - Multi-hat mode (custom hats defined): Always returns "ralph" if ANY hat has pending events
    pub fn next_hat(&self) -> Option<&HatId> {
        let next = self.bus.next_hat_with_pending();

        // If no pending events, return None
        next.as_ref()?;

        // In multi-hat mode, always route to Ralph (custom hats define topology only)
        // Ralph's prompt includes the ## HATS section for coordination awareness
        if self.registry.is_empty() {
            // Solo mode - return the next hat (which is "ralph")
            next
        } else {
            // Return "ralph" - the constant coordinator
            // Find ralph in the bus's registered hats
            self.bus.hat_ids().find(|id| id.as_str() == "ralph")
        }
    }

    /// Checks if any hats have pending events.
    ///
    /// Use this after `process_output` to detect if the LLM failed to publish an event.
    /// If false after processing, the loop will terminate on the next iteration.
    pub fn has_pending_events(&self) -> bool {
        self.bus.next_hat_with_pending().is_some()
    }

    /// Gets the topics a hat is allowed to publish.
    ///
    /// Used to build retry prompts when the LLM forgets to publish an event.
    pub fn get_hat_publishes(&self, hat_id: &HatId) -> Vec<String> {
        self.registry
            .get(hat_id)
            .map(|hat| hat.publishes.iter().map(|t| t.to_string()).collect())
            .unwrap_or_default()
    }

    /// Injects a fallback event to recover from a stalled loop.
    ///
    /// When no hats have pending events (agent failed to publish), this method
    /// injects a `task.resume` event which Ralph will handle to attempt recovery.
    ///
    /// Returns true if a fallback event was injected, false if recovery is not possible.
    pub fn inject_fallback_event(&mut self) -> bool {
        let fallback_event = Event::new(
            "task.resume",
            "RECOVERY: Previous iteration did not publish an event. \
             Review the scratchpad and either dispatch the next task or complete the loop.",
        );

        // If a custom hat was last executing, target the fallback back to it
        // This preserves hat context instead of always falling back to Ralph
        let fallback_event = match &self.state.last_hat {
            Some(hat_id) if hat_id.as_str() != "ralph" => {
                debug!(
                    hat = %hat_id.as_str(),
                    "Injecting fallback event to recover - targeting last hat with task.resume"
                );
                fallback_event.with_target(hat_id.clone())
            }
            _ => {
                debug!("Injecting fallback event to recover - triggering Ralph with task.resume");
                fallback_event
            }
        };

        self.bus.publish(fallback_event);
        true
    }

    /// Builds the prompt for a hat's execution.
    ///
    /// Per "Hatless Ralph" architecture:
    /// - Solo mode: Ralph handles everything with his own prompt
    /// - Multi-hat mode: Ralph is the sole executor, custom hats define topology only
    ///
    /// When in multi-hat mode, this method collects ALL pending events across all hats
    /// and builds Ralph's prompt with that context. The `## HATS` section in Ralph's
    /// prompt documents the topology for coordination awareness.
    ///
    /// If memories are configured with `inject: auto`, this method also prepends
    /// primed memories to the prompt context.
    pub fn build_prompt(&mut self, hat_id: &HatId) -> Option<String> {
        // Handle "ralph" hat - the constant coordinator
        // Per spec: "Hatless Ralph is constant — Cannot be replaced, overwritten, or configured away"
        if hat_id.as_str() == "ralph" {
            if self.registry.is_empty() {
                // Solo mode - just Ralph's events, no hats to filter
                let events = self.bus.take_pending(&hat_id.clone());
                let events_context = events
                    .iter()
                    .map(|e| Self::format_event(e))
                    .collect::<Vec<_>>()
                    .join("\n");

                // Build base prompt and prepend memories if enabled
                let base_prompt = self.ralph.build_prompt(&events_context, &[]);
                let final_prompt = self.prepend_memories(base_prompt);

                debug!("build_prompt: routing to HatlessRalph (solo mode)");
                return Some(final_prompt);
            } else {
                // Multi-hat mode: collect events and determine active hats
                let mut all_hat_ids: Vec<HatId> = self.bus.hat_ids().cloned().collect();
                // Deterministic ordering (avoid HashMap iteration order nondeterminism).
                all_hat_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));

                let mut all_events = Vec::new();
                let mut system_events = Vec::new();

                for id in &all_hat_ids {
                    let pending = self.bus.take_pending(id);
                    if pending.is_empty() {
                        continue;
                    }

                    let (drop_pending, exhausted_event) = self.check_hat_exhaustion(id, &pending);
                    if drop_pending {
                        // Drop the pending events that would have activated the hat.
                        if let Some(exhausted_event) = exhausted_event {
                            all_events.push(exhausted_event.clone());
                            system_events.push(exhausted_event);
                        }
                        continue;
                    }

                    all_events.extend(pending);
                }

                // Publish orchestrator-generated system events after consuming pending events,
                // so they become visible in the event log and can be handled next iteration.
                for event in system_events {
                    self.bus.publish(event);
                }

                // Determine which hats are active based on events
                let active_hat_ids = self.determine_active_hat_ids(&all_events);
                self.record_hat_activations(&active_hat_ids);
                let active_hats = self.determine_active_hats(&all_events);

                // Format events for context
                let events_context = all_events
                    .iter()
                    .map(|e| Self::format_event(e))
                    .collect::<Vec<_>>()
                    .join("\n");

                // Build base prompt and prepend memories if enabled
                let base_prompt = self.ralph.build_prompt(&events_context, &active_hats);
                let final_prompt = self.prepend_memories(base_prompt);

                // Build prompt with active hats - filters instructions to only active hats
                debug!(
                    "build_prompt: routing to HatlessRalph (multi-hat coordinator mode), active_hats: {:?}",
                    active_hats
                        .iter()
                        .map(|h| h.id.as_str())
                        .collect::<Vec<_>>()
                );
                return Some(final_prompt);
            }
        }

        // Non-ralph hat requested - this shouldn't happen in multi-hat mode since
        // next_hat() always returns "ralph" when custom hats are defined.
        // But we keep this code path for backward compatibility and tests.
        let events = self.bus.take_pending(&hat_id.clone());
        let events_context = events
            .iter()
            .map(|e| Self::format_event(e))
            .collect::<Vec<_>>()
            .join("\n");

        let hat = self.registry.get(hat_id)?;

        // Debug logging to trace hat routing
        debug!(
            "build_prompt: hat_id='{}', instructions.is_empty()={}",
            hat_id.as_str(),
            hat.instructions.is_empty()
        );

        // All hats use build_custom_hat with ghuntley-style prompts
        debug!(
            "build_prompt: routing to build_custom_hat() for '{}'",
            hat_id.as_str()
        );
        Some(
            self.instruction_builder
                .build_custom_hat(hat, &events_context),
        )
    }

    /// Prepends memories and usage skill to the prompt if auto-injection is enabled.
    ///
    /// Per spec: When `memories.inject: auto` is configured, memories are loaded
    /// from `.agent/memories.md` and prepended to every prompt.
    fn prepend_memories(&self, prompt: String) -> String {
        let memories_config = &self.config.memories;

        info!(
            "Memory injection check: enabled={}, inject={:?}, workspace_root={:?}",
            memories_config.enabled, memories_config.inject, self.config.core.workspace_root
        );

        // Only inject if enabled and set to auto mode
        if !memories_config.enabled || memories_config.inject != InjectMode::Auto {
            info!(
                "Memory injection skipped: enabled={}, inject={:?}",
                memories_config.enabled, memories_config.inject
            );
            return prompt;
        }

        // Load memories from the store using workspace root for path resolution
        let workspace_root = &self.config.core.workspace_root;
        let store = MarkdownMemoryStore::with_default_path(workspace_root);
        let memories_path = workspace_root.join(".agent/memories.md");

        info!(
            "Looking for memories at: {:?} (exists: {})",
            memories_path,
            memories_path.exists()
        );

        let memories = match store.load() {
            Ok(memories) => {
                info!("Successfully loaded {} memories from store", memories.len());
                memories
            }
            Err(e) => {
                info!(
                    "Failed to load memories for injection: {} (path: {:?})",
                    e, memories_path
                );
                return prompt;
            }
        };

        if memories.is_empty() {
            info!("Memory store is empty - no memories to inject");
            return prompt;
        }

        // Format memories as markdown
        let mut memories_content = format_memories_as_markdown(&memories);

        // Apply budget if configured
        if memories_config.budget > 0 {
            let original_len = memories_content.len();
            memories_content = truncate_to_budget(&memories_content, memories_config.budget);
            debug!(
                "Applied budget: {} chars -> {} chars (budget: {})",
                original_len,
                memories_content.len(),
                memories_config.budget
            );
        }

        info!(
            "Injecting {} memories ({} chars) into prompt",
            memories.len(),
            memories_content.len()
        );

        // Build final prompt with memories prefix
        let mut final_prompt = memories_content;

        // Always add usage skill when memories are enabled (implicit skill injection)
        final_prompt.push_str(MEMORIES_SKILL);
        debug!("Added memory usage skill to prompt");

        final_prompt.push_str("\n\n");
        final_prompt.push_str(&prompt);

        final_prompt
    }

    /// Builds the Ralph prompt (coordination mode).
    pub fn build_ralph_prompt(&self, prompt_content: &str) -> String {
        self.ralph.build_prompt(prompt_content, &[])
    }

    /// Determines which hats should be active based on pending events.
    /// Returns list of Hat references that are triggered by any pending event.
    fn determine_active_hats(&self, events: &[Event]) -> Vec<&Hat> {
        let mut active_hats = Vec::new();
        for id in self.determine_active_hat_ids(events) {
            if let Some(hat) = self.registry.get(&id) {
                active_hats.push(hat);
            }
        }
        active_hats
    }

    fn determine_active_hat_ids(&self, events: &[Event]) -> Vec<HatId> {
        let mut active_hat_ids = Vec::new();
        for event in events {
            if let Some(hat) = self.registry.get_for_topic(event.topic.as_str()) {
                // Avoid duplicates
                if !active_hat_ids.iter().any(|id| id == &hat.id) {
                    active_hat_ids.push(hat.id.clone());
                }
            }
        }
        active_hat_ids
    }

    /// Formats an event for prompt context.
    ///
    /// For top-level prompts (task.start, task.resume), wraps the payload in
    /// `<top-level-prompt>` XML tags to clearly delineate the user's original request.
    fn format_event(event: &Event) -> String {
        let topic = &event.topic;
        let payload = &event.payload;

        if topic.as_str() == "task.start" || topic.as_str() == "task.resume" {
            format!(
                "Event: {} - <top-level-prompt>\n{}\n</top-level-prompt>",
                topic, payload
            )
        } else {
            format!("Event: {} - {}", topic, payload)
        }
    }

    fn check_hat_exhaustion(&mut self, hat_id: &HatId, dropped: &[Event]) -> (bool, Option<Event>) {
        let Some(config) = self.registry.get_config(hat_id) else {
            return (false, None);
        };
        let Some(max) = config.max_activations else {
            return (false, None);
        };

        let count = *self.state.hat_activation_counts.get(hat_id).unwrap_or(&0);
        if count < max {
            return (false, None);
        }

        // Emit only once per hat per run (avoid flooding).
        let should_emit = self.state.exhausted_hats.insert(hat_id.clone());

        if !should_emit {
            // Hat is already exhausted - drop pending events silently.
            return (true, None);
        }

        let mut dropped_topics: Vec<String> = dropped.iter().map(|e| e.topic.to_string()).collect();
        dropped_topics.sort();

        let payload = format!(
            "Hat '{hat}' exhausted.\n- max_activations: {max}\n- activations: {count}\n- dropped_topics:\n  - {topics}",
            hat = hat_id.as_str(),
            max = max,
            count = count,
            topics = dropped_topics.join("\n  - ")
        );

        warn!(
            hat = %hat_id.as_str(),
            max_activations = max,
            activations = count,
            "Hat exhausted (max_activations reached)"
        );

        (
            true,
            Some(Event::new(
                format!("{}.exhausted", hat_id.as_str()),
                payload,
            )),
        )
    }

    fn record_hat_activations(&mut self, active_hat_ids: &[HatId]) {
        for hat_id in active_hat_ids {
            *self
                .state
                .hat_activation_counts
                .entry(hat_id.clone())
                .or_insert(0) += 1;
        }
    }

    /// Returns the primary active hat ID for display purposes.
    /// Returns the first active hat, or "ralph" if no specific hat is active.
    pub fn get_active_hat_id(&self) -> HatId {
        // Peek at pending events (don't consume them)
        for hat_id in self.bus.hat_ids() {
            let Some(events) = self.bus.peek_pending(hat_id) else {
                continue;
            };
            let Some(event) = events.first() else {
                continue;
            };
            if let Some(active_hat) = self.registry.get_for_topic(event.topic.as_str()) {
                return active_hat.id.clone();
            }
        }
        HatId::new("ralph")
    }

    /// Records the current event count before hat execution.
    ///
    /// Call this before executing a hat, then use `check_default_publishes`
    /// after execution to inject a fallback event if needed.
    pub fn record_event_count(&mut self) -> usize {
        self.event_reader
            .read_new_events()
            .map(|r| r.events.len())
            .unwrap_or(0)
    }

    /// Checks if hat wrote any events, and injects default if configured.
    ///
    /// Call this after hat execution with the count from `record_event_count`.
    /// If no new events were written AND the hat has `default_publishes` configured,
    /// this will inject the default event automatically.
    pub fn check_default_publishes(&mut self, hat_id: &HatId, _events_before: usize) {
        let events_after = self
            .event_reader
            .read_new_events()
            .map(|r| r.events.len())
            .unwrap_or(0);

        if events_after == 0
            && let Some(config) = self.registry.get_config(hat_id)
            && let Some(default_topic) = &config.default_publishes
        {
            // No new events written - inject default event
            let default_event = Event::new(default_topic.as_str(), "").with_source(hat_id.clone());

            debug!(
                hat = %hat_id.as_str(),
                topic = %default_topic,
                "No events written by hat, injecting default_publishes event"
            );

            self.bus.publish(default_event);
        }
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

        // Log iteration started
        self.diagnostics.log_orchestration(
            self.state.iteration,
            "loop",
            crate::diagnostics::OrchestrationEvent::IterationStarted,
        );

        // Log hat selected
        self.diagnostics.log_orchestration(
            self.state.iteration,
            "loop",
            crate::diagnostics::OrchestrationEvent::HatSelected {
                hat: hat_id.to_string(),
                reason: "process_output".to_string(),
            },
        );

        // Track failures
        if success {
            self.state.consecutive_failures = 0;
        } else {
            self.state.consecutive_failures += 1;
        }

        // Check for completion promise - only valid from Ralph (the coordinator)
        // Per spec: Requires dual condition (task state + consecutive confirmation)
        // When memories are enabled, verify tasks instead of scratchpad
        if hat_id.as_str() == "ralph"
            && EventParser::contains_promise(output, &self.config.event_loop.completion_promise)
        {
            let verification_result = if self.config.memories.enabled {
                self.verify_tasks_complete()
            } else {
                self.verify_scratchpad_complete()
            };

            match verification_result {
                Ok(true) => {
                    // All tasks complete - increment confirmation counter
                    self.state.completion_confirmations += 1;

                    if self.state.completion_confirmations >= 2 {
                        // Second consecutive confirmation - terminate
                        info!(
                            confirmations = self.state.completion_confirmations,
                            "Completion confirmed on consecutive iterations - terminating"
                        );

                        // Log loop terminated
                        self.diagnostics.log_orchestration(
                            self.state.iteration,
                            "loop",
                            crate::diagnostics::OrchestrationEvent::LoopTerminated {
                                reason: "completion_promise".to_string(),
                            },
                        );

                        return Some(TerminationReason::CompletionPromise);
                    }
                    // First confirmation - continue to next iteration
                    info!(
                        confirmations = self.state.completion_confirmations,
                        "Completion detected but requires consecutive confirmation - continuing"
                    );
                }
                Ok(false) => {
                    // Pending tasks exist - reject completion
                    debug!(
                        "Completion promise detected but scratchpad has pending [ ] tasks - rejected"
                    );
                    self.state.completion_confirmations = 0;
                }
                Err(e) => {
                    // Scratchpad doesn't exist or can't be read - reject completion
                    debug!(
                        error = %e,
                        "Completion promise detected but scratchpad verification failed - rejected"
                    );
                    self.state.completion_confirmations = 0;
                }
            }
        }

        // Parse and publish events from output
        let parser = EventParser::new().with_source(hat_id.clone());
        let events = parser.parse(output);

        // Validate build.done events have backpressure evidence
        let mut validated_events = Vec::new();
        for event in events {
            if event.topic.as_str() == "build.done" {
                if let Some(evidence) = EventParser::parse_backpressure_evidence(&event.payload) {
                    if evidence.all_passed() {
                        validated_events.push(event);
                    } else {
                        // Evidence present but checks failed - synthesize build.blocked
                        warn!(
                            hat = %hat_id.as_str(),
                            tests = evidence.tests_passed,
                            lint = evidence.lint_passed,
                            typecheck = evidence.typecheck_passed,
                            "build.done rejected: backpressure checks failed"
                        );

                        // Log backpressure triggered
                        self.diagnostics.log_orchestration(
                            self.state.iteration,
                            hat_id.as_str(),
                            crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                                reason: format!(
                                    "backpressure checks failed: tests={}, lint={}, typecheck={}",
                                    evidence.tests_passed,
                                    evidence.lint_passed,
                                    evidence.typecheck_passed
                                ),
                            },
                        );

                        let blocked = Event::new(
                            "build.blocked",
                            "Backpressure checks failed. Fix tests/lint/typecheck before emitting build.done."
                        ).with_source(hat_id.clone());
                        validated_events.push(blocked);
                    }
                } else {
                    // No evidence found - synthesize build.blocked
                    warn!(
                        hat = %hat_id.as_str(),
                        "build.done rejected: missing backpressure evidence"
                    );

                    // Log backpressure triggered
                    self.diagnostics.log_orchestration(
                        self.state.iteration,
                        hat_id.as_str(),
                        crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                            reason: "missing backpressure evidence".to_string(),
                        },
                    );

                    let blocked = Event::new(
                        "build.blocked",
                        "Missing backpressure evidence. Include 'tests: pass', 'lint: pass', 'typecheck: pass' in build.done payload."
                    ).with_source(hat_id.clone());
                    validated_events.push(blocked);
                }
            } else {
                validated_events.push(event);
            }
        }

        // Track build.blocked events for task-level thrashing detection
        let blocked_events: Vec<_> = validated_events
            .iter()
            .filter(|e| e.topic == "build.blocked".into())
            .collect();

        for blocked_event in &blocked_events {
            // Extract task ID from first line of payload
            let task_id = Self::extract_task_id(&blocked_event.payload);

            // Increment block count for this task
            let count = self
                .state
                .task_block_counts
                .entry(task_id.clone())
                .or_insert(0);
            *count += 1;

            debug!(
                task_id = %task_id,
                block_count = *count,
                "Task blocked"
            );

            // After 3 blocks on same task, emit build.task.abandoned
            if *count >= 3 && !self.state.abandoned_tasks.contains(&task_id) {
                warn!(
                    task_id = %task_id,
                    "Task abandoned after 3 consecutive blocks"
                );

                self.state.abandoned_tasks.push(task_id.clone());

                // Log task abandoned
                self.diagnostics.log_orchestration(
                    self.state.iteration,
                    hat_id.as_str(),
                    crate::diagnostics::OrchestrationEvent::TaskAbandoned {
                        reason: format!(
                            "3 consecutive build.blocked events for task '{}'",
                            task_id
                        ),
                    },
                );

                let abandoned_event = Event::new(
                    "build.task.abandoned",
                    format!(
                        "Task '{}' abandoned after 3 consecutive build.blocked events",
                        task_id
                    ),
                )
                .with_source(hat_id.clone());

                self.bus.publish(abandoned_event);
            }
        }

        // Track build.task events to detect redispatch of abandoned tasks
        let task_events: Vec<_> = validated_events
            .iter()
            .filter(|e| e.topic == "build.task".into())
            .collect();

        for task_event in task_events {
            let task_id = Self::extract_task_id(&task_event.payload);

            // Check if this task was already abandoned
            if self.state.abandoned_tasks.contains(&task_id) {
                self.state.abandoned_task_redispatches += 1;
                warn!(
                    task_id = %task_id,
                    redispatch_count = self.state.abandoned_task_redispatches,
                    "Planner redispatched abandoned task"
                );
            } else {
                // Reset redispatch counter on non-abandoned task
                self.state.abandoned_task_redispatches = 0;
            }
        }

        // Track hat-level blocking for legacy thrashing detection
        let has_blocked_event = !blocked_events.is_empty();

        if has_blocked_event {
            // Check if same hat as last blocked event
            if self.state.last_blocked_hat.as_ref() == Some(hat_id) {
                self.state.consecutive_blocked += 1;
            } else {
                self.state.consecutive_blocked = 1;
                self.state.last_blocked_hat = Some(hat_id.clone());
            }
        } else {
            // Reset counter on any non-blocked event
            self.state.consecutive_blocked = 0;
            self.state.last_blocked_hat = None;
        }

        for event in validated_events {
            debug!(
                topic = %event.topic,
                source = ?event.source,
                target = ?event.target,
                "Publishing event from output"
            );
            let topic = event.topic.clone();

            // Log event published
            self.diagnostics.log_orchestration(
                self.state.iteration,
                hat_id.as_str(),
                crate::diagnostics::OrchestrationEvent::EventPublished {
                    topic: topic.to_string(),
                },
            );

            let recipients = self.bus.publish(event);

            // Per spec: "Unknown topic → Log warning, event dropped"
            if recipients.is_empty() {
                warn!(
                    topic = %topic,
                    source = %hat_id.as_str(),
                    "Event has no subscribers - will be dropped. Check hat triggers configuration."
                );
            }
        }

        // Check termination conditions
        self.check_termination()
    }

    /// Extracts task identifier from build.blocked payload.
    /// Uses first line of payload as task ID.
    fn extract_task_id(payload: &str) -> String {
        payload
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string()
    }

    /// Adds cost to the cumulative total.
    pub fn add_cost(&mut self, cost: f64) {
        self.state.cumulative_cost += cost;
    }

    /// Verifies all tasks in scratchpad are complete or cancelled.
    ///
    /// Returns:
    /// - `Ok(true)` if all tasks are `[x]` or `[~]`
    /// - `Ok(false)` if any tasks are `[ ]` (pending)
    /// - `Err(...)` if scratchpad doesn't exist or can't be read
    fn verify_scratchpad_complete(&self) -> Result<bool, std::io::Error> {
        use std::path::Path;

        let scratchpad_path = Path::new(&self.config.core.scratchpad);

        if !scratchpad_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Scratchpad does not exist",
            ));
        }

        let content = std::fs::read_to_string(scratchpad_path)?;

        let has_pending = content
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"));

        Ok(!has_pending)
    }

    fn verify_tasks_complete(&self) -> Result<bool, std::io::Error> {
        use crate::task_store::TaskStore;
        use std::path::Path;

        let tasks_path = Path::new(".agent").join("tasks.jsonl");

        // No tasks file = no pending tasks = complete
        if !tasks_path.exists() {
            return Ok(true);
        }

        let store = TaskStore::load(&tasks_path)?;
        Ok(!store.has_open_tasks())
    }

    /// Processes events from JSONL and routes orphaned events to Ralph.
    ///
    /// Also handles backpressure for malformed JSONL lines by:
    /// 1. Emitting `event.malformed` system events for each parse failure
    /// 2. Tracking consecutive failures for termination check
    /// 3. Resetting counter when valid events are parsed
    ///
    /// Returns true if Ralph should be invoked to handle orphaned events.
    pub fn process_events_from_jsonl(&mut self) -> std::io::Result<bool> {
        let result = self.event_reader.read_new_events()?;

        // Handle malformed lines with backpressure
        for malformed in &result.malformed {
            let payload = format!(
                "Line {}: {}\nContent: {}",
                malformed.line_number, malformed.error, &malformed.content
            );
            let event = Event::new("event.malformed", &payload);
            self.bus.publish(event);
            self.state.consecutive_malformed_events += 1;
            warn!(
                line = malformed.line_number,
                consecutive = self.state.consecutive_malformed_events,
                "Malformed event line detected"
            );
        }

        // Reset counter when valid events are parsed
        if !result.events.is_empty() {
            self.state.consecutive_malformed_events = 0;
        }

        if result.events.is_empty() && result.malformed.is_empty() {
            return Ok(false);
        }

        let mut has_orphans = false;

        for event in result.events {
            // Check if any hat subscribes to this event
            if self.registry.has_subscriber(&event.topic) {
                // Route to subscriber via EventBus
                let proto_event = if let Some(payload) = event.payload {
                    Event::new(event.topic.as_str(), &payload)
                } else {
                    Event::new(event.topic.as_str(), "")
                };
                self.bus.publish(proto_event);
            } else {
                // Orphaned event - Ralph will handle it
                debug!(
                    topic = %event.topic,
                    "Event has no subscriber - will be handled by Ralph"
                );
                has_orphans = true;
            }
        }

        Ok(has_orphans)
    }

    /// Checks if output contains LOOP_COMPLETE from Ralph.
    ///
    /// Only Ralph can trigger loop completion. Hat outputs are ignored.
    pub fn check_ralph_completion(&self, output: &str) -> bool {
        EventParser::contains_promise(output, &self.config.event_loop.completion_promise)
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
        TerminationReason::LoopThrashing => {
            "Loop thrashing detected - same hat repeatedly blocked."
        }
        TerminationReason::ValidationFailure => "Too many consecutive malformed JSONL events.",
        TerminationReason::Stopped => "Manually stopped.",
        TerminationReason::Interrupted => "Interrupted by signal.",
    }
}
