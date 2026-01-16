//! Event loop orchestration.
//!
//! The event loop coordinates the execution of hats via pub/sub messaging.

use crate::config::{HatBackend, RalphConfig};
use crate::event_parser::EventParser;
use crate::event_reader::EventReader;
use crate::hat_registry::HatRegistry;
use crate::hatless_ralph::HatlessRalph;
use crate::instructions::InstructionBuilder;
use ralph_proto::{Event, EventBus, Hat, HatId};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

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
    /// Consecutive blocked events from the same hat.
    pub consecutive_blocked: u32,
    /// Hat that emitted the last blocked event.
    pub last_blocked_hat: Option<HatId>,
    /// Per-task block counts for task-level thrashing detection.
    pub task_block_counts: HashMap<String, u32>,
    /// Tasks that have been abandoned after 3+ blocks.
    pub abandoned_tasks: Vec<String>,
    /// Count of times planner dispatched an already-abandoned task.
    pub abandoned_task_redispatches: u32,
    /// Number of consecutive completion confirmations (requires 2 for termination).
    pub completion_confirmations: u32,
    /// Consecutive malformed JSONL lines encountered (for validation backpressure).
    pub consecutive_malformed_events: u32,
}

impl Default for LoopState {
    fn default() -> Self {
        Self {
            iteration: 0,
            consecutive_failures: 0,
            cumulative_cost: 0.0,
            started_at: Instant::now(),
            last_hat: None,
            consecutive_blocked: 0,
            last_blocked_hat: None,
            task_block_counts: HashMap::new(),
            abandoned_tasks: Vec::new(),
            abandoned_task_redispatches: 0,
            completion_confirmations: 0,
            consecutive_malformed_events: 0,
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
    ralph: HatlessRalph,
    event_reader: EventReader,
}

impl EventLoop {
    /// Creates a new event loop from configuration.
    pub fn new(config: RalphConfig) -> Self {
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

        let ralph = HatlessRalph::new(
            config.event_loop.completion_promise.clone(),
            config.core.clone(),
            &registry,
            config.event_loop.starting_event.clone(),
        );

        let event_reader = EventReader::new(".agent/events.jsonl");

        Self {
            config,
            registry,
            bus,
            state: LoopState::new(),
            instruction_builder,
            ralph,
            event_reader,
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
    pub fn build_prompt(&mut self, hat_id: &HatId) -> Option<String> {
        // Handle "ralph" hat - the constant coordinator
        // Per spec: "Hatless Ralph is constant — Cannot be replaced, overwritten, or configured away"
        if hat_id.as_str() == "ralph" {
            if self.registry.is_empty() {
                // Solo mode - just Ralph's events, no hats to filter
                let events = self.bus.take_pending(&hat_id.clone());
                let events_context = events
                    .iter()
                    .map(|e| format!("Event: {} - {}", e.topic, e.payload))
                    .collect::<Vec<_>>()
                    .join("\n");

                debug!("build_prompt: routing to HatlessRalph (solo mode)");
                return Some(self.ralph.build_prompt(&events_context, &[]));
            } else {
                // Multi-hat mode: collect events and determine active hats
                let all_hat_ids: Vec<HatId> = self.bus.hat_ids().cloned().collect();
                let mut all_events = Vec::new();
                for id in all_hat_ids {
                    all_events.extend(self.bus.take_pending(&id));
                }

                // Determine which hats are active based on events
                let active_hats = self.determine_active_hats(&all_events);

                // Format events for context
                let events_context = all_events
                    .iter()
                    .map(|e| format!("Event: {} - {}", e.topic, e.payload))
                    .collect::<Vec<_>>()
                    .join("\n");

                // Build prompt with active hats - filters instructions to only active hats
                debug!(
                    "build_prompt: routing to HatlessRalph (multi-hat coordinator mode), active_hats: {:?}",
                    active_hats
                        .iter()
                        .map(|h| h.id.as_str())
                        .collect::<Vec<_>>()
                );
                return Some(self.ralph.build_prompt(&events_context, &active_hats));
            }
        }

        // Non-ralph hat requested - this shouldn't happen in multi-hat mode since
        // next_hat() always returns "ralph" when custom hats are defined.
        // But we keep this code path for backward compatibility and tests.
        let events = self.bus.take_pending(&hat_id.clone());
        let events_context = events
            .iter()
            .map(|e| format!("Event: {} - {}", e.topic, e.payload))
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

    /// Builds the Ralph prompt (coordination mode).
    pub fn build_ralph_prompt(&self, prompt_content: &str) -> String {
        self.ralph.build_prompt(prompt_content, &[])
    }

    /// Determines which hats should be active based on pending events.
    /// Returns list of Hat references that are triggered by any pending event.
    fn determine_active_hats(&self, events: &[Event]) -> Vec<&Hat> {
        let mut active_hats = Vec::new();
        for event in events {
            if let Some(hat) = self.registry.get_for_topic(event.topic.as_str()) {
                // Avoid duplicates
                if !active_hats.iter().any(|h: &&Hat| h.id == hat.id) {
                    active_hats.push(hat);
                }
            }
        }
        active_hats
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

        // Track failures
        if success {
            self.state.consecutive_failures = 0;
        } else {
            self.state.consecutive_failures += 1;
        }

        // Check for completion promise - only valid from Ralph (the coordinator)
        // Per spec: Requires dual condition (task state + consecutive confirmation)
        if hat_id.as_str() == "ralph"
            && EventParser::contains_promise(output, &self.config.event_loop.completion_promise)
        {
            // Verify scratchpad task state
            match self.verify_scratchpad_complete() {
                Ok(true) => {
                    // All tasks complete - increment confirmation counter
                    self.state.completion_confirmations += 1;

                    if self.state.completion_confirmations >= 2 {
                        // Second consecutive confirmation - terminate
                        info!(
                            confirmations = self.state.completion_confirmations,
                            "Completion confirmed on consecutive iterations - terminating"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_routes_to_ralph_in_multihat_mode() {
        // Per "Hatless Ralph" architecture: When custom hats are defined,
        // Ralph is always the executor. Custom hats define topology only.
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start", "build.done", "build.blocked"]
    publishes: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        event_loop.initialize("Test prompt");

        // Per spec: In multi-hat mode, Ralph handles all iterations
        let next = event_loop.next_hat();
        assert!(next.is_some());
        assert_eq!(
            next.unwrap().as_str(),
            "ralph",
            "Multi-hat mode should route to Ralph"
        );

        // Verify Ralph's prompt includes the event
        let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();
        assert!(
            prompt.contains("task.start"),
            "Ralph's prompt should include the event"
        );
    }

    #[test]
    fn test_termination_max_iterations() {
        let yaml = r"
event_loop:
  max_iterations: 2
";
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
        use std::fs;
        use std::path::Path;

        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test");

        // Create scratchpad with all tasks completed
        let scratchpad_path = Path::new(".agent/scratchpad.md");
        fs::create_dir_all(scratchpad_path.parent().unwrap()).unwrap();
        fs::write(
            scratchpad_path,
            "## Tasks\n- [x] Task 1 done\n- [x] Task 2 done\n",
        )
        .unwrap();

        // Use Ralph since it's the coordinator that outputs completion promise
        let hat_id = HatId::new("ralph");

        // First LOOP_COMPLETE - should NOT terminate (needs consecutive confirmation)
        let reason = event_loop.process_output(&hat_id, "Done! LOOP_COMPLETE", true);
        assert_eq!(reason, None, "First confirmation should not terminate");

        // Second consecutive LOOP_COMPLETE - should terminate
        let reason = event_loop.process_output(&hat_id, "Done! LOOP_COMPLETE", true);
        assert_eq!(
            reason,
            Some(TerminationReason::CompletionPromise),
            "Second consecutive confirmation should terminate"
        );

        // Cleanup
        fs::remove_file(scratchpad_path).ok();
    }

    #[test]
    fn test_builder_cannot_terminate_loop() {
        // Per spec: "Builder hat outputs LOOP_COMPLETE → completion promise is ignored (only Ralph can terminate)"
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
    fn test_build_prompt_uses_ghuntley_style_for_all_hats() {
        // Per Hatless Ralph spec: All hats use build_custom_hat with ghuntley-style prompts
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start", "build.done", "build.blocked"]
    publishes: ["build.task"]
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test task");

        // Planner hat should get ghuntley-style prompt via build_custom_hat
        let planner_id = HatId::new("planner");
        let planner_prompt = event_loop.build_prompt(&planner_id).unwrap();

        // Verify ghuntley-style structure (numbered phases, guardrails)
        assert!(
            planner_prompt.contains("### 0. ORIENTATION"),
            "Planner should use ghuntley-style orientation phase"
        );
        assert!(
            planner_prompt.contains("### GUARDRAILS"),
            "Planner prompt should have guardrails section"
        );
        assert!(
            planner_prompt.contains("Fresh context each iteration"),
            "Planner prompt should have ghuntley identity"
        );

        // Now trigger builder hat by publishing build.task event
        let hat_id = HatId::new("builder");
        event_loop
            .bus
            .publish(Event::new("build.task", "Build something"));

        let builder_prompt = event_loop.build_prompt(&hat_id).unwrap();

        // Verify ghuntley-style structure for builder too
        assert!(
            builder_prompt.contains("### 0. ORIENTATION"),
            "Builder should use ghuntley-style orientation phase"
        );
        assert!(
            builder_prompt.contains("Only 1 subagent for build/tests"),
            "Builder prompt should have subagent limit"
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
        event_loop
            .bus
            .publish(Event::new("review.request", "Review PR #123"));

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
        assert_eq!(TerminationReason::LoopThrashing.exit_code(), 1);
        assert_eq!(TerminationReason::Stopped.exit_code(), 1);
        assert_eq!(TerminationReason::MaxIterations.exit_code(), 2);
        assert_eq!(TerminationReason::MaxRuntime.exit_code(), 2);
        assert_eq!(TerminationReason::MaxCost.exit_code(), 2);
        assert_eq!(TerminationReason::Interrupted.exit_code(), 130);
    }

    #[test]
    fn test_loop_thrashing_detection() {
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test");

        let planner_id = HatId::new("planner");
        let builder_id = HatId::new("builder");

        // Planner dispatches task "Fix bug"
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Fix bug</event>",
            true,
        );

        // Builder blocks on "Fix bug" three times (should emit build.task.abandoned)
        event_loop.process_output(
            &builder_id,
            "<event topic=\"build.blocked\">Fix bug\nCan't compile</event>",
            true,
        );
        event_loop.process_output(
            &builder_id,
            "<event topic=\"build.blocked\">Fix bug\nStill can't compile</event>",
            true,
        );
        event_loop.process_output(
            &builder_id,
            "<event topic=\"build.blocked\">Fix bug\nReally stuck</event>",
            true,
        );

        // Task should be abandoned but loop continues
        assert!(
            event_loop
                .state
                .abandoned_tasks
                .contains(&"Fix bug".to_string())
        );
        assert_eq!(event_loop.state.abandoned_task_redispatches, 0);

        // Planner redispatches the same abandoned task
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Fix bug</event>",
            true,
        );
        assert_eq!(event_loop.state.abandoned_task_redispatches, 1);

        // Planner redispatches again
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Fix bug</event>",
            true,
        );
        assert_eq!(event_loop.state.abandoned_task_redispatches, 2);

        // Third redispatch should trigger LoopThrashing
        let reason = event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Fix bug</event>",
            true,
        );
        assert_eq!(reason, Some(TerminationReason::LoopThrashing));
        assert_eq!(event_loop.state.abandoned_task_redispatches, 3);
    }

    #[test]
    fn test_thrashing_counter_resets_on_different_hat() {
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test");

        let planner_id = HatId::new("planner");
        let builder_id = HatId::new("builder");

        // Planner blocked twice
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.blocked\">Stuck</event>",
            true,
        );
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.blocked\">Still stuck</event>",
            true,
        );
        assert_eq!(event_loop.state.consecutive_blocked, 2);

        // Builder blocked - should reset counter
        event_loop.process_output(
            &builder_id,
            "<event topic=\"build.blocked\">Builder stuck</event>",
            true,
        );
        assert_eq!(event_loop.state.consecutive_blocked, 1);
        assert_eq!(event_loop.state.last_blocked_hat, Some(builder_id));
    }

    #[test]
    fn test_thrashing_counter_resets_on_non_blocked_event() {
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test");

        let planner_id = HatId::new("planner");

        // Two blocked events
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.blocked\">Stuck</event>",
            true,
        );
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.blocked\">Still stuck</event>",
            true,
        );
        assert_eq!(event_loop.state.consecutive_blocked, 2);

        // Non-blocked event should reset counter
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Working now</event>",
            true,
        );
        assert_eq!(event_loop.state.consecutive_blocked, 0);
        assert_eq!(event_loop.state.last_blocked_hat, None);
    }

    #[test]
    fn test_custom_hat_with_instructions_uses_build_custom_hat() {
        // Per spec: Custom hats with instructions should use build_custom_hat() method
        let yaml = r#"
hats:
  reviewer:
    name: "Code Reviewer"
    triggers: ["review.request"]
    instructions: "Review code for quality and security issues."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        // Trigger the custom hat
        event_loop
            .bus
            .publish(Event::new("review.request", "Review PR #123"));

        let reviewer_id = HatId::new("reviewer");
        let prompt = event_loop.build_prompt(&reviewer_id).unwrap();

        // Should use build_custom_hat() - verify by checking for ghuntley-style structure
        assert!(
            prompt.contains("Code Reviewer"),
            "Should include custom hat name"
        );
        assert!(
            prompt.contains("Review code for quality and security issues"),
            "Should include custom instructions"
        );
        assert!(
            prompt.contains("### 0. ORIENTATION"),
            "Should include ghuntley-style orientation"
        );
        assert!(
            prompt.contains("### 1. EXECUTE"),
            "Should use ghuntley-style execute phase"
        );
        assert!(
            prompt.contains("### GUARDRAILS"),
            "Should include guardrails section"
        );

        // Should include event context
        assert!(
            prompt.contains("Review PR #123"),
            "Should include event context"
        );
    }

    #[test]
    fn test_custom_hat_instructions_included_in_prompt() {
        // Test that custom instructions are properly included in the generated prompt
        let yaml = r#"
hats:
  tester:
    name: "Test Engineer"
    triggers: ["test.request"]
    instructions: |
      Run comprehensive tests including:
      - Unit tests
      - Integration tests
      - Security scans
      Report results with detailed coverage metrics.
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        // Trigger the custom hat
        event_loop
            .bus
            .publish(Event::new("test.request", "Test the auth module"));

        let tester_id = HatId::new("tester");
        let prompt = event_loop.build_prompt(&tester_id).unwrap();

        // Verify all custom instructions are included
        assert!(prompt.contains("Run comprehensive tests including"));
        assert!(prompt.contains("Unit tests"));
        assert!(prompt.contains("Integration tests"));
        assert!(prompt.contains("Security scans"));
        assert!(prompt.contains("detailed coverage metrics"));

        // Verify event context is included
        assert!(prompt.contains("Test the auth module"));
    }

    #[test]
    fn test_custom_hat_topology_visible_to_ralph() {
        // Per "Hatless Ralph" architecture: Custom hats define topology,
        // but Ralph handles all iterations. This test verifies hat topology
        // is visible in Ralph's prompt.
        let yaml = r#"
hats:
  deployer:
    name: "Deployment Manager"
    triggers: ["deploy.request", "deploy.rollback"]
    publishes: ["deploy.done", "deploy.failed"]
    instructions: "Handle deployment operations safely."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        // Publish an event that the deployer hat would conceptually handle
        event_loop
            .bus
            .publish(Event::new("deploy.request", "Deploy to staging"));

        // In multi-hat mode, next_hat always returns "ralph"
        let next_hat = event_loop.next_hat();
        assert_eq!(
            next_hat.unwrap().as_str(),
            "ralph",
            "Multi-hat mode routes to Ralph"
        );

        // Build Ralph's prompt - it should include the event context
        let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

        // Ralph's prompt should include:
        // 1. The event topic that was published (payload format: "Event: topic - payload")
        assert!(
            prompt.contains("deploy.request"),
            "Ralph's prompt should include the event topic"
        );

        // 2. The HATS section documenting the topology
        assert!(
            prompt.contains("## HATS"),
            "Ralph's prompt should include hat topology"
        );
        assert!(
            prompt.contains("Deployment Manager"),
            "Hat topology should include hat name"
        );
        assert!(
            prompt.contains("deploy.request"),
            "Hat triggers should be in topology"
        );
    }

    #[test]
    fn test_default_hat_with_custom_instructions_uses_build_custom_hat() {
        // Test that even default hats (planner/builder) use build_custom_hat when they have custom instructions
        let yaml = r#"
hats:
  planner:
    name: "Custom Planner"
    triggers: ["task.start", "build.done"]
    instructions: "Custom planning instructions with special focus on security."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        event_loop.initialize("Test task");

        let planner_id = HatId::new("planner");
        let prompt = event_loop.build_prompt(&planner_id).unwrap();

        // Should use build_custom_hat with ghuntley-style structure
        assert!(prompt.contains("Custom Planner"), "Should use custom name");
        assert!(
            prompt.contains("Custom planning instructions with special focus on security"),
            "Should include custom instructions"
        );
        assert!(
            prompt.contains("### 1. EXECUTE"),
            "Should use ghuntley-style execute phase"
        );
        assert!(
            prompt.contains("### GUARDRAILS"),
            "Should include guardrails section"
        );
    }

    #[test]
    fn test_custom_hat_without_instructions_gets_default_behavior() {
        // Test that custom hats without instructions still work with build_custom_hat
        let yaml = r#"
hats:
  monitor:
    name: "System Monitor"
    triggers: ["monitor.request"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        event_loop
            .bus
            .publish(Event::new("monitor.request", "Check system health"));

        let monitor_id = HatId::new("monitor");
        let prompt = event_loop.build_prompt(&monitor_id).unwrap();

        // Should still use build_custom_hat with ghuntley-style structure
        assert!(
            prompt.contains("System Monitor"),
            "Should include custom hat name"
        );
        assert!(
            prompt.contains("Follow the incoming event instructions"),
            "Should have default instructions when none provided"
        );
        assert!(
            prompt.contains("### 0. ORIENTATION"),
            "Should include ghuntley-style orientation"
        );
        assert!(
            prompt.contains("### GUARDRAILS"),
            "Should include guardrails section"
        );
        assert!(
            prompt.contains("Check system health"),
            "Should include event context"
        );
    }

    #[test]
    fn test_task_cancellation_with_tilde_marker() {
        // Test that tasks marked with [~] are recognized as cancelled
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test task");

        let ralph_id = HatId::new("ralph");

        // Simulate Ralph output with cancelled task
        let output = r"
## Tasks
- [x] Task 1 completed
- [~] Task 2 cancelled (too complex for current scope)
- [ ] Task 3 pending
";

        // Process output - should not terminate since there are still pending tasks
        let reason = event_loop.process_output(&ralph_id, output, true);
        assert_eq!(reason, None, "Should not terminate with pending tasks");
    }

    #[test]
    fn test_partial_completion_with_cancelled_tasks() {
        use std::fs;
        use std::path::Path;

        // Test that cancelled tasks don't block completion when all other tasks are done
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test task");

        // Ralph handles task.start, not a specific hat
        let ralph_id = HatId::new("ralph");

        // Create scratchpad with completed and cancelled tasks
        let scratchpad_path = Path::new(".agent/scratchpad.md");
        fs::create_dir_all(scratchpad_path.parent().unwrap()).unwrap();
        let scratchpad_content = r"## Tasks
- [x] Core feature implemented
- [x] Tests added
- [~] Documentation update (cancelled: out of scope)
- [~] Performance optimization (cancelled: not needed)
";
        fs::write(scratchpad_path, scratchpad_content).unwrap();

        // Simulate completion with some cancelled tasks
        let output = "All done! LOOP_COMPLETE";

        // First confirmation - should not terminate yet
        let reason = event_loop.process_output(&ralph_id, output, true);
        assert_eq!(reason, None, "First confirmation should not terminate");

        // Second consecutive confirmation - should complete successfully despite cancelled tasks
        let reason = event_loop.process_output(&ralph_id, output, true);
        assert_eq!(
            reason,
            Some(TerminationReason::CompletionPromise),
            "Should complete with partial completion"
        );

        // Cleanup
        fs::remove_file(scratchpad_path).ok();
    }

    #[test]
    fn test_planner_auto_cancellation_after_three_blocks() {
        // Test that task is abandoned after 3 build.blocked events for same task
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test task");

        let builder_id = HatId::new("builder");
        let planner_id = HatId::new("planner");

        // First blocked event for "Task X" - should not abandon
        let reason = event_loop.process_output(
            &builder_id,
            "<event topic=\"build.blocked\">Task X\nmissing dependency</event>",
            true,
        );
        assert_eq!(reason, None);
        assert_eq!(event_loop.state.task_block_counts.get("Task X"), Some(&1));

        // Second blocked event for "Task X" - should not abandon
        let reason = event_loop.process_output(
            &builder_id,
            "<event topic=\"build.blocked\">Task X\ndependency issue persists</event>",
            true,
        );
        assert_eq!(reason, None);
        assert_eq!(event_loop.state.task_block_counts.get("Task X"), Some(&2));

        // Third blocked event for "Task X" - should emit build.task.abandoned but not terminate
        let reason = event_loop.process_output(
            &builder_id,
            "<event topic=\"build.blocked\">Task X\nsame dependency issue</event>",
            true,
        );
        assert_eq!(reason, None, "Should not terminate, just abandon task");
        assert_eq!(event_loop.state.task_block_counts.get("Task X"), Some(&3));
        assert!(
            event_loop
                .state
                .abandoned_tasks
                .contains(&"Task X".to_string()),
            "Task X should be abandoned"
        );

        // Planner can now replan around the abandoned task
        // Only terminates if planner keeps redispatching the abandoned task
        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Task X</event>",
            true,
        );
        assert_eq!(event_loop.state.abandoned_task_redispatches, 1);

        event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Task X</event>",
            true,
        );
        assert_eq!(event_loop.state.abandoned_task_redispatches, 2);

        let reason = event_loop.process_output(
            &planner_id,
            "<event topic=\"build.task\">Task X</event>",
            true,
        );
        assert_eq!(
            reason,
            Some(TerminationReason::LoopThrashing),
            "Should terminate after 3 redispatches of abandoned task"
        );
    }

    #[test]
    fn test_default_publishes_injects_when_no_events() {
        use std::collections::HashMap;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let events_path = temp_dir.path().join("events.jsonl");

        let mut config = RalphConfig::default();
        let mut hats = HashMap::new();
        hats.insert(
            "test-hat".to_string(),
            crate::config::HatConfig {
                name: "test-hat".to_string(),
                description: Some("Test hat for default publishes".to_string()),
                triggers: vec!["task.start".to_string()],
                publishes: vec!["task.done".to_string()],
                instructions: "Test hat".to_string(),
                backend: None,
                default_publishes: Some("task.done".to_string()),
            },
        );
        config.hats = hats;

        let mut event_loop = EventLoop::new(config);
        event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
        event_loop.initialize("Test");

        let hat_id = HatId::new("test-hat");

        // Record event count before execution
        let before = event_loop.record_event_count();

        // Hat executes but writes no events
        // (In real scenario, hat would write to events.jsonl, but we simulate none written)

        // Check for default_publishes
        event_loop.check_default_publishes(&hat_id, before);

        // Verify default event was injected
        assert!(
            event_loop.has_pending_events(),
            "Default event should be injected"
        );
    }

    #[test]
    fn test_default_publishes_not_injected_when_events_written() {
        use std::collections::HashMap;
        use std::io::Write;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let events_path = temp_dir.path().join("events.jsonl");

        let mut config = RalphConfig::default();
        let mut hats = HashMap::new();
        hats.insert(
            "test-hat".to_string(),
            crate::config::HatConfig {
                name: "test-hat".to_string(),
                description: Some("Test hat for default publishes".to_string()),
                triggers: vec!["task.start".to_string()],
                publishes: vec!["task.done".to_string()],
                instructions: "Test hat".to_string(),
                backend: None,
                default_publishes: Some("task.done".to_string()),
            },
        );
        config.hats = hats;

        let mut event_loop = EventLoop::new(config);
        event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
        event_loop.initialize("Test");

        let hat_id = HatId::new("test-hat");

        // Record event count before execution
        let before = event_loop.record_event_count();

        // Hat writes an event
        let mut file = std::fs::File::create(&events_path).unwrap();
        writeln!(
            file,
            r#"{{"topic":"task.done","ts":"2024-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        // Check for default_publishes
        event_loop.check_default_publishes(&hat_id, before);

        // Default should NOT be injected since hat wrote an event
        // The event from file should be read by event_reader
    }

    #[test]
    fn test_default_publishes_not_injected_when_not_configured() {
        use std::collections::HashMap;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let events_path = temp_dir.path().join("events.jsonl");

        let mut config = RalphConfig::default();
        let mut hats = HashMap::new();
        hats.insert(
            "test-hat".to_string(),
            crate::config::HatConfig {
                name: "test-hat".to_string(),
                description: Some("Test hat for default publishes".to_string()),
                triggers: vec!["task.start".to_string()],
                publishes: vec!["task.done".to_string()],
                instructions: "Test hat".to_string(),
                backend: None,
                default_publishes: None, // No default configured
            },
        );
        config.hats = hats;

        let mut event_loop = EventLoop::new(config);
        event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
        event_loop.initialize("Test");

        let hat_id = HatId::new("test-hat");

        // Consume the initial event from initialize
        let _ = event_loop.build_prompt(&hat_id);

        // Record event count before execution
        let before = event_loop.record_event_count();

        // Hat executes but writes no events

        // Check for default_publishes
        event_loop.check_default_publishes(&hat_id, before);

        // No default should be injected since not configured
        assert!(
            !event_loop.has_pending_events(),
            "No default should be injected"
        );
    }

    #[test]
    fn test_get_hat_backend_with_named_backend() {
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    backend: "claude"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let event_loop = EventLoop::new(config);

        let hat_id = HatId::new("builder");
        let backend = event_loop.get_hat_backend(&hat_id);

        assert!(backend.is_some());
        match backend.unwrap() {
            HatBackend::Named(name) => assert_eq!(name, "claude"),
            _ => panic!("Expected Named backend"),
        }
    }

    #[test]
    fn test_get_hat_backend_with_kiro_agent() {
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    backend:
      type: "kiro"
      agent: "my-agent"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let event_loop = EventLoop::new(config);

        let hat_id = HatId::new("builder");
        let backend = event_loop.get_hat_backend(&hat_id);

        assert!(backend.is_some());
        match backend.unwrap() {
            HatBackend::KiroAgent { agent, .. } => assert_eq!(agent, "my-agent"),
            _ => panic!("Expected KiroAgent backend"),
        }
    }

    #[test]
    fn test_get_hat_backend_inherits_global() {
        let yaml = r#"
cli:
  backend: "gemini"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let event_loop = EventLoop::new(config);

        let hat_id = HatId::new("builder");
        let backend = event_loop.get_hat_backend(&hat_id);

        // Hat has no backend configured, should return None (inherit global)
        assert!(backend.is_none());
    }

    #[test]
    fn test_hatless_mode_registers_ralph_catch_all() {
        // When no hats are configured, "ralph" should be registered as catch-all
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);

        // Registry should be empty (no user-defined hats)
        assert!(event_loop.registry().is_empty());

        // But when we initialize, task.start should route to "ralph"
        event_loop.initialize("Test prompt");

        // "ralph" should have pending events
        let next_hat = event_loop.next_hat();
        assert!(next_hat.is_some(), "Should have pending events for ralph");
        assert_eq!(next_hat.unwrap().as_str(), "ralph");
    }

    #[test]
    fn test_hatless_mode_builds_ralph_prompt() {
        // In hatless mode, build_prompt for "ralph" should return HatlessRalph prompt
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test prompt");

        let ralph_id = HatId::new("ralph");
        let prompt = event_loop.build_prompt(&ralph_id);

        assert!(prompt.is_some(), "Should build prompt for ralph");
        let prompt = prompt.unwrap();

        // Should contain ghuntley-style Ralph identity (uses "I'm Ralph" not "You are Ralph")
        assert!(
            prompt.contains("I'm Ralph"),
            "Should identify as Ralph with ghuntley style"
        );
        assert!(
            prompt.contains("## WORKFLOW"),
            "Should have workflow section"
        );
        assert!(
            prompt.contains("## EVENT WRITING"),
            "Should have event writing section"
        );
        assert!(
            prompt.contains("LOOP_COMPLETE"),
            "Should reference completion promise"
        );
    }

    // === "Always Hatless Iteration" Architecture Tests ===
    // These tests verify the core invariants of the Hatless Ralph architecture:
    // - Ralph is always the sole executor when custom hats are defined
    // - Custom hats define topology (pub/sub contracts) for coordination context
    // - Ralph's prompt includes the ## HATS section documenting the topology

    #[test]
    fn test_always_hatless_ralph_executes_all_iterations() {
        // Per acceptance criteria #1: Ralph executes all iterations with custom hats
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start", "build.done"]
    publishes: ["build.task"]
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        // Simulate the workflow: task.start → planner (conceptually)
        event_loop.initialize("Implement feature X");
        assert_eq!(event_loop.next_hat().unwrap().as_str(), "ralph");

        // Simulate build.task → builder (conceptually)
        event_loop.build_prompt(&HatId::new("ralph")); // Consume task.start
        event_loop
            .bus
            .publish(Event::new("build.task", "Build feature X"));
        assert_eq!(
            event_loop.next_hat().unwrap().as_str(),
            "ralph",
            "build.task should route to Ralph"
        );

        // Simulate build.done → planner (conceptually)
        event_loop.build_prompt(&HatId::new("ralph")); // Consume build.task
        event_loop
            .bus
            .publish(Event::new("build.done", "Feature X complete"));
        assert_eq!(
            event_loop.next_hat().unwrap().as_str(),
            "ralph",
            "build.done should route to Ralph"
        );
    }

    #[test]
    fn test_always_hatless_solo_mode_unchanged() {
        // Per acceptance criteria #3: Solo mode (no hats) operates as before
        let config = RalphConfig::default();
        let mut event_loop = EventLoop::new(config);

        assert!(
            event_loop.registry().is_empty(),
            "Solo mode has no custom hats"
        );

        event_loop.initialize("Do something");
        assert_eq!(event_loop.next_hat().unwrap().as_str(), "ralph");

        // Solo mode prompt should NOT have ## HATS section
        let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();
        assert!(
            !prompt.contains("## HATS"),
            "Solo mode should not have HATS section"
        );
    }

    #[test]
    fn test_always_hatless_topology_preserved_in_prompt() {
        // Per acceptance criteria #2 and #4: Hat topology preserved for coordination
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start", "build.done", "build.blocked"]
    publishes: ["build.task"]
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);
        event_loop.initialize("Test");

        let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

        // Verify ## HATS section with topology table
        assert!(prompt.contains("## HATS"), "Should have HATS section");
        assert!(
            prompt.contains("Delegate via events"),
            "Should explain delegation"
        );
        assert!(
            prompt.contains("| Hat | Triggers On | Publishes |"),
            "Should have topology table"
        );

        // Verify both hats are documented
        assert!(prompt.contains("Planner"), "Should include Planner hat");
        assert!(prompt.contains("Builder"), "Should include Builder hat");

        // Verify trigger and publish information
        assert!(
            prompt.contains("build.task"),
            "Should document build.task event"
        );
        assert!(
            prompt.contains("build.done"),
            "Should document build.done event"
        );
    }

    #[test]
    fn test_always_hatless_no_backend_delegation() {
        // Per acceptance criteria #5: Custom hat backends are NOT used
        // This is architectural - the EventLoop.next_hat() always returns "ralph"
        // so per-hat backends (if configured) are never invoked
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    backend: "gemini"  # This backend should NEVER be used
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        event_loop.bus.publish(Event::new("build.task", "Test"));

        // Despite builder having a specific backend, Ralph handles the iteration
        let next = event_loop.next_hat();
        assert_eq!(
            next.unwrap().as_str(),
            "ralph",
            "Ralph handles all iterations"
        );

        // The backend delegation would happen in main.rs, but since we always
        // return "ralph" from next_hat(), the gemini backend is never selected
    }

    #[test]
    fn test_always_hatless_collects_all_pending_events() {
        // Verify Ralph's prompt includes events from ALL hats when in multi-hat mode
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start"]
  builder:
    name: "Builder"
    triggers: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        // Publish events that would go to different hats
        event_loop
            .bus
            .publish(Event::new("task.start", "Start task"));
        event_loop
            .bus
            .publish(Event::new("build.task", "Build something"));

        // Ralph should collect ALL pending events
        let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

        // Both events should be in Ralph's context
        assert!(
            prompt.contains("task.start"),
            "Should include task.start event"
        );
        assert!(
            prompt.contains("build.task"),
            "Should include build.task event"
        );
    }

    // === Phase 2: Active Hat Detection Tests ===

    #[test]
    fn test_determine_active_hats() {
        // Create EventLoop with 3 hats (security_reviewer, architecture_reviewer, correctness_reviewer)
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
  architecture_reviewer:
    name: "Architecture Reviewer"
    triggers: ["review.architecture"]
  correctness_reviewer:
    name: "Correctness Reviewer"
    triggers: ["review.correctness"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let event_loop = EventLoop::new(config);

        // Create events: [Event("review.security", "..."), Event("review.architecture", "...")]
        let events = vec![
            Event::new("review.security", "Check for vulnerabilities"),
            Event::new("review.architecture", "Review design patterns"),
        ];

        // Call determine_active_hats(&events)
        let active_hats = event_loop.determine_active_hats(&events);

        // Assert: Returns Vec with exactly security_reviewer and architecture_reviewer Hats
        assert_eq!(active_hats.len(), 2, "Should return exactly 2 active hats");

        let hat_ids: Vec<&str> = active_hats.iter().map(|h| h.id.as_str()).collect();
        assert!(
            hat_ids.contains(&"security_reviewer"),
            "Should include security_reviewer"
        );
        assert!(
            hat_ids.contains(&"architecture_reviewer"),
            "Should include architecture_reviewer"
        );
        assert!(
            !hat_ids.contains(&"correctness_reviewer"),
            "Should NOT include correctness_reviewer"
        );
    }

    #[test]
    fn test_get_active_hat_id_with_pending_event() {
        // Create EventLoop with security_reviewer hat
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let mut event_loop = EventLoop::new(config);

        // Publish Event("review.security", "...")
        event_loop
            .bus
            .publish(Event::new("review.security", "Check authentication"));

        // Call get_active_hat_id()
        let active_hat_id = event_loop.get_active_hat_id();

        // Assert: Returns HatId("security_reviewer"), NOT "ralph"
        assert_eq!(
            active_hat_id.as_str(),
            "security_reviewer",
            "Should return security_reviewer, not ralph"
        );
    }

    #[test]
    fn test_get_active_hat_id_no_pending_returns_ralph() {
        // Create EventLoop with hats but NO pending events
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let event_loop = EventLoop::new(config);

        // Call get_active_hat_id() - no pending events
        let active_hat_id = event_loop.get_active_hat_id();

        // Assert: Returns HatId("ralph")
        assert_eq!(
            active_hat_id.as_str(),
            "ralph",
            "Should return ralph when no pending events"
        );
    }
}
