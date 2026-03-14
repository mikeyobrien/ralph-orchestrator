@hooks @suspend-resume
Feature: Hook suspend and resume
  # Suspend mode behavior and CLI resume path
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-10..AC-12)

  @AC-10
  Scenario: AC-10 Suspend default mode
    Given `on_error: suspend` with no explicit mode
    When hook fails
    Then orchestrator suspends in `wait_for_resume` mode

  @AC-11
  Scenario: AC-11 CLI resume path
    Given a suspended loop
    When operator runs `ralph loops resume <id>`
    Then loop receives resume signal and continues from suspended boundary

  @AC-12
  Scenario: AC-12 Resume idempotency
    Given a loop already resumed or not suspended
    When resume is requested again
    Then command returns non-destructive informative result
