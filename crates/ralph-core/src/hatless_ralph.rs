//! Hatless Ralph - the constant coordinator.
//!
//! Ralph is always present, cannot be configured away, and acts as a universal fallback.

use crate::config::CoreConfig;
use crate::hat_registry::HatRegistry;
use ralph_proto::Topic;
use std::path::Path;

/// Hatless Ralph - the constant coordinator.
pub struct HatlessRalph {
    completion_promise: String,
    core: CoreConfig,
    hat_topology: Option<HatTopology>,
    /// Event to publish after coordination to start the hat workflow.
    starting_event: Option<String>,
}

/// Hat topology for multi-hat mode prompt generation.
pub struct HatTopology {
    hats: Vec<HatInfo>,
}

/// Information about a hat for prompt generation.
pub struct HatInfo {
    pub name: String,
    pub description: String,
    pub subscribes_to: Vec<String>,
    pub publishes: Vec<String>,
    pub instructions: String,
}

impl HatTopology {
    /// Creates topology from registry.
    pub fn from_registry(registry: &HatRegistry) -> Self {
        let hats = registry
            .all()
            .map(|hat| HatInfo {
                name: hat.name.clone(),
                description: hat.description.clone(),
                subscribes_to: hat
                    .subscriptions
                    .iter()
                    .map(|t| t.as_str().to_string())
                    .collect(),
                publishes: hat
                    .publishes
                    .iter()
                    .map(|t| t.as_str().to_string())
                    .collect(),
                instructions: hat.instructions.clone(),
            })
            .collect();

        Self { hats }
    }
}

impl HatlessRalph {
    /// Creates a new HatlessRalph.
    ///
    /// # Arguments
    /// * `completion_promise` - String that signals loop completion
    /// * `core` - Core configuration (scratchpad, specs_dir, guardrails)
    /// * `registry` - Hat registry for topology generation
    /// * `starting_event` - Optional event to publish after coordination to start hat workflow
    pub fn new(
        completion_promise: impl Into<String>,
        core: CoreConfig,
        registry: &HatRegistry,
        starting_event: Option<String>,
    ) -> Self {
        let hat_topology = if registry.is_empty() {
            None
        } else {
            Some(HatTopology::from_registry(registry))
        };

        Self {
            completion_promise: completion_promise.into(),
            core,
            hat_topology,
            starting_event,
        }
    }

    /// Builds Ralph's prompt with filtered instructions for only active hats.
    ///
    /// This method reduces token usage by including instructions only for hats
    /// that are currently triggered by pending events, while still showing the
    /// full hat topology table for context.
    ///
    /// For solo mode (no hats), pass an empty slice: `&[]`
    pub fn build_prompt(&self, context: &str, active_hats: &[&ralph_proto::Hat]) -> String {
        let mut prompt = self.core_prompt();

        // Include pending events BEFORE workflow so Ralph sees the task first
        if !context.trim().is_empty() {
            prompt.push_str("## PENDING EVENTS\n\n");
            prompt.push_str(context);
            prompt.push_str("\n\n");
        }

        // Check if any active hat has custom instructions
        // If so, skip the generic workflow - the hat's instructions ARE the workflow
        let has_custom_workflow = active_hats
            .iter()
            .any(|h| !h.instructions.trim().is_empty());

        if !has_custom_workflow {
            prompt.push_str(&self.workflow_section());
        }

        if let Some(topology) = &self.hat_topology {
            prompt.push_str(&self.hats_section(topology, active_hats));
        }

        prompt.push_str(&self.event_writing_section());
        prompt.push_str(&self.done_section());

        prompt
    }

    /// Always returns true - Ralph handles all events as fallback.
    pub fn should_handle(&self, _topic: &Topic) -> bool {
        true
    }

    /// Checks if this is a fresh start (starting_event set, no scratchpad).
    ///
    /// Used to enable fast path delegation that skips the PLAN step
    /// when immediate delegation to specialized hats is appropriate.
    fn is_fresh_start(&self) -> bool {
        // Fast path only applies when starting_event is configured
        if self.starting_event.is_none() {
            return false;
        }

        // Check if scratchpad exists
        let path = Path::new(&self.core.scratchpad);
        !path.exists()
    }

    fn core_prompt(&self) -> String {
        let guardrails = self
            .core
            .guardrails
            .iter()
            .enumerate()
            .map(|(i, g)| format!("{}. {g}", 999 + i))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r"I'm Ralph. Fresh context each iteration.

### 0a. ORIENTATION
Study `{specs_dir}` to understand requirements.
Don't assume features aren't implemented—search first.

### 0b. SCRATCHPAD
Study `{scratchpad}`. It's shared state. It's memory.

Task markers:
- `[ ]` pending
- `[x]` done
- `[~]` cancelled (with reason)

### GUARDRAILS
{guardrails}

",
            scratchpad = self.core.scratchpad,
            specs_dir = self.core.specs_dir,
            guardrails = guardrails,
        )
    }

    fn workflow_section(&self) -> String {
        // Different workflow for solo mode vs multi-hat mode
        if self.hat_topology.is_some() {
            // Check for fast path: starting_event set AND no scratchpad
            if self.is_fresh_start() {
                // Fast path: immediate delegation without planning
                return format!(
                    r"## WORKFLOW

**FAST PATH**: Publish `{}` immediately to start the hat workflow.
Do not plan or analyze — delegate now.

",
                    self.starting_event.as_ref().unwrap()
                );
            }

            // Multi-hat mode: Ralph coordinates and delegates
            format!(
                r"## WORKFLOW

### 1. PLAN
Update `{scratchpad}` with prioritized tasks.

### 2. DELEGATE
You have one job. Publish ONE event to hand off to specialized hats. Do
NOT do any work.

",
                scratchpad = self.core.scratchpad
            )
        } else {
            // Solo mode: Ralph does everything
            format!(
                r"## WORKFLOW

### 1. Study the prompt. 
Study, explore, and research what needs to be done. Use parallel subagents (up to 10) for searches.

### 2. PLAN
Update `{scratchpad}` with prioritized tasks.

### 3. IMPLEMENT
Pick ONE task. Only 1 subagent for build/tests.

### 4. COMMIT
Capture the why, not just the what. Mark `[x]` in scratchpad.

### 5. REPEAT
Until all tasks `[x]` or `[~]`.

",
                scratchpad = self.core.scratchpad
            )
        }
    }

    fn hats_section(&self, topology: &HatTopology, active_hats: &[&ralph_proto::Hat]) -> String {
        let mut section = String::from("## HATS\n\nDelegate via events.\n\n");

        // Include starting_event instruction if configured
        if let Some(ref starting_event) = self.starting_event {
            section.push_str(&format!(
                "**After coordination, publish `{}` to start the workflow.**\n\n",
                starting_event
            ));
        }

        // Derive Ralph's triggers and publishes from topology
        // Ralph triggers on: task.start + all hats' publishes (results Ralph handles)
        // Ralph publishes: all hats' subscribes_to (events Ralph can emit to delegate)
        let mut ralph_triggers: Vec<&str> = vec!["task.start"];
        let mut ralph_publishes: Vec<&str> = Vec::new();

        for hat in &topology.hats {
            for pub_event in &hat.publishes {
                if !ralph_triggers.contains(&pub_event.as_str()) {
                    ralph_triggers.push(pub_event.as_str());
                }
            }
            for sub_event in &hat.subscribes_to {
                if !ralph_publishes.contains(&sub_event.as_str()) {
                    ralph_publishes.push(sub_event.as_str());
                }
            }
        }

        // Build hat table with Description column - ALWAYS shows ALL hats for context
        section.push_str("| Hat | Triggers On | Publishes | Description |\n");
        section.push_str("|-----|-------------|----------|-------------|\n");

        // Add Ralph coordinator row first
        section.push_str(&format!(
            "| Ralph | {} | {} | Coordinates workflow, delegates to specialized hats |\n",
            ralph_triggers.join(", "),
            ralph_publishes.join(", ")
        ));

        // Add all other hats
        for hat in &topology.hats {
            let subscribes = hat.subscribes_to.join(", ");
            let publishes = hat.publishes.join(", ");
            section.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                hat.name, subscribes, publishes, hat.description
            ));
        }

        section.push('\n');

        // Generate Mermaid topology diagram
        section.push_str(&self.generate_mermaid_diagram(topology, &ralph_publishes));
        section.push('\n');

        // Validate topology and log warnings for unreachable hats
        self.validate_topology_reachability(topology);

        // Add instructions sections ONLY for active hats
        // If the slice is empty, no instructions are added (no active hats)
        for active_hat in active_hats {
            if !active_hat.instructions.trim().is_empty() {
                section.push_str(&format!("### {} Instructions\n\n", active_hat.name));
                section.push_str(&active_hat.instructions);
                if !active_hat.instructions.ends_with('\n') {
                    section.push('\n');
                }
                section.push('\n');
            }
        }

        section
    }

    /// Generates a Mermaid flowchart showing event flow between hats.
    fn generate_mermaid_diagram(&self, topology: &HatTopology, ralph_publishes: &[&str]) -> String {
        let mut diagram = String::from("```mermaid\nflowchart LR\n");

        // Entry point: task.start -> Ralph
        diagram.push_str("    task.start((task.start)) --> Ralph\n");

        // Ralph -> hats (via ralph_publishes which are hat triggers)
        for hat in &topology.hats {
            for trigger in &hat.subscribes_to {
                if ralph_publishes.contains(&trigger.as_str()) {
                    // Sanitize hat name for Mermaid (remove emojis and special chars for node ID)
                    let node_id = hat
                        .name
                        .chars()
                        .filter(|c| c.is_alphanumeric())
                        .collect::<String>();
                    if node_id == hat.name {
                        diagram.push_str(&format!("    Ralph -->|{}| {}\n", trigger, hat.name));
                    } else {
                        // If name has special chars, use label syntax
                        diagram.push_str(&format!(
                            "    Ralph -->|{}| {}[{}]\n",
                            trigger, node_id, hat.name
                        ));
                    }
                }
            }
        }

        // Hats -> Ralph (via hat publishes)
        for hat in &topology.hats {
            let node_id = hat
                .name
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>();
            for pub_event in &hat.publishes {
                diagram.push_str(&format!("    {} -->|{}| Ralph\n", node_id, pub_event));
            }
        }

        // Hat -> Hat connections (when one hat publishes what another triggers on)
        for source_hat in &topology.hats {
            let source_id = source_hat
                .name
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>();
            for pub_event in &source_hat.publishes {
                for target_hat in &topology.hats {
                    if target_hat.name != source_hat.name
                        && target_hat.subscribes_to.contains(pub_event)
                    {
                        let target_id = target_hat
                            .name
                            .chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect::<String>();
                        diagram.push_str(&format!(
                            "    {} -->|{}| {}\n",
                            source_id, pub_event, target_id
                        ));
                    }
                }
            }
        }

        diagram.push_str("```\n");
        diagram
    }

    /// Validates that all hats are reachable from task.start.
    /// Logs warnings for unreachable hats but doesn't fail.
    fn validate_topology_reachability(&self, topology: &HatTopology) {
        use std::collections::HashSet;
        use tracing::warn;

        // Collect all events that are published (reachable)
        let mut reachable_events: HashSet<&str> = HashSet::new();
        reachable_events.insert("task.start");

        // Ralph publishes all hat triggers, so add those
        for hat in &topology.hats {
            for trigger in &hat.subscribes_to {
                reachable_events.insert(trigger.as_str());
            }
        }

        // Now add all events published by hats (they become reachable after hat runs)
        for hat in &topology.hats {
            for pub_event in &hat.publishes {
                reachable_events.insert(pub_event.as_str());
            }
        }

        // Check each hat's triggers - warn if none of them are reachable
        for hat in &topology.hats {
            let hat_reachable = hat
                .subscribes_to
                .iter()
                .any(|t| reachable_events.contains(t.as_str()));
            if !hat_reachable {
                warn!(
                    hat = %hat.name,
                    triggers = ?hat.subscribes_to,
                    "Hat has triggers that are never published - it may be unreachable"
                );
            }
        }
    }

    fn event_writing_section(&self) -> String {
        format!(
            r#"## EVENT WRITING

Events are **routing signals**, not data transport. Keep payloads brief.

**Use `ralph emit` to write events** (handles JSON escaping correctly):
```bash
ralph emit "build.done" "tests: pass, lint: pass"
ralph emit "review.done" --json '{{"status": "approved", "issues": 0}}'
```

⚠️ **NEVER use echo/cat to write events** — shell escaping breaks JSON.

For detailed output, write to `{scratchpad}` and emit a brief event.

**CRITICAL: STOP after publishing the event.** A new iteration will start
with fresh context to handle the work. Do NOT continue working in this
iteration — let the next iteration handle the event with the appropriate
hat persona. By doing the work now, you won't be wearing the correct hat 
the specialty to do an even better job.
"#,
            scratchpad = self.core.scratchpad
        )
    }

    fn done_section(&self) -> String {
        format!(
            r"## DONE

Output {} when all tasks complete.
",
            self.completion_promise
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RalphConfig;

    #[test]
    fn test_prompt_without_hats() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new(); // Empty registry
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        // Identity with ghuntley style
        assert!(prompt.contains("I'm Ralph. Fresh context each iteration."));

        // Numbered orientation phases
        assert!(prompt.contains("### 0a. ORIENTATION"));
        assert!(prompt.contains("Study"));
        assert!(prompt.contains("Don't assume features aren't implemented"));

        // Scratchpad section with task markers
        assert!(prompt.contains("### 0b. SCRATCHPAD"));
        assert!(prompt.contains("Task markers:"));
        assert!(prompt.contains("- `[ ]` pending"));
        assert!(prompt.contains("- `[x]` done"));
        assert!(prompt.contains("- `[~]` cancelled"));

        // Workflow with numbered steps (solo mode)
        assert!(prompt.contains("## WORKFLOW"));
        assert!(prompt.contains("### 1. Study the prompt"));
        assert!(prompt.contains("Use parallel subagents (up to 10)"));
        assert!(prompt.contains("### 2. PLAN"));
        assert!(prompt.contains("### 3. IMPLEMENT"));
        assert!(prompt.contains("Only 1 subagent for build/tests"));
        assert!(prompt.contains("### 4. COMMIT"));
        assert!(prompt.contains("Capture the why"));
        assert!(prompt.contains("### 5. REPEAT"));

        // Should NOT have hats section when no hats
        assert!(!prompt.contains("## HATS"));

        // Event writing and completion
        assert!(prompt.contains("## EVENT WRITING"));
        assert!(prompt.contains("ralph emit"));
        assert!(prompt.contains("NEVER use echo/cat"));
        assert!(prompt.contains("LOOP_COMPLETE"));
    }

    #[test]
    fn test_prompt_with_hats() {
        // Test multi-hat mode WITHOUT starting_event (no fast path)
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["planning.start", "build.done", "build.blocked"]
    publishes: ["build.task"]
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        // Note: No starting_event - tests normal multi-hat workflow (not fast path)
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        // Identity with ghuntley style
        assert!(prompt.contains("I'm Ralph. Fresh context each iteration."));

        // Orientation phases
        assert!(prompt.contains("### 0a. ORIENTATION"));
        assert!(prompt.contains("### 0b. SCRATCHPAD"));

        // Multi-hat workflow: PLAN + DELEGATE, not IMPLEMENT
        assert!(prompt.contains("## WORKFLOW"));
        assert!(prompt.contains("### 1. PLAN"));
        assert!(
            prompt.contains("### 2. DELEGATE"),
            "Multi-hat mode should have DELEGATE step"
        );
        assert!(
            !prompt.contains("### 3. IMPLEMENT"),
            "Multi-hat mode should NOT tell Ralph to implement"
        );
        assert!(
            prompt.contains("CRITICAL: STOP after publishing"),
            "Should explicitly tell Ralph to stop after publishing event"
        );

        // Hats section when hats are defined
        assert!(prompt.contains("## HATS"));
        assert!(prompt.contains("Delegate via events"));
        assert!(prompt.contains("| Hat | Triggers On | Publishes |"));

        // Event writing and completion
        assert!(prompt.contains("## EVENT WRITING"));
        assert!(prompt.contains("LOOP_COMPLETE"));
    }

    #[test]
    fn test_should_handle_always_true() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        assert!(ralph.should_handle(&Topic::new("any.topic")));
        assert!(ralph.should_handle(&Topic::new("build.task")));
        assert!(ralph.should_handle(&Topic::new("unknown.event")));
    }

    #[test]
    fn test_ghuntley_patterns_present() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        // Key ghuntley language patterns
        assert!(prompt.contains("Study"), "Should use 'study' verb");
        assert!(
            prompt.contains("Don't assume features aren't implemented"),
            "Should have 'don't assume' guardrail"
        );
        assert!(
            prompt.contains("parallel subagents"),
            "Should mention parallel subagents for reads"
        );
        assert!(
            prompt.contains("Only 1 subagent"),
            "Should limit to 1 subagent for builds"
        );
        assert!(
            prompt.contains("Capture the why"),
            "Should emphasize 'why' in commits"
        );

        // Numbered guardrails (999+)
        assert!(
            prompt.contains("### GUARDRAILS"),
            "Should have guardrails section"
        );
        assert!(
            prompt.contains("999."),
            "Guardrails should use high numbers"
        );
    }

    #[test]
    fn test_scratchpad_format_documented() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        // Task marker format is documented
        assert!(prompt.contains("- `[ ]` pending"));
        assert!(prompt.contains("- `[x]` done"));
        assert!(prompt.contains("- `[~]` cancelled (with reason)"));
    }

    #[test]
    fn test_starting_event_in_prompt() {
        // When starting_event is configured, prompt should include delegation instruction
        let yaml = r#"
hats:
  tdd_writer:
    name: "TDD Writer"
    triggers: ["tdd.start"]
    publishes: ["test.written"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new(
            "LOOP_COMPLETE",
            config.core.clone(),
            &registry,
            Some("tdd.start".to_string()),
        );

        let prompt = ralph.build_prompt("", &[]);

        // Should include delegation instruction
        assert!(
            prompt.contains("After coordination, publish `tdd.start` to start the workflow"),
            "Prompt should include starting_event delegation instruction"
        );
    }

    #[test]
    fn test_no_starting_event_instruction_when_none() {
        // When starting_event is None, no delegation instruction should appear
        let yaml = r#"
hats:
  some_hat:
    name: "Some Hat"
    triggers: ["some.event"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        // Should NOT include delegation instruction
        assert!(
            !prompt.contains("After coordination, publish"),
            "Prompt should NOT include starting_event delegation when None"
        );
    }

    #[test]
    fn test_hat_instructions_propagated_to_prompt() {
        // When a hat has instructions defined in config,
        // those instructions should appear in the generated prompt
        let yaml = r#"
hats:
  tdd_writer:
    name: "TDD Writer"
    triggers: ["tdd.start"]
    publishes: ["test.written"]
    instructions: |
      You are a Test-Driven Development specialist.
      Always write failing tests before implementation.
      Focus on edge cases and error handling.
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new(
            "LOOP_COMPLETE",
            config.core.clone(),
            &registry,
            Some("tdd.start".to_string()),
        );

        // Get the tdd_writer hat as active to see its instructions
        let tdd_writer = registry
            .get(&ralph_proto::HatId::new("tdd_writer"))
            .unwrap();
        let prompt = ralph.build_prompt("", &[tdd_writer]);

        // Instructions should appear in the prompt
        assert!(
            prompt.contains("### TDD Writer Instructions"),
            "Prompt should include hat instructions section header"
        );
        assert!(
            prompt.contains("Test-Driven Development specialist"),
            "Prompt should include actual instructions content"
        );
        assert!(
            prompt.contains("Always write failing tests"),
            "Prompt should include full instructions"
        );
    }

    #[test]
    fn test_empty_instructions_not_rendered() {
        // When a hat has empty/no instructions, no instructions section should appear
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        // No instructions section should appear for hats without instructions
        assert!(
            !prompt.contains("### Builder Instructions"),
            "Prompt should NOT include instructions section for hat with empty instructions"
        );
    }

    #[test]
    fn test_multiple_hats_with_instructions() {
        // When multiple hats have instructions, each should have its own section
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["planning.start"]
    publishes: ["build.task"]
    instructions: "Plan carefully before implementation."
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
    instructions: "Focus on clean, testable code."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        // Get both hats as active to see their instructions
        let planner = registry.get(&ralph_proto::HatId::new("planner")).unwrap();
        let builder = registry.get(&ralph_proto::HatId::new("builder")).unwrap();
        let prompt = ralph.build_prompt("", &[planner, builder]);

        // Both hats' instructions should appear
        assert!(
            prompt.contains("### Planner Instructions"),
            "Prompt should include Planner instructions section"
        );
        assert!(
            prompt.contains("Plan carefully before implementation"),
            "Prompt should include Planner instructions content"
        );
        assert!(
            prompt.contains("### Builder Instructions"),
            "Prompt should include Builder instructions section"
        );
        assert!(
            prompt.contains("Focus on clean, testable code"),
            "Prompt should include Builder instructions content"
        );
    }

    #[test]
    fn test_fast_path_with_starting_event() {
        // When starting_event is configured AND scratchpad doesn't exist,
        // should use fast path (skip PLAN step)
        let yaml = r#"
core:
  scratchpad: "/nonexistent/path/scratchpad.md"
hats:
  tdd_writer:
    name: "TDD Writer"
    triggers: ["tdd.start"]
    publishes: ["test.written"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new(
            "LOOP_COMPLETE",
            config.core.clone(),
            &registry,
            Some("tdd.start".to_string()),
        );

        let prompt = ralph.build_prompt("", &[]);

        // Should use fast path - immediate delegation
        assert!(
            prompt.contains("FAST PATH"),
            "Prompt should indicate fast path when starting_event set and no scratchpad"
        );
        assert!(
            prompt.contains("Publish `tdd.start` immediately"),
            "Prompt should instruct immediate event publishing"
        );
        assert!(
            !prompt.contains("### 1. PLAN"),
            "Fast path should skip PLAN step"
        );
    }

    #[test]
    fn test_events_context_included_in_prompt() {
        // Given a non-empty events context
        // When build_prompt(context) is called
        // Then the prompt contains ## PENDING EVENTS section with the context
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let events_context = r"[task.start] User's task: Review this code for security vulnerabilities
[build.done] Build completed successfully";

        let prompt = ralph.build_prompt(events_context, &[]);

        assert!(
            prompt.contains("## PENDING EVENTS"),
            "Prompt should contain PENDING EVENTS section"
        );
        assert!(
            prompt.contains("Review this code for security vulnerabilities"),
            "Prompt should contain the user's task"
        );
        assert!(
            prompt.contains("Build completed successfully"),
            "Prompt should contain all events from context"
        );
    }

    #[test]
    fn test_empty_context_no_pending_events_section() {
        // Given an empty events context
        // When build_prompt("") is called
        // Then no PENDING EVENTS section appears
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        assert!(
            !prompt.contains("## PENDING EVENTS"),
            "Empty context should not produce PENDING EVENTS section"
        );
    }

    #[test]
    fn test_whitespace_only_context_no_pending_events_section() {
        // Given a whitespace-only events context
        // When build_prompt is called
        // Then no PENDING EVENTS section appears
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("   \n\t  ", &[]);

        assert!(
            !prompt.contains("## PENDING EVENTS"),
            "Whitespace-only context should not produce PENDING EVENTS section"
        );
    }

    #[test]
    fn test_events_section_before_workflow() {
        // Given events context with a task
        // When prompt is built
        // Then ## PENDING EVENTS appears BEFORE ## WORKFLOW
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let events_context = "[task.start] Implement feature X";
        let prompt = ralph.build_prompt(events_context, &[]);

        let events_pos = prompt
            .find("## PENDING EVENTS")
            .expect("Should have PENDING EVENTS");
        let workflow_pos = prompt.find("## WORKFLOW").expect("Should have WORKFLOW");

        assert!(
            events_pos < workflow_pos,
            "PENDING EVENTS ({}) should come before WORKFLOW ({})",
            events_pos,
            workflow_pos
        );
    }

    // === Phase 3: Filtered Hat Instructions Tests ===

    #[test]
    fn test_only_active_hat_instructions_included() {
        // Scenario 4 from plan.md: Only active hat instructions included in prompt
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
    instructions: "Review code for security vulnerabilities."
  architecture_reviewer:
    name: "Architecture Reviewer"
    triggers: ["review.architecture"]
    instructions: "Review system design and architecture."
  correctness_reviewer:
    name: "Correctness Reviewer"
    triggers: ["review.correctness"]
    instructions: "Review logic and correctness."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        // Get active hats - only security_reviewer is active
        let security_hat = registry
            .get(&ralph_proto::HatId::new("security_reviewer"))
            .unwrap();
        let active_hats = vec![security_hat];

        let prompt = ralph.build_prompt("Event: review.security - Check auth", &active_hats);

        // Should contain ONLY security_reviewer instructions
        assert!(
            prompt.contains("### Security Reviewer Instructions"),
            "Should include Security Reviewer instructions section"
        );
        assert!(
            prompt.contains("Review code for security vulnerabilities"),
            "Should include Security Reviewer instructions content"
        );

        // Should NOT contain other hats' instructions
        assert!(
            !prompt.contains("### Architecture Reviewer Instructions"),
            "Should NOT include Architecture Reviewer instructions"
        );
        assert!(
            !prompt.contains("Review system design and architecture"),
            "Should NOT include Architecture Reviewer instructions content"
        );
        assert!(
            !prompt.contains("### Correctness Reviewer Instructions"),
            "Should NOT include Correctness Reviewer instructions"
        );
    }

    #[test]
    fn test_multiple_active_hats_all_included() {
        // Scenario 6 from plan.md: Multiple active hats includes all instructions
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
    instructions: "Review code for security vulnerabilities."
  architecture_reviewer:
    name: "Architecture Reviewer"
    triggers: ["review.architecture"]
    instructions: "Review system design and architecture."
  correctness_reviewer:
    name: "Correctness Reviewer"
    triggers: ["review.correctness"]
    instructions: "Review logic and correctness."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        // Get active hats - both security_reviewer and architecture_reviewer are active
        let security_hat = registry
            .get(&ralph_proto::HatId::new("security_reviewer"))
            .unwrap();
        let arch_hat = registry
            .get(&ralph_proto::HatId::new("architecture_reviewer"))
            .unwrap();
        let active_hats = vec![security_hat, arch_hat];

        let prompt = ralph.build_prompt("Events", &active_hats);

        // Should contain BOTH active hats' instructions
        assert!(
            prompt.contains("### Security Reviewer Instructions"),
            "Should include Security Reviewer instructions"
        );
        assert!(
            prompt.contains("Review code for security vulnerabilities"),
            "Should include Security Reviewer content"
        );
        assert!(
            prompt.contains("### Architecture Reviewer Instructions"),
            "Should include Architecture Reviewer instructions"
        );
        assert!(
            prompt.contains("Review system design and architecture"),
            "Should include Architecture Reviewer content"
        );

        // Should NOT contain inactive hat's instructions
        assert!(
            !prompt.contains("### Correctness Reviewer Instructions"),
            "Should NOT include Correctness Reviewer instructions"
        );
    }

    #[test]
    fn test_no_active_hats_no_instructions() {
        // No active hats = no instructions section (but topology table still present)
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
    instructions: "Review code for security vulnerabilities."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        // No active hats
        let active_hats: Vec<&ralph_proto::Hat> = vec![];

        let prompt = ralph.build_prompt("Events", &active_hats);

        // Should NOT contain any instructions
        assert!(
            !prompt.contains("### Security Reviewer Instructions"),
            "Should NOT include instructions when no active hats"
        );
        assert!(
            !prompt.contains("Review code for security vulnerabilities"),
            "Should NOT include instructions content when no active hats"
        );

        // But topology table should still be present
        assert!(prompt.contains("## HATS"), "Should still have HATS section");
        assert!(
            prompt.contains("| Hat | Triggers On | Publishes |"),
            "Should still have topology table"
        );
    }

    #[test]
    fn test_topology_table_always_present() {
        // Scenario 7 from plan.md: Full hat topology table always shown
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
    instructions: "Security instructions."
  architecture_reviewer:
    name: "Architecture Reviewer"
    triggers: ["review.architecture"]
    instructions: "Architecture instructions."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        // Only security_reviewer is active
        let security_hat = registry
            .get(&ralph_proto::HatId::new("security_reviewer"))
            .unwrap();
        let active_hats = vec![security_hat];

        let prompt = ralph.build_prompt("Events", &active_hats);

        // Topology table should show ALL hats (not just active ones)
        assert!(
            prompt.contains("| Security Reviewer |"),
            "Topology table should include Security Reviewer"
        );
        assert!(
            prompt.contains("| Architecture Reviewer |"),
            "Topology table should include Architecture Reviewer even though inactive"
        );
        assert!(
            prompt.contains("review.security"),
            "Topology table should show triggers"
        );
        assert!(
            prompt.contains("review.architecture"),
            "Topology table should show all triggers"
        );
    }
}
