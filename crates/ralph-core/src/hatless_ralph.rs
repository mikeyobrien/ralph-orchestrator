//! Hatless Ralph - the constant coordinator.
//!
//! Ralph is always present, cannot be configured away, and acts as a universal fallback.

use crate::config::CoreConfig;
use crate::hat_registry::HatRegistry;
use ralph_proto::Topic;
use std::collections::HashMap;
use std::path::Path;

/// Hatless Ralph - the constant coordinator.
pub struct HatlessRalph {
    completion_promise: String,
    core: CoreConfig,
    hat_topology: Option<HatTopology>,
    /// Event to publish after coordination to start the hat workflow.
    starting_event: Option<String>,
    /// Whether to include scratchpad instructions in the prompt.
    /// When memories are enabled, scratchpad is excluded (mutually exclusive).
    include_scratchpad: bool,
}

/// Hat topology for multi-hat mode prompt generation.
pub struct HatTopology {
    hats: Vec<HatInfo>,
}

/// Information about a hat that receives an event.
#[derive(Debug, Clone)]
pub struct EventReceiver {
    pub name: String,
    pub description: String,
}

/// Information about a hat for prompt generation.
pub struct HatInfo {
    pub name: String,
    pub description: String,
    pub subscribes_to: Vec<String>,
    pub publishes: Vec<String>,
    pub instructions: String,
    /// Maps each published event to the hats that receive it.
    pub event_receivers: HashMap<String, Vec<EventReceiver>>,
}

impl HatInfo {
    /// Generates an Event Publishing Guide section showing what happens when this hat publishes events.
    ///
    /// Returns `None` if the hat doesn't publish any events.
    pub fn event_publishing_guide(&self) -> Option<String> {
        if self.publishes.is_empty() {
            return None;
        }

        let mut guide = String::from(
            "### Event Publishing Guide\n\n\
             You MUST publish exactly ONE event when your work is complete.\n\
             Publishing hands off to the next hat and starts a fresh iteration with clear context.\n\n\
             When you publish:\n",
        );

        for pub_event in &self.publishes {
            let receivers = self.event_receivers.get(pub_event);
            let receiver_text = match receivers {
                Some(r) if !r.is_empty() => r
                    .iter()
                    .map(|recv| {
                        if recv.description.is_empty() {
                            recv.name.clone()
                        } else {
                            format!("{} ({})", recv.name, recv.description)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => "Ralph (coordinates next steps)".to_string(),
            };
            guide.push_str(&format!(
                "- `{}` → Received by: {}\n",
                pub_event, receiver_text
            ));
        }

        Some(guide)
    }
}

impl HatTopology {
    /// Creates topology from registry.
    pub fn from_registry(registry: &HatRegistry) -> Self {
        let hats = registry
            .all()
            .map(|hat| {
                // Compute who receives each event this hat publishes
                let event_receivers: HashMap<String, Vec<EventReceiver>> = hat
                    .publishes
                    .iter()
                    .map(|pub_topic| {
                        let receivers: Vec<EventReceiver> = registry
                            .subscribers(pub_topic)
                            .into_iter()
                            .filter(|h| h.id != hat.id) // Exclude self
                            .map(|h| EventReceiver {
                                name: h.name.clone(),
                                description: h.description.clone(),
                            })
                            .collect();
                        (pub_topic.as_str().to_string(), receivers)
                    })
                    .collect();

                HatInfo {
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
                    event_receivers,
                }
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
            include_scratchpad: true, // Default: include scratchpad
        }
    }

    /// Sets whether to include scratchpad instructions in the prompt.
    ///
    /// When memories are enabled, scratchpad should be excluded (mutually exclusive).
    pub fn with_scratchpad(mut self, include: bool) -> Self {
        self.include_scratchpad = include;
        self
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

        // Extract the original objective from task.start event
        let objective = self.extract_objective(context);

        // Add prominent OBJECTIVE section first
        if let Some(ref obj) = objective {
            prompt.push_str(&self.objective_section(obj));
        }

        // Include pending events BEFORE workflow so Ralph sees the task first
        if !context.trim().is_empty() {
            prompt.push_str("## PENDING EVENTS\n\n");
            prompt.push_str("You MUST handle these events in this iteration:\n\n");
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
        prompt.push_str(&self.done_section(objective.as_deref()));

        prompt
    }

    /// Extracts the original user objective from the task.start event in context.
    fn extract_objective(&self, context: &str) -> Option<String> {
        // Look for [task.start] event which contains the original user prompt
        for line in context.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[task.start]") {
                // Extract everything after [task.start]
                let payload = trimmed.strip_prefix("[task.start]")?.trim();
                if !payload.is_empty() {
                    return Some(payload.to_string());
                }
            }
        }
        None
    }

    /// Generates the OBJECTIVE section - the primary goal Ralph must achieve.
    fn objective_section(&self, objective: &str) -> String {
        format!(
            r"## OBJECTIVE

**This is your primary goal. All work must advance this objective.**

> {objective}

You MUST keep this objective in mind throughout the iteration.
You MUST NOT get distracted by workflow mechanics — they serve this goal.

",
            objective = objective
        )
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
        // Adapt guardrails based on whether scratchpad or memories mode is active
        let guardrails = self
            .core
            .guardrails
            .iter()
            .enumerate()
            .map(|(i, g)| {
                // Replace scratchpad reference with memories reference when memories are enabled
                let guardrail = if !self.include_scratchpad && g.contains("scratchpad is memory") {
                    g.replace(
                        "scratchpad is memory",
                        "save learnings to memories for next time",
                    )
                } else {
                    g.clone()
                };
                format!("{}. {guardrail}", 999 + i)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut prompt = format!(
            r"You are Ralph. You have fresh context each iteration.

### 0a. ORIENTATION
You MUST study `{specs_dir}` to understand requirements.
You MUST NOT assume features aren't implemented — search first.

",
            specs_dir = self.core.specs_dir,
        );

        // Include scratchpad section only when enabled (disabled when memories are active)
        if self.include_scratchpad {
            prompt.push_str(&format!(
                r"### 0b. SCRATCHPAD
You MUST study `{scratchpad}`. It is shared state and memory across iterations.

Task markers:
- `[ ]` pending
- `[x]` done
- `[~]` cancelled (with reason)

",
                scratchpad = self.core.scratchpad,
            ));
        } else {
            // When memories are enabled, include task tracking instructions
            prompt.push_str(
                "### 0b. TASKS

Runtime work tracking. For implementation planning, use code tasks (`tasks/*.code-task.md`).

**When you SHOULD create tasks:**
- You need to defer work (blocked, out of scope, lower priority)
- Dependencies exist between pieces of work (use `--blocked-by`)

**Commands:**
```bash
ralph tools task add 'Title' -p 2           # Create (priority 1-5, 1=highest)
ralph tools task add 'X' --blocked-by Y     # With dependency
ralph tools task list                        # All tasks
ralph tools task list -s open -d 7           # Open tasks from last 7 days
ralph tools task ready                       # Unblocked tasks only
ralph tools task close <id>                  # Mark complete (ONLY after verification)
```

You MUST NOT use echo/cat — use CLI tools only.

**CRITICAL: Task Closure Requirements**
You MUST NOT close a task unless ALL of these conditions are met:
1. The implementation is actually complete (not partially done)
2. Tests pass (run them and verify output)
3. Build succeeds (if applicable)
4. You have evidence of completion (command output, test results)

You MUST close all tasks before LOOP_COMPLETE. 

",
            );
        }

        // Add task breakdown guidance
        prompt.push_str(
            "### TASK BREAKDOWN\n\n\
- One task = one testable unit of work\n\
- Tasks should be completable in 1-2 iterations\n\
- Break large features into smaller tasks\n\
\n",
        );

        // Add state management guidance
        prompt.push_str(
            "### STATE MANAGEMENT\n\n\
**Memories** (`.agent/memories.md`) — Persistent learning:\n\
- Codebase patterns and conventions\n\
- Architectural decisions and rationale\n\
- Recurring problem solutions\n\
- Project-specific context\n\
\n\
**Context Files** (`.agent/*.md`) — Session-specific research:\n\
- Use descriptive names: `api-research.md`, `cli-ux-findings.md`\n\
- Store research, analysis, and temporary notes\n\
- Agent reads based on filename when needed\n\
- Not injected automatically (unlike memories)\n\
\n\
**When to use which:**\n\
- Memories: Knowledge that persists across sessions\n\
- Context files: Research/notes for current work session\n\
\n",
        );

        // List available context files in .agent/
        if let Ok(entries) = std::fs::read_dir(".agent") {
            let md_files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("md")
                        && path.file_name().and_then(|s| s.to_str()) != Some("memories.md")
                    {
                        path.file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();

            if !md_files.is_empty() {
                prompt.push_str("### AVAILABLE CONTEXT FILES\n\n");
                prompt.push_str("Context files in `.agent/` (read if relevant to current work):\n");
                for file in md_files {
                    prompt.push_str(&format!("- `.agent/{}`\n", file));
                }
                prompt.push('\n');
            }
        }

        prompt.push_str(&format!(
            r"### GUARDRAILS
{guardrails}

",
            guardrails = guardrails,
        ));

        prompt
    }

    fn workflow_section(&self) -> String {
        // Different workflow for solo mode vs multi-hat mode
        if self.hat_topology.is_some() {
            // Check for fast path: starting_event set AND no scratchpad
            if self.is_fresh_start() {
                // Fast path: immediate delegation without planning
                return format!(
                    r"## WORKFLOW

**FAST PATH**: You MUST publish `{}` immediately to start the hat workflow.
You MUST NOT plan or analyze — delegate now.

",
                    self.starting_event.as_ref().unwrap()
                );
            }

            // Multi-hat mode: Ralph coordinates and delegates
            if self.include_scratchpad {
                format!(
                    r"## WORKFLOW

### 1. PLAN
You MUST update `{scratchpad}` with prioritized tasks.

### 2. DELEGATE
You MUST publish exactly ONE event to hand off to specialized hats.
You MUST NOT do implementation work — delegation is your only job.

",
                    scratchpad = self.core.scratchpad
                )
            } else {
                // Memories mode: no scratchpad reference
                r"## WORKFLOW

### 1. PLAN
You MUST review memories and pending events to understand context.
You MUST create tasks with `ralph tools task add` to represent units of work.

### 2. DELEGATE
You MUST publish exactly ONE event to hand off ONE task to specialized hats.
You MUST NOT do implementation work — delegation is your only job.

"
                .to_string()
            }
        } else {
            // Solo mode: Ralph does everything
            if self.include_scratchpad {
                format!(
                    r"## WORKFLOW

### 1. Study the prompt.
You MUST study, explore, and research what needs to be done.
You MAY use parallel subagents (up to 10) for searches.

### 2. PLAN
You MUST update `{scratchpad}` with prioritized tasks.

### 3. IMPLEMENT
You MUST pick exactly ONE task to implement.
You MUST NOT use more than 1 subagent for build/tests.

### 4. COMMIT
You MUST capture the why, not just the what.
You MUST mark the task `[x]` in scratchpad when complete.

### 5. REPEAT
You MUST continue until all tasks are `[x]` or `[~]`.

",
                    scratchpad = self.core.scratchpad
                )
            } else {
                // Memories mode: no scratchpad reference, use tasks CLI
                r"## WORKFLOW

### 1. Study the prompt.
You MUST study, explore, and research what needs to be done.
You MAY use parallel subagents (up to 10) for searches.

### 2. PLAN
You MUST review memories for context.
You MUST create tasks with `ralph tools task add` for multi-step work.

### 3. IMPLEMENT
You MUST pick exactly ONE task from `ralph tools task ready`.
You MUST NOT use more than 1 subagent for build/tests.

### 4. VERIFY & COMMIT
You MUST run tests and verify the implementation works before closing.
You MUST NOT close a task without evidence of completion (test output, build success).
You MUST capture the why, not just the what.
You MUST close the task with `ralph tools task close` only AFTER verification passes.
You SHOULD save any learnings with `ralph tools memory add`.

### 5. EXIT
You MUST exit after completing ONE task.
The next iteration will continue with fresh context.

"
                .to_string()
            }
        }
    }

    fn hats_section(&self, topology: &HatTopology, active_hats: &[&ralph_proto::Hat]) -> String {
        let mut section = String::new();

        // When a specific hat is active, skip the topology overview (table + Mermaid)
        // The hat just needs its instructions and publishing guide
        if active_hats.is_empty() {
            // Ralph is coordinating - show full topology for delegation decisions
            section.push_str("## HATS\n\nDelegate via events.\n\n");

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

            // Build hat table with Description column
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
        } else {
            // Specific hat(s) active - minimal section with just instructions + guide
            section.push_str("## ACTIVE HAT\n\n");

            for active_hat in active_hats {
                // Find matching HatInfo from topology to access event_receivers
                let hat_info = topology.hats.iter().find(|h| h.name == active_hat.name);

                if !active_hat.instructions.trim().is_empty() {
                    section.push_str(&format!("### {} Instructions\n\n", active_hat.name));
                    section.push_str(&active_hat.instructions);
                    if !active_hat.instructions.ends_with('\n') {
                        section.push('\n');
                    }
                    section.push('\n');
                }

                // Add Event Publishing Guide after instructions (if hat publishes events)
                if let Some(guide) = hat_info.and_then(|info| info.event_publishing_guide()) {
                    section.push_str(&guide);
                    section.push('\n');
                }
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
        let detailed_output_hint = if self.include_scratchpad {
            format!(
                "You SHOULD write detailed output to `{}` and emit only a brief event.",
                self.core.scratchpad
            )
        } else {
            "You SHOULD create a memory with `ralph tools memory add` for detailed output and emit only a brief event."
                .to_string()
        };

        format!(
            r#"## EVENT WRITING

Events are routing signals, not data transport. You SHOULD keep payloads brief.

You MUST use `ralph emit` to write events (handles JSON escaping correctly):
```bash
ralph emit "build.done" "tests: pass, lint: pass"
ralph emit "review.done" --json '{{"status": "approved", "issues": 0}}'
```

You MUST NOT use echo/cat to write events because shell escaping breaks JSON.

{detailed_output_hint}

**Constraints:**
- You MUST stop working after publishing an event because a new iteration will start with fresh context
- You MUST NOT continue with additional work after publishing because the next iteration handles it with the appropriate hat persona
"#,
            detailed_output_hint = detailed_output_hint
        )
    }

    fn done_section(&self, objective: Option<&str>) -> String {
        let mut section = format!(
            r"## DONE

You MUST output {} when the objective is complete and all tasks are done.
",
            self.completion_promise
        );

        // Reinforce the objective at the end to bookend the prompt
        if let Some(obj) = objective {
            section.push_str(&format!(
                r"
**Remember your objective:**
> {}

Do not declare completion until this objective is fully satisfied.
",
                obj
            ));
        }

        section
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

        // Identity with RFC2119 style
        assert!(prompt.contains("You are Ralph. You have fresh context each iteration."));

        // Numbered orientation phases (RFC2119)
        assert!(prompt.contains("### 0a. ORIENTATION"));
        assert!(prompt.contains("MUST study"));
        assert!(prompt.contains("MUST NOT assume features aren't implemented"));

        // Scratchpad section with task markers
        assert!(prompt.contains("### 0b. SCRATCHPAD"));
        assert!(prompt.contains("Task markers:"));
        assert!(prompt.contains("- `[ ]` pending"));
        assert!(prompt.contains("- `[x]` done"));
        assert!(prompt.contains("- `[~]` cancelled"));

        // Workflow with numbered steps (solo mode) using RFC2119
        assert!(prompt.contains("## WORKFLOW"));
        assert!(prompt.contains("### 1. Study the prompt"));
        assert!(prompt.contains("You MAY use parallel subagents (up to 10)"));
        assert!(prompt.contains("### 2. PLAN"));
        assert!(prompt.contains("### 3. IMPLEMENT"));
        assert!(prompt.contains("You MUST NOT use more than 1 subagent for build/tests"));
        assert!(prompt.contains("### 4. COMMIT"));
        assert!(prompt.contains("You MUST capture the why"));
        assert!(prompt.contains("### 5. REPEAT"));

        // Should NOT have hats section when no hats
        assert!(!prompt.contains("## HATS"));

        // Event writing and completion using RFC2119
        assert!(prompt.contains("## EVENT WRITING"));
        assert!(prompt.contains("You MUST use `ralph emit`"));
        assert!(prompt.contains("You MUST NOT use echo/cat"));
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

        // Identity with RFC2119 style
        assert!(prompt.contains("You are Ralph. You have fresh context each iteration."));

        // Orientation phases
        assert!(prompt.contains("### 0a. ORIENTATION"));
        assert!(prompt.contains("### 0b. SCRATCHPAD"));

        // Multi-hat workflow: PLAN + DELEGATE, not IMPLEMENT (RFC2119)
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
            prompt.contains("You MUST stop working after publishing"),
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
    fn test_rfc2119_patterns_present() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        // Key RFC2119 language patterns
        assert!(
            prompt.contains("You MUST study"),
            "Should use RFC2119 MUST with 'study' verb"
        );
        assert!(
            prompt.contains("You MUST NOT assume features aren't implemented"),
            "Should have RFC2119 MUST NOT assume guardrail"
        );
        assert!(
            prompt.contains("You MAY use parallel subagents"),
            "Should mention parallel subagents with MAY"
        );
        assert!(
            prompt.contains("You MUST NOT use more than 1 subagent"),
            "Should limit to 1 subagent for builds with MUST NOT"
        );
        assert!(
            prompt.contains("You MUST capture the why"),
            "Should emphasize 'why' in commits with MUST"
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

        // Should use fast path - immediate delegation with RFC2119
        assert!(
            prompt.contains("FAST PATH"),
            "Prompt should indicate fast path when starting_event set and no scratchpad"
        );
        assert!(
            prompt.contains("You MUST publish `tdd.start` immediately"),
            "Prompt should instruct immediate event publishing with MUST"
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
    fn test_topology_table_only_when_ralph_coordinating() {
        // Topology table + Mermaid shown only when Ralph is coordinating (no active hats)
        // When a hat is active, skip the table to reduce token usage
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

        // Test 1: No active hats (Ralph coordinating) - should show table + Mermaid
        let prompt_coordinating = ralph.build_prompt("Events", &[]);

        assert!(
            prompt_coordinating.contains("## HATS"),
            "Should have HATS section when coordinating"
        );
        assert!(
            prompt_coordinating.contains("| Hat | Triggers On | Publishes |"),
            "Should have topology table when coordinating"
        );
        assert!(
            prompt_coordinating.contains("```mermaid"),
            "Should have Mermaid diagram when coordinating"
        );

        // Test 2: Active hat - should NOT show table/Mermaid, just instructions
        let security_hat = registry
            .get(&ralph_proto::HatId::new("security_reviewer"))
            .unwrap();
        let prompt_active = ralph.build_prompt("Events", &[security_hat]);

        assert!(
            prompt_active.contains("## ACTIVE HAT"),
            "Should have ACTIVE HAT section when hat is active"
        );
        assert!(
            !prompt_active.contains("| Hat | Triggers On | Publishes |"),
            "Should NOT have topology table when hat is active"
        );
        assert!(
            !prompt_active.contains("```mermaid"),
            "Should NOT have Mermaid diagram when hat is active"
        );
        assert!(
            prompt_active.contains("### Security Reviewer Instructions"),
            "Should still have the active hat's instructions"
        );
    }

    // === Memories/Scratchpad Exclusivity Tests ===

    #[test]
    fn test_scratchpad_included_by_default() {
        // By default, scratchpad instructions should be included
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let prompt = ralph.build_prompt("", &[]);

        assert!(
            prompt.contains("### 0b. SCRATCHPAD"),
            "Scratchpad section should be included by default"
        );
        assert!(
            prompt.contains("You MUST study `.agent/scratchpad.md`"),
            "Scratchpad path should be referenced with MUST"
        );
        assert!(
            prompt.contains("Task markers:"),
            "Task markers should be documented"
        );
    }

    #[test]
    fn test_scratchpad_excluded_when_disabled() {
        // When with_scratchpad(false), scratchpad instructions should be excluded
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(false);

        let prompt = ralph.build_prompt("", &[]);

        assert!(
            !prompt.contains("### 0b. SCRATCHPAD"),
            "Scratchpad section should NOT be included when disabled"
        );
        assert!(
            !prompt.contains("Task markers:"),
            "Task markers should NOT be documented when scratchpad disabled"
        );

        // But orientation should still be present
        assert!(
            prompt.contains("### 0a. ORIENTATION"),
            "Orientation should still be present"
        );
        assert!(
            prompt.contains("### GUARDRAILS"),
            "Guardrails should still be present"
        );
    }

    #[test]
    fn test_workflow_references_memories_when_scratchpad_disabled() {
        // When scratchpad is disabled, workflow should reference memories instead
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(false);

        let prompt = ralph.build_prompt("", &[]);

        // Workflow should mention memories, not scratchpad (RFC2119)
        assert!(
            prompt.contains("You MUST review memories"),
            "Workflow should reference memories with MUST when scratchpad disabled"
        );
        assert!(
            !prompt.contains("Update `.agent/scratchpad.md`"),
            "Workflow should NOT reference scratchpad when disabled"
        );
    }

    #[test]
    fn test_event_writing_references_memories_when_scratchpad_disabled() {
        // When scratchpad is disabled, event writing hints should reference memories
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(false);

        let prompt = ralph.build_prompt("", &[]);

        assert!(
            prompt.contains("ralph tools memory add"),
            "Event writing should mention ralph tools memory add when scratchpad disabled"
        );
    }

    #[test]
    fn test_multi_hat_mode_workflow_with_scratchpad_disabled() {
        // Multi-hat mode should also adapt workflow when scratchpad disabled
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(false);

        let prompt = ralph.build_prompt("", &[]);

        // Multi-hat workflow should mention memories (RFC2119)
        assert!(
            prompt.contains("You MUST review memories and pending events"),
            "Multi-hat workflow should reference memories with MUST when scratchpad disabled"
        );
        assert!(
            !prompt.contains("Update `.agent/scratchpad.md`"),
            "Multi-hat workflow should NOT reference scratchpad when disabled"
        );
    }

    #[test]
    fn test_guardrails_adapt_to_memories_mode() {
        // When scratchpad is disabled (memories enabled), guardrails should not mention scratchpad
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(false);

        let prompt = ralph.build_prompt("", &[]);

        assert!(
            !prompt.contains("scratchpad is memory"),
            "Guardrails should NOT mention 'scratchpad is memory' when memories enabled"
        );
        assert!(
            prompt.contains("save learnings to memories"),
            "Guardrails should encourage saving to memories when memories enabled"
        );
    }

    #[test]
    fn test_guardrails_mention_scratchpad_when_enabled() {
        // When scratchpad is enabled, guardrails should mention scratchpad
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(true);

        let prompt = ralph.build_prompt("", &[]);

        assert!(
            prompt.contains("scratchpad is memory"),
            "Guardrails should mention 'scratchpad is memory' when scratchpad enabled"
        );
        assert!(
            !prompt.contains("save learnings to memories"),
            "Guardrails should NOT mention memories when scratchpad enabled"
        );
    }

    // === Task Completion Verification Tests ===

    #[test]
    fn test_task_closure_verification_in_tasks_section() {
        // When memories/tasks mode is enabled, the TASKS section should include
        // verification requirements before closing tasks
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(false);

        let prompt = ralph.build_prompt("", &[]);

        // Should contain task closure verification requirements
        assert!(
            prompt.contains("CRITICAL: Task Closure Requirements"),
            "Should include CRITICAL task closure section"
        );
        assert!(
            prompt.contains("You MUST NOT close a task unless ALL"),
            "Should require verification before closing"
        );
        assert!(
            prompt.contains("implementation is actually complete"),
            "Should require complete implementation"
        );
        assert!(
            prompt.contains("Tests pass"),
            "Should require tests to pass"
        );
        assert!(
            prompt.contains("evidence of completion"),
            "Should require evidence"
        );
    }

    #[test]
    fn test_workflow_verify_and_commit_step() {
        // Solo mode with memories should have VERIFY & COMMIT step
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(false);

        let prompt = ralph.build_prompt("", &[]);

        // Should have VERIFY & COMMIT step (not just COMMIT)
        assert!(
            prompt.contains("### 4. VERIFY & COMMIT"),
            "Should have VERIFY & COMMIT step in workflow"
        );
        assert!(
            prompt
                .contains("You MUST run tests and verify the implementation works before closing"),
            "Should require verification before closing"
        );
        assert!(
            prompt.contains("You MUST NOT close a task without evidence of completion"),
            "Should require evidence before closing"
        );
        assert!(
            prompt.contains("only AFTER verification passes"),
            "Should emphasize closing only after verification"
        );
    }

    #[test]
    fn test_scratchpad_mode_still_has_commit_step() {
        // Scratchpad mode should still have commit step (but not task verification)
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None)
            .with_scratchpad(true);

        let prompt = ralph.build_prompt("", &[]);

        // Scratchpad mode uses different format - COMMIT step without task CLI
        assert!(
            prompt.contains("### 4. COMMIT"),
            "Should have COMMIT step in workflow"
        );
        assert!(
            prompt.contains("mark the task `[x]`"),
            "Should mark task in scratchpad"
        );
        // Scratchpad mode doesn't have the detailed task closure requirements
        assert!(
            !prompt.contains("CRITICAL: Task Closure Requirements"),
            "Scratchpad mode should not have CRITICAL task closure section"
        );
    }

    // === Objective Section Tests ===

    #[test]
    fn test_objective_section_present_with_task_start() {
        // When context contains [task.start], OBJECTIVE section should appear
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let context = "[task.start] Implement user authentication with JWT tokens";
        let prompt = ralph.build_prompt(context, &[]);

        assert!(
            prompt.contains("## OBJECTIVE"),
            "Should have OBJECTIVE section when task.start present"
        );
        assert!(
            prompt.contains("Implement user authentication with JWT tokens"),
            "OBJECTIVE should contain the original user prompt"
        );
        assert!(
            prompt.contains("This is your primary goal"),
            "OBJECTIVE should emphasize this is the primary goal"
        );
    }

    #[test]
    fn test_objective_reinforced_in_done_section() {
        // The objective should be restated in the DONE section (bookend pattern)
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let context = "[task.start] Fix the login bug in auth module";
        let prompt = ralph.build_prompt(context, &[]);

        // Check DONE section contains objective reinforcement
        let done_pos = prompt.find("## DONE").expect("Should have DONE section");
        let after_done = &prompt[done_pos..];

        assert!(
            after_done.contains("Remember your objective"),
            "DONE section should remind about objective"
        );
        assert!(
            after_done.contains("Fix the login bug in auth module"),
            "DONE section should restate the objective"
        );
    }

    #[test]
    fn test_objective_appears_before_pending_events() {
        // OBJECTIVE should appear BEFORE PENDING EVENTS for prominence
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let context = "[task.start] Build feature X";
        let prompt = ralph.build_prompt(context, &[]);

        let objective_pos = prompt.find("## OBJECTIVE").expect("Should have OBJECTIVE");
        let events_pos = prompt
            .find("## PENDING EVENTS")
            .expect("Should have PENDING EVENTS");

        assert!(
            objective_pos < events_pos,
            "OBJECTIVE ({}) should appear before PENDING EVENTS ({})",
            objective_pos,
            events_pos
        );
    }

    #[test]
    fn test_no_objective_without_task_start() {
        // When context has no task.start, no OBJECTIVE section should appear
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let context = "[build.done] Build completed successfully";
        let prompt = ralph.build_prompt(context, &[]);

        assert!(
            !prompt.contains("## OBJECTIVE"),
            "Should NOT have OBJECTIVE section without task.start"
        );
    }

    #[test]
    fn test_objective_extracted_correctly() {
        // Test that objective extraction handles various formats
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        // Test with whitespace
        let context = "  [task.start]   Review this PR for security issues  ";
        let prompt = ralph.build_prompt(context, &[]);

        assert!(
            prompt.contains("Review this PR for security issues"),
            "Should extract objective with trimmed whitespace"
        );
    }

    #[test]
    fn test_objective_with_multiple_events() {
        // When multiple events exist, objective is still extracted from task.start
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let context = r"[task.start] Implement feature Y
[build.done] Previous build succeeded
[test.passed] All tests green";
        let prompt = ralph.build_prompt(context, &[]);

        assert!(
            prompt.contains("## OBJECTIVE"),
            "Should have OBJECTIVE section"
        );
        assert!(
            prompt.contains("Implement feature Y"),
            "OBJECTIVE should contain the task.start payload"
        );
        // Should NOT include other events in objective
        assert!(
            !prompt.contains("## OBJECTIVE")
                || !prompt[..prompt.find("## PENDING EVENTS").unwrap_or(prompt.len())]
                    .contains("Previous build succeeded"),
            "OBJECTIVE should NOT include other event payloads"
        );
    }

    #[test]
    fn test_done_section_without_objective() {
        // When no objective, DONE section should still work but without reinforcement
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let context = "[build.done] Build completed";
        let prompt = ralph.build_prompt(context, &[]);

        assert!(prompt.contains("## DONE"), "Should have DONE section");
        assert!(
            prompt.contains("LOOP_COMPLETE"),
            "DONE should mention completion promise"
        );
        assert!(
            !prompt.contains("Remember your objective"),
            "Should NOT have objective reinforcement without task.start"
        );
    }

    // === Event Publishing Guide Tests ===

    #[test]
    fn test_event_publishing_guide_with_receivers() {
        // When a hat publishes events and other hats receive them,
        // the guide should show who receives each event
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    description: "Builds and tests code"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
  confessor:
    name: "Confessor"
    description: "Produces a ConfessionReport; rewarded for honesty"
    triggers: ["build.done"]
    publishes: ["confession.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        // Get the builder hat as active
        let builder = registry.get(&ralph_proto::HatId::new("builder")).unwrap();
        let prompt = ralph.build_prompt("[build.task] Build the feature", &[builder]);

        // Should include Event Publishing Guide
        assert!(
            prompt.contains("### Event Publishing Guide"),
            "Should include Event Publishing Guide section"
        );
        assert!(
            prompt.contains("When you publish:"),
            "Guide should explain what happens when publishing"
        );
        // build.done has a receiver (Confessor)
        assert!(
            prompt.contains("`build.done` → Received by: Confessor"),
            "Should show Confessor receives build.done"
        );
        assert!(
            prompt.contains("Produces a ConfessionReport; rewarded for honesty"),
            "Should include receiver's description"
        );
        // build.blocked has no receiver, so falls back to Ralph
        assert!(
            prompt.contains("`build.blocked` → Received by: Ralph (coordinates next steps)"),
            "Should show Ralph receives orphan events"
        );
    }

    #[test]
    fn test_event_publishing_guide_no_publishes() {
        // When a hat doesn't publish any events, no guide should appear
        let yaml = r#"
hats:
  observer:
    name: "Observer"
    description: "Only observes"
    triggers: ["events.*"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let observer = registry.get(&ralph_proto::HatId::new("observer")).unwrap();
        let prompt = ralph.build_prompt("[events.start] Start", &[observer]);

        // Should NOT include Event Publishing Guide
        assert!(
            !prompt.contains("### Event Publishing Guide"),
            "Should NOT include Event Publishing Guide when hat has no publishes"
        );
    }

    #[test]
    fn test_event_publishing_guide_all_orphan_events() {
        // When all published events have no receivers, all should show Ralph
        let yaml = r#"
hats:
  solo:
    name: "Solo"
    triggers: ["solo.start"]
    publishes: ["solo.done", "solo.failed"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let solo = registry.get(&ralph_proto::HatId::new("solo")).unwrap();
        let prompt = ralph.build_prompt("[solo.start] Go", &[solo]);

        assert!(
            prompt.contains("### Event Publishing Guide"),
            "Should include guide even for orphan events"
        );
        assert!(
            prompt.contains("`solo.done` → Received by: Ralph (coordinates next steps)"),
            "Orphan solo.done should go to Ralph"
        );
        assert!(
            prompt.contains("`solo.failed` → Received by: Ralph (coordinates next steps)"),
            "Orphan solo.failed should go to Ralph"
        );
    }

    #[test]
    fn test_event_publishing_guide_multiple_receivers() {
        // When an event has multiple receivers, all should be listed
        let yaml = r#"
hats:
  broadcaster:
    name: "Broadcaster"
    triggers: ["broadcast.start"]
    publishes: ["signal.sent"]
  listener1:
    name: "Listener1"
    description: "First listener"
    triggers: ["signal.sent"]
  listener2:
    name: "Listener2"
    description: "Second listener"
    triggers: ["signal.sent"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let broadcaster = registry
            .get(&ralph_proto::HatId::new("broadcaster"))
            .unwrap();
        let prompt = ralph.build_prompt("[broadcast.start] Go", &[broadcaster]);

        assert!(
            prompt.contains("### Event Publishing Guide"),
            "Should include guide"
        );
        // Both listeners should be mentioned (order may vary due to HashMap iteration)
        assert!(
            prompt.contains("Listener1 (First listener)"),
            "Should list Listener1 as receiver"
        );
        assert!(
            prompt.contains("Listener2 (Second listener)"),
            "Should list Listener2 as receiver"
        );
    }

    #[test]
    fn test_event_publishing_guide_excludes_self() {
        // If a hat subscribes to its own event, it should NOT be listed as receiver
        let yaml = r#"
hats:
  looper:
    name: "Looper"
    triggers: ["loop.continue", "loop.start"]
    publishes: ["loop.continue"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let looper = registry.get(&ralph_proto::HatId::new("looper")).unwrap();
        let prompt = ralph.build_prompt("[loop.start] Start", &[looper]);

        assert!(
            prompt.contains("### Event Publishing Guide"),
            "Should include guide"
        );
        // Self-reference should be excluded, so should fall back to Ralph
        assert!(
            prompt.contains("`loop.continue` → Received by: Ralph (coordinates next steps)"),
            "Self-subscription should be excluded, falling back to Ralph"
        );
    }

    #[test]
    fn test_event_publishing_guide_receiver_without_description() {
        // When a receiver has no description, just show the name
        let yaml = r#"
hats:
  sender:
    name: "Sender"
    triggers: ["send.start"]
    publishes: ["message.sent"]
  receiver:
    name: "NoDescReceiver"
    triggers: ["message.sent"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry, None);

        let sender = registry.get(&ralph_proto::HatId::new("sender")).unwrap();
        let prompt = ralph.build_prompt("[send.start] Go", &[sender]);

        assert!(
            prompt.contains("`message.sent` → Received by: NoDescReceiver"),
            "Should show receiver name without parentheses when no description"
        );
        // Should NOT have empty parentheses
        assert!(
            !prompt.contains("NoDescReceiver ()"),
            "Should NOT have empty parentheses for receiver without description"
        );
    }
}
