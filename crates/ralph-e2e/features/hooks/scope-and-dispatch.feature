@hooks @scope-and-dispatch
Feature: Hooks scope and dispatch
  # Per-project scope and lifecycle event dispatch
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-01..AC-04)

  @AC-01
  Scenario: AC-01 Per-project scope only
    Given a project with hooks configured
    When Ralph runs in that project
    Then hooks from that project config are loaded and no global hook source is required

  @AC-02
  Scenario: AC-02 Mandatory lifecycle events supported
    Given hooks for all required v1 events
    When those lifecycle boundaries occur
    Then corresponding hook phases are dispatched with structured payloads

  @AC-03
  Scenario: AC-03 Pre/post phase support
    Given `pre.E` and `post.E` hooks
    When event `E` occurs
    Then pre hooks run before and post hooks run after the lifecycle boundary

  @AC-04
  Scenario: AC-04 Deterministic ordering
    Given multiple hooks for a phase
    When phase dispatch executes
    Then hooks run sequentially in declaration order
