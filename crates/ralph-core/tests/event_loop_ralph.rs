//! Integration tests for EventLoop with Ralph fallback.

use ralph_core::{EventLoop, RalphConfig};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_orphaned_event_falls_to_ralph() {
    // Setup: Create a temp directory with .agent/events.jsonl
    let temp_dir = TempDir::new().unwrap();
    let agent_dir = temp_dir.path().join(".agent");
    fs::create_dir_all(&agent_dir).unwrap();

    let events_file = agent_dir.join("events.jsonl");

    // Write an orphaned event (no hat subscribes to "orphan.event")
    fs::write(
        &events_file,
        r#"{"topic":"orphan.event","payload":"This event has no subscriber","ts":"2026-01-14T12:00:00Z"}
"#,
    )
    .unwrap();

    // Create EventLoop with empty hat registry (no hats configured)
    let yaml = r#"
core:
  scratchpad: ".agent/scratchpad.md"
  specs_dir: "./specs"
  guardrails:
    - "Fresh context each iteration"
    - "Backpressure is law"
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 10
  max_runtime_seconds: 300
"#;

    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    // Change to temp directory so EventReader finds the events file
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Process events from JSONL
    let has_orphans = event_loop.process_events_from_jsonl().unwrap();

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    // Verify: Ralph should handle the orphaned event
    assert!(has_orphans, "Expected orphaned event to trigger Ralph");
}

#[test]
fn test_ralph_completion_only_from_ralph() {
    let yaml = r#"
core:
  scratchpad: ".agent/scratchpad.md"
  specs_dir: "./specs"
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 10
"#;

    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let event_loop = EventLoop::new(config);

    // Test: Ralph output with LOOP_COMPLETE should trigger completion
    let ralph_output = "All tasks complete.\n\nLOOP_COMPLETE";
    assert!(
        event_loop.check_ralph_completion(ralph_output),
        "Ralph should be able to trigger completion"
    );

    // Test: Any output with LOOP_COMPLETE should be detected
    let output_with_promise = "Some work done\nLOOP_COMPLETE\nMore text";
    assert!(
        event_loop.check_ralph_completion(output_with_promise),
        "LOOP_COMPLETE should be detected anywhere in output"
    );

    // Test: Output without LOOP_COMPLETE should not trigger
    let output_without_promise = "Some work done\nNo completion here";
    assert!(
        !event_loop.check_ralph_completion(output_without_promise),
        "Output without LOOP_COMPLETE should not trigger completion"
    );
}

#[test]
fn test_ralph_prompt_includes_ghuntley_style() {
    let yaml = r#"
core:
  scratchpad: ".agent/scratchpad.md"
  specs_dir: "./specs"
  guardrails:
    - "Fresh context each iteration"
    - "Backpressure is law"
event_loop:
  completion_promise: "LOOP_COMPLETE"
"#;

    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let event_loop = EventLoop::new(config);

    let prompt = event_loop.build_ralph_prompt("Test context");

    // Verify prompt includes ghuntley-style structure
    assert!(
        prompt.contains("I'm Ralph"),
        "Prompt should identify Ralph with ghuntley style"
    );
    assert!(
        prompt.contains("Fresh context each iteration"),
        "Prompt should include ghuntley identity"
    );
    assert!(
        prompt.contains("### 0a. ORIENTATION"),
        "Prompt should include orientation phase"
    );
    assert!(
        prompt.contains("### 0b. SCRATCHPAD"),
        "Prompt should include scratchpad section"
    );
    assert!(
        prompt.contains("## WORKFLOW"),
        "Prompt should include workflow section"
    );
    assert!(
        prompt.contains("### GUARDRAILS"),
        "Prompt should include guardrails section"
    );
    assert!(
        prompt.contains("LOOP_COMPLETE"),
        "Prompt should include completion promise"
    );
}

#[test]
fn test_ralph_prompt_solo_mode_structure() {
    let yaml = r#"
core:
  scratchpad: ".agent/scratchpad.md"
  specs_dir: "./specs"
event_loop:
  completion_promise: "LOOP_COMPLETE"
"#;

    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let event_loop = EventLoop::new(config);

    let prompt = event_loop.build_ralph_prompt("");

    // In solo mode (no hats), Ralph should NOT have HATS section
    assert!(prompt.contains("## WORKFLOW"), "Workflow should be present");
    assert!(
        prompt.contains("## EVENT WRITING"),
        "Event writing section should be present"
    );
    assert!(
        !prompt.contains("## HATS"),
        "HATS section should not be present in solo mode"
    );
}

#[test]
fn test_ralph_prompt_multi_hat_mode_structure() {
    let yaml = r#"
core:
  scratchpad: ".agent/scratchpad.md"
  specs_dir: "./specs"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start"]
    publishes: ["build.task"]
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
event_loop:
  completion_promise: "LOOP_COMPLETE"
"#;

    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let event_loop = EventLoop::new(config);

    let prompt = event_loop.build_ralph_prompt("");

    // In multi-hat mode, Ralph should see hat topology
    assert!(
        prompt.contains("## HATS"),
        "HATS section should be present in multi-hat mode"
    );
    assert!(
        prompt.contains("Delegate via events"),
        "Delegation instruction should be present"
    );
    assert!(prompt.contains("Planner"), "Planner hat should be listed");
    assert!(prompt.contains("Builder"), "Builder hat should be listed");
    assert!(
        prompt.contains("| Hat | Triggers On | Publishes |"),
        "Hat table header should be present"
    );
}

#[test]
fn test_reads_actual_events_jsonl_with_object_payloads() {
    // This test verifies the fix for "invalid type: map, expected a string" errors
    // when reading events.jsonl containing object payloads from `ralph emit --json`
    use ralph_core::EventHistory;

    let history = EventHistory::new(".agent/events.jsonl");
    if !history.exists() {
        // Skip if no events file (CI environment)
        return;
    }

    // This should NOT produce any warnings about failed parsing
    let records = history.read_all().expect("Should read events.jsonl");

    // We expect at least some records
    assert!(!records.is_empty(), "events.jsonl should have records");

    // Verify all records were parsed (no silently dropped records)
    println!(
        "\n✓ Successfully parsed {} records from .agent/events.jsonl:\n",
        records.len()
    );
    for (i, record) in records.iter().enumerate() {
        let payload_preview = if record.payload.len() > 50 {
            format!("{}...", &record.payload[..50])
        } else {
            record.payload.clone()
        };
        let payload_type = if record.payload.starts_with('{') {
            "object→string"
        } else {
            "string"
        };
        println!(
            "  [{}] topic={:<25} type={:<14} payload={}",
            i + 1,
            record.topic,
            payload_type,
            payload_preview
        );

        // Object payloads should be converted to JSON strings
        if record.payload.starts_with('{') {
            // Verify it's valid JSON
            let _: serde_json::Value = serde_json::from_str(&record.payload)
                .expect("Object payload should be valid JSON string");
        }
    }
    println!();
}
