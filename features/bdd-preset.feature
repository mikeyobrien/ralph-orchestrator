Feature: BDD preset for dark factory workflow
  As a developer practicing Level 5 automation
  I want hats to accept BDD feature files as input
  So that I can get verified implementations without reviewing code

  Scenario: Run a loop from a feature file
    Given I have a project with "features/auth.feature"
    When I run "hats run --preset bdd --spec features/auth.feature"
    Then hats should parse the feature file
    And generate acceptance tests from the scenarios
    And implement code to pass the acceptance tests
    And emit LOOP_COMPLETE only when all acceptance tests pass

  Scenario: Proof artifact generation
    Given a BDD loop has completed successfully
    When I check the proofs directory
    Then a proof file should exist with:
      | field           | present |
      | spec_file       | yes     |
      | scenarios_count | yes     |
      | tests_pass      | yes     |
      | tests_fail      | yes     |
      | coverage        | yes     |
      | cost            | yes     |
      | iterations      | yes     |
      | duration        | yes     |
      | files_changed   | yes     |

  Scenario: Loop fails if acceptance tests fail
    Given I have a feature file with 3 scenarios
    And the agent produces code that passes only 2 scenarios
    When the backpressure gate runs
    Then the loop should NOT emit LOOP_COMPLETE
    And should iterate again

  Scenario: Hat rotation in BDD mode
    Given I run a BDD loop
    Then the spec-writer hat should run first
    And the implementer hat should run second
    And the reviewer hat should run last
    And each hat should use its configured backend
