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
fn test_hat_max_activations_emits_exhausted_event() {
    // Repro for issue #66: per-hat max_activations should prevent infinite reviewer loops.
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
        event_loop.process_output(
            &ralph,
            "<event topic=\"implementation.done\">done</event>",
            true,
        );

        // Reviewer active (up to max_activations=3).
        let prompt = event_loop.build_prompt(&ralph).unwrap();
        assert!(
            !prompt.contains("Event: code_reviewer.exhausted"),
            "Reviewer should not be exhausted yet"
        );
        event_loop.process_output(
            &ralph,
            "<event topic=\"review.changes_requested\">fix</event>",
            true,
        );
    }

    // One more implementation.done should attempt a 4th reviewer activation.
    let _ = event_loop.build_prompt(&ralph).unwrap();
    event_loop.process_output(
        &ralph,
        "<event topic=\"implementation.done\">done</event>",
        true,
    );

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
    config.core.scratchpad = scratchpad_path.to_string_lossy().to_string();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test");

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
    config.core.scratchpad = scratchpad_path.to_string_lossy().to_string();
    let mut event_loop = EventLoop::new(config);
    event_loop.initialize("Test task");

    // Ralph handles task.start, not a specific hat
    let ralph_id = HatId::new("ralph");

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
            max_activations: None,
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
            max_activations: None,
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
            max_activations: None,
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
