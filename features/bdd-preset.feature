Feature: BDD preset for dark factory workflow
  As a developer practicing Level 5 automation
  I want hats to accept BDD feature files as input
  So that I can get verified implementations without reviewing code

  Background:
    Given a git repository with at least one commit
    And hats is installed and on PATH

  # ---------------------------------------------------------------------------
  # Happy path
  # ---------------------------------------------------------------------------

  Scenario: Initialize a project with the BDD preset
    When I run "hats init --preset bdd"
    Then a file "hats.yml" should exist
    And it should define a "spec_writer" hat that triggers on "spec.start"
    And it should define an "implementer" hat that triggers on "spec.approved"
    And it should define a "verifier" hat that triggers on "implementation.done"

  Scenario: Run a loop from a feature file
    Given a file "features/auth.feature" containing:
      """
      Feature: Authentication
        Scenario: Valid login
          Given a user "alice" with password "secret"
          When I POST /login with credentials "alice:secret"
          Then the response status should be 200
          And the response body should contain an "access_token" field
      """
    And a "hats.yml" configured with the BDD preset
    When I run "hats run --prompt-file features/auth.feature"
    Then hats should parse the Gherkin scenarios from the file
    And the spec_writer hat should extract acceptance criteria from each scenario
    And the implementer hat should generate code that satisfies the criteria
    And the verifier hat should run all acceptance tests
    And the loop should emit "LOOP_COMPLETE" only when all tests pass
    And the exit code should be 0

  Scenario: Multiple scenarios in one feature file
    Given a file "features/cart.feature" containing 3 scenarios
    And a "hats.yml" configured with the BDD preset
    When I run "hats run --prompt-file features/cart.feature"
    Then hats should generate at least one test per scenario
    And all 3 scenarios should have corresponding passing tests

  # ---------------------------------------------------------------------------
  # Proof artifacts
  # ---------------------------------------------------------------------------

  Scenario: Proof artifact is generated on successful completion
    Given a BDD loop has completed successfully
    Then a proof file should exist at ".hats/proofs/<loop-id>.json"
    And the proof file should be valid JSON containing these fields:
      | field           | type    | description                              |
      | spec_file       | string  | Path to the input feature file           |
      | scenarios_total | integer | Number of scenarios in the feature file  |
      | tests_pass      | integer | Number of passing acceptance tests       |
      | tests_fail      | integer | Number of failing acceptance tests       |
      | iterations      | integer | Number of loop iterations executed       |
      | duration_secs   | number  | Wall-clock seconds from start to finish  |
      | files_changed   | array   | List of files created or modified        |
      | git_sha         | string  | Commit SHA at loop completion            |
      | exit_code       | integer | 0 for success, non-zero for failure      |

  Scenario: Proof artifact records failure state on loop termination
    Given a BDD loop that terminates due to max_iterations
    Then a proof file should still exist at ".hats/proofs/<loop-id>.json"
    And the "tests_fail" field should be greater than 0
    And the "exit_code" field should be non-zero

  # ---------------------------------------------------------------------------
  # Backpressure and iteration
  # ---------------------------------------------------------------------------

  Scenario: Loop iterates when acceptance tests partially pass
    Given a feature file with 3 scenarios
    And the implementer produces code that passes only 2 of 3 tests
    When the verifier hat runs
    Then the verifier should emit "spec.violated" (not "task.complete")
    And the loop should begin another iteration with the implementer hat
    And the verifier output should list which scenarios failed

  Scenario: Loop terminates at max_iterations with failing tests
    Given a feature file with 3 scenarios
    And max_iterations is set to 3
    And the agent cannot make all tests pass within 3 iterations
    When the loop reaches iteration 3
    Then the loop should terminate with reason "MAX_ITERATIONS"
    And the exit code should be non-zero
    And a proof artifact should record the partial results

  # ---------------------------------------------------------------------------
  # Hat rotation
  # ---------------------------------------------------------------------------

  Scenario: Hats execute in the correct order
    Given a "hats.yml" configured with the BDD preset
    When a BDD loop runs
    Then the first hat should be "spec_writer" (triggered by "spec.start")
    And the second hat should be "implementer" (triggered by "spec.approved")
    And the third hat should be "verifier" (triggered by "implementation.done")

  Scenario: Each hat uses its configured backend
    Given a "hats.yml" where spec_writer uses "claude" and implementer uses "codex"
    When a BDD loop runs
    Then the spec_writer hat should invoke the "claude" backend
    And the implementer hat should invoke the "codex" backend

  # ---------------------------------------------------------------------------
  # Error and edge cases
  # ---------------------------------------------------------------------------

  Scenario: Feature file does not exist
    When I run "hats run --prompt-file features/nonexistent.feature"
    Then the exit code should be non-zero
    And stderr should contain "not found" or "No such file"

  Scenario: Feature file contains no scenarios
    Given a file "features/empty.feature" containing:
      """
      Feature: Empty feature
        No scenarios here.
      """
    When I run "hats run --prompt-file features/empty.feature"
    Then the spec_writer should report that no scenarios were found
    And the loop should terminate without entering the implementer hat

  Scenario: Feature file with Scenario Outline and Examples
    Given a file "features/login.feature" containing:
      """
      Feature: Login validation
        Scenario Outline: Reject invalid credentials
          Given a user "<user>" with password "<password>"
          When I POST /login with credentials "<user>:<password>"
          Then the response status should be <status>

          Examples:
            | user    | password | status |
            | alice   | wrong    | 401    |
            |         | secret   | 400    |
            | alice   |          | 400    |
      """
    When a BDD loop runs with this feature file
    Then the spec_writer should expand the Examples table into 3 test cases
    And all 3 expanded scenarios should have corresponding tests

  Scenario: Feature file is not valid Gherkin
    Given a file "features/bad.feature" containing "this is not gherkin"
    When I run "hats run --prompt-file features/bad.feature"
    Then the spec_writer hat should report a parsing error
    And the loop should terminate with a non-zero exit code
