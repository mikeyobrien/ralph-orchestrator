@hooks @executor-safeguards
Feature: Hook executor safeguards
  # Execution guardrails for hook commands
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-05..AC-07)

  @AC-05
  Scenario: AC-05 JSON stdin contract
    Given a hook invocation
    When the command starts
    Then it receives a valid JSON payload on stdin
    And environment variables are set: RALPH_HOOK_EVENT, RALPH_HOOK_PHASE, RALPH_LOOP_ID

  @AC-06
  Scenario: AC-06 Timeout safeguard
    Given a hook configured with "timeout_seconds"
    When hook execution exceeds the timeout
    Then execution is terminated
    And the hook is recorded as timed out

  @AC-07
  Scenario: AC-07 Output-size safeguard
    Given a hook configured with "max_output_bytes"
    When stdout or stderr exceed the limit
    Then stored output is truncated deterministically
    And truncation is indicated in the telemetry
