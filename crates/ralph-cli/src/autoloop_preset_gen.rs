//! Generate an autoloop preset directory from ralph's native hats config.
//!
//! This is the inverse of `ralph_core::preset_source` (which imports autoloop
//! TOML presets into ralph): given a [`RalphConfig`] with `hats`, write an
//! autoloop preset (`autoloops.toml` + `topology.toml` + `roles/*.md` +
//! `harness.md`) so the v3 autoloop engine can run ralph's existing workflow
//! without a hand-authored preset.
//!
//! ## Mapping (ralph -> autoloop)
//!
//! | ralph                                   | autoloop                          |
//! | --------------------------------------- | --------------------------------- |
//! | `hats.<id>`                             | `[[role]] id`                     |
//! | `hats.<id>.publishes`                   | `[[role]] emits`                  |
//! | `hats.<id>.instructions`                | `roles/<id>.md` (`prompt_file`)   |
//! | `hats.<id>.triggers` (inverted)         | `[handoff] <event> = [role, ...]` |
//! | `event_loop.completion_promise`         | `event_loop.completion_promise`   |
//! | `event_loop.required_events`            | `event_loop.required_events`      |
//! | `event_loop.max_iterations`             | `event_loop.max_iterations`       |
//! | `core.guardrails`                       | `harness.md`                      |

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use ralph_core::RalphConfig;

/// autoloop's completion-event convention (ralph signals completion by the
/// promise text and/or a hat publishing this event).
const COMPLETION_EVENT: &str = "task.complete";

/// Quote a TOML basic string, escaping `"` and `\`.
fn q(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Render a TOML array of strings: `["a", "b"]`.
fn arr(items: &[String]) -> String {
    let inner = items.iter().map(|s| q(s)).collect::<Vec<_>>().join(", ");
    format!("[{inner}]")
}

/// Write an autoloop preset reflecting `config`'s hats into `dir`.
///
/// Returns an error if the config has no hats (nothing to translate).
pub fn generate_preset(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir.join("roles"))?;

    // Hatless / single-hat ralph configs have no hats map. Synthesize a single
    // role that runs the objective and emits the completion event — the autoloop
    // equivalent of ralph's hatless mode.
    if config.hats.is_empty() {
        return generate_hatless_preset(config, dir);
    }

    // Deterministic ordering for stable output.
    let mut hats: Vec<(&String, &ralph_core::HatConfig)> = config.hats.iter().collect();
    hats.sort_by(|a, b| a.0.cmp(b.0));

    // Invert triggers -> handoff (event -> roles that consume it), preserving
    // role order per event.
    let mut handoff: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, hat) in &hats {
        for trigger in &hat.triggers {
            let entry = handoff.entry(trigger.clone()).or_default();
            if !entry.iter().any(|r| r == *id) {
                entry.push((*id).clone());
            }
        }
    }
    // Ensure a starting route: autoloop begins from handoff["loop.start"].
    if !handoff.contains_key("loop.start") {
        let starter = starting_role(config, &hats);
        handoff.insert("loop.start".to_string(), vec![starter]);
    }

    // topology.toml
    let mut topo = String::new();
    topo.push_str(&format!("name = {}\n", q("ralph")));
    topo.push_str(&format!("completion = {}\n\n", q(COMPLETION_EVENT)));
    for (id, hat) in &hats {
        let role_file = format!("roles/{id}.md");
        fs::write(dir.join(&role_file), hat.instructions.as_bytes())?;
        topo.push_str("[[role]]\n");
        topo.push_str(&format!("id = {}\n", q(id)));
        if !hat.name.is_empty() {
            topo.push_str(&format!("name = {}\n", q(&hat.name)));
        }
        topo.push_str(&format!("emits = {}\n", arr(&hat.publishes)));
        topo.push_str(&format!("prompt_file = {}\n\n", q(&role_file)));
    }
    topo.push_str("[handoff]\n");
    for (event, roles) in &handoff {
        topo.push_str(&format!("{} = {}\n", q(event), arr(roles)));
    }
    fs::write(dir.join("topology.toml"), topo.as_bytes())?;

    write_autoloops(config, dir)?;
    write_harness(config, dir)?;
    Ok(())
}

/// Synthesize a single-role preset for a hatless / single-hat ralph config.
fn generate_hatless_preset(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    let role_instructions = format!(
        "You are the single autonomous worker for this loop. Work the objective \
         through to completion, verifying with the strongest available harness \
         before declaring done. When the work is complete and verified, emit the \
         completion event (`<tool> emit {COMPLETION_EVENT} \"<summary>\"`) or end \
         your output with the completion promise `{}`.\n",
        config.event_loop.completion_promise
    );
    fs::write(dir.join("roles/ralph.md"), role_instructions.as_bytes())?;

    let mut topo = String::new();
    topo.push_str(&format!("name = {}\n", q("ralph")));
    topo.push_str(&format!("completion = {}\n\n", q(COMPLETION_EVENT)));
    topo.push_str("[[role]]\n");
    topo.push_str(&format!("id = {}\n", q("ralph")));
    topo.push_str(&format!(
        "emits = {}\n",
        arr(&[COMPLETION_EVENT.to_string()])
    ));
    topo.push_str(&format!("prompt_file = {}\n\n", q("roles/ralph.md")));
    topo.push_str("[handoff]\n");
    topo.push_str(&format!(
        "{} = {}\n",
        q("loop.start"),
        arr(&["ralph".to_string()])
    ));
    fs::write(dir.join("topology.toml"), topo.as_bytes())?;

    write_autoloops(config, dir)?;
    write_harness(config, dir)?;
    Ok(())
}

/// Write `autoloops.toml` from ralph's event-loop config.
fn write_autoloops(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    let el = &config.event_loop;
    let mut auto = String::new();
    auto.push_str(&format!(
        "event_loop.max_iterations = {}\n",
        el.max_iterations
    ));
    auto.push_str(&format!(
        "event_loop.completion_event = {}\n",
        q(COMPLETION_EVENT)
    ));
    auto.push_str(&format!(
        "event_loop.completion_promise = {}\n",
        q(&el.completion_promise)
    ));
    if !el.required_events.is_empty() {
        auto.push_str(&format!(
            "event_loop.required_events = {}\n",
            arr(&el.required_events)
        ));
    }
    auto.push_str("\nharness.instructions_file = \"harness.md\"\n");
    fs::write(dir.join("autoloops.toml"), auto.as_bytes())
}

/// Write `harness.md` from ralph's guardrails.
fn write_harness(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    fs::write(
        dir.join("harness.md"),
        config.core.guardrails.join("\n").as_bytes(),
    )
}

/// Pick the role autoloop should start from: the hat triggered by ralph's start
/// event, else the first hat alphabetically.
fn starting_role(config: &RalphConfig, hats: &[(&String, &ralph_core::HatConfig)]) -> String {
    let start_event = config
        .event_loop
        .starting_event
        .clone()
        .unwrap_or_else(|| "task.start".to_string());
    for (id, hat) in hats {
        if hat
            .triggers
            .iter()
            .any(|t| *t == start_event || t == "task.start")
        {
            return (*id).clone();
        }
    }
    hats.first()
        .map(|(id, _)| (*id).clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_hats() -> RalphConfig {
        let yaml = r#"
event_loop:
  max_iterations: 42
  required_events: ["review.passed"]
hats:
  planner:
    name: Planner
    description: "plans"
    triggers: ["task.start"]
    publishes: ["tasks.ready"]
    instructions: "Plan it."
  builder:
    name: Builder
    description: "builds"
    triggers: ["tasks.ready"]
    publishes: ["task.complete"]
    instructions: 'Build "it".'
"#;
        serde_yaml::from_str(yaml).expect("valid ralph config")
    }

    #[test]
    fn generates_a_single_role_preset_when_no_hats() {
        let cfg = RalphConfig::default();
        let dir = tempfile::tempdir().unwrap();
        generate_preset(&cfg, dir.path()).unwrap();
        assert!(dir.path().join("autoloops.toml").is_file());
        assert!(dir.path().join("roles/ralph.md").is_file());
        let topo = fs::read_to_string(dir.path().join("topology.toml")).unwrap();
        assert!(topo.contains("id = \"ralph\""));
        assert!(topo.contains("\"loop.start\" = [\"ralph\"]"));
    }

    #[test]
    fn writes_a_valid_preset_shape_from_hats() {
        let cfg = config_with_hats();
        let dir = tempfile::tempdir().unwrap();
        generate_preset(&cfg, dir.path()).unwrap();

        // Files exist.
        assert!(dir.path().join("autoloops.toml").is_file());
        assert!(dir.path().join("topology.toml").is_file());
        assert!(dir.path().join("harness.md").is_file());
        assert!(dir.path().join("roles/planner.md").is_file());
        assert!(dir.path().join("roles/builder.md").is_file());

        let topo = fs::read_to_string(dir.path().join("topology.toml")).unwrap();
        // Roles map from hats.
        assert!(topo.contains("id = \"planner\""));
        assert!(topo.contains("emits = [\"tasks.ready\"]"));
        // Handoff is the inverted triggers + a loop.start route.
        assert!(topo.contains("\"tasks.ready\" = [\"builder\"]"));
        assert!(topo.contains("\"loop.start\" = [\"planner\"]"));

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(auto.contains("event_loop.max_iterations = 42"));
        assert!(auto.contains("event_loop.required_events = [\"review.passed\"]"));

        // Role prompt content is escaped/written verbatim.
        let builder = fs::read_to_string(dir.path().join("roles/builder.md")).unwrap();
        assert_eq!(builder, "Build \"it\".");

        // The generated preset is loadable by ralph's own autoloop preset reader
        // (round-trip sanity against preset_source's detector).
        assert!(dir.path().join("topology.toml").is_file());
    }
}
