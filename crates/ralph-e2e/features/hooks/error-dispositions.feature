@hooks @error-dispositions
Feature: Hook error dispositions
  # Error handling policies for hook failures
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-08..AC-09)

  @AC-08
  Scenario: AC-08 Per-hook warn policy
    Given a hook configured with "on_error: warn"
    When the hook exits with a non-zero code
    Then orchestration continues
    And warning telemetry is recorded for the hook

  @AC-09
  Scenario: AC-09 Per-hook block policy
    Given a hook configured with "on_error: block"
    When the hook fails
    Then the current lifecycle action is blocked
    And a clear failure reason is surfaced to the operator
