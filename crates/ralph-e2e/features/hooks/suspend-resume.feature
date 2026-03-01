@hooks @suspend-resume
Feature: Hook suspend and resume
  # Suspend mode behavior and CLI resume path
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-10..AC-12)

  @AC-10
  Scenario: AC-10 Suspend default mode
    Given a hook configured with "on_error: suspend" and no explicit suspend_mode
    When the hook fails
    Then the orchestrator enters "wait_for_resume" mode
    And suspend state is persisted to .ralph/suspend-state.json

  @AC-11
  Scenario: AC-11 CLI resume path
    Given a loop that is suspended
    When the operator runs "ralph loops resume <loop-id>"
    Then the loop receives the resume signal
    And orchestration continues from the suspended boundary

  @AC-12
  Scenario: AC-12 Resume idempotency
    Given a loop that is already resumed or not suspended
    When resume is requested again
    Then the command returns a non-destructive informative result
    And no state is corrupted
