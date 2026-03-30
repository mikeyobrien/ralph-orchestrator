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
fn test_guidance_persists_across_iterations_solo_mode() {
    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    let ralph_id = HatId::new("ralph");

    event_loop
        .bus
        .publish(Event::new("human.guidance", "Keep this in mind"));

    let prompt = event_loop.build_prompt(&ralph_id).unwrap();
    assert!(
        prompt.contains("## ROBOT GUIDANCE"),
        "Prompt should include guidance section"
    );
    assert!(
        prompt.contains("Keep this in mind"),
        "Prompt should include guidance payload"
    );
    assert!(
        !event_loop.has_pending_events(),
        "Guidance should not remain pending after prompt build"
    );

    let prompt_again = event_loop.build_prompt(&ralph_id).unwrap();
    assert!(
        prompt_again.contains("Keep this in mind"),
        "Guidance should persist across iterations"
    );
}

#[test]
fn test_guidance_persists_across_iterations_multi_hat_mode() {
    let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start"]
    publishes: ["task.plan"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    let ralph_id = HatId::new("ralph");

    event_loop
        .bus
        .publish(Event::new("human.guidance", "Focus on error handling"));

    let prompt = event_loop.build_prompt(&ralph_id).unwrap();
    assert!(
        prompt.contains("Focus on error handling"),
        "Prompt should include guidance payload"
    );

    let prompt_again = event_loop.build_prompt(&ralph_id).unwrap();
    assert!(
        prompt_again.contains("Focus on error handling"),
        "Guidance should persist across iterations in multi-hat mode"
    );
}

#[test]
fn test_guidance_persisted_to_scratchpad() {
    let dir = tempfile::tempdir().unwrap();
    let scratchpad_path = dir.path().join("scratchpad.md");

    let yaml = format!(
        r#"
core:
  workspace_root: "{}"
  scratchpad: "{}"
"#,
        dir.path().display(),
        scratchpad_path.display()
    );
    let config: RalphConfig = serde_yaml::from_str(&yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    let ralph_id = HatId::new("ralph");

    // Publish guidance and build prompt to trigger persistence
    event_loop
        .bus
        .publish(Event::new("human.guidance", "Use the new API for auth"));

    let prompt = event_loop.build_prompt(&ralph_id).unwrap();
    assert!(
        prompt.contains("Use the new API for auth"),
        "Prompt should include guidance"
    );

    // Verify guidance was persisted to scratchpad file
    let scratchpad_content = std::fs::read_to_string(&scratchpad_path)
        .expect("Scratchpad file should exist after guidance persistence");
    assert!(
        scratchpad_content.contains("HUMAN GUIDANCE"),
        "Scratchpad should contain HUMAN GUIDANCE header"
    );
    assert!(
        scratchpad_content.contains("Use the new API for auth"),
        "Scratchpad should contain guidance text"
    );
}

#[test]
fn test_guidance_appends_to_existing_scratchpad() {
    let dir = tempfile::tempdir().unwrap();
    let scratchpad_path = dir.path().join("scratchpad.md");

    // Pre-populate scratchpad with existing content
    std::fs::write(&scratchpad_path, "## Existing Notes\n\nSome prior work.\n").unwrap();

    let yaml = format!(
        r#"
core:
  workspace_root: "{}"
  scratchpad: "{}"
"#,
        dir.path().display(),
        scratchpad_path.display()
    );
    let config: RalphConfig = serde_yaml::from_str(&yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    let ralph_id = HatId::new("ralph");

    event_loop
        .bus
        .publish(Event::new("human.guidance", "Focus on error handling"));
    let _ = event_loop.build_prompt(&ralph_id).unwrap();

    let content = std::fs::read_to_string(&scratchpad_path).unwrap();
    assert!(
        content.starts_with("## Existing Notes"),
        "Existing scratchpad content should be preserved"
    );
    assert!(
        content.contains("Focus on error handling"),
        "New guidance should be appended"
    );
}

#[test]
fn test_hat_max_activations_emits_exhausted_event() {
    // Repro for issue #66: per-hat max_activations should prevent infinite reviewer loops.
    // Events are now published directly to the bus (simulating what ralph emit writes to JSONL
    // and process_events_from_jsonl publishes).
    let yaml = r#"
hats:
  executor:
    name: "Executor"
    description: "Implements requested changes"
    triggers: ["work.start", "review.changes_requested"]
    publishes: ["implementation.done"]
  code_reviewer:
    name: "Code Reviewer"
    description: "Reviews changes and requests fixes"
    triggers: ["implementation.done"]
    publishes: ["review.changes_requested"]
    max_activations: 3
  escalator:
    name: "Escalator"
    description: "Handles exhausted hats"
    triggers: ["code_reviewer.exhausted"]
    publishes: []
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    let ralph = HatId::new("ralph");

    // Seed the loop with an executor event.
    event_loop
        .bus
        .publish(Event::new("work.start", "begin").with_source(ralph.clone()));

    // Cycle: executor -> implementation.done; reviewer -> review.changes_requested.
    for _ in 0..3 {
        // Executor active.
        let _ = event_loop.build_prompt(&ralph).unwrap();
        // Simulate event from JSONL (ralph emit writes to file, process_events_from_jsonl publishes)
        event_loop
            .bus
            .publish(Event::new("implementation.done", "done"));

        // Reviewer active (up to max_activations=3).
        let prompt = event_loop.build_prompt(&ralph).unwrap();
        assert!(
            !prompt.contains("Event: code_reviewer.exhausted"),
            "Reviewer should not be exhausted yet"
        );
        event_loop
            .bus
            .publish(Event::new("review.changes_requested", "fix"));
    }

    // One more implementation.done should attempt a 4th reviewer activation.
    let _ = event_loop.build_prompt(&ralph).unwrap();
    event_loop
        .bus
        .publish(Event::new("implementation.done", "done"));

    let prompt = event_loop.build_prompt(&ralph).unwrap();
    assert!(
        prompt.contains("Event: code_reviewer.exhausted"),
        "Expected code_reviewer.exhausted to be emitted when max_activations is exceeded"
    );
    let escalator_id = HatId::new("escalator");
    assert!(
        event_loop
            .bus
            .peek_pending(&escalator_id)
            .is_some_and(|events| {
                events
                    .iter()
                    .any(|e| e.topic.as_str() == "code_reviewer.exhausted")
            }),
        "Expected code_reviewer.exhausted to be published for escalator"
    );

    // Further would-trigger events are dropped (no re-activation beyond max).
    let reviewer_id = HatId::new("code_reviewer");
    assert_eq!(
        *event_loop
            .state
            .hat_activation_counts
            .get(&reviewer_id)
            .unwrap_or(&0),
        3,
        "Reviewer should have exactly max activations recorded"
    );

    event_loop
        .bus
        .publish(Event::new("implementation.done", "done again").with_source(ralph.clone()));
    let prompt = event_loop.build_prompt(&ralph).unwrap();
    assert!(
        !prompt.contains("Event: implementation.done"),
        "Pending events for an exhausted hat should be dropped"
    );
    assert_eq!(
        *event_loop
            .state
            .hat_activation_counts
            .get(&reviewer_id)
            .unwrap_or(&0),
        3,
        "Reviewer should not be activated after exhaustion"
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
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    // Create scratchpad with all tasks completed (use absolute path, no set_current_dir)
    let agent_dir = temp_dir.path().join(".agent");
    fs::create_dir_all(&agent_dir).unwrap();
    let scratchpad_path = agent_dir.join("scratchpad.md");
    fs::write(
        &scratchpad_path,
        "## Tasks\n- [x] Task 1 done\n- [x] Task 2 done\n",
    )
    .unwrap();

    // Configure event loop to use temp directory scratchpad
    let mut config = RalphConfig::default();
    config.core.scratchpad.path = scratchpad_path.to_string_lossy().to_string();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

    let events_path = temp_dir.path().join("events.jsonl");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // LOOP_COMPLETE event with all tasks done - should terminate immediately
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "Should terminate immediately when LOOP_COMPLETE + tasks verified"
    );
}

#[test]
fn test_completion_promise_with_open_tasks_in_scratchpad_still_terminates() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    // Create scratchpad with PENDING tasks ([ ] markers)
    let agent_dir = temp_dir.path().join(".agent");
    fs::create_dir_all(&agent_dir).unwrap();
    let scratchpad_path = agent_dir.join("scratchpad.md");
    fs::write(
        &scratchpad_path,
        "## Tasks\n- [x] Task 1 done\n- [ ] Task 2 still pending\n",
    )
    .unwrap();

    // Configure event loop to use temp directory scratchpad
    let mut config = RalphConfig::default();
    config.core.scratchpad.path = scratchpad_path.to_string_lossy().to_string();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

    let events_path = temp_dir.path().join("events.jsonl");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Scratchpad mode still trusts the agent's completion signal even with open checklist items.
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "Scratchpad mode should still trust the agent's decision"
    );
}

#[test]
fn test_completion_promise_with_pending_tasks_in_task_store_is_rejected() {
    use crate::loop_context::LoopContext;
    use crate::task::{Task, TaskStatus};
    use crate::task_store::TaskStore;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let tasks_path = temp_dir.path().join(".ralph/agent/tasks.jsonl");

    // Create task store with one open and one closed task
    let mut store = TaskStore::load(&tasks_path).unwrap();
    let mut task1 = Task::new("Completed task".to_string(), 1);
    task1.status = TaskStatus::Closed;
    store.add(task1);

    let task2 = Task::new("Still open task".to_string(), 2);
    store.add(task2);
    store.save().unwrap();

    // Configure event loop with memories enabled and pointing to temp dir
    let mut config = RalphConfig::default();
    config.memories.enabled = true;
    config.core.workspace_root = temp_dir.path().to_path_buf();

    let loop_context = LoopContext::primary(temp_dir.path().to_path_buf());
    let mut event_loop = EventLoop::with_context(config, loop_context);
    event_loop.initialize("Test");

    let events_path = temp_dir.path().join("events.jsonl");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Runtime tasks are the canonical queue in memories/tasks mode, so completion should be rejected.
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason, None,
        "Should reject completion while runtime tasks remain pending"
    );
    assert!(
        event_loop.has_pending_events(),
        "Rejecting completion should inject task.resume so the loop continues"
    );
}

#[test]
fn test_completion_promise_requires_last_event() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Completion should be ignored if it is not the last event in the JSONL batch.
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    write_event_to_jsonl(&events_path, "task.resume", "Continue");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason, None,
        "Completion should be ignored when it is not the last event"
    );
}

#[test]
fn test_builder_cannot_terminate_loop() {
    // Per spec: completion requires an emitted event; output-only tokens are ignored
    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

    // Builder output containing completion promise - should be IGNORED
    let hat_id = HatId::new("builder");
    let reason = event_loop.process_output(&hat_id, "Done!\nLOOP_COMPLETE", true);

    // Builder cannot terminate, so no termination reason
    assert_eq!(reason, None);

    // Completion event should still terminate
    let temp_dir = tempfile::tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let completion = event_loop.check_completion_event();
    assert_eq!(completion, Some(TerminationReason::CompletionPromise));
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
        planner_prompt.contains("You have fresh context each iteration"),
        "Planner prompt should have RFC2119 identity"
    );

    // Now trigger builder hat by publishing build.task event
    let hat_id = HatId::new("builder");
    event_loop
        .bus
        .publish(Event::new("build.task", "Build something"));

    let builder_prompt = event_loop.build_prompt(&hat_id).unwrap();

    // Verify RFC2119-style structure for builder too
    assert!(
        builder_prompt.contains("### 0. ORIENTATION"),
        "Builder should use RFC2119-style orientation phase"
    );
    assert!(
        builder_prompt.contains("You MUST NOT use more than 1 subagent for build/tests"),
        "Builder prompt should have subagent limit with MUST NOT"
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

/// Helper to write an event to a JSONL file for testing.
fn write_event_to_jsonl(path: &std::path::Path, topic: &str, payload: &str) {
    use std::io::Write;
    let ts = chrono::Utc::now().to_rfc3339();
    let event_json = serde_json::json!({
        "topic": topic,
        "payload": payload,
        "ts": ts
    });
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{}", event_json).unwrap();
}

#[test]
fn test_loop_thrashing_detection() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    // Builder blocks on "Fix bug" three times (should emit build.task.abandoned)
    write_event_to_jsonl(&events_path, "build.blocked", "Fix bug\nCan't compile");
    let _ = event_loop.process_events_from_jsonl();

    write_event_to_jsonl(
        &events_path,
        "build.blocked",
        "Fix bug\nStill can't compile",
    );
    let _ = event_loop.process_events_from_jsonl();

    write_event_to_jsonl(&events_path, "build.blocked", "Fix bug\nReally stuck");
    let _ = event_loop.process_events_from_jsonl();

    // Task should be abandoned
    assert!(
        event_loop
            .state
            .abandoned_tasks
            .contains(&"Fix bug".to_string()),
        "Task should be abandoned after 3 blocks"
    );
}

#[test]
fn test_thrashing_counter_increments_on_blocked_events() {
    // Events now come from JSONL file via `ralph emit`, not from text output.
    // Per-hat tracking is removed since events don't carry hat context.
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    // Two blocked events should increment counter
    write_event_to_jsonl(&events_path, "build.blocked", "Stuck");
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.consecutive_blocked, 1);

    write_event_to_jsonl(&events_path, "build.blocked", "Still stuck");
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.consecutive_blocked, 2);
}

#[test]
fn test_thrashing_counter_resets_on_non_blocked_event() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    // Two blocked events
    write_event_to_jsonl(&events_path, "build.blocked", "Stuck");
    let _ = event_loop.process_events_from_jsonl();

    write_event_to_jsonl(&events_path, "build.blocked", "Still stuck");
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.consecutive_blocked, 2);

    // Non-blocked event should reset counter
    write_event_to_jsonl(&events_path, "build.task", "Working now");
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.consecutive_blocked, 0);
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
fn test_active_hat_with_instructions_and_publishing_guide() {
    // When a hat is triggered by an event, show ACTIVE HAT section with
    // instructions and Event Publishing Guide instead of full topology.
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

    // Publish an event that triggers the deployer hat
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

    // Build Ralph's prompt - should include active hat info
    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    // The event topic should be in PENDING EVENTS
    assert!(
        prompt.contains("deploy.request"),
        "Should include the event topic in pending events"
    );

    // Should have ACTIVE HAT section (not HATS topology table)
    assert!(
        prompt.contains("## ACTIVE HAT"),
        "Should have ACTIVE HAT section when hat is triggered"
    );
    assert!(
        !prompt.contains("| Hat | Triggers On | Publishes |"),
        "Should NOT have topology table when hat is active"
    );

    // Should include the hat's instructions
    assert!(
        prompt.contains("Handle deployment operations safely"),
        "Should include active hat's instructions"
    );

    // Should have Event Publishing Guide
    assert!(
        prompt.contains("### Event Publishing Guide"),
        "Should have Event Publishing Guide"
    );
    assert!(
        prompt.contains("`deploy.done`"),
        "Guide should list deploy.done"
    );
    assert!(
        prompt.contains("`deploy.failed`"),
        "Guide should list deploy.failed"
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
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    // Create scratchpad with completed and cancelled tasks (use absolute path, no set_current_dir)
    let agent_dir = temp_dir.path().join(".agent");
    fs::create_dir_all(&agent_dir).unwrap();
    let scratchpad_path = agent_dir.join("scratchpad.md");
    let scratchpad_content = r"## Tasks
- [x] Core feature implemented
- [x] Tests added
- [~] Documentation update (cancelled: out of scope)
- [~] Performance optimization (cancelled: not needed)
";
    fs::write(&scratchpad_path, scratchpad_content).unwrap();

    // Test that cancelled tasks don't block completion when all other tasks are done
    let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
"#;
    let mut config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    config.core.scratchpad.path = scratchpad_path.to_string_lossy().to_string();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test task");

    // Simulate completion with some cancelled tasks - should complete immediately
    let events_path = temp_dir.path().join("events.jsonl");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "Should complete immediately with partial completion (cancelled tasks ok)"
    );
}

#[test]
fn test_planner_auto_cancellation_after_three_blocks() {
    // Test that task is abandoned after 3 build.blocked events for same task
    // Events now come from JSONL via `ralph emit`.
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test task");

    // First blocked event for "Task X" - should not abandon
    write_event_to_jsonl(&events_path, "build.blocked", "Task X\nmissing dependency");
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.task_block_counts.get("Task X"), Some(&1));

    // Second blocked event for "Task X" - should not abandon
    write_event_to_jsonl(
        &events_path,
        "build.blocked",
        "Task X\ndependency issue persists",
    );
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.task_block_counts.get("Task X"), Some(&2));

    // Third blocked event for "Task X" - should emit build.task.abandoned
    write_event_to_jsonl(
        &events_path,
        "build.blocked",
        "Task X\nsame dependency issue",
    );
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.task_block_counts.get("Task X"), Some(&3));
    assert!(
        event_loop
            .state
            .abandoned_tasks
            .contains(&"Task X".to_string()),
        "Task X should be abandoned"
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
            extra_instructions: vec![],
            backend_args: None,
            backend: None,
            default_publishes: Some("task.done".to_string()),
            max_activations: None,
            scratchpad: None,
            disallowed_tools: vec![],
            timeout: None,
            concurrency: 1,
            aggregate: None,
        },
    );
    config.hats = hats;

    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    let hat_id = HatId::new("test-hat");

    // Agent wrote no events — process_events_from_jsonl would return had_events: false
    let result = event_loop.process_events_from_jsonl().unwrap();
    assert!(!result.had_events, "No events should be found");

    // check_default_publishes should inject the default
    event_loop.check_default_publishes(&hat_id);

    assert!(
        event_loop.has_pending_events(),
        "Default event should be injected"
    );

    // The default_publishes topic should be recorded in seen_topics
    assert!(
        event_loop.state.seen_topics.contains("task.done"),
        "default_publishes should record topic in seen_topics for chain validation"
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
            extra_instructions: vec![],
            backend_args: None,
            backend: None,
            default_publishes: Some("task.done".to_string()),
            max_activations: None,
            scratchpad: None,
            disallowed_tools: vec![],
            timeout: None,
            concurrency: 1,
            aggregate: None,
        },
    );
    config.hats = hats;

    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    let _hat_id = HatId::new("test-hat");

    // Agent writes an event to the JSONL file
    let mut file = std::fs::File::create(&events_path).unwrap();
    writeln!(
        file,
        r#"{{"topic":"task.done","ts":"2024-01-01T00:00:00Z"}}"#
    )
    .unwrap();
    file.flush().unwrap();

    // process_events_from_jsonl reads them — caller should NOT call check_default_publishes
    let result = event_loop.process_events_from_jsonl().unwrap();
    assert!(result.had_events, "Events should be found from JSONL");

    // Verify: even if someone mistakenly calls check_default_publishes, the
    // call site guards with `if !agent_wrote_events`, so defaults won't fire.
    // But we assert the guard condition here:
    assert!(
        result.had_events,
        "Caller should skip check_default_publishes when agent wrote events"
    );
}

#[test]
fn test_has_pending_plan_events_in_jsonl_peeks_without_consuming() {
    use std::io::Write;
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut event_loop = EventLoop::new(RalphConfig::default());
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let mut file = std::fs::File::create(&events_path).unwrap();
    writeln!(
        file,
        r#"{{"topic":"plan.created","payload":"ready","ts":"2024-01-01T00:00:00Z"}}"#
    )
    .unwrap();
    file.flush().unwrap();

    assert!(
        event_loop
            .has_pending_plan_events_in_jsonl()
            .expect("peek should succeed"),
        "peek should report unread plan.* topics"
    );

    let processed = event_loop.process_events_from_jsonl().unwrap();
    assert!(processed.had_events);
    assert!(
        processed.had_plan_events,
        "processed metadata should preserve semantic plan.* detection"
    );
    assert!(
        processed.human_interact_context.is_none(),
        "plan-only batches should not synthesize human.interact metadata"
    );

    assert!(
        !event_loop
            .has_pending_plan_events_in_jsonl()
            .expect("peek after consume should succeed"),
        "peek should return false after unread events are consumed"
    );
}

#[test]
fn test_pending_human_interact_context_in_jsonl_peeks_without_consuming() {
    use std::io::Write;
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut event_loop = EventLoop::new(RalphConfig::default());
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let mut file = std::fs::File::create(&events_path).unwrap();
    writeln!(
        file,
        r#"{{"topic":"human.interact","payload":"Need approval?","ts":"2024-01-01T00:00:00Z"}}"#
    )
    .unwrap();
    file.flush().unwrap();

    let pending_context = event_loop
        .pending_human_interact_context_in_jsonl()
        .expect("peek should succeed")
        .expect("peek should include pending human.interact context");
    assert_eq!(
        pending_context["question"],
        serde_json::json!("Need approval?")
    );
    assert!(
        pending_context.get("outcome").is_none(),
        "pre-dispatch context should not include outcome metadata"
    );

    let processed = event_loop.process_events_from_jsonl().unwrap();
    assert!(processed.had_events);
    let processed_context = processed
        .human_interact_context
        .expect("processed metadata should include human.interact context");
    assert_eq!(
        processed_context["question"],
        serde_json::json!("Need approval?")
    );
    assert_eq!(
        processed_context["outcome"],
        serde_json::json!("no_robot_service")
    );

    assert!(
        event_loop
            .pending_human_interact_context_in_jsonl()
            .expect("peek after consume should succeed")
            .is_none(),
        "peek should return no pending human.interact events after consume"
    );
}

#[test]
fn test_process_events_from_jsonl_reports_when_plan_topics_absent() {
    use std::io::Write;
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut event_loop = EventLoop::new(RalphConfig::default());
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let mut file = std::fs::File::create(&events_path).unwrap();
    writeln!(
        file,
        r#"{{"topic":"task.start","payload":"start","ts":"2024-01-01T00:00:00Z"}}"#
    )
    .unwrap();
    file.flush().unwrap();

    let processed = event_loop.process_events_from_jsonl().unwrap();
    assert!(processed.had_events);
    assert!(
        !processed.had_plan_events,
        "semantic plan.* flag should remain false when no plan topics were published"
    );
    assert!(
        processed.human_interact_context.is_none(),
        "non-human batches should not expose human.interact metadata"
    );
}

/// Regression: when agent writes a non-orphan event (one whose topic IS a trigger for
/// a hat), the caller must NOT inject default_publishes. This test replicates the exact
/// caller logic from loop_runner.rs to detect the mismatch between has_orphans and had_events.
///
/// Before the fix, `process_events_from_jsonl` returned a single bool = has_orphans.
/// For non-orphan events (e.g. task.start which triggers hat-a), has_orphans was false,
/// causing the caller to think "no events were written" and inject default_publishes.
#[test]
fn test_default_publishes_skipped_when_non_orphan_event_written() {
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    let mut hats = HashMap::new();
    // hat-a triggers on task.start → task.start is NOT an orphan
    hats.insert(
        "hat-a".to_string(),
        crate::config::HatConfig {
            name: "hat-a".to_string(),
            description: Some("Hat triggered by task.start".to_string()),
            triggers: vec!["task.start".to_string()],
            publishes: vec!["task.done".to_string()],
            instructions: "Do the task".to_string(),
            extra_instructions: vec![],
            backend_args: None,
            backend: None,
            default_publishes: Some("task.done".to_string()),
            max_activations: None,
            scratchpad: None,
            disallowed_tools: vec![],
            timeout: None,
            concurrency: 1,
            aggregate: None,
        },
    );
    config.hats = hats;

    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    let hat_id = HatId::new("hat-a");

    // Consume the initial event from initialize so pending state starts clean
    let _ = event_loop.build_prompt(&hat_id);

    // Agent writes a non-orphan event (task.start → triggers hat-a)
    let mut file = std::fs::File::create(&events_path).unwrap();
    writeln!(
        file,
        r#"{{"topic":"task.start","ts":"2024-01-01T00:00:00Z","payload":"starting work"}}"#
    )
    .unwrap();
    file.flush().unwrap();

    // Process events — this is what the event loop calls
    let result = event_loop.process_events_from_jsonl().unwrap();

    // The caller in loop_runner.rs uses `had_events` to decide whether to inject defaults:
    //   let agent_wrote_events = result.had_events;
    //   if !agent_wrote_events { check_default_publishes(...) }
    //
    // Before the fix, the return was a single bool (= has_orphans). For a non-orphan
    // event like task.start, has_orphans=false, so the caller would see
    // agent_wrote_events=false and incorrectly inject default_publishes.
    assert!(
        result.had_events,
        "had_events must be true when agent wrote events (even non-orphan ones)"
    );
    // Also verify has_orphans is false — this was the old return value that got conflated
    assert!(
        !result.has_orphans,
        "has_orphans should be false for non-orphan events"
    );
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
            extra_instructions: vec![],
            backend_args: None,
            backend: None,
            default_publishes: None, // No default configured
            max_activations: None,
            scratchpad: None,
            disallowed_tools: vec![],
            timeout: None,
            concurrency: 1,
            aggregate: None,
        },
    );
    config.hats = hats;

    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    let hat_id = HatId::new("test-hat");

    // Consume the initial event from initialize
    let _ = event_loop.build_prompt(&hat_id);

    // Agent wrote no events
    let result = event_loop.process_events_from_jsonl().unwrap();
    assert!(!result.had_events);

    // check_default_publishes should NOT inject since not configured
    event_loop.check_default_publishes(&hat_id);

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

    // Should contain RFC2119-style Ralph identity (uses "You are Ralph")
    assert!(
        prompt.contains("You are Ralph"),
        "Should identify as Ralph with RFC2119 style"
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
        "Should reference completion event"
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
fn test_wave_results_activate_synthesizer() {
    // Simulates the wave review scenario:
    // 1. Coordinator dispatches review.perspective (wave events)
    // 2. After wave, review.done events are published to bus
    // 3. On next iteration, synthesizer should be the active hat
    let yaml = r#"
hats:
  coordinator:
    name: "Coordinator"
    triggers: ["review.start"]
    publishes: ["review.perspective"]
    instructions: "Dispatch reviewers as a wave."
  reviewer:
    name: "Reviewer"
    triggers: ["review.perspective"]
    publishes: ["review.done"]
    concurrency: 3
    instructions: "Review code from your specialty."
  synthesizer:
    name: "Synthesizer"
    triggers: ["review.done"]
    publishes: ["review.complete"]
    instructions: "SYNTHESIZER MODE - Aggregate all review.done findings into a report."
    aggregate:
      mode: wait_for_all
      timeout: 300
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    // Step 1: Initialize with review.start — coordinator activates
    event_loop.initialize("Review the code");
    assert_eq!(event_loop.next_hat().unwrap().as_str(), "ralph");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();
    assert!(
        prompt.contains("Coordinator"),
        "Should activate coordinator for review.start"
    );

    // Step 2: Simulate wave results — publish review.done events directly to bus
    // (this is what loop_runner does after merge_wave_results_to_events_file + re-read)
    event_loop
        .bus
        .publish(Event::new("review.done", "Rust review findings"));
    event_loop
        .bus
        .publish(Event::new("review.done", "Frontend review findings"));
    event_loop
        .bus
        .publish(Event::new("review.done", "Docs review findings"));

    // Step 3: next_hat should find pending events and return ralph
    assert!(
        event_loop.next_hat().is_some(),
        "Should have pending events for next hat"
    );

    // Step 4: build_prompt should activate synthesizer (not coordinator)
    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert!(
        prompt.contains("SYNTHESIZER MODE"),
        "Should activate synthesizer for review.done events"
    );
    assert!(
        !prompt.contains("Dispatch reviewers"),
        "Should NOT have coordinator instructions"
    );
    assert!(
        prompt.contains("review.done"),
        "Should contain review.done events in context"
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
fn test_active_hat_gets_publishing_guide_not_topology() {
    // When a hat is triggered, show its instructions + Event Publishing Guide
    // Skip the topology table/Mermaid to reduce token usage
    let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start", "build.done", "build.blocked"]
    publishes: ["build.task"]
  builder:
    name: "Builder"
    description: "Builds code"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test"); // Publishes task.start which triggers Planner

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    // Planner is active (triggered by task.start), so we get ACTIVE HAT section
    assert!(
        prompt.contains("## ACTIVE HAT"),
        "Should have ACTIVE HAT section when hat is triggered"
    );

    // Should NOT have topology table when a hat is active
    assert!(
        !prompt.contains("| Hat | Triggers On | Publishes |"),
        "Should NOT have topology table when hat is active"
    );
    assert!(
        !prompt.contains("```mermaid"),
        "Should NOT have Mermaid diagram when hat is active"
    );

    // Should have Event Publishing Guide showing who receives build.task
    assert!(
        prompt.contains("### Event Publishing Guide"),
        "Should have Event Publishing Guide for active hat"
    );
    assert!(
        prompt.contains("`build.task` → Received by: Builder"),
        "Should show Builder receives build.task"
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
    // Verify Ralph's prompt includes downstream events from all hats when in multi-hat mode.
    // Kickoff events like task.start should drop out once a more specific downstream event exists.
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

    // The downstream event should be in Ralph's context, and the kickoff event
    // should not dominate once downstream work is pending.
    assert!(
        !prompt.contains("task.start"),
        "task.start should be filtered once a downstream event is pending"
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

#[test]
fn test_get_active_hat_id_deterministic_with_multiple_pending() {
    // Two hats with pending events → get_active_hat_id returns alphabetically first matching hat
    let yaml = r#"
hats:
  zebra_hat:
    name: "Zebra"
    triggers: ["work.*"]
  alpha_hat:
    name: "Alpha"
    triggers: ["work.*"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    // Publish event that both hats subscribe to
    event_loop
        .bus
        .publish(Event::new("work.start", "Begin work"));

    // Should deterministically return "alpha_hat" (alphabetically first)
    let active = event_loop.get_active_hat_id();
    assert_eq!(
        active.as_str(),
        "alpha_hat",
        "get_active_hat_id should return alphabetically first matching hat"
    );

    // Run multiple times to confirm determinism
    for _ in 0..100 {
        let active = event_loop.get_active_hat_id();
        assert_eq!(active.as_str(), "alpha_hat");
    }
}

#[test]
fn test_get_active_hat_id_matches_prompt_active_hat_selection() {
    let yaml = r#"
hats:
  investigator:
    name: "Investigator"
    triggers: ["debug.start", "hypothesis.confirmed"]
  tester:
    name: "Tester"
    triggers: ["hypothesis.test"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    event_loop
        .bus
        .publish(Event::new("debug.start", "Investigate a bug"));
    event_loop
        .bus
        .publish(Event::new("hypothesis.test", "Test the hypothesis"));

    let preview_active_hat = event_loop.get_active_hat_id();

    event_loop
        .build_prompt(&HatId::new("ralph"))
        .expect("prompt should build");

    let built_active_hat = event_loop
        .state
        .last_active_hat_ids
        .first()
        .expect("build_prompt should set active hats")
        .clone();

    assert_eq!(
        preview_active_hat.as_str(),
        "tester",
        "downstream hypothesis.test should outrank kickoff debug.start in preview selection"
    );
    assert_eq!(
        built_active_hat.as_str(),
        "tester",
        "build_prompt should select tester when debug.start and hypothesis.test are both pending"
    );
    assert_eq!(
        preview_active_hat, built_active_hat,
        "display hat preview should match prompt-selected active hat"
    );
}

#[test]
fn test_get_active_hat_id_prefers_semantic_event_over_targeted_task_resume() {
    let yaml = r#"
hats:
  investigator:
    name: "Investigator"
    triggers: ["task.resume", "debug.start", "hypothesis.confirmed"]
  tester:
    name: "Tester"
    triggers: ["hypothesis.test"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    event_loop
        .bus
        .publish(Event::new("task.resume", "Recovery").with_target("investigator"));
    event_loop
        .bus
        .publish(Event::new("hypothesis.test", "Test the hypothesis"));

    let preview_active_hat = event_loop.get_active_hat_id();
    assert_eq!(
        preview_active_hat.as_str(),
        "tester",
        "semantic downstream work should outrank fallback task.resume for display selection"
    );

    event_loop
        .build_prompt(&HatId::new("ralph"))
        .expect("prompt should build");

    let built_active_hat = event_loop
        .state
        .last_active_hat_ids
        .first()
        .expect("build_prompt should set active hats")
        .clone();
    assert_eq!(
        built_active_hat.as_str(),
        "tester",
        "prompt-selected active hat should ignore fallback task.resume when real work is pending"
    );
}

#[test]
fn test_get_active_hat_id_honors_direct_target_before_topic_lookup() {
    let yaml = r#"
hats:
  alpha_hat:
    name: "Alpha"
    triggers: ["task.resume"]
  zebra_hat:
    name: "Zebra"
    triggers: ["task.resume"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    event_loop
        .bus
        .publish(Event::new("task.resume", "Recovery").with_target("zebra_hat"));

    let active_hat_id = event_loop.get_active_hat_id();
    assert_eq!(
        active_hat_id.as_str(),
        "zebra_hat",
        "direct event targets should override generic topic subscriber ordering"
    );
}

#[test]
fn test_determine_active_hat_ids_excludes_entrypoint_hats_when_progressed_events_exist() {
    // When both a stale entrypoint event (review.start) and a progressed event
    // (review.done) are pending, only the downstream hat should be activated —
    // not the entrypoint hat. This prevents the coordinator from being
    // re-included alongside the synthesizer after wave workers complete.
    let yaml = r#"
event_loop:
  starting_event: "review.start"
  completion_promise: "review.complete"
hats:
  coordinator:
    name: "Coordinator"
    triggers: ["review.start"]
    publishes: ["review.perspective"]
  synthesizer:
    name: "Synthesizer"
    triggers: ["review.done"]
    publishes: ["review.complete"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Review the auth module");

    // Simulate state after wave workers complete: stale review.start +
    // new review.done events are both pending.
    let events = vec![
        Event::new("review.start", "Review the auth module"),
        Event::new("review.done", "## Rust Review\n..."),
        Event::new("review.done", "## Frontend Review\n..."),
    ];

    let active = event_loop.determine_active_hat_ids(&events);
    assert_eq!(
        active.len(),
        1,
        "Only the synthesizer should be active, not coordinator + synthesizer"
    );
    assert_eq!(
        active[0].as_str(),
        "synthesizer",
        "The synthesizer (triggered by review.done) should be selected over the coordinator (triggered by stale review.start)"
    );
}

#[test]
fn test_determine_active_hat_ids_falls_back_to_entrypoint_when_no_progressed_events() {
    // When only entrypoint events are pending, the entrypoint hat should be activated.
    let yaml = r#"
event_loop:
  starting_event: "review.start"
hats:
  coordinator:
    name: "Coordinator"
    triggers: ["review.start"]
    publishes: ["review.perspective"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Review the auth module");

    let events = vec![Event::new("review.start", "Review the auth module")];

    let active = event_loop.determine_active_hat_ids(&events);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].as_str(), "coordinator");
}

#[test]
fn test_check_for_user_prompt_detects_user_prompt_event() {
    // Create EventLoop
    let config: RalphConfig = serde_yaml::from_str("hats: {}").unwrap();
    let event_loop = EventLoop::new(config);

    // Create events with a user.prompt event
    // The id is embedded in the XML payload
    let events = vec![
        Event::new("build.task", "Some task"),
        Event::new(
            "user.prompt",
            r#"<event topic="user.prompt" id="q1">What is the feature name?</event>"#,
        ),
        Event::new("other.event", "Other"),
    ];

    // Check for user prompt
    let user_prompt = event_loop.check_for_user_prompt(&events);

    assert!(user_prompt.is_some(), "Should detect user.prompt event");
    assert_eq!(user_prompt.unwrap().id, "q1");
}

#[test]
fn test_check_for_user_prompt_returns_none_when_no_user_prompt() {
    // Create EventLoop
    let config: RalphConfig = serde_yaml::from_str("hats: {}").unwrap();
    let event_loop = EventLoop::new(config);

    // Create events WITHOUT a user.prompt event
    let events = vec![
        Event::new("build.task", "Some task"),
        Event::new("build.done", "Task completed"),
    ];

    // Check for user prompt
    let user_prompt = event_loop.check_for_user_prompt(&events);

    assert!(
        user_prompt.is_none(),
        "Should not detect user.prompt when not present"
    );
}

#[test]
fn test_extract_prompt_id_from_xml_format() {
    // Create EventLoop
    let config: RalphConfig = serde_yaml::from_str("hats: {}").unwrap();
    let event_loop = EventLoop::new(config);

    // Create event with XML attribute format
    let event = Event::new(
        "user.prompt",
        r#"<event topic="user.prompt" id="q42">What's the deadline?</event>"#,
    );
    let events = vec![event];

    let user_prompt = event_loop.check_for_user_prompt(&events).unwrap();
    assert_eq!(user_prompt.id, "q42");
}

// Note: Orphan event detection is now handled in loop_runner.rs::log_events_from_output()
// which logs to events.jsonl. The `event.orphaned` system event appears in the events file
// when an event has no subscriber hat, making it visible via `ralph events`.

// === Objective Persistence Tests ===

#[test]
fn test_initialize_stores_objective_in_ralph() {
    // initialize() should store the prompt as the objective in HatlessRalph
    // so that subsequent iterations always see it, even after bus.take_pending() consumes the start event.
    let yaml = r#"
hats:
  test_writer:
    name: "Test Writer"
    triggers: ["tdd.start"]
    publishes: ["test.written"]
    instructions: "Write failing tests."
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    event_loop.initialize("Implement a binary search tree with insert and search");

    // Consume the start event (simulates iteration 1 completing)
    let ralph_id = HatId::new("ralph");
    let prompt1 = event_loop.build_prompt(&ralph_id).unwrap();
    assert!(
        prompt1.contains("## OBJECTIVE"),
        "Iteration 1 should have OBJECTIVE section"
    );
    assert!(
        prompt1.contains("Implement a binary search tree"),
        "Iteration 1 should show the objective"
    );

    // Simulate iteration 2: hat publishes an event, start event is gone
    event_loop
        .bus
        .publish(Event::new("test.written", "tests/bst_test.rs"));

    let prompt2 = event_loop.build_prompt(&ralph_id).unwrap();

    // Objective should STILL be present even though task.start was consumed
    assert!(
        prompt2.contains("## OBJECTIVE"),
        "Iteration 2+ should still have OBJECTIVE section"
    );
    assert!(
        prompt2.contains("Implement a binary search tree"),
        "Objective should persist across iterations"
    );
}

#[test]
fn test_done_section_suppressed_for_active_hat_via_event_loop() {
    // When a hat is active (triggered by an event), the DONE section should NOT appear.
    // This prevents intermediate hats from seeing completion instructions.
    let yaml = r#"
hats:
  implementer:
    name: "Implementer"
    triggers: ["test.written"]
    publishes: ["test.passing"]
    instructions: "Make the failing test pass."
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Build a calculator");

    // Consume the start event
    let ralph_id = HatId::new("ralph");
    let _ = event_loop.build_prompt(&ralph_id);

    // Simulate implementer being triggered
    event_loop
        .bus
        .publish(Event::new("test.written", "tests/calc_test.rs"));

    let prompt = event_loop.build_prompt(&ralph_id).unwrap();

    // Implementer hat is active — DONE section should be suppressed
    assert!(
        !prompt.contains("## DONE"),
        "DONE section should be suppressed when a hat is active"
    );
    assert!(
        !prompt.contains("You MUST emit a completion event"),
        "Completion instruction should not appear for active hat"
    );

    // But the objective should still be visible
    assert!(
        prompt.contains("## OBJECTIVE"),
        "OBJECTIVE should still be visible to active hat"
    );
    assert!(
        prompt.contains("Build a calculator"),
        "Objective content should be visible"
    );
}

// === Mutant-killing tests ===

#[test]
fn test_consecutive_failures_increments_on_failed_output() {
    // Kills: line 928 `+= 1` → `-=` / `*=`
    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

    let ralph = HatId::new("ralph");

    event_loop.process_output(&ralph, "output", false);
    assert_eq!(event_loop.state.consecutive_failures, 1);

    event_loop.process_output(&ralph, "output", false);
    assert_eq!(event_loop.state.consecutive_failures, 2);
}

#[test]
fn test_consecutive_failures_resets_on_success() {
    // Kills: line 926 reset branch
    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

    let ralph = HatId::new("ralph");

    event_loop.process_output(&ralph, "output", false);
    assert_eq!(event_loop.state.consecutive_failures, 1);

    event_loop.process_output(&ralph, "output", true);
    assert_eq!(event_loop.state.consecutive_failures, 0);
}

#[test]
fn test_cost_based_termination() {
    // Kills: line 383 `>=` → `<`, lines 987 `add_cost` noop / `-=` / `*=`
    let yaml = r"
event_loop:
  max_cost_usd: 10.0
";
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);

    event_loop.add_cost(9.99);
    assert_eq!(
        event_loop.check_termination(),
        None,
        "Should NOT terminate below max cost"
    );

    event_loop.add_cost(0.01);
    assert_eq!(
        event_loop.check_termination(),
        Some(TerminationReason::MaxCost),
        "Should terminate at exactly max cost"
    );
}

#[test]
fn test_malformed_events_increment_counter() {
    // Kills: line 1063 `+= 1` → `-=` / `*=`
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    // Write invalid JSONL
    std::fs::write(&events_path, "not valid json\n").unwrap();
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(
        event_loop.state.consecutive_malformed_events, 1,
        "First malformed line should set counter to 1"
    );

    // Write another invalid line (append)
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&events_path)
        .unwrap();
    writeln!(file, "also not json").unwrap();
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(
        event_loop.state.consecutive_malformed_events, 2,
        "Second malformed line should set counter to 2"
    );
}

#[test]
fn test_malformed_counter_resets_on_valid_event() {
    // Kills: line 1072 `!` deletion
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.initialize("Test");

    // Write invalid JSONL
    std::fs::write(&events_path, "not valid json\n").unwrap();
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(event_loop.state.consecutive_malformed_events, 1);

    // Write a valid event
    write_event_to_jsonl(&events_path, "build.done", "success");
    let _ = event_loop.process_events_from_jsonl();
    assert_eq!(
        event_loop.state.consecutive_malformed_events, 0,
        "Counter should reset when valid events are parsed"
    );
}

#[test]
fn test_validation_failure_termination_at_threshold() {
    // Kills: line 1165 `>=` → `<` and `&&` → `||`
    // (Note: line 1165 refers to validation threshold at line 398)
    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);

    event_loop.state.consecutive_malformed_events = 2;
    assert_eq!(
        event_loop.check_termination(),
        None,
        "Should NOT terminate at 2 malformed events (threshold is 3)"
    );

    event_loop.state.consecutive_malformed_events = 3;
    assert_eq!(
        event_loop.check_termination(),
        Some(TerminationReason::ValidationFailure),
        "Should terminate at 3 malformed events"
    );
}

#[test]
fn test_stop_requested_termination_clears_signal() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();
    let event_loop = EventLoop::new(config);

    let stop_path = temp_dir.path().join(".ralph/stop-requested");
    std::fs::create_dir_all(stop_path.parent().unwrap()).unwrap();
    std::fs::write(&stop_path, "").unwrap();

    assert_eq!(
        event_loop.check_termination(),
        Some(TerminationReason::Stopped),
        "Should terminate when stop requested signal exists"
    );
    assert!(
        !stop_path.exists(),
        "Stop signal should be removed after detection"
    );
}

#[test]
fn test_format_event_wraps_top_level_prompts() {
    // Kills: line 761 `==` → `!=` and `||` → `&&`
    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Build a web server");

    let ralph = HatId::new("ralph");
    let prompt = event_loop.build_prompt(&ralph).unwrap();

    // task.start event should be wrapped in <top-level-prompt>
    assert!(
        prompt.contains("<top-level-prompt>"),
        "task.start events should be wrapped in <top-level-prompt> tags"
    );

    // Consume the start event, publish a non-top-level event
    event_loop
        .bus
        .publish(Event::new("build.done", "completed"));
    let prompt2 = event_loop.build_prompt(&ralph).unwrap();

    // build.done is NOT a top-level prompt, should NOT have the tag
    assert!(
        !prompt2.contains("<top-level-prompt>"),
        "Non-top-level events should NOT be wrapped in <top-level-prompt> tags"
    );
}

#[test]
fn test_check_ralph_completion_detection() {
    // Kills: line 1241 return `true` / `false`
    let config = RalphConfig::default();
    let event_loop = EventLoop::new(config);

    assert!(
        event_loop.check_ralph_completion(r#"<event topic="LOOP_COMPLETE">done</event>"#),
        "Should detect completion event"
    );
    assert!(
        !event_loop.check_ralph_completion("LOOP_COMPLETE\nMore text"),
        "Completion requires emitted event, not plain text"
    );
    assert!(
        !event_loop.check_ralph_completion("no match here"),
        "Should not detect completion in unrelated text"
    );
}

#[test]
fn test_scratchpad_injection_with_content() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let scratchpad_path = temp_dir.path().join(".ralph/agent/scratchpad.md");
    std::fs::create_dir_all(scratchpad_path.parent().unwrap()).unwrap();
    std::fs::write(
        &scratchpad_path,
        "## Progress\n- [x] Step 1\n- [ ] Step 2\n",
    )
    .unwrap();

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();

    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test prompt");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert!(
        prompt.contains("<scratchpad"),
        "Prompt should contain scratchpad header"
    );
    assert!(
        prompt.contains("Step 1"),
        "Prompt should contain scratchpad content"
    );
    assert!(
        prompt.contains("Step 2"),
        "Prompt should contain scratchpad content"
    );
}

#[test]
fn test_scratchpad_injection_no_file() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    // Do NOT create scratchpad file

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();

    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test prompt");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert!(
        !prompt.contains("<scratchpad path="),
        "Prompt should NOT contain scratchpad injection when file doesn't exist"
    );
}

#[test]
fn test_scratchpad_injection_empty_file() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let scratchpad_path = temp_dir.path().join(".ralph/agent/scratchpad.md");
    std::fs::create_dir_all(scratchpad_path.parent().unwrap()).unwrap();
    std::fs::write(&scratchpad_path, "   \n\n  ").unwrap();

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();

    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test prompt");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert!(
        !prompt.contains("<scratchpad path="),
        "Prompt should NOT contain scratchpad injection when file is empty/whitespace"
    );
}

#[test]
fn test_scratchpad_injection_ordering() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let scratchpad_path = temp_dir.path().join(".ralph/agent/scratchpad.md");
    std::fs::create_dir_all(scratchpad_path.parent().unwrap()).unwrap();
    std::fs::write(&scratchpad_path, "scratchpad marker content").unwrap();

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();

    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test prompt");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    let scratchpad_pos = prompt
        .find("<scratchpad")
        .expect("Should contain scratchpad");
    let orientation_pos = prompt
        .find("### 0a. ORIENTATION")
        .expect("Should contain orientation");

    assert!(
        scratchpad_pos < orientation_pos,
        "Scratchpad should appear before ORIENTATION in the prompt"
    );
}

#[test]
fn test_scratchpad_injection_tail_truncation() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let scratchpad_path = temp_dir.path().join(".ralph/agent/scratchpad.md");
    std::fs::create_dir_all(scratchpad_path.parent().unwrap()).unwrap();

    // Create content exceeding 16000 chars (4000 tokens * 4 chars/token)
    // Include markdown headings so truncation summary captures them
    let mut large_content = String::new();
    large_content.push_str("### Initial Analysis\n\n");
    for i in 0..500 {
        large_content.push_str(&format!("Line {}: some padding content here\n", i));
    }
    large_content.push_str("### Research Phase\n\n");
    for i in 500..1000 {
        large_content.push_str(&format!("Line {}: some padding content here\n", i));
    }
    large_content.push_str("### Implementation Notes\n\n");
    for i in 1000..2000 {
        large_content.push_str(&format!("Line {}: some padding content here\n", i));
    }
    assert!(
        large_content.len() > 16000,
        "Test content should exceed budget"
    );
    std::fs::write(&scratchpad_path, &large_content).unwrap();

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();

    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test prompt");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert!(
        prompt.contains("<scratchpad"),
        "Prompt should contain scratchpad header even when truncated"
    );
    assert!(
        prompt.contains("earlier content truncated"),
        "Prompt should indicate truncation occurred"
    );
    // Discarded headings should be summarized
    assert!(
        prompt.contains("discarded sections:"),
        "Prompt should summarize discarded section headings"
    );
    assert!(
        prompt.contains("### Initial Analysis"),
        "Prompt should list the discarded heading"
    );
    // The tail (most recent lines) should be kept
    assert!(
        prompt.contains("Line 1999"),
        "Last line should be preserved (tail kept)"
    );
    // Early lines should be truncated
    assert!(
        !prompt.contains("Line 0:"),
        "First line should be truncated (head removed)"
    );
}

#[test]
fn test_build_done_backpressure_accepts_mutants_warning() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass\nmutants: warn (65%)";
    write_event_to_jsonl(&events_path, "build.done", payload);
    let _ = event_loop.process_events_from_jsonl();

    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"build.done".to_string()),
        "build.done with mutants warning should pass through. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"build.blocked".to_string()),
        "build.done should not be blocked by mutation warnings"
    );
}

#[test]
fn test_build_done_backpressure_rejects_high_complexity() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 12\nduplication: pass";
    write_event_to_jsonl(&events_path, "build.done", payload);
    let _ = event_loop.process_events_from_jsonl();

    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"build.blocked".to_string()),
        "build.done with high complexity should be blocked. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"build.done".to_string()),
        "build.done should not pass through when complexity is too high"
    );
}

#[test]
fn test_build_done_backpressure_rejects_duplication() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: fail";
    write_event_to_jsonl(&events_path, "build.done", payload);
    let _ = event_loop.process_events_from_jsonl();

    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"build.blocked".to_string()),
        "build.done with duplication should be blocked. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"build.done".to_string()),
        "build.done should not pass through when duplication fails"
    );
}

#[test]
fn test_build_done_backpressure_rejects_performance_regression() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: regression";
    write_event_to_jsonl(&events_path, "build.done", payload);
    let _ = event_loop.process_events_from_jsonl();

    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"build.blocked".to_string()),
        "build.done with performance regression should be blocked. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"build.done".to_string()),
        "build.done should not pass through when performance regresses"
    );
}

#[test]
fn test_review_done_backpressure_accepts_verified() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Write a review.done event WITH verification evidence
    write_event_to_jsonl(&events_path, "review.done", "tests: pass\nbuild: pass");
    let _ = event_loop.process_events_from_jsonl();

    // Should pass through as review.done (not blocked)
    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"review.done".to_string()),
        "Verified review.done should pass through. Got: {:?}",
        pending_topics
    );
}

#[test]
fn test_review_done_backpressure_rejects_unverified() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Write a review.done event WITHOUT verification evidence
    write_event_to_jsonl(&events_path, "review.done", "Looks good, approved!");
    let _ = event_loop.process_events_from_jsonl();

    // Should be transformed into review.blocked
    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"review.blocked".to_string()),
        "Unverified review.done should be blocked. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"review.done".to_string()),
        "review.done should not pass through without evidence"
    );
}

#[test]
fn test_review_done_backpressure_rejects_failed_checks() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Write a review.done event with failed checks
    write_event_to_jsonl(&events_path, "review.done", "tests: fail\nbuild: pass");
    let _ = event_loop.process_events_from_jsonl();

    // Should be transformed into review.blocked
    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"review.blocked".to_string()),
        "review.done with failed tests should be blocked. Got: {:?}",
        pending_topics
    );
}

#[test]
fn test_verify_passed_backpressure_accepts_quality_report() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let payload = "quality.tests: pass\nquality.coverage: 82%\nquality.lint: pass\nquality.audit: pass\nquality.mutation: 72%\nquality.complexity: 7";
    write_event_to_jsonl(&events_path, "verify.passed", payload);
    let _ = event_loop.process_events_from_jsonl();

    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"verify.passed".to_string()),
        "verify.passed with quality report should pass through. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"verify.failed".to_string()),
        "verify.passed should not be blocked by quality report"
    );
}

#[test]
fn test_verify_passed_backpressure_rejects_missing_quality_report() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    write_event_to_jsonl(&events_path, "verify.passed", "All good");
    let _ = event_loop.process_events_from_jsonl();

    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"verify.failed".to_string()),
        "verify.passed without quality report should be blocked. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"verify.passed".to_string()),
        "verify.passed should not pass through without quality report"
    );
}

#[test]
fn test_verify_passed_backpressure_rejects_failed_thresholds() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    let payload = "quality.tests: pass\nquality.coverage: 60%\nquality.lint: pass\nquality.audit: pass\nquality.mutation: 50%\nquality.complexity: 12";
    write_event_to_jsonl(&events_path, "verify.passed", payload);
    let _ = event_loop.process_events_from_jsonl();

    let empty = Vec::new();
    let pending_topics: Vec<String> = event_loop
        .bus
        .hat_ids()
        .flat_map(|id| {
            event_loop
                .bus
                .peek_pending(id)
                .unwrap_or(&empty)
                .iter()
                .map(|e| e.topic.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        pending_topics.contains(&"verify.failed".to_string()),
        "verify.passed with failing thresholds should be blocked. Got: {:?}",
        pending_topics
    );
    assert!(
        !pending_topics.contains(&"verify.passed".to_string()),
        "verify.passed should not pass through with failing thresholds"
    );
}

// === RObot Interaction Skill Injection Tests ===

#[test]
fn test_inject_robot_skill_when_enabled() {
    let yaml = r#"
RObot:
  enabled: true
  telegram:
    bot_token: "fake-token"
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test prompt");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert!(
        prompt.contains("<robot-skill>"),
        "Prompt should contain <robot-skill> when RObot is enabled"
    );
    assert!(
        prompt.contains("human.interact"),
        "Robot skill should mention human.interact"
    );
    assert!(
        prompt.contains("</robot-skill>"),
        "Robot skill should have closing tag"
    );
}

#[test]
fn test_inject_robot_skill_skipped_when_disabled() {
    let config = RalphConfig::default(); // RObot disabled by default
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test prompt");

    let prompt = event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert!(
        !prompt.contains("<robot-skill>"),
        "Prompt should NOT contain <robot-skill> when RObot is disabled"
    );
}

#[test]
fn test_persistent_mode_suppresses_loop_complete() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    let agent_dir = temp_dir.path().join(".agent");
    fs::create_dir_all(&agent_dir).unwrap();
    let scratchpad_path = agent_dir.join("scratchpad.md");
    fs::write(&scratchpad_path, "## Tasks\n- [x] All done\n").unwrap();

    let mut config = RalphConfig::default();
    config.core.scratchpad.path = scratchpad_path.to_string_lossy().to_string();
    config.event_loop.persistent = true;
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

    let events_path = temp_dir.path().join("events.jsonl");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // LOOP_COMPLETE should NOT terminate in persistent mode
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason, None,
        "Persistent mode should suppress LOOP_COMPLETE termination"
    );

    // Verify a task.resume event was injected so the loop continues
    let ralph_id = HatId::new("ralph");
    let pending = event_loop.bus.peek_pending(&ralph_id);
    assert!(
        pending.is_some_and(|events| events
            .iter()
            .any(|e| e.topic.as_str() == "task.resume" && e.payload.contains("Persistent mode"))),
        "A task.resume event should be injected after suppressed LOOP_COMPLETE"
    );
}

#[test]
fn test_non_persistent_mode_terminates_on_loop_complete() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    let agent_dir = temp_dir.path().join(".agent");
    fs::create_dir_all(&agent_dir).unwrap();
    let scratchpad_path = agent_dir.join("scratchpad.md");
    fs::write(&scratchpad_path, "## Tasks\n- [x] All done\n").unwrap();

    let mut config = RalphConfig::default();
    config.core.scratchpad.path = scratchpad_path.to_string_lossy().to_string();
    // persistent defaults to false, but be explicit
    config.event_loop.persistent = false;
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

    let events_path = temp_dir.path().join("events.jsonl");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // LOOP_COMPLETE should terminate normally when not persistent
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "Non-persistent mode should terminate on LOOP_COMPLETE"
    );
}

#[test]
fn test_persistent_mode_still_respects_hard_limits() {
    let yaml = r"
event_loop:
  max_iterations: 2
  persistent: true
";
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.state.iteration = 2;

    // Hard limits should still terminate even in persistent mode
    assert_eq!(
        event_loop.check_termination(),
        Some(TerminationReason::MaxIterations),
        "Persistent mode should still respect max_iterations"
    );
}

#[test]
fn test_termination_reason_mappings() {
    let cases = vec![
        (TerminationReason::CompletionPromise, "completed", 0, true),
        (TerminationReason::MaxIterations, "max_iterations", 2, false),
        (TerminationReason::MaxRuntime, "max_runtime", 2, false),
        (TerminationReason::MaxCost, "max_cost", 2, false),
        (
            TerminationReason::ConsecutiveFailures,
            "consecutive_failures",
            1,
            false,
        ),
        (TerminationReason::LoopThrashing, "loop_thrashing", 1, false),
        (
            TerminationReason::ValidationFailure,
            "validation_failure",
            1,
            false,
        ),
        (TerminationReason::Stopped, "stopped", 1, false),
        (TerminationReason::Interrupted, "interrupted", 130, false),
        (
            TerminationReason::RestartRequested,
            "restart_requested",
            3,
            false,
        ),
    ];

    for (reason, expected_str, expected_code, is_success) in cases {
        assert_eq!(reason.as_str(), expected_str);
        assert_eq!(reason.exit_code(), expected_code);
        assert_eq!(reason.is_success(), is_success);
    }
}

#[test]
fn test_termination_status_texts() {
    let cases = vec![
        (
            TerminationReason::CompletionPromise,
            "All tasks completed successfully.",
        ),
        (
            TerminationReason::MaxIterations,
            "Stopped at iteration limit.",
        ),
        (TerminationReason::MaxRuntime, "Stopped at runtime limit."),
        (TerminationReason::MaxCost, "Stopped at cost limit."),
        (
            TerminationReason::ConsecutiveFailures,
            "Too many consecutive failures.",
        ),
        (
            TerminationReason::LoopThrashing,
            "Loop thrashing detected - same hat repeatedly blocked.",
        ),
        (
            TerminationReason::ValidationFailure,
            "Too many consecutive malformed JSONL events.",
        ),
        (TerminationReason::Stopped, "Manually stopped."),
        (TerminationReason::Interrupted, "Interrupted by signal."),
        (
            TerminationReason::RestartRequested,
            "Restarting by human request.",
        ),
    ];

    for (reason, expected) in cases {
        assert_eq!(termination_status_text(&reason), expected);
    }
}

#[test]
fn test_format_duration_variants() {
    use std::time::Duration;

    assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    assert_eq!(format_duration(Duration::from_secs(61)), "1m 1s");
    assert_eq!(format_duration(Duration::from_secs(3600)), "1h 0m 0s");
    assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m 1s");
}

#[test]
fn test_extract_task_id_first_line_and_default() {
    assert_eq!(
        EventLoop::extract_task_id(" task-123 \nMore details"),
        "task-123"
    );
    assert_eq!(EventLoop::extract_task_id(""), "unknown");
}

#[test]
fn test_mutation_warning_reason_variants() {
    let fail = MutationEvidence {
        status: MutationStatus::Fail,
        score_percent: Some(12.5),
    };
    assert_eq!(
        EventLoop::mutation_warning_reason(&fail, Some(80.0)).unwrap(),
        "mutation testing failed"
    );

    let warn = MutationEvidence {
        status: MutationStatus::Warn,
        score_percent: Some(65.5),
    };
    assert_eq!(
        EventLoop::mutation_warning_reason(&warn, Some(80.0)).unwrap(),
        "mutation score below threshold (65.50%)"
    );

    let unknown = MutationEvidence {
        status: MutationStatus::Unknown,
        score_percent: None,
    };
    assert_eq!(
        EventLoop::mutation_warning_reason(&unknown, Some(80.0)).unwrap(),
        "mutation testing status unknown"
    );

    let pass_low = MutationEvidence {
        status: MutationStatus::Pass,
        score_percent: Some(70.0),
    };
    assert_eq!(
        EventLoop::mutation_warning_reason(&pass_low, Some(80.0)).unwrap(),
        "mutation score 70.00% below threshold 80.00%"
    );

    let pass_missing = MutationEvidence {
        status: MutationStatus::Pass,
        score_percent: None,
    };
    assert_eq!(
        EventLoop::mutation_warning_reason(&pass_missing, Some(80.0)).unwrap(),
        "mutation score missing (threshold 80.00%)"
    );

    let pass_high = MutationEvidence {
        status: MutationStatus::Pass,
        score_percent: Some(95.0),
    };
    assert_eq!(
        EventLoop::mutation_warning_reason(&pass_high, Some(80.0)),
        None
    );

    let pass_no_threshold = MutationEvidence {
        status: MutationStatus::Pass,
        score_percent: Some(10.0),
    };
    assert_eq!(
        EventLoop::mutation_warning_reason(&pass_no_threshold, None),
        None
    );
}

#[test]
fn test_extract_prompt_id_prefers_xml_id() {
    let payload = r#"<event topic="user.prompt" id="q42">Question?</event>"#;
    assert_eq!(EventLoop::extract_prompt_id(payload), "q42");
}

#[test]
fn test_extract_prompt_id_fallback_prefix() {
    let id = EventLoop::extract_prompt_id("Plain question");
    assert!(id.starts_with('q'));
    assert!(id.len() > 1);
}

#[test]
fn test_check_for_user_prompt_extracts_id_and_text() {
    let event_loop = EventLoop::new(RalphConfig::default());
    let payload = r#"<event topic="user.prompt" id="q7">Need input</event>"#;
    let events = vec![
        Event::new("build.done", "ok"),
        Event::new("user.prompt", payload),
    ];

    let prompt = event_loop.check_for_user_prompt(&events).expect("prompt");
    assert_eq!(prompt.id, "q7");
    assert_eq!(prompt.text, payload);
}

#[test]
fn test_task_counts_and_open_task_list() {
    use crate::loop_context::LoopContext;
    use crate::task::{Task, TaskStatus};
    use crate::task_store::TaskStore;

    let temp_dir = tempfile::tempdir().unwrap();
    let loop_context = LoopContext::primary(temp_dir.path().to_path_buf());
    let event_loop = EventLoop::with_context(RalphConfig::default(), loop_context);

    let tasks_path = temp_dir.path().join(".ralph/agent/tasks.jsonl");
    let mut store = TaskStore::load(&tasks_path).unwrap();
    let mut closed = Task::new("Closed task".to_string(), 1);
    closed.status = TaskStatus::Closed;
    let open = Task::new("Open task".to_string(), 1);
    let open_id = open.id.clone();
    store.add(closed);
    store.add(open);
    store.save().unwrap();

    let (open_count, closed_count) = event_loop.count_tasks();
    assert_eq!(open_count, 1);
    assert_eq!(closed_count, 1);

    let open_list = event_loop.get_open_task_list();
    assert_eq!(open_list.len(), 1);
    assert!(open_list[0].contains(&open_id));
    assert!(open_list[0].contains("Open task"));
}

#[test]
fn test_verify_tasks_complete_missing_and_pending() {
    use crate::loop_context::LoopContext;
    use crate::task::Task;
    use crate::task_store::TaskStore;

    let temp_dir = tempfile::tempdir().unwrap();
    let loop_context = LoopContext::primary(temp_dir.path().to_path_buf());
    let event_loop = EventLoop::with_context(RalphConfig::default(), loop_context);

    // Missing tasks file should be treated as complete.
    assert!(event_loop.verify_tasks_complete().unwrap());

    let tasks_path = temp_dir.path().join(".ralph/agent/tasks.jsonl");
    let mut store = TaskStore::load(&tasks_path).unwrap();
    store.add(Task::new("Open task".to_string(), 1));
    store.save().unwrap();

    assert!(!event_loop.verify_tasks_complete().unwrap());
}

#[test]
fn test_verify_scratchpad_complete_variants() {
    use crate::loop_context::LoopContext;
    use std::fs;

    let temp_dir = tempfile::tempdir().unwrap();
    let loop_context = LoopContext::primary(temp_dir.path().to_path_buf());
    let event_loop = EventLoop::with_context(RalphConfig::default(), loop_context);

    assert!(event_loop.verify_scratchpad_complete().is_err());

    let scratchpad_path = temp_dir.path().join(".ralph/agent/scratchpad.md");
    fs::create_dir_all(scratchpad_path.parent().unwrap()).unwrap();
    fs::write(&scratchpad_path, "## Tasks\n- [ ] Pending\n").unwrap();
    assert!(!event_loop.verify_scratchpad_complete().unwrap());

    fs::write(&scratchpad_path, "## Tasks\n- [x] Done\n- [~] Cancelled\n").unwrap();
    assert!(event_loop.verify_scratchpad_complete().unwrap());
}

#[test]
fn test_termination_reason_exit_codes() {
    let cases = [
        (TerminationReason::CompletionPromise, 0),
        (TerminationReason::ConsecutiveFailures, 1),
        (TerminationReason::LoopThrashing, 1),
        (TerminationReason::ValidationFailure, 1),
        (TerminationReason::Stopped, 1),
        (TerminationReason::MaxIterations, 2),
        (TerminationReason::MaxRuntime, 2),
        (TerminationReason::MaxCost, 2),
        (TerminationReason::Interrupted, 130),
        (TerminationReason::RestartRequested, 3),
    ];

    for (reason, code) in cases {
        assert_eq!(reason.exit_code(), code, "{reason:?} exit code mismatch");
    }
}

#[test]
fn test_termination_reason_strings_and_flags() {
    let cases = [
        (TerminationReason::CompletionPromise, "completed", true),
        (TerminationReason::MaxIterations, "max_iterations", false),
        (TerminationReason::MaxRuntime, "max_runtime", false),
        (TerminationReason::MaxCost, "max_cost", false),
        (
            TerminationReason::ConsecutiveFailures,
            "consecutive_failures",
            false,
        ),
        (TerminationReason::LoopThrashing, "loop_thrashing", false),
        (
            TerminationReason::ValidationFailure,
            "validation_failure",
            false,
        ),
        (TerminationReason::Stopped, "stopped", false),
        (TerminationReason::Interrupted, "interrupted", false),
        (
            TerminationReason::RestartRequested,
            "restart_requested",
            false,
        ),
    ];

    for (reason, expected_str, is_success) in cases {
        assert_eq!(reason.as_str(), expected_str, "{reason:?} as_str mismatch");
        assert_eq!(
            reason.is_success(),
            is_success,
            "{reason:?} success mismatch"
        );
    }
}

#[test]
fn test_has_pending_human_events_detects_guidance() {
    let mut event_loop = EventLoop::new(RalphConfig::default());
    event_loop
        .bus
        .publish(Event::new("human.guidance", "Please focus on tests"));

    assert!(event_loop.has_pending_human_events());
}

#[test]
fn test_has_pending_human_events_ignores_non_human() {
    let mut event_loop = EventLoop::new(RalphConfig::default());
    event_loop.bus.publish(Event::new("task.start", "Do work"));

    assert!(!event_loop.has_pending_human_events());
}

#[test]
fn test_get_hat_publishes_returns_configured_topics() {
    let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start"]
    publishes: ["task.plan", "build.done"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let event_loop = EventLoop::new(config);

    let publishes = event_loop.get_hat_publishes(&HatId::new("planner"));
    assert_eq!(
        publishes,
        vec!["task.plan".to_string(), "build.done".to_string()]
    );

    let missing = event_loop.get_hat_publishes(&HatId::new("missing"));
    assert!(missing.is_empty());
}

#[test]
fn test_inject_fallback_event_targets_last_hat() {
    let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.resume"]
    publishes: ["task.plan"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    let planner_id = HatId::new("planner");

    event_loop.state.last_hat = Some(planner_id.clone());
    assert!(event_loop.inject_fallback_event());

    let pending = event_loop
        .bus
        .peek_pending(&planner_id)
        .expect("planner pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].topic.as_str(), "task.resume");
    assert_eq!(
        pending[0].target.as_ref().map(|id| id.as_str()),
        Some("planner")
    );
    assert!(
        pending[0]
            .payload
            .contains("Previous iteration by hat `planner` did not publish an event"),
        "Fallback payload should name the stalled hat"
    );
    assert!(
        pending[0].payload.contains("Allowed topics: `task.plan`"),
        "Fallback payload should list allowed publish topics"
    );

    let ralph_id = HatId::new("ralph");
    let ralph_pending = event_loop.bus.peek_pending(&ralph_id);
    assert!(ralph_pending.is_none_or(|events| events.is_empty()));
}

#[test]
fn test_inject_fallback_event_defaults_to_ralph() {
    let mut event_loop = EventLoop::new(RalphConfig::default());
    event_loop.state.last_hat = None;

    assert!(event_loop.inject_fallback_event());

    let ralph_id = HatId::new("ralph");
    let pending = event_loop
        .bus
        .peek_pending(&ralph_id)
        .expect("ralph pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].topic.as_str(), "task.resume");
    assert!(pending[0].target.is_none());
    assert!(pending[0].payload.contains("Review the scratchpad"));
}

#[test]
fn test_paths_use_loop_context_when_present() {
    use crate::loop_context::LoopContext;

    let temp_dir = tempfile::tempdir().unwrap();
    let loop_context = LoopContext::primary(temp_dir.path().to_path_buf());
    let event_loop = EventLoop::with_context(RalphConfig::default(), loop_context);

    assert_eq!(
        event_loop.tasks_path(),
        temp_dir.path().join(".ralph/agent/tasks.jsonl")
    );
    assert_eq!(
        event_loop.scratchpad_path(),
        temp_dir.path().join(".ralph/agent/scratchpad.md")
    );
}

#[test]
fn test_custom_scratchpad_overrides_loop_context() {
    use crate::loop_context::LoopContext;

    let temp_dir = tempfile::tempdir().unwrap();
    let loop_context = LoopContext::primary(temp_dir.path().to_path_buf());
    let mut config = RalphConfig::default();
    config.core.scratchpad.path = ".ralph/debug/global.md".to_string();

    let event_loop = EventLoop::with_context(config, loop_context);

    // Custom scratchpad path should be resolved relative to loop context workspace
    assert_eq!(
        event_loop.scratchpad_path(),
        temp_dir.path().join(".ralph/debug/global.md"),
        "Custom scratchpad in config should be resolved relative to workspace"
    );
}

#[test]
fn test_paths_fallback_to_config_when_no_context() {
    let temp_dir = tempfile::tempdir().unwrap();
    let scratchpad_path = temp_dir.path().join("scratchpad.md");
    let mut config = RalphConfig::default();
    config.core.scratchpad.path = scratchpad_path.to_string_lossy().to_string();

    let event_loop = EventLoop::new(config);

    assert_eq!(
        event_loop.tasks_path(),
        std::path::PathBuf::from(".ralph/agent/tasks.jsonl")
    );
    assert_eq!(event_loop.scratchpad_path(), scratchpad_path);
}

#[test]
fn test_record_hat_activations_increments_counts() {
    let mut event_loop = EventLoop::new(RalphConfig::default());
    let planner = HatId::new("planner");
    let reviewer = HatId::new("reviewer");

    event_loop.record_hat_activations(&[planner.clone(), reviewer.clone()]);
    event_loop.record_hat_activations(std::slice::from_ref(&planner));

    assert_eq!(
        event_loop.state.hat_activation_counts.get(&planner),
        Some(&2)
    );
    assert_eq!(
        event_loop.state.hat_activation_counts.get(&reviewer),
        Some(&1)
    );
}

#[test]
fn test_check_hat_exhaustion_emits_once_at_limit() {
    let yaml = r#"
hats:
  reviewer:
    name: "Reviewer"
    triggers: ["review.done"]
    publishes: ["review.blocked"]
    max_activations: 2
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    let hat_id = HatId::new("reviewer");
    let dropped = vec![
        Event::new("review.done", "ok"),
        Event::new("build.done", "ok"),
    ];

    event_loop
        .state
        .hat_activation_counts
        .insert(hat_id.clone(), 1);
    let (drop, event) = event_loop.check_hat_exhaustion(&hat_id, &dropped);
    assert!(!drop);
    assert!(event.is_none());

    event_loop
        .state
        .hat_activation_counts
        .insert(hat_id.clone(), 2);
    let (drop, event) = event_loop.check_hat_exhaustion(&hat_id, &dropped);
    assert!(drop);
    let exhausted = event.expect("exhausted event");
    assert_eq!(exhausted.topic.as_str(), "reviewer.exhausted");
    assert!(exhausted.payload.contains("max_activations: 2"));
    assert!(exhausted.payload.contains("activations: 2"));

    let (drop_again, event_again) = event_loop.check_hat_exhaustion(&hat_id, &dropped);
    assert!(drop_again);
    assert!(event_again.is_none());
}

// ── Phase 1: Hat Scope Enforcement Tests ──────────────────────────────

#[test]
fn test_scope_enforcement_drops_unauthorized_event() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let yaml = r#"
event_loop:
  enforce_hat_scope: true
hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Set builder as the active hat
    event_loop.state.last_active_hat_ids = vec![HatId::new("builder")];

    // Builder tries to emit LOOP_COMPLETE (not in its publishes)
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();

    // completion_requested should be false — the event was dropped by scope enforcement
    assert!(
        !event_loop.state.completion_requested,
        "LOOP_COMPLETE should be dropped when builder hat is active (not in publishes)"
    );
}

#[test]
fn test_scope_enforcement_allows_authorized_event() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let yaml = r#"
event_loop:
  enforce_hat_scope: true
hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done", "build.blocked"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Set builder as the active hat
    event_loop.state.last_active_hat_ids = vec![HatId::new("builder")];

    // Builder emits build.done (in its publishes) — should pass through
    write_event_to_jsonl(
        &events_path,
        "build.done",
        "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass",
    );
    let _ = event_loop.process_events_from_jsonl();

    // The event should have been published to the bus (not dropped)
    assert!(
        event_loop.has_pending_events(),
        "build.done should pass scope enforcement when builder is active"
    );
}

#[test]
fn test_scope_enforcement_skipped_when_no_active_hats() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let yaml = r#"
event_loop:
  enforce_hat_scope: true
hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // No active hats (Ralph coordinating)
    event_loop.state.last_active_hat_ids = vec![];

    // LOOP_COMPLETE should pass through when Ralph is coordinating
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();

    assert!(
        event_loop.state.completion_requested,
        "LOOP_COMPLETE should be accepted when no active hats (Ralph coordinating)"
    );
}

#[test]
fn test_scope_violation_event_published() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let yaml = r#"
event_loop:
  enforce_hat_scope: true
hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done"]
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Set builder as the active hat
    event_loop.state.last_active_hat_ids = vec![HatId::new("builder")];

    // Builder tries to emit plan.approved (not in its publishes)
    write_event_to_jsonl(&events_path, "plan.approved", "Auto-approved");
    let _ = event_loop.process_events_from_jsonl();

    // A scope_violation event should have been published to the bus
    assert!(
        event_loop.has_pending_events(),
        "Scope violation event should be published to the bus"
    );
}

// ── Phase 2: Event Chain Validation + loop.cancel Tests ───────────────

#[test]
fn test_chain_validation_rejects_completion_without_required_events() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.required_events = vec!["plan.approved".to_string(), "all.built".to_string()];
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Only emit plan.approved, missing all.built
    write_event_to_jsonl(&events_path, "plan.approved", "OK");
    let _ = event_loop.process_events_from_jsonl();

    // Now try to complete
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason, None,
        "LOOP_COMPLETE should be rejected when required events are missing"
    );
}

#[test]
fn test_chain_validation_accepts_completion_with_all_required_events() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.required_events = vec!["plan.approved".to_string(), "all.built".to_string()];
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Emit both required events across iterations
    write_event_to_jsonl(&events_path, "plan.approved", "OK");
    let _ = event_loop.process_events_from_jsonl();

    write_event_to_jsonl(&events_path, "all.built", "Done");
    let _ = event_loop.process_events_from_jsonl();

    // Now complete
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "LOOP_COMPLETE should be accepted when all required events have been seen"
    );
}

#[test]
fn test_chain_validation_tracks_topics_across_iterations() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.required_events = vec![
        "research.complete".to_string(),
        "plan.approved".to_string(),
        "all.built".to_string(),
    ];
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Iteration 1: research.complete
    write_event_to_jsonl(&events_path, "research.complete", "findings");
    let _ = event_loop.process_events_from_jsonl();

    // Iteration 2: plan.approved
    write_event_to_jsonl(&events_path, "plan.approved", "ok");
    let _ = event_loop.process_events_from_jsonl();

    // Iteration 3: all.built + LOOP_COMPLETE
    write_event_to_jsonl(&events_path, "all.built", "done");
    let _ = event_loop.process_events_from_jsonl();

    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "Topics should be tracked across iterations"
    );
}

#[test]
fn test_chain_validation_empty_required_events_allows_completion() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default(); // No required_events
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "Empty required_events should allow completion (backward compatible)"
    );
}

#[test]
fn test_chain_validation_injects_task_resume_on_rejection() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.required_events = vec!["plan.approved".to_string()];
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Try to complete without the required event
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(reason, None, "Should reject completion");

    // A task.resume event should have been published to the bus
    assert!(
        event_loop.has_pending_events(),
        "task.resume should be published on rejection"
    );
}

#[test]
fn test_loop_cancel_terminates_without_chain_validation() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.cancellation_promise = "loop.cancel".to_string();
    config.event_loop.required_events = vec!["plan.approved".to_string(), "all.built".to_string()];
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Send loop.cancel without any required events seen
    write_event_to_jsonl(&events_path, "loop.cancel", "rejected by human");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_cancellation_event();
    assert_eq!(
        reason,
        Some(TerminationReason::Cancelled),
        "loop.cancel should terminate without chain validation"
    );
}

#[test]
fn test_default_publishes_satisfies_required_events_for_completion() {
    use std::collections::HashMap;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.required_events = vec!["plan.draft".to_string(), "all.built".to_string()];

    let mut hats = HashMap::new();
    hats.insert(
        "planner".to_string(),
        crate::config::HatConfig {
            name: "planner".to_string(),
            description: Some("Plans work".to_string()),
            triggers: vec!["research.complete".to_string()],
            publishes: vec!["plan.draft".to_string()],
            instructions: "Plan".to_string(),
            extra_instructions: vec![],
            backend: None,
            backend_args: None,
            default_publishes: Some("plan.draft".to_string()),
            max_activations: None,
            scratchpad: None,
            disallowed_tools: vec![],
            timeout: None,
            concurrency: 1,
            aggregate: None,
        },
    );
    config.hats = hats;

    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Simulate: planner wrote no events, default_publishes injects plan.draft
    let planner_id = HatId::new("planner");
    event_loop.check_default_publishes(&planner_id);

    // Then all.built arrives via JSONL
    write_event_to_jsonl(&events_path, "all.built", "done");
    let _ = event_loop.process_events_from_jsonl();

    // Now LOOP_COMPLETE should be accepted (plan.draft was from default_publishes)
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "default_publishes events should satisfy required_events chain validation"
    );
}

#[test]
fn test_default_publishes_completion_promise_triggers_termination() {
    use std::collections::HashMap;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.completion_promise = "LOOP_COMPLETE".to_string();
    config.event_loop.required_events = vec!["all.built".to_string()];

    let mut hats = HashMap::new();
    hats.insert(
        "final_committer".to_string(),
        crate::config::HatConfig {
            name: "FinalCommitter".to_string(),
            description: Some("Verifies all work is complete".to_string()),
            triggers: vec!["all.built".to_string()],
            publishes: vec!["LOOP_COMPLETE".to_string()],
            instructions: "Verify and complete".to_string(),
            extra_instructions: vec![],
            backend: None,
            backend_args: None,
            default_publishes: Some("LOOP_COMPLETE".to_string()),
            max_activations: None,
            scratchpad: None,
            disallowed_tools: vec![],
            timeout: None,
            concurrency: 1,
            aggregate: None,
        },
    );
    config.hats = hats;

    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Satisfy required_events: all.built arrives via JSONL
    write_event_to_jsonl(&events_path, "all.built", "done");
    let _ = event_loop.process_events_from_jsonl();

    // Set active hat so check_default_publishes targets the right hat
    event_loop.state.last_active_hat_ids = vec![HatId::new("final_committer")];

    // Simulate: final_committer wrote no events, default_publishes injects LOOP_COMPLETE
    let hat_id = HatId::new("final_committer");
    event_loop.check_default_publishes(&hat_id);

    // completion_requested should be set directly by check_default_publishes
    // (not requiring a JSONL round-trip)
    let reason = event_loop.check_completion_event();
    assert_eq!(
        reason,
        Some(TerminationReason::CompletionPromise),
        "default_publishes of completion_promise should trigger termination directly, \
         not just publish to the bus where it would be lost"
    );
}

#[test]
fn test_loop_cancel_exit_code_is_zero() {
    assert_eq!(
        TerminationReason::Cancelled.exit_code(),
        0,
        "Cancelled should have exit code 0"
    );
}

#[test]
fn test_loop_cancel_is_not_success() {
    assert!(
        !TerminationReason::Cancelled.is_success(),
        "Cancelled should not be a success"
    );
}

#[test]
fn test_loop_cancel_takes_priority_over_completion() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.cancellation_promise = "loop.cancel".to_string();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // Both loop.cancel and LOOP_COMPLETE in same batch
    write_event_to_jsonl(&events_path, "loop.cancel", "rejected");
    write_event_to_jsonl(&events_path, "LOOP_COMPLETE", "Done");
    let _ = event_loop.process_events_from_jsonl();

    // Cancellation should take priority (checked first)
    let cancel_reason = event_loop.check_cancellation_event();
    assert_eq!(
        cancel_reason,
        Some(TerminationReason::Cancelled),
        "Cancellation should take priority over completion"
    );
}

#[test]
fn test_loop_cancel_disabled_when_empty_string() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.cancellation_promise = String::new(); // Disabled
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // loop.cancel should pass through as a normal event (no termination)
    write_event_to_jsonl(&events_path, "loop.cancel", "rejected");
    let _ = event_loop.process_events_from_jsonl();

    let reason = event_loop.check_cancellation_event();
    assert_eq!(
        reason, None,
        "loop.cancel should not trigger cancellation when disabled"
    );
}

// ── Phase 3: Human Timeout Event Injection Tests ──────────────────────

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

struct MockRobotService {
    timeout: u64,
    should_timeout: bool,
}

impl ralph_proto::RobotService for MockRobotService {
    fn send_question(&self, _payload: &str) -> anyhow::Result<i32> {
        Ok(1)
    }
    fn wait_for_response(&self, _events_path: &Path) -> anyhow::Result<Option<String>> {
        if self.should_timeout {
            Ok(None)
        } else {
            Ok(Some("approved".to_string()))
        }
    }
    fn send_checkin(
        &self,
        _: u32,
        _: Duration,
        _: Option<&ralph_proto::CheckinContext>,
    ) -> anyhow::Result<i32> {
        Ok(0)
    }
    fn timeout_secs(&self) -> u64 {
        self.timeout
    }
    fn shutdown_flag(&self) -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }
    fn stop(self: Box<Self>) {}
}

struct RestartRequestRobotService;

impl ralph_proto::RobotService for RestartRequestRobotService {
    fn send_question(&self, _payload: &str) -> anyhow::Result<i32> {
        Ok(1)
    }

    fn wait_for_response(&self, _events_path: &Path) -> anyhow::Result<Option<String>> {
        Ok(Some("Please restart yourself now".to_string()))
    }

    fn send_checkin(
        &self,
        _: u32,
        _: Duration,
        _: Option<&ralph_proto::CheckinContext>,
    ) -> anyhow::Result<i32> {
        Ok(0)
    }

    fn timeout_secs(&self) -> u64 {
        5
    }

    fn shutdown_flag(&self) -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    fn stop(self: Box<Self>) {}
}

#[test]
fn test_human_timeout_injects_timeout_event() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.set_robot_service(Box::new(MockRobotService {
        timeout: 5,
        should_timeout: true,
    }));

    // Write a human.interact event
    write_event_to_jsonl(&events_path, "human.interact", "Please review this plan");
    let _ = event_loop.process_events_from_jsonl();

    // The bus should have a human.timeout event (from the mock timeout)
    assert!(
        event_loop.has_pending_events(),
        "human.timeout event should be published on timeout"
    );
}

#[test]
fn test_human_response_still_works() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let config = RalphConfig::default();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.set_robot_service(Box::new(MockRobotService {
        timeout: 5,
        should_timeout: false,
    }));

    // Write a human.interact event — mock returns "approved"
    write_event_to_jsonl(&events_path, "human.interact", "Please review this plan");
    let _ = event_loop.process_events_from_jsonl();

    // The bus should have a human.response event
    assert!(
        event_loop.has_pending_events(),
        "human.response event should be published when response received"
    );
}

/// Regression: start event written to JSONL by EventLogger must not be
/// re-read by `process_events_from_jsonl`, which would cause double-delivery.
/// The fix is to call `sync_event_reader_to_file_end()` after writing the
/// start event so the reader skips past it.
#[test]
fn test_sync_event_reader_prevents_start_event_double_delivery() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.event_loop.starting_event = Some("work.start".to_string());

    let mut event_loop = EventLoop::new(config);
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    // 1. Initialize publishes start event to the bus (in-memory).
    event_loop.initialize("Run the test");

    // 2. Simulate EventLogger writing the same start event to the JSONL file.
    write_event_to_jsonl(&events_path, "work.start", "Run the test");

    // 3. Advance the reader past the logged entry.
    event_loop.sync_event_reader_to_file_end();

    // 4. Simulate an agent emitting a new event via `ralph emit`.
    write_event_to_jsonl(&events_path, "seed.ready", "initialized");

    // 5. process_events_from_jsonl should pick up ONLY seed.ready,
    //    not the already-published work.start.
    let processed = event_loop.process_events_from_jsonl().unwrap();
    assert!(
        processed.had_events,
        "seed.ready should have been processed"
    );

    // Drain the bus and verify work.start appears exactly once (from initialize),
    // not twice (which would happen without the sync).
    let ralph_id = ralph_proto::HatId::new("ralph");
    let pending = event_loop.bus.take_pending(&ralph_id);
    let work_start_count = pending
        .iter()
        .filter(|e| e.topic.as_str() == "work.start")
        .count();
    assert_eq!(
        work_start_count, 1,
        "work.start must appear exactly once (from initialize), got {work_start_count}"
    );
    let seed_ready_count = pending
        .iter()
        .filter(|e| e.topic.as_str() == "seed.ready")
        .count();
    assert_eq!(
        seed_ready_count, 1,
        "seed.ready must appear exactly once (from JSONL), got {seed_ready_count}"
    );
}

#[test]
fn test_user_prompt_restart_request_creates_restart_signal_file() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

    write_event_to_jsonl(&events_path, "user.prompt", "Please restart yourself");
    let _ = event_loop.process_events_from_jsonl();

    assert!(
        temp_dir.path().join(".ralph/restart-requested").exists(),
        "user.prompt restart request should create restart signal file"
    );
}

#[test]
fn test_human_response_restart_request_creates_restart_signal_file() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let events_path = temp_dir.path().join("events.jsonl");

    let mut config = RalphConfig::default();
    config.core.workspace_root = temp_dir.path().to_path_buf();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");
    event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
    event_loop.set_robot_service(Box::new(RestartRequestRobotService));

    write_event_to_jsonl(&events_path, "human.interact", "Need approval");
    let _ = event_loop.process_events_from_jsonl();

    assert!(
        temp_dir.path().join(".ralph/restart-requested").exists(),
        "human.response restart request should create restart signal file"
    );
}
