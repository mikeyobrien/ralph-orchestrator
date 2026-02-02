//! Event loop orchestration.
//!
//! The event loop coordinates the execution of hats via pub/sub messaging.

mod loop_state;
#[cfg(test)]
mod tests;

pub use loop_state::LoopState;

use crate::config::{HatBackend, InjectMode, RalphConfig};
use crate::event_parser::{EventParser, MutationEvidence, MutationStatus};
use crate::event_reader::EventReader;
use crate::hat_registry::HatRegistry;
use crate::hatless_ralph::HatlessRalph;
use crate::instructions::InstructionBuilder;
use crate::loop_context::LoopContext;
use crate::memory_store::{MarkdownMemoryStore, format_memories_as_markdown, truncate_to_budget};
use crate::skill_registry::SkillRegistry;
use ralph_proto::{CheckinContext, Event, EventBus, Hat, HatId, RobotService};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
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
    /// Restart requested via Telegram `/restart` command.
    RestartRequested,
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
            // Restart uses exit code 3 to signal the caller to exec-replace
            TerminationReason::RestartRequested => 3,
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
            TerminationReason::RestartRequested => "restart_requested",
        }
    }

    /// Returns true if this is a successful completion (not an error or limit).
    pub fn is_success(&self) -> bool {
        matches!(self, TerminationReason::CompletionPromise)
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
    /// Cached human guidance messages that should persist across iterations.
    robot_guidance: Vec<String>,
    /// Event reader for consuming events from JSONL file.
    /// Made pub(crate) to allow tests to override the path.
    pub(crate) event_reader: EventReader,
    diagnostics: crate::diagnostics::DiagnosticsCollector,
    /// Loop context for path resolution (None for legacy single-loop mode).
    loop_context: Option<LoopContext>,
    /// Skill registry for the current loop.
    skill_registry: SkillRegistry,
    /// Robot service for human-in-the-loop communication.
    /// Injected externally when `human.enabled` is true and this is the primary loop.
    robot_service: Option<Box<dyn RobotService>>,
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

    /// Creates a new event loop with a loop context for path resolution.
    ///
    /// The loop context determines where events, tasks, and other state files
    /// are located. Use this for multi-loop scenarios where each loop runs
    /// in an isolated workspace (git worktree).
    pub fn with_context(config: RalphConfig, context: LoopContext) -> Self {
        let diagnostics = crate::diagnostics::DiagnosticsCollector::new(context.workspace())
            .unwrap_or_else(|e| {
                debug!(
                    "Failed to initialize diagnostics: {}, using disabled collector",
                    e
                );
                crate::diagnostics::DiagnosticsCollector::disabled()
            });

        Self::with_context_and_diagnostics(config, context, diagnostics)
    }

    /// Creates a new event loop with explicit loop context and diagnostics.
    pub fn with_context_and_diagnostics(
        config: RalphConfig,
        context: LoopContext,
        diagnostics: crate::diagnostics::DiagnosticsCollector,
    ) -> Self {
        let registry = HatRegistry::from_config(&config);
        let instruction_builder =
            InstructionBuilder::with_events(config.core.clone(), config.events.clone());

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

        // Build skill registry from config
        let skill_registry = if config.skills.enabled {
            SkillRegistry::from_config(
                &config.skills,
                context.workspace(),
                Some(config.cli.backend.as_str()),
            )
            .unwrap_or_else(|e| {
                warn!(
                    "Failed to build skill registry: {}, using empty registry",
                    e
                );
                SkillRegistry::new(Some(config.cli.backend.as_str()))
            })
        } else {
            SkillRegistry::new(Some(config.cli.backend.as_str()))
        };

        let skill_index = if config.skills.enabled {
            skill_registry.build_index(None)
        } else {
            String::new()
        };

        // When memories are enabled, add tasks CLI instructions alongside scratchpad
        let ralph = HatlessRalph::new(
            config.event_loop.completion_promise.clone(),
            config.core.clone(),
            &registry,
            config.event_loop.starting_event.clone(),
        )
        .with_memories_enabled(config.memories.enabled)
        .with_skill_index(skill_index);

        // Read timestamped events path from marker file, fall back to default
        // The marker file contains a relative path like ".ralph/events-20260127-123456.jsonl"
        // which we resolve relative to the workspace root
        let events_path = std::fs::read_to_string(context.current_events_marker())
            .map(|s| {
                let relative = s.trim();
                context.workspace().join(relative)
            })
            .unwrap_or_else(|_| context.events_path());
        let event_reader = EventReader::new(&events_path);

        Self {
            config,
            registry,
            bus,
            state: LoopState::new(),
            instruction_builder,
            ralph,
            robot_guidance: Vec::new(),
            event_reader,
            diagnostics,
            loop_context: Some(context),
            skill_registry,
            robot_service: None,
        }
    }

    /// Creates a new event loop with explicit diagnostics collector (for testing).
    pub fn with_diagnostics(
        config: RalphConfig,
        diagnostics: crate::diagnostics::DiagnosticsCollector,
    ) -> Self {
        let registry = HatRegistry::from_config(&config);
        let instruction_builder =
            InstructionBuilder::with_events(config.core.clone(), config.events.clone());

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

        // Build skill registry from config
        let workspace_root = std::path::Path::new(".");
        let skill_registry = if config.skills.enabled {
            SkillRegistry::from_config(
                &config.skills,
                workspace_root,
                Some(config.cli.backend.as_str()),
            )
            .unwrap_or_else(|e| {
                warn!(
                    "Failed to build skill registry: {}, using empty registry",
                    e
                );
                SkillRegistry::new(Some(config.cli.backend.as_str()))
            })
        } else {
            SkillRegistry::new(Some(config.cli.backend.as_str()))
        };

        let skill_index = if config.skills.enabled {
            skill_registry.build_index(None)
        } else {
            String::new()
        };

        // When memories are enabled, add tasks CLI instructions alongside scratchpad
        let ralph = HatlessRalph::new(
            config.event_loop.completion_promise.clone(),
            config.core.clone(),
            &registry,
            config.event_loop.starting_event.clone(),
        )
        .with_memories_enabled(config.memories.enabled)
        .with_skill_index(skill_index);

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
            robot_guidance: Vec::new(),
            event_reader,
            diagnostics,
            loop_context: None,
            skill_registry,
            robot_service: None,
        }
    }

    /// Injects a robot service for human-in-the-loop communication.
    ///
    /// Call this after construction to enable `human.interact` event handling,
    /// periodic check-ins, and question/response flow. The service is typically
    /// created by the CLI layer (e.g., `TelegramService`) and injected here,
    /// keeping the core event loop decoupled from any specific communication
    /// platform.
    pub fn set_robot_service(&mut self, service: Box<dyn RobotService>) {
        self.robot_service = Some(service);
    }

    /// Returns the loop context, if one was provided.
    pub fn loop_context(&self) -> Option<&LoopContext> {
        self.loop_context.as_ref()
    }

    /// Returns the tasks path based on loop context or default.
    fn tasks_path(&self) -> PathBuf {
        self.loop_context
            .as_ref()
            .map(|ctx| ctx.tasks_path())
            .unwrap_or_else(|| PathBuf::from(".ralph/agent/tasks.jsonl"))
    }

    /// Returns the scratchpad path based on loop context or config.
    fn scratchpad_path(&self) -> PathBuf {
        self.loop_context
            .as_ref()
            .map(|ctx| ctx.scratchpad_path())
            .unwrap_or_else(|| PathBuf::from(&self.config.core.scratchpad))
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

        // Check for stop signal from Telegram /stop or CLI stop-requested
        let stop_path =
            std::path::Path::new(&self.config.core.workspace_root).join(".ralph/stop-requested");
        if stop_path.exists() {
            let _ = std::fs::remove_file(&stop_path);
            return Some(TerminationReason::Stopped);
        }

        // Check for restart signal from Telegram /restart command
        let restart_path =
            std::path::Path::new(&self.config.core.workspace_root).join(".ralph/restart-requested");
        if restart_path.exists() {
            return Some(TerminationReason::RestartRequested);
        }

        None
    }

    /// Checks if a completion event was received and returns termination reason.
    ///
    /// Completion is only accepted via JSONL events (e.g., `ralph emit`).
    pub fn check_completion_event(&mut self) -> Option<TerminationReason> {
        if !self.state.completion_requested {
            return None;
        }

        self.state.completion_requested = false;

        // In persistent mode, suppress completion and keep the loop alive
        if self.config.event_loop.persistent {
            info!("Completion event suppressed - persistent mode active, loop staying alive");

            self.diagnostics.log_orchestration(
                self.state.iteration,
                "loop",
                crate::diagnostics::OrchestrationEvent::LoopTerminated {
                    reason: "completion_event_suppressed_persistent".to_string(),
                },
            );

            // Inject a task.resume event so the loop continues with an idle prompt
            let resume_event = Event::new(
                "task.resume",
                "Persistent mode: loop staying alive after completion signal. \
                 Check for new tasks or await human guidance.",
            );
            self.bus.publish(resume_event);

            return None;
        }

        // Log warning if tasks remain open (informational only)
        if self.config.memories.enabled {
            if let Ok(false) = self.verify_tasks_complete() {
                let open_tasks = self.get_open_task_list();
                warn!(
                    open_tasks = ?open_tasks,
                    "Completion event with {} open task(s) - trusting agent decision",
                    open_tasks.len()
                );
            }
        } else if let Ok(false) = self.verify_scratchpad_complete() {
            warn!("Completion event with pending scratchpad tasks - trusting agent decision");
        }

        info!("Completion event detected - terminating");

        // Log loop terminated
        self.diagnostics.log_orchestration(
            self.state.iteration,
            "loop",
            crate::diagnostics::OrchestrationEvent::LoopTerminated {
                reason: "completion_event".to_string(),
            },
        );

        Some(TerminationReason::CompletionPromise)
    }

    /// Initializes the loop by publishing the start event.
    pub fn initialize(&mut self, prompt_content: &str) {
        // Use configured starting_event or default to task.start for backward compatibility
        let topic = self
            .config
            .event_loop
            .starting_event
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
        // Store the objective so it persists across all iterations.
        // After iteration 1, bus.take_pending() consumes the start event,
        // so without this the objective would be invisible to later hats.
        self.ralph.set_objective(prompt_content.to_string());

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

        // If no pending hat events but human interactions are pending, route to Ralph.
        if next.is_none() && self.bus.has_human_pending() {
            return self.bus.hat_ids().find(|id| id.as_str() == "ralph");
        }

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
        self.bus.next_hat_with_pending().is_some() || self.bus.has_human_pending()
    }

    /// Checks if any pending events are human-related (human.response, human.guidance).
    ///
    /// Used to skip cooldown delays when a human event is next, since we don't
    /// want to artificially delay the response to a human interaction.
    pub fn has_pending_human_events(&self) -> bool {
        self.bus.has_human_pending()
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
    /// primed memories to the prompt context. If a scratchpad file exists and is
    /// non-empty, its content is also prepended (before memories).
    pub fn build_prompt(&mut self, hat_id: &HatId) -> Option<String> {
        // Handle "ralph" hat - the constant coordinator
        // Per spec: "Hatless Ralph is constant — Cannot be replaced, overwritten, or configured away"
        if hat_id.as_str() == "ralph" {
            if self.registry.is_empty() {
                // Solo mode - just Ralph's events, no hats to filter
                let mut events = self.bus.take_pending(&hat_id.clone());
                let mut human_events = self.bus.take_human_pending();
                events.append(&mut human_events);

                // Separate human.guidance events from regular events
                let (guidance_events, regular_events): (Vec<_>, Vec<_>) = events
                    .into_iter()
                    .partition(|e| e.topic.as_str() == "human.guidance");

                let events_context = regular_events
                    .iter()
                    .map(|e| Self::format_event(e))
                    .collect::<Vec<_>>()
                    .join("\n");

                // Persist and inject human guidance into prompt if present
                self.update_robot_guidance(guidance_events);
                self.apply_robot_guidance();

                // Build base prompt and prepend memories + scratchpad + ready tasks
                let base_prompt = self.ralph.build_prompt(&events_context, &[]);
                self.ralph.clear_robot_guidance();
                let with_skills = self.prepend_auto_inject_skills(base_prompt);
                let with_scratchpad = self.prepend_scratchpad(with_skills);
                let final_prompt = self.prepend_ready_tasks(with_scratchpad);

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

                let mut human_events = self.bus.take_human_pending();
                all_events.append(&mut human_events);

                // Publish orchestrator-generated system events after consuming pending events,
                // so they become visible in the event log and can be handled next iteration.
                for event in system_events {
                    self.bus.publish(event);
                }

                // Separate human.guidance events from regular events
                let (guidance_events, regular_events): (Vec<_>, Vec<_>) = all_events
                    .into_iter()
                    .partition(|e| e.topic.as_str() == "human.guidance");

                // Persist and inject human guidance before building prompt (must happen before
                // immutable borrows from determine_active_hats)
                self.update_robot_guidance(guidance_events);
                self.apply_robot_guidance();

                // Determine which hats are active based on regular events
                let active_hat_ids = self.determine_active_hat_ids(&regular_events);
                self.record_hat_activations(&active_hat_ids);
                let active_hats = self.determine_active_hats(&regular_events);

                // Format events for context
                let events_context = regular_events
                    .iter()
                    .map(|e| Self::format_event(e))
                    .collect::<Vec<_>>()
                    .join("\n");

                // Build base prompt and prepend memories + scratchpad if available
                let base_prompt = self.ralph.build_prompt(&events_context, &active_hats);

                // Build prompt with active hats - filters instructions to only active hats
                debug!(
                    "build_prompt: routing to HatlessRalph (multi-hat coordinator mode), active_hats: {:?}",
                    active_hats
                        .iter()
                        .map(|h| h.id.as_str())
                        .collect::<Vec<_>>()
                );

                // Clear guidance after active_hats references are no longer needed
                self.ralph.clear_robot_guidance();
                let with_skills = self.prepend_auto_inject_skills(base_prompt);
                let with_scratchpad = self.prepend_scratchpad(with_skills);
                let final_prompt = self.prepend_ready_tasks(with_scratchpad);

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

    /// Stores guidance payloads, persists them to scratchpad, and prepares them for prompt injection.
    ///
    /// Guidance events are ephemeral in the event bus (consumed by `take_pending`).
    /// This method both caches them in memory for prompt injection and appends
    /// them to the scratchpad file so they survive across process restarts.
    fn update_robot_guidance(&mut self, guidance_events: Vec<Event>) {
        if guidance_events.is_empty() {
            return;
        }

        // Persist new guidance to scratchpad before caching
        self.persist_guidance_to_scratchpad(&guidance_events);

        self.robot_guidance
            .extend(guidance_events.into_iter().map(|e| e.payload));
    }

    /// Appends human guidance entries to the scratchpad file for durability.
    ///
    /// Each guidance message is written as a timestamped markdown entry so it
    /// appears alongside the agent's own thinking and survives process restarts.
    fn persist_guidance_to_scratchpad(&self, guidance_events: &[Event]) {
        use std::io::Write;

        let scratchpad_path = self.scratchpad_path();
        let resolved_path = if scratchpad_path.is_relative() {
            self.config.core.workspace_root.join(&scratchpad_path)
        } else {
            scratchpad_path
        };

        // Create parent directories if needed
        if let Some(parent) = resolved_path.parent()
            && !parent.exists()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            warn!("Failed to create scratchpad directory: {}", e);
            return;
        }

        let mut file = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&resolved_path)
        {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to open scratchpad for guidance persistence: {}", e);
                return;
            }
        };

        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        for event in guidance_events {
            let entry = format!(
                "\n### HUMAN GUIDANCE ({})\n\n{}\n",
                timestamp, event.payload
            );
            if let Err(e) = file.write_all(entry.as_bytes()) {
                warn!("Failed to write guidance to scratchpad: {}", e);
            }
        }

        info!(
            count = guidance_events.len(),
            "Persisted human guidance to scratchpad"
        );
    }

    /// Injects cached guidance into the next prompt build.
    fn apply_robot_guidance(&mut self) {
        if self.robot_guidance.is_empty() {
            return;
        }

        self.ralph.set_robot_guidance(self.robot_guidance.clone());
    }

    /// Prepends auto-injected skill content to the prompt.
    ///
    /// This generalizes the former `prepend_memories()` into a skill auto-injection
    /// pipeline that handles memories, tools, and any other auto-inject skills.
    ///
    /// Injection order:
    /// 1. Memory data + ralph-tools skill (special case: loads memory data from store, applies budget)
    /// 2. RObot interaction skill (gated by `robot.enabled`)
    /// 3. Other auto-inject skills from the registry (wrapped in XML tags)
    fn prepend_auto_inject_skills(&self, prompt: String) -> String {
        let mut prefix = String::new();

        // 1. Memory data + ralph-tools skill — special case with data loading
        self.inject_memories_and_tools_skill(&mut prefix);

        // 2. RObot interaction skill — gated by robot.enabled
        self.inject_robot_skill(&mut prefix);

        // 3. Other auto-inject skills from the registry
        self.inject_custom_auto_skills(&mut prefix);

        if prefix.is_empty() {
            return prompt;
        }

        prefix.push_str("\n\n");
        prefix.push_str(&prompt);
        prefix
    }

    /// Injects memory data and the ralph-tools skill into the prefix.
    ///
    /// Special case: loads memory entries from the store, applies budget
    /// truncation, then appends the ralph-tools skill content (which covers
    /// both tasks and memories CLI usage).
    /// Memory data is gated by `memories.enabled && memories.inject == Auto`.
    /// The ralph-tools skill is injected when either memories or tasks are enabled.
    fn inject_memories_and_tools_skill(&self, prefix: &mut String) {
        let memories_config = &self.config.memories;

        // Inject memory DATA if memories are enabled with auto-inject
        if memories_config.enabled && memories_config.inject == InjectMode::Auto {
            info!(
                "Memory injection check: enabled={}, inject={:?}, workspace_root={:?}",
                memories_config.enabled, memories_config.inject, self.config.core.workspace_root
            );

            let workspace_root = &self.config.core.workspace_root;
            let store = MarkdownMemoryStore::with_default_path(workspace_root);
            let memories_path = workspace_root.join(".ralph/agent/memories.md");

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
                    Vec::new()
                }
            };

            if memories.is_empty() {
                info!("Memory store is empty - no memories to inject");
            } else {
                let mut memories_content = format_memories_as_markdown(&memories);

                if memories_config.budget > 0 {
                    let original_len = memories_content.len();
                    memories_content =
                        truncate_to_budget(&memories_content, memories_config.budget);
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

                prefix.push_str(&memories_content);
            }
        }

        // Inject the ralph-tools skill when either memories or tasks are enabled
        if memories_config.enabled || self.config.tasks.enabled {
            if let Some(skill) = self.skill_registry.get("ralph-tools") {
                if !prefix.is_empty() {
                    prefix.push_str("\n\n");
                }
                prefix.push_str(&format!(
                    "<ralph-tools-skill>\n{}\n</ralph-tools-skill>",
                    skill.content.trim()
                ));
                debug!("Injected ralph-tools skill from registry");
            } else {
                debug!("ralph-tools skill not found in registry - skill content not injected");
            }
        }
    }

    /// Injects the RObot interaction skill content into the prefix.
    ///
    /// Gated by `robot.enabled`. Teaches agents how and when to interact
    /// with humans via `human.interact` events.
    fn inject_robot_skill(&self, prefix: &mut String) {
        if !self.config.robot.enabled {
            return;
        }

        if let Some(skill) = self.skill_registry.get("robot-interaction") {
            if !prefix.is_empty() {
                prefix.push_str("\n\n");
            }
            prefix.push_str(&format!(
                "<robot-skill>\n{}\n</robot-skill>",
                skill.content.trim()
            ));
            debug!("Injected robot interaction skill from registry");
        }
    }

    /// Injects any user-configured auto-inject skills (excluding built-in ralph-tools/robot-interaction).
    fn inject_custom_auto_skills(&self, prefix: &mut String) {
        for skill in self.skill_registry.auto_inject_skills(None) {
            // Skip built-in skills handled above
            if skill.name == "ralph-tools" || skill.name == "robot-interaction" {
                continue;
            }

            if !prefix.is_empty() {
                prefix.push_str("\n\n");
            }
            prefix.push_str(&format!(
                "<{name}-skill>\n{content}\n</{name}-skill>",
                name = skill.name,
                content = skill.content.trim()
            ));
            debug!("Injected auto-inject skill: {}", skill.name);
        }
    }

    /// Prepends scratchpad content to the prompt if the file exists and is non-empty.
    ///
    /// The scratchpad is the agent's working memory for the current objective.
    /// Auto-injecting saves one tool call per iteration.
    /// When the file exceeds the budget, the TAIL is kept (most recent entries).
    fn prepend_scratchpad(&self, prompt: String) -> String {
        let scratchpad_path = self.scratchpad_path();

        let resolved_path = if scratchpad_path.is_relative() {
            self.config.core.workspace_root.join(&scratchpad_path)
        } else {
            scratchpad_path
        };

        if !resolved_path.exists() {
            debug!(
                "Scratchpad not found at {:?}, skipping injection",
                resolved_path
            );
            return prompt;
        }

        let content = match std::fs::read_to_string(&resolved_path) {
            Ok(c) => c,
            Err(e) => {
                info!("Failed to read scratchpad for injection: {}", e);
                return prompt;
            }
        };

        if content.trim().is_empty() {
            debug!("Scratchpad is empty, skipping injection");
            return prompt;
        }

        // Budget: 4000 tokens ~16000 chars. Keep the TAIL (most recent content).
        let char_budget = 4000 * 4;
        let content = if content.len() > char_budget {
            // Find a line boundary near the start of the tail (ensure UTF-8 boundary)
            let mut start = content.len() - char_budget;
            while start < content.len() && !content.is_char_boundary(start) {
                start += 1;
            }
            let line_start = content[start..].find('\n').map_or(start, |n| start + n + 1);
            let discarded = &content[..line_start];

            // Summarize discarded content by extracting markdown headings
            let headings: Vec<&str> = discarded
                .lines()
                .filter(|line| line.starts_with('#'))
                .collect();
            let summary = if headings.is_empty() {
                format!(
                    "<!-- earlier content truncated ({} chars omitted) -->",
                    line_start
                )
            } else {
                format!(
                    "<!-- earlier content truncated ({} chars omitted) -->\n\
                     <!-- discarded sections: {} -->",
                    line_start,
                    headings.join(" | ")
                )
            };

            format!("{}\n\n{}", summary, &content[line_start..])
        } else {
            content
        };

        info!("Injecting scratchpad ({} chars) into prompt", content.len());

        let mut final_prompt = format!(
            "<scratchpad path=\"{}\">\n{}\n</scratchpad>\n\n",
            self.config.core.scratchpad, content
        );
        final_prompt.push_str(&prompt);
        final_prompt
    }

    /// Prepends ready tasks to the prompt if tasks are enabled and any exist.
    ///
    /// Loads the task store and formats ready (unblocked, open) tasks into
    /// a `<ready-tasks>` XML block. This saves the agent a tool call per
    /// iteration and puts tasks at the same prominence as the scratchpad.
    fn prepend_ready_tasks(&self, prompt: String) -> String {
        if !self.config.tasks.enabled {
            return prompt;
        }

        use crate::task::TaskStatus;
        use crate::task_store::TaskStore;

        let tasks_path = self.tasks_path();
        let resolved_path = if tasks_path.is_relative() {
            self.config.core.workspace_root.join(&tasks_path)
        } else {
            tasks_path
        };

        if !resolved_path.exists() {
            return prompt;
        }

        let store = match TaskStore::load(&resolved_path) {
            Ok(s) => s,
            Err(e) => {
                info!("Failed to load task store for injection: {}", e);
                return prompt;
            }
        };

        let ready = store.ready();
        let open = store.open();
        let closed_count = store.all().len() - open.len();

        if open.is_empty() && closed_count == 0 {
            return prompt;
        }

        let mut section = String::from("<ready-tasks>\n");
        if ready.is_empty() && open.is_empty() {
            section.push_str("No open tasks. Create tasks with `ralph tools task add`.\n");
        } else {
            section.push_str(&format!(
                "## Tasks: {} ready, {} open, {} closed\n\n",
                ready.len(),
                open.len(),
                closed_count
            ));
            for task in &ready {
                let status_icon = match task.status {
                    TaskStatus::Open => "[ ]",
                    TaskStatus::InProgress => "[~]",
                    _ => "[?]",
                };
                section.push_str(&format!(
                    "- {} [P{}] {} ({})\n",
                    status_icon, task.priority, task.title, task.id
                ));
            }
            // Show blocked tasks separately so agent knows they exist
            let ready_ids: Vec<&str> = ready.iter().map(|t| t.id.as_str()).collect();
            let blocked: Vec<_> = open
                .iter()
                .filter(|t| !ready_ids.contains(&t.id.as_str()))
                .collect();
            if !blocked.is_empty() {
                section.push_str("\nBlocked:\n");
                for task in blocked {
                    section.push_str(&format!(
                        "- [blocked] [P{}] {} ({}) — blocked by: {}\n",
                        task.priority,
                        task.title,
                        task.id,
                        task.blocked_by.join(", ")
                    ));
                }
            }
        }
        section.push_str("</ready-tasks>\n\n");

        info!(
            "Injecting ready tasks ({} ready, {} open, {} closed) into prompt",
            ready.len(),
            open.len(),
            closed_count
        );

        let mut final_prompt = section;
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

    /// Returns a mutable reference to the event bus for direct event publishing.
    ///
    /// This is primarily used for planning sessions to inject user responses
    /// as events into the orchestration loop.
    pub fn bus(&mut self) -> &mut EventBus {
        &mut self.bus
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

        // Periodic robot check-in
        if let Some(interval_secs) = self.config.robot.checkin_interval_seconds
            && let Some(ref robot_service) = self.robot_service
        {
            let elapsed = self.state.elapsed();
            let interval = std::time::Duration::from_secs(interval_secs);
            let last = self
                .state
                .last_checkin_at
                .map(|t| t.elapsed())
                .unwrap_or(elapsed);

            if last >= interval {
                let context = self.build_checkin_context(hat_id);
                match robot_service.send_checkin(self.state.iteration, elapsed, Some(&context)) {
                    Ok(_) => {
                        self.state.last_checkin_at = Some(std::time::Instant::now());
                        debug!(iteration = self.state.iteration, "Sent robot check-in");
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to send robot check-in");
                    }
                }
            }
        }

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

        let _ = output;

        // Events are ONLY read from the JSONL file written by `ralph emit`.
        // This enforces tool use and prevents confabulation (agent claiming to emit without actually doing so).
        // See process_events_from_jsonl() for event processing.

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
        let scratchpad_path = self.scratchpad_path();

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

        let tasks_path = self.tasks_path();

        // No tasks file = no pending tasks = complete
        if !tasks_path.exists() {
            return Ok(true);
        }

        let store = TaskStore::load(&tasks_path)?;
        Ok(!store.has_pending_tasks())
    }

    /// Builds a [`CheckinContext`] with current loop state for robot check-ins.
    fn build_checkin_context(&self, hat_id: &HatId) -> CheckinContext {
        let (open_tasks, closed_tasks) = self.count_tasks();
        CheckinContext {
            current_hat: Some(hat_id.as_str().to_string()),
            open_tasks,
            closed_tasks,
            cumulative_cost: self.state.cumulative_cost,
        }
    }

    /// Counts open and closed tasks from the task store.
    ///
    /// Returns `(open_count, closed_count)`. "Open" means non-terminal tasks,
    /// "closed" means tasks with `TaskStatus::Closed`.
    fn count_tasks(&self) -> (usize, usize) {
        use crate::task::TaskStatus;
        use crate::task_store::TaskStore;

        let tasks_path = self.tasks_path();
        if !tasks_path.exists() {
            return (0, 0);
        }

        match TaskStore::load(&tasks_path) {
            Ok(store) => {
                let total = store.all().len();
                let open = store.open().len();
                let closed = total - open;
                // Verify: closed should match Closed status count
                debug_assert_eq!(
                    closed,
                    store
                        .all()
                        .iter()
                        .filter(|t| t.status == TaskStatus::Closed)
                        .count()
                );
                (open, closed)
            }
            Err(_) => (0, 0),
        }
    }

    /// Returns a list of open task descriptions for logging purposes.
    fn get_open_task_list(&self) -> Vec<String> {
        use crate::task_store::TaskStore;

        let tasks_path = self.tasks_path();
        if let Ok(store) = TaskStore::load(&tasks_path) {
            return store
                .open()
                .iter()
                .map(|t| format!("{}: {}", t.id, t.title))
                .collect();
        }
        vec![]
    }

    fn warn_on_mutation_evidence(&self, evidence: &crate::event_parser::BackpressureEvidence) {
        let threshold = self.config.event_loop.mutation_score_warn_threshold;

        match &evidence.mutants {
            Some(mutants) => {
                if let Some(reason) = Self::mutation_warning_reason(mutants, threshold) {
                    warn!(
                        reason = %reason,
                        mutants_status = ?mutants.status,
                        mutants_score = mutants.score_percent,
                        mutants_threshold = threshold,
                        "Mutation testing warning"
                    );
                }
            }
            None => {
                if let Some(threshold) = threshold {
                    warn!(
                        mutants_threshold = threshold,
                        "Mutation testing warning: missing mutation evidence in build.done payload"
                    );
                }
            }
        }
    }

    fn mutation_warning_reason(
        mutants: &MutationEvidence,
        threshold: Option<f64>,
    ) -> Option<String> {
        match mutants.status {
            MutationStatus::Fail => Some("mutation testing failed".to_string()),
            MutationStatus::Warn => Some(Self::format_mutation_message(
                "mutation score below threshold",
                mutants.score_percent,
            )),
            MutationStatus::Unknown => Some("mutation testing status unknown".to_string()),
            MutationStatus::Pass => {
                let threshold = threshold?;

                match mutants.score_percent {
                    Some(score) if score < threshold => Some(format!(
                        "mutation score {:.2}% below threshold {:.2}%",
                        score, threshold
                    )),
                    Some(_) => None,
                    None => Some(format!(
                        "mutation score missing (threshold {:.2}%)",
                        threshold
                    )),
                }
            }
        }
    }

    fn format_mutation_message(message: &str, score: Option<f64>) -> String {
        match score {
            Some(score) => format!("{message} ({score:.2}%)"),
            None => message.to_string(),
        }
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

        // Validate and transform events (apply backpressure for build.done)
        let mut validated_events = Vec::new();
        let completion_topic = self.config.event_loop.completion_promise.as_str();
        let total_events = result.events.len();
        for (index, event) in result.events.into_iter().enumerate() {
            let payload = event.payload.clone().unwrap_or_default();

            if event.topic == completion_topic {
                if index + 1 == total_events {
                    self.state.completion_requested = true;
                    self.diagnostics.log_orchestration(
                        self.state.iteration,
                        "jsonl",
                        crate::diagnostics::OrchestrationEvent::EventPublished {
                            topic: event.topic.clone(),
                        },
                    );
                    info!(
                        topic = %event.topic,
                        "Completion event detected in JSONL"
                    );
                } else {
                    warn!(
                        topic = %event.topic,
                        index = index,
                        total_events = total_events,
                        "Completion event ignored because it was not the last event"
                    );
                }
                continue;
            }

            if event.topic == "build.done" {
                // Validate build.done events have backpressure evidence
                if let Some(evidence) = EventParser::parse_backpressure_evidence(&payload) {
                    if evidence.all_passed() {
                        self.warn_on_mutation_evidence(&evidence);
                        validated_events.push(Event::new(event.topic.as_str(), &payload));
                    } else {
                        // Evidence present but checks failed - synthesize build.blocked
                        warn!(
                            tests = evidence.tests_passed,
                            lint = evidence.lint_passed,
                            typecheck = evidence.typecheck_passed,
                            audit = evidence.audit_passed,
                            coverage = evidence.coverage_passed,
                            complexity = evidence.complexity_score,
                            duplication = evidence.duplication_passed,
                            performance = evidence.performance_regression,
                            specs = evidence.specs_verified,
                            "build.done rejected: backpressure checks failed"
                        );

                        let complexity = evidence
                            .complexity_score
                            .map(|value| format!("{value:.2}"))
                            .unwrap_or_else(|| "missing".to_string());
                        let performance = match evidence.performance_regression {
                            Some(true) => "regression".to_string(),
                            Some(false) => "pass".to_string(),
                            None => "missing".to_string(),
                        };
                        let specs = match evidence.specs_verified {
                            Some(true) => "pass".to_string(),
                            Some(false) => "fail".to_string(),
                            None => "not reported".to_string(),
                        };

                        self.diagnostics.log_orchestration(
                            self.state.iteration,
                            "jsonl",
                            crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                                reason: format!(
                                    "backpressure checks failed: tests={}, lint={}, typecheck={}, audit={}, coverage={}, complexity={}, duplication={}, performance={}, specs={}",
                                    evidence.tests_passed,
                                    evidence.lint_passed,
                                    evidence.typecheck_passed,
                                    evidence.audit_passed,
                                    evidence.coverage_passed,
                                    complexity,
                                    evidence.duplication_passed,
                                    performance,
                                    specs
                                ),
                            },
                        );

                        validated_events.push(Event::new(
                            "build.blocked",
                            "Backpressure checks failed. Fix tests/lint/typecheck/audit/coverage/complexity/duplication/specs before emitting build.done.",
                        ));
                    }
                } else {
                    // No evidence found - synthesize build.blocked
                    warn!("build.done rejected: missing backpressure evidence");

                    self.diagnostics.log_orchestration(
                        self.state.iteration,
                        "jsonl",
                        crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                            reason: "missing backpressure evidence".to_string(),
                        },
                    );

                    validated_events.push(Event::new(
                        "build.blocked",
                        "Missing backpressure evidence. Include 'tests: pass', 'lint: pass', 'typecheck: pass', 'audit: pass', 'coverage: pass', 'complexity: <score>', 'duplication: pass', 'performance: pass' (optional), 'specs: pass' (optional) in build.done payload.",
                    ));
                }
            } else if event.topic == "review.done" {
                // Validate review.done events have verification evidence
                if let Some(evidence) = EventParser::parse_review_evidence(&payload) {
                    if evidence.is_verified() {
                        validated_events.push(Event::new(event.topic.as_str(), &payload));
                    } else {
                        // Evidence present but checks failed - synthesize review.blocked
                        warn!(
                            tests = evidence.tests_passed,
                            build = evidence.build_passed,
                            "review.done rejected: verification checks failed"
                        );

                        self.diagnostics.log_orchestration(
                            self.state.iteration,
                            "jsonl",
                            crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                                reason: format!(
                                    "review verification failed: tests={}, build={}",
                                    evidence.tests_passed, evidence.build_passed
                                ),
                            },
                        );

                        validated_events.push(Event::new(
                            "review.blocked",
                            "Review verification failed. Run tests and build before emitting review.done.",
                        ));
                    }
                } else {
                    // No evidence found - synthesize review.blocked
                    warn!("review.done rejected: missing verification evidence");

                    self.diagnostics.log_orchestration(
                        self.state.iteration,
                        "jsonl",
                        crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                            reason: "missing review verification evidence".to_string(),
                        },
                    );

                    validated_events.push(Event::new(
                        "review.blocked",
                        "Missing verification evidence. Include 'tests: pass' and 'build: pass' in review.done payload.",
                    ));
                }
            } else if event.topic == "verify.passed" {
                if let Some(report) = EventParser::parse_quality_report(&payload) {
                    if report.meets_thresholds() {
                        validated_events.push(Event::new(event.topic.as_str(), &payload));
                    } else {
                        let failed = report.failed_dimensions();
                        let reason = if failed.is_empty() {
                            "quality thresholds failed".to_string()
                        } else {
                            format!("quality thresholds failed: {}", failed.join(", "))
                        };

                        warn!(
                            failed_dimensions = ?failed,
                            "verify.passed rejected: quality thresholds failed"
                        );

                        self.diagnostics.log_orchestration(
                            self.state.iteration,
                            "jsonl",
                            crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                                reason,
                            },
                        );

                        validated_events.push(Event::new(
                            "verify.failed",
                            "Quality thresholds failed. Include quality.tests, quality.coverage, quality.lint, quality.audit, quality.mutation, quality.complexity with thresholds in verify.passed payload.",
                        ));
                    }
                } else {
                    // No quality report found - synthesize verify.failed
                    warn!("verify.passed rejected: missing quality report");

                    self.diagnostics.log_orchestration(
                        self.state.iteration,
                        "jsonl",
                        crate::diagnostics::OrchestrationEvent::BackpressureTriggered {
                            reason: "missing quality report".to_string(),
                        },
                    );

                    validated_events.push(Event::new(
                        "verify.failed",
                        "Missing quality report. Include quality.tests, quality.coverage, quality.lint, quality.audit, quality.mutation, quality.complexity in verify.passed payload.",
                    ));
                }
            } else if event.topic == "verify.failed" {
                if EventParser::parse_quality_report(&payload).is_none() {
                    warn!("verify.failed missing quality report");
                }
                validated_events.push(Event::new(event.topic.as_str(), &payload));
            } else {
                // Non-backpressure events pass through unchanged
                validated_events.push(Event::new(event.topic.as_str(), &payload));
            }
        }

        // Track build.blocked events for thrashing detection
        let blocked_events: Vec<_> = validated_events
            .iter()
            .filter(|e| e.topic == "build.blocked".into())
            .collect();

        for blocked_event in &blocked_events {
            let task_id = Self::extract_task_id(&blocked_event.payload);

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

                self.diagnostics.log_orchestration(
                    self.state.iteration,
                    "jsonl",
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
                );

                self.bus.publish(abandoned_event);
            }
        }

        // Track hat-level blocking for legacy thrashing detection
        let has_blocked_event = !blocked_events.is_empty();

        if has_blocked_event {
            self.state.consecutive_blocked += 1;
        } else {
            self.state.consecutive_blocked = 0;
            self.state.last_blocked_hat = None;
        }

        // Handle human.interact blocking behavior:
        // When a human.interact event is detected and robot service is active,
        // send the question and block until human.response or timeout.
        let mut response_event = None;
        let ask_human_idx = validated_events
            .iter()
            .position(|e| e.topic == "human.interact".into());

        if let Some(idx) = ask_human_idx {
            let ask_event = &validated_events[idx];
            let payload = ask_event.payload.clone();

            if let Some(ref robot_service) = self.robot_service {
                info!(
                    payload = %payload,
                    "human.interact event detected — sending question via robot service"
                );

                // Send the question (includes retry with exponential backoff)
                let send_ok = match robot_service.send_question(&payload) {
                    Ok(_message_id) => true,
                    Err(e) => {
                        warn!(
                            error = %e,
                            "Failed to send human.interact question after retries — treating as timeout"
                        );
                        // Log to diagnostics
                        self.diagnostics.log_error(
                            self.state.iteration,
                            "telegram",
                            crate::diagnostics::DiagnosticError::TelegramSendError {
                                operation: "send_question".to_string(),
                                error: e.to_string(),
                                retry_count: 3,
                            },
                        );
                        false
                    }
                };

                // Block: poll events file for human.response
                // Per spec, even on send failure we treat as timeout (continue without blocking)
                if send_ok {
                    // Read the active events path from the current-events marker,
                    // falling back to the default events.jsonl if not available.
                    let events_path = self
                        .loop_context
                        .as_ref()
                        .and_then(|ctx| {
                            std::fs::read_to_string(ctx.current_events_marker())
                                .ok()
                                .map(|s| ctx.workspace().join(s.trim()))
                        })
                        .or_else(|| {
                            std::fs::read_to_string(".ralph/current-events")
                                .ok()
                                .map(|s| PathBuf::from(s.trim()))
                        })
                        .unwrap_or_else(|| {
                            self.loop_context
                                .as_ref()
                                .map(|ctx| ctx.events_path())
                                .unwrap_or_else(|| PathBuf::from(".ralph/events.jsonl"))
                        });

                    match robot_service.wait_for_response(&events_path) {
                        Ok(Some(response)) => {
                            info!(
                                response = %response,
                                "Received human.response — continuing loop"
                            );
                            // Create a human.response event to inject into the bus
                            response_event = Some(Event::new("human.response", &response));
                        }
                        Ok(None) => {
                            warn!(
                                timeout_secs = robot_service.timeout_secs(),
                                "Human response timeout — continuing without response"
                            );
                        }
                        Err(e) => {
                            warn!(
                                error = %e,
                                "Error waiting for human response — continuing without response"
                            );
                        }
                    }
                }
            } else {
                debug!(
                    "human.interact event detected but no robot service active — passing through"
                );
            }
        }

        // Publish validated events to the bus.
        // Ralph is always registered with subscribe("*"), so every event has at least
        // one subscriber. Events without a specific hat subscriber are "orphaned" —
        // Ralph handles them as the universal fallback.
        for event in validated_events {
            self.diagnostics.log_orchestration(
                self.state.iteration,
                "jsonl",
                crate::diagnostics::OrchestrationEvent::EventPublished {
                    topic: event.topic.to_string(),
                },
            );

            if !self.registry.has_subscriber(event.topic.as_str()) {
                has_orphans = true;
            }

            debug!(
                topic = %event.topic,
                "Publishing event from JSONL"
            );
            self.bus.publish(event);
        }

        // Publish human.response event if one was received during blocking
        if let Some(response) = response_event {
            info!(
                topic = %response.topic,
                "Publishing human.response event from robot service"
            );
            self.bus.publish(response);
        }

        Ok(has_orphans)
    }

    /// Checks if output contains a completion event from Ralph.
    ///
    /// Completion must be emitted as an `<event>` tag, not plain text.
    pub fn check_ralph_completion(&self, output: &str) -> bool {
        let events = EventParser::new().parse(output);
        events
            .iter()
            .any(|event| event.topic.as_str() == self.config.event_loop.completion_promise)
    }

    /// Publishes the loop.terminate system event to observers.
    ///
    /// Per spec: "Published by the orchestrator (not agents) when the loop exits."
    /// This is an observer-only event—hats cannot trigger on it.
    ///
    /// Returns the event for logging purposes.
    pub fn publish_terminate_event(&mut self, reason: &TerminationReason) -> Event {
        // Stop the robot service if it was running
        self.stop_robot_service();

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

    /// Returns the robot service's shutdown flag, if active.
    ///
    /// Signal handlers can set this flag to interrupt `wait_for_response()`
    /// without waiting for the full timeout.
    pub fn robot_shutdown_flag(&self) -> Option<Arc<AtomicBool>> {
        self.robot_service.as_ref().map(|s| s.shutdown_flag())
    }

    /// Stops the robot service if it's running.
    ///
    /// Called during loop termination to cleanly shut down the communication backend.
    fn stop_robot_service(&mut self) {
        if let Some(service) = self.robot_service.take() {
            service.stop();
        }
    }

    // -------------------------------------------------------------------------
    // Human-in-the-loop planning support
    // -------------------------------------------------------------------------

    /// Check if any event is a `user.prompt` event.
    ///
    /// Returns the first user prompt event found, or None.
    pub fn check_for_user_prompt(&self, events: &[Event]) -> Option<UserPrompt> {
        events
            .iter()
            .find(|e| e.topic.as_str() == "user.prompt")
            .map(|e| UserPrompt {
                id: Self::extract_prompt_id(&e.payload),
                text: e.payload.clone(),
            })
    }

    /// Extract a prompt ID from the event payload.
    ///
    /// Supports both XML attribute format: `<event topic="user.prompt" id="q1">...</event>`
    /// and JSON format in payload.
    fn extract_prompt_id(payload: &str) -> String {
        // Try to extract id attribute from XML-like format first
        if let Some(start) = payload.find("id=\"")
            && let Some(end) = payload[start + 4..].find('"')
        {
            return payload[start + 4..start + 4 + end].to_string();
        }

        // Fallback: generate a simple ID based on timestamp
        format!("q{}", Self::generate_prompt_id())
    }

    /// Generate a simple unique ID for prompts.
    /// Uses timestamp-based generation since uuid crate isn't available.
    fn generate_prompt_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{:x}", nanos % 0xFFFF_FFFF)
    }
}

/// A user prompt that requires human input.
///
/// Created when the agent emits a `user.prompt` event during planning.
#[derive(Debug, Clone)]
pub struct UserPrompt {
    /// Unique identifier for this prompt (e.g., "q1", "q2")
    pub id: String,
    /// The prompt/question text
    pub text: String,
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
        TerminationReason::RestartRequested => "Restarting by human request.",
    }
}
