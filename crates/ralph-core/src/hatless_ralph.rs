//! Hatless Ralph - the constant coordinator.
//!
//! Ralph is always present, cannot be configured away, and acts as a universal fallback.

use crate::config::CoreConfig;
use crate::hat_registry::HatRegistry;
use ralph_proto::Topic;

/// Hatless Ralph - the constant coordinator.
pub struct HatlessRalph {
    completion_promise: String,
    core: CoreConfig,
    hat_topology: Option<HatTopology>,
}

/// Hat topology for multi-hat mode prompt generation.
pub struct HatTopology {
    hats: Vec<HatInfo>,
}

/// Information about a hat for prompt generation.
pub struct HatInfo {
    pub name: String,
    pub subscribes_to: Vec<String>,
    pub publishes: Vec<String>,
}

impl HatTopology {
    /// Creates topology from registry.
    pub fn from_registry(registry: &HatRegistry) -> Self {
        let hats = registry
            .all()
            .map(|hat| HatInfo {
                name: hat.name.clone(),
                subscribes_to: hat.subscriptions.iter().map(|t| t.as_str().to_string()).collect(),
                publishes: hat.publishes.iter().map(|t| t.as_str().to_string()).collect(),
            })
            .collect();

        Self { hats }
    }
}

impl HatlessRalph {
    /// Creates a new HatlessRalph.
    pub fn new(completion_promise: impl Into<String>, core: CoreConfig, registry: &HatRegistry) -> Self {
        let hat_topology = if registry.is_empty() {
            None
        } else {
            Some(HatTopology::from_registry(registry))
        };

        Self {
            completion_promise: completion_promise.into(),
            core,
            hat_topology,
        }
    }

    /// Builds Ralph's prompt based on context.
    pub fn build_prompt(&self, _context: &str) -> String {
        let mut prompt = self.core_prompt();

        if let Some(topology) = &self.hat_topology {
            prompt.push_str(&self.multi_hat_section(topology));
        } else {
            prompt.push_str(&self.solo_mode_section());
        }

        prompt.push_str(&self.event_writing_section());
        prompt.push_str(&self.done_section());

        prompt
    }

    /// Always returns true - Ralph handles all events as fallback.
    pub fn should_handle(&self, _topic: &Topic) -> bool {
        true
    }

    fn core_prompt(&self) -> String {
        let guardrails = self
            .core
            .guardrails
            .iter()
            .map(|g| format!("- {g}"))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r"You are Ralph. You're the coordinator.

## CORE BEHAVIORS
**Scratchpad:** `{scratchpad}` is shared state. Read it. Update it.
**Specs:** `{specs_dir}` is the source of truth. Implementations must match.
**Backpressure:** Tests/typecheck/lint must pass.

### Guardrails
{guardrails}

",
            scratchpad = self.core.scratchpad,
            specs_dir = self.core.specs_dir,
            guardrails = guardrails,
        )
    }

    fn solo_mode_section(&self) -> String {
        r"## SOLO MODE

You're doing everything yourself. Plan, implement, validate.

1. **Gap analysis.** Compare specs against codebase.
2. **Own the scratchpad.** Create/update with prioritized tasks.
3. **Implement.** Pick ONE task, write code, validate.
4. **Commit.** Mark `[x]` in scratchpad.
5. **Repeat** until all tasks done.

"
        .to_string()
    }

    fn multi_hat_section(&self, topology: &HatTopology) -> String {
        let mut section = String::from("## MULTI-HAT MODE\n\nYou coordinate a team. Delegate to hats or handle yourself.\n\n### MY TEAM\n\n");

        // Build hat table
        section.push_str("| Hat | Subscribes To | Publishes |\n");
        section.push_str("|-----|---------------|----------|\n");

        for hat in &topology.hats {
            let subscribes = hat.subscribes_to.join(", ");
            let publishes = hat.publishes.join(", ");
            section.push_str(&format!("| {} | {} | {} |\n", hat.name, subscribes, publishes));
        }

        section.push_str("\n**Your role:** Catch orphaned events, coordinate work, ensure completion.\n\n");
        section
    }

    fn event_writing_section(&self) -> String {
        format!(
            r#"## EVENT WRITING

Write events to `{events_file}` as:
{{"topic": "build.task", "payload": "...", "ts": "2026-01-14T12:00:00Z"}}

"#,
            events_file = ".agent/events.jsonl"
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
    fn test_solo_mode_prompt() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new(); // Empty registry
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry);

        let prompt = ralph.build_prompt("");

        assert!(prompt.contains("You are Ralph. You're the coordinator."));
        assert!(prompt.contains("## CORE BEHAVIORS"));
        assert!(prompt.contains("## SOLO MODE"));
        assert!(prompt.contains("You're doing everything yourself"));
        assert!(!prompt.contains("## MULTI-HAT MODE"));
        assert!(prompt.contains("## EVENT WRITING"));
        assert!(prompt.contains(".agent/events.jsonl"));
        assert!(prompt.contains("LOOP_COMPLETE"));
    }

    #[test]
    fn test_multi_hat_mode_prompt() {
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
        let registry = HatRegistry::from_config(&config);
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry);

        let prompt = ralph.build_prompt("");

        assert!(prompt.contains("You are Ralph. You're the coordinator."));
        assert!(prompt.contains("## CORE BEHAVIORS"));
        assert!(prompt.contains("## MULTI-HAT MODE"));
        assert!(prompt.contains("### MY TEAM"));
        assert!(prompt.contains("| Hat | Subscribes To | Publishes |"));
        assert!(!prompt.contains("## SOLO MODE"));
        assert!(prompt.contains("## EVENT WRITING"));
        assert!(prompt.contains("LOOP_COMPLETE"));
    }

    #[test]
    fn test_should_handle_always_true() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry);

        assert!(ralph.should_handle(&Topic::new("any.topic")));
        assert!(ralph.should_handle(&Topic::new("build.task")));
        assert!(ralph.should_handle(&Topic::new("unknown.event")));
    }

    #[test]
    fn test_core_behaviors_always_present() {
        let config = RalphConfig::default();
        let registry = HatRegistry::new();
        let ralph = HatlessRalph::new("LOOP_COMPLETE", config.core.clone(), &registry);

        let prompt = ralph.build_prompt("");

        assert!(prompt.contains("**Scratchpad:**"));
        assert!(prompt.contains("**Specs:**"));
        assert!(prompt.contains("**Backpressure:**"));
        assert!(prompt.contains("### Guardrails"));
    }
}
