@hooks @executor-safeguards
Feature: Hook executor safeguards
  # Execution guardrails for hook commands
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-05..AC-07)

  @AC-05
  Scenario: AC-05 JSON stdin contract
    Given a hook invocation
    When the command starts
    Then it receives a valid JSON payload on stdin and minimal env vars

  @AC-06
  Scenario: AC-06 Timeout safeguard
    Given `timeout_seconds` is configured
    When hook execution exceeds timeout
    Then execution is terminated and recorded as timed out

  @AC-07
  Scenario: AC-07 Output-size safeguard
    Given `max_output_bytes` is configured
    When stdout/stderr exceed the limit
    Then stored output is truncated deterministically
