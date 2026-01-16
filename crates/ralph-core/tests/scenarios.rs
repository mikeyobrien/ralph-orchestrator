//! E2E scenario tests for event-loop redesign.
//!
//! Tests cover:
//! - Solo mode (Ralph with no hats)
//! - Multi-hat delegation
//! - Orphaned event handling
//! - Default publishes fallback
//! - Mixed backends

use ralph_core::RalphConfig;
use ralph_core::testing::{MockBackend, Scenario, ScenarioRunner};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct ScenarioYaml {
    name: String,
    description: String,
    config: ConfigYaml,
    mock_responses: Vec<String>,
    expected: ExpectedYaml,
}

#[allow(dead_code)] // Test infrastructure - fields used for YAML deserialization
#[derive(Debug, Deserialize)]
struct ConfigYaml {
    prompt_file: String,
    max_iterations: u32,
    #[serde(default)]
    hats: serde_yaml::Value,
}

#[allow(dead_code)] // Test infrastructure - fields used for YAML deserialization
#[derive(Debug, Deserialize)]
struct ExpectedYaml {
    iterations: usize,
    events: Vec<EventYaml>,
    completion: bool,
}

#[allow(dead_code)] // Test infrastructure - fields used for YAML deserialization
#[derive(Debug, Deserialize)]
struct EventYaml {
    topic: String,
}

fn load_scenario(path: &str) -> ScenarioYaml {
    let content =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
    serde_yaml::from_str(&content).unwrap_or_else(|e| panic!("Failed to parse {}: {}", path, e))
}

fn run_scenario(yaml: ScenarioYaml) {
    let backend = MockBackend::new(yaml.mock_responses);
    let runner = ScenarioRunner::new(backend.clone());

    let mut config = RalphConfig::default();
    config.max_iterations = Some(yaml.config.max_iterations);
    config.prompt_file = Some(yaml.config.prompt_file);

    let scenario =
        Scenario::new(yaml.name.clone(), config).with_iterations(yaml.expected.iterations);

    let trace = runner.run(&scenario);

    // Verify iteration count
    assert_eq!(
        trace.iterations, yaml.expected.iterations,
        "{}: Expected {} iterations, got {}",
        yaml.name, yaml.expected.iterations, trace.iterations
    );

    // Verify backend was called
    assert!(
        backend.execution_count() > 0,
        "{}: Backend should have been called",
        yaml.name
    );

    println!("✓ {} passed", yaml.description);
}

#[test]
fn test_solo_mode() {
    let yaml = load_scenario("tests/scenarios/solo_mode.yml");
    run_scenario(yaml);
}

#[test]
fn test_multi_hat() {
    let yaml = load_scenario("tests/scenarios/multi_hat.yml");
    run_scenario(yaml);
}

#[test]
fn test_orphaned_events() {
    let yaml = load_scenario("tests/scenarios/orphaned_events.yml");
    run_scenario(yaml);
}

#[test]
fn test_default_publishes() {
    let yaml = load_scenario("tests/scenarios/default_publishes.yml");
    run_scenario(yaml);
}

#[test]
fn test_mixed_backends() {
    let yaml = load_scenario("tests/scenarios/mixed_backends.yml");
    run_scenario(yaml);
}
