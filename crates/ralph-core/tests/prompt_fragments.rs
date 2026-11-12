//! Model-specific prompt fragments, exercised through the real config-load
//! path (`parse_yaml` → `normalize` → `validate`), asserting on the resolved
//! hat instructions that the prompt builder renders.

use ralph_core::{ConfigError, EventLoop, LoopContext, RalphConfig};
use ralph_proto::HatId;
use tempfile::TempDir;

const FABLE_TEXT: &str = "FRAGMENT-FABLE-OWNER";

/// Loads YAML the way `ralph run` does: parse, normalize, validate.
fn load(yaml: &str) -> Result<RalphConfig, ConfigError> {
    let mut config = RalphConfig::parse_yaml(yaml)?;
    config.normalize();
    config.validate()?;
    Ok(config)
}

fn load_ok(yaml: &str) -> RalphConfig {
    load(yaml).unwrap_or_else(|e| panic!("config should load: {e}\n---\n{yaml}"))
}

fn instructions<'a>(config: &'a RalphConfig, hat: &str) -> &'a str {
    &config.hats[hat].instructions
}

/// Fable Planner + Opus Builder, one `claude-fable-*` fragment.
fn two_hat_config(planner_model: &str) -> String {
    format!(
        r#"
cli:
  backend: claude
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["build.start"]
    publishes: ["plan.ready"]
    model: {planner_model}
    instructions: "PLANNER BASE"
  builder:
    name: Builder
    description: Builds
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    model: claude-opus-5-5
    instructions: "BUILDER BASE"
fragments:
  - models: ["claude-fable-*"]
    text: "{FABLE_TEXT}"
"#
    )
}

#[test]
fn fragment_applies_only_to_the_hat_on_a_matching_model() {
    // AC3
    let config = load_ok(&two_hat_config("claude-fable-5-2"));
    let planner = instructions(&config, "planner");
    assert!(planner.starts_with("PLANNER BASE"), "{planner:?}");
    assert!(planner.contains(FABLE_TEXT), "{planner:?}");
    assert!(!instructions(&config, "builder").contains(FABLE_TEXT));
}

#[test]
fn swapping_the_model_swaps_the_fragment_away() {
    // AC4: only the Planner's model changes.
    let config = load_ok(&two_hat_config("claude-opus-5-5"));
    for hat in ["planner", "builder"] {
        assert!(
            !instructions(&config, hat).contains(FABLE_TEXT),
            "{hat}: {:?}",
            instructions(&config, hat)
        );
    }
}

#[test]
fn model_pinned_only_in_backend_args_matches() {
    // AC5, through `backend_args`, the hat backend's own `args`, `--model=`
    // and codex's `-m`.
    for hat_fields in [
        r#"backend_args: ["--model", "claude-fable-5-2"]"#,
        r#"args: ["--model=claude-fable-5-2"]"#,
        "backend:\n      type: claude\n      args: [\"--model\", \"claude-fable-5-2\"]",
        "backend: codex\n    backend_args: [\"-m\", \"claude-fable-5-2\"]",
    ] {
        let yaml = format!(
            r#"
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["build.start"]
    {hat_fields}
fragments:
  - models: ["claude-fable-*"]
    text: "{FABLE_TEXT}"
"#
        );
        let config = load_ok(&yaml);
        assert!(
            instructions(&config, "planner").contains(FABLE_TEXT),
            "{hat_fields}"
        );
    }
}

#[test]
fn explicit_model_flag_in_args_decides_the_match_over_model_field() {
    // The backend runs the model pinned in args (it wins over `model:`), so
    // fragments must follow that model, not the one the command line ignores.
    let yaml = format!(
        r#"
cli:
  model: claude-fable-5-2
hats:
  pinned:
    name: Pinned
    description: Pinned via args
    triggers: ["a"]
    model: claude-fable-5-2
    backend_args: ["--model", "claude-opus-5-5"]
  inherits:
    name: Inherits
    description: Inherits cli.model
    triggers: ["b"]
fragments:
  - models: ["claude-fable-*"]
    text: "{FABLE_TEXT}"
"#
    );
    let config = load_ok(&yaml);
    assert!(!instructions(&config, "pinned").contains(FABLE_TEXT));
    assert!(instructions(&config, "inherits").contains(FABLE_TEXT));
}

#[test]
fn cli_model_and_cli_args_supply_the_model_for_hats_without_one() {
    for cli_fields in [
        "model: claude-fable-5-2",
        r#"args: ["--model", "claude-fable-5-2"]"#,
    ] {
        let yaml = format!(
            r#"
cli:
  backend: claude
  {cli_fields}
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["build.start"]
fragments:
  - models: ["claude-fable-*"]
    text: "{FABLE_TEXT}"
"#
        );
        let config = load_ok(&yaml);
        assert!(
            instructions(&config, "planner").contains(FABLE_TEXT),
            "{cli_fields}"
        );
    }
}

#[test]
fn hat_with_no_model_anywhere_gets_no_fragment() {
    // AC6 (config half), including a pattern that would match anything.
    let yaml = format!(
        r#"
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["build.start"]
    instructions: "BASE"
    backend_args: ["--verbose"]
fragments:
  - models: ["*"]
    text: "{FABLE_TEXT}"
"#
    );
    let config = load_ok(&yaml);
    assert_eq!(instructions(&config, "planner"), "BASE");
    assert!(config.fragment_indices_for_hat("planner").is_empty());
}

#[test]
fn fragment_naming_a_script_hat_fails_load() {
    // AC8
    let yaml = r#"
cli:
  model: claude-fable-5-2
hats:
  advance:
    name: Advance
    description: Script step
    triggers: ["step.go"]
    run: "true"
fragments:
  - models: ["claude-fable-*"]
    hats: [advance]
    text: "x"
"#;
    let err = load(yaml).unwrap_err();
    let message = err.to_string();
    assert!(
        matches!(&err, ConfigError::ScriptHatInFragment { hat, index: 0 } if hat == "advance"),
        "{err:?}"
    );
    assert!(message.contains("advance"), "{message}");
    assert!(message.contains("fragments"), "{message}");
}

#[test]
fn omitted_hats_targets_every_model_hat_and_skips_script_hats() {
    let yaml = format!(
        r#"
cli:
  model: claude-fable-5-2
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["a"]
  builder:
    name: Builder
    description: Builds
    triggers: ["b"]
  advance:
    name: Advance
    description: Script step
    triggers: ["c"]
    run: "true"
fragments:
  - models: ["claude-fable-*"]
    text: "{FABLE_TEXT}"
"#
    );
    let config = load_ok(&yaml);
    assert!(instructions(&config, "planner").contains(FABLE_TEXT));
    assert!(instructions(&config, "builder").contains(FABLE_TEXT));
    assert!(instructions(&config, "advance").is_empty());
    assert!(config.fragment_indices_for_hat("advance").is_empty());
}

#[test]
fn hats_list_restricts_targets() {
    let yaml = format!(
        r#"
cli:
  model: claude-fable-5-2
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["a"]
  builder:
    name: Builder
    description: Builds
    triggers: ["b"]
fragments:
  - models: ["claude-fable-*"]
    hats: [builder]
    text: "{FABLE_TEXT}"
"#
    );
    let config = load_ok(&yaml);
    assert!(!instructions(&config, "planner").contains(FABLE_TEXT));
    assert!(instructions(&config, "builder").contains(FABLE_TEXT));
}

#[test]
fn matching_fragments_append_after_extra_instructions_in_config_order() {
    let yaml = r#"
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["a"]
    model: claude-fable-5-2
    instructions: "BASE"
    extra_instructions: ["EXTRA"]
fragments:
  - models: ["claude-fable-*"]
    text: "FIRST"
  - models: ["claude-opus-*"]
    text: "NEVER"
  - models: ["claude-?able-5-2"]
    text: "SECOND"
"#;
    let config = load_ok(yaml);
    let text = instructions(&config, "planner");
    let pos = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing: {text:?}"))
    };
    assert!(pos("BASE") < pos("EXTRA"));
    assert!(pos("EXTRA") < pos("FIRST"));
    assert!(pos("FIRST") < pos("SECOND"));
    assert!(!text.contains("NEVER"));
    assert_eq!(config.fragment_indices_for_hat("planner"), vec![0, 2]);
}

#[test]
fn matching_is_case_sensitive_and_on_the_full_id() {
    for (pattern, applies) in [
        ("claude-fable-5-2", true),
        ("Claude-Fable-*", false),
        ("fable", false),
        ("claude-fable", false),
        ("*fable*", true),
    ] {
        let yaml = format!(
            r#"
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["a"]
    model: claude-fable-5-2
fragments:
  - models: ["{pattern}"]
    text: "{FABLE_TEXT}"
"#
        );
        let config = load_ok(&yaml);
        assert_eq!(
            instructions(&config, "planner").contains(FABLE_TEXT),
            applies,
            "pattern {pattern}"
        );
    }
}

#[test]
fn yaml_anchor_text_is_shared_across_fragments() {
    let yaml = r#"
_reports: &reports |
  SHARED-REPORTS-BLOCK
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["a"]
    model: claude-fable-5-2
  builder:
    name: Builder
    description: Builds
    triggers: ["b"]
    model: gpt-5
fragments:
  - models: ["claude-fable-*"]
    hats: [planner]
    text: *reports
  - models: ["gpt-*"]
    hats: [builder]
    text: *reports
"#;
    let config = load_ok(yaml);
    assert!(instructions(&config, "planner").contains("SHARED-REPORTS-BLOCK"));
    assert!(instructions(&config, "builder").contains("SHARED-REPORTS-BLOCK"));
}

#[test]
fn coordinator_fragments_are_resolved_but_stay_out_of_hats() {
    let yaml = r#"
cli:
  model: claude-fable-5-2
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["a"]
fragments:
  - models: ["claude-fable-*"]
    hats: [ralph]
    text: "COORD-ONLY"
  - models: ["claude-fable-*"]
    hats: [planner]
    text: "HAT-ONLY"
  - models: ["claude-fable-*"]
    text: "BOTH"
"#;
    let config = load_ok(yaml);
    let planner = instructions(&config, "planner");
    assert!(!planner.contains("COORD-ONLY"));
    assert!(planner.contains("HAT-ONLY") && planner.contains("BOTH"));
    assert_eq!(config.coordinator_fragments(), vec!["COORD-ONLY", "BOTH"]);

    // A non-matching cli.model resolves nothing for the coordinator.
    let config = load_ok(&yaml.replace("model: claude-fable-5-2", "model: claude-opus-5-5"));
    assert!(config.coordinator_fragments().is_empty());
}

#[test]
fn normalize_is_idempotent_across_a_serialize_reload_round_trip() {
    // The bot daemon serializes the resolved config and `ralph run` loads it
    // again, so a reload must not append the fragment a second time.
    let config = load_ok(&two_hat_config("claude-fable-5-2"));
    let yaml = serde_yaml::to_string(&config).unwrap();
    let mut reloaded = load_ok(&yaml);
    reloaded.normalize();
    let planner = instructions(&reloaded, "planner");
    assert_eq!(planner.matches(FABLE_TEXT).count(), 1, "{planner:?}");
    assert_eq!(reloaded.fragment_indices_for_hat("planner"), vec![0]);
}

#[test]
fn malformed_fragments_fail_load_naming_the_fragment() {
    let hat = r#"
hats:
  planner:
    name: Planner
    description: Plans
    triggers: ["a"]
    model: claude-fable-5-2
"#;
    for (fragment, needle) in [
        ("  - models: []\n    text: x\n", "models"),
        ("  - models: [\"\"]\n    text: x\n", "pattern"),
        ("  - models: [\"  \"]\n    text: x\n", "pattern"),
        ("  - models: [\"claude-*\"]\n    text: \"  \"\n", "text"),
        (
            "  - models: [\"claude-*\"]\n    hats: []\n    text: x\n",
            "hats",
        ),
        (
            "  - models: [\"claude-*\"]\n    hats: [\"\"]\n    text: x\n",
            "hats",
        ),
    ] {
        let yaml = format!("{hat}fragments:\n  - models: [\"x\"]\n    text: ok\n{fragment}");
        let err = load(&yaml).unwrap_err();
        assert!(
            matches!(&err, ConfigError::InvalidFragment { index: 1, .. }),
            "{fragment}: {err:?}"
        );
        let message = err.to_string();
        assert!(message.contains("fragments[1]"), "{message}");
        assert!(message.contains(needle), "{needle}: {message}");
    }
}

#[test]
fn fragment_keys_are_checked_at_parse_time() {
    // Missing `models`/`text` and misspelled keys (`hat:` would otherwise
    // silently widen the fragment to every hat) are parse errors.
    for fragment in [
        "  - text: x\n",
        "  - models: [\"claude-*\"]\n",
        "  - models: [\"claude-*\"]\n    hat: [planner]\n    text: x\n",
    ] {
        let yaml = format!("fragments:\n{fragment}");
        let err = load(&yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Yaml(_)), "{fragment}: {err:?}");
    }
}

#[test]
fn hat_named_ralph_conflicts_with_coordinator_target() {
    let yaml = r#"
hats:
  ralph:
    name: Ralph hat
    description: Collides with the coordinator id
    triggers: ["a"]
    model: claude-fable-5-2
fragments:
  - models: ["claude-fable-*"]
    hats: [ralph]
    text: x
"#;
    let err = load(yaml).unwrap_err();
    assert!(
        matches!(&err, ConfigError::InvalidFragment { index: 0, reason } if reason.contains("ralph")),
        "{err:?}"
    );
}

/// Builds the prompt for the hat that `work.start` activates, through a real
/// `EventLoop`, the way the loop runner does for an iteration.
fn active_hat_prompt(planner_model: &str) -> String {
    let yaml = format!(
        "event_loop:\n  starting_event: work.start\n{}",
        two_hat_config(planner_model).replace("build.start", "work.start")
    );
    let dir = TempDir::new().expect("temp dir");
    let mut config = load_ok(&yaml);
    config.core.workspace_root = dir.path().to_path_buf();
    let context = LoopContext::primary(dir.path().to_path_buf());
    std::fs::create_dir_all(context.ralph_dir()).expect("create .ralph");
    std::fs::write(context.current_events_marker(), ".ralph/events.jsonl")
        .expect("write events marker");
    let mut event_loop = EventLoop::with_context(config, context);
    event_loop.initialize("plan the work");
    assert_eq!(event_loop.get_active_hat_id().as_str(), "planner");
    event_loop
        .build_prompt(&HatId::new("ralph"))
        .expect("prompt for the planner")
}

#[test]
fn resolved_fragment_reaches_the_built_hat_prompt() {
    let prompt = active_hat_prompt("claude-fable-5-2");
    assert!(prompt.contains("PLANNER BASE"), "hat instructions rendered");
    assert_eq!(prompt.matches(FABLE_TEXT).count(), 1, "{prompt}");

    let prompt = active_hat_prompt("claude-opus-5-5");
    assert!(prompt.contains("PLANNER BASE"));
    assert!(!prompt.contains(FABLE_TEXT), "{prompt}");
}
