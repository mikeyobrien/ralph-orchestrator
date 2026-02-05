Feature: Hats Cloud API for hosted loop execution
  As a developer using hats
  I want to run orchestration loops on managed infrastructure via a REST API
  So that I can execute loops without local setup and integrate with CI/CD pipelines

  Background:
    Given the Hats Cloud API is running at "https://api.hats.sh"
    And I have a valid Bearer token from "hats login"

  # ---------------------------------------------------------------------------
  # POST /v1/loops - Create and start a loop
  # ---------------------------------------------------------------------------

  Scenario: Create a loop with only a prompt
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Add a health check endpoint to the API"
      }
      """
    Then the response status should be 201
    And the response body should contain:
      | field         | type   | description                                |
      | loop_id       | string | Unique identifier for the created loop     |
      | stream_url    | string | SSE endpoint for streaming loop progress   |
      | dashboard_url | string | URL to view the loop in the web dashboard  |
    And the "stream_url" should match "https://api.hats.sh/v1/loops/.+/stream"
    And the "dashboard_url" should match "https://app.hats.sh/loops/.+"

  Scenario: Create a loop with all optional fields
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Refactor auth module",
        "backend": "codex",
        "repo": "https://github.com/user/project.git",
        "max_iterations": 25,
        "max_cost": 5.00,
        "preset": "spec-driven",
        "hats_config": "event_loop:\n  max_iterations: 25",
        "webhook_url": "https://example.com/hooks/hats",
        "webhook_secret": "whsec_abc123secret",
        "push_as_pr": true,
        "pr_branch": "hats/refactor-auth"
      }
      """
    Then the response status should be 201
    And the response body "loop_id" should be a non-empty string

  Scenario: Create a loop uses defaults when optional fields are omitted
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Fix the login bug"
      }
      """
    Then the response status should be 201
    And the loop should be created with backend "claude"
    And the loop should be created with max_iterations 50

  Scenario: Create a loop fails without a prompt
    When I POST /v1/loops with body:
      """
      {}
      """
    Then the response status should be 400
    And the response body should contain:
      | field   | value       |
      | error   | Bad Request |
    And the "message" field should mention "prompt"

  Scenario: Create a loop fails with empty prompt
    When I POST /v1/loops with body:
      """
      {
        "prompt": ""
      }
      """
    Then the response status should be 400
    And the response body "error" should be "Bad Request"

  Scenario: Create a loop fails with invalid backend
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "backend": "nonexistent-backend"
      }
      """
    Then the response status should be 422
    And the response body "error" should be "Unprocessable Entity"
    And the "message" field should list valid backends

  Scenario: Create a loop fails with invalid preset
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "preset": "no-such-preset"
      }
      """
    Then the response status should be 422
    And the response body "error" should be "Unprocessable Entity"
    And the "message" field should mention "preset"

  Scenario: Create a loop fails with negative max_cost
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "max_cost": -1.00
      }
      """
    Then the response status should be 400
    And the "message" field should mention "max_cost"

  Scenario: Create a loop fails with max_iterations exceeding tier limit
    Given I am on the Free tier with max 10 iterations per loop
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Big refactor",
        "max_iterations": 50
      }
      """
    Then the response status should be 400
    And the "message" field should mention "iterations" and "tier"

  Scenario: Create a loop fails with invalid webhook_url format
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "webhook_url": "not-a-valid-url"
      }
      """
    Then the response status should be 400
    And the "message" field should mention "webhook_url"

  Scenario: Create a loop fails with push_as_pr but no repo
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "push_as_pr": true,
        "pr_branch": "hats/feature"
      }
      """
    Then the response status should be 400
    And the "message" field should mention "repo" and "push_as_pr"

  Scenario: Create a loop fails with malformed JSON body
    When I POST /v1/loops with raw body "{ invalid json }"
    Then the response status should be 400
    And the response body "error" should be "Bad Request"
    And the "message" field should mention "JSON"

  Scenario: Create a loop fails with max_iterations of 0
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "max_iterations": 0
      }
      """
    Then the response status should be 400
    And the "message" field should mention "max_iterations"

  Scenario: Create a loop fails with max_cost of 0
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "max_cost": 0
      }
      """
    Then the response status should be 400
    And the "message" field should mention "max_cost"

  Scenario: Create a loop fails with invalid hats_config YAML
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "hats_config": "event_loop:\n  max_iterations: [invalid\nyaml"
      }
      """
    Then the response status should be 422
    And the response body "error" should be "Unprocessable Entity"
    And the "message" field should mention "hats_config"

  # ---------------------------------------------------------------------------
  # Authentication and authorization
  # ---------------------------------------------------------------------------

  Scenario: Request without auth token is rejected
    Given I have no Bearer token
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something"
      }
      """
    Then the response status should be 401
    And the response body "error" should be "Unauthorized"

  Scenario: Request with expired token is rejected
    Given I have an expired Bearer token
    When I GET /v1/loops
    Then the response status should be 401
    And the "message" field should mention "expired"

  Scenario: Request with invalid token is rejected
    Given I have Bearer token "invalid-garbage-token"
    When I GET /v1/loops
    Then the response status should be 401
    And the response body "error" should be "Unauthorized"

  Scenario: API key authentication works for CI/CD
    Given I have a valid API key from "hats api-key create"
    And I set the Authorization header to "Bearer <api-key>"
    When I GET /v1/loops
    Then the response status should be 200

  Scenario: Health check does not require authentication
    Given I have no Bearer token
    When I GET /v1/health
    Then the response status should be 200
    And the response body "status" should be "ok"

  # ---------------------------------------------------------------------------
  # Tier system and rate limits
  # ---------------------------------------------------------------------------

  Scenario: Free tier enforces 5 loops per month
    Given I am on the Free tier
    And I have already created 5 loops this month
    When I POST /v1/loops with body:
      """
      {
        "prompt": "One more loop"
      }
      """
    Then the response status should be 402
    And the response body "error" should be "Payment Required"
    And the "message" field should mention "monthly loop limit"

  Scenario: Pro tier allows up to 100 loops per month
    Given I am on the Pro tier
    And I have already created 99 loops this month
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Almost at the limit"
      }
      """
    Then the response status should be 201

  Scenario: Team tier has unlimited loops
    Given I am on the Team tier
    And I have already created 500 loops this month
    When I POST /v1/loops with body:
      """
      {
        "prompt": "No limits"
      }
      """
    Then the response status should be 201

  Scenario: Free tier limits max_iterations to 10
    Given I am on the Free tier
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Quick task"
      }
      """
    Then the response status should be 201
    And the effective max_iterations should be capped at 10

  Scenario: Pro tier limits max_iterations to 50
    Given I am on the Pro tier
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Medium task",
        "max_iterations": 100
      }
      """
    Then the response status should be 201
    And the effective max_iterations should be capped at 50

  Scenario: Team tier limits max_iterations to 100
    Given I am on the Team tier
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Large task",
        "max_iterations": 200
      }
      """
    Then the response status should be 201
    And the effective max_iterations should be capped at 100

  Scenario: Rate limit headers are present on all responses
    When I GET /v1/loops
    Then the response should include these headers:
      | header                | description                                   |
      | X-RateLimit-Limit     | Maximum loops allowed in the current period    |
      | X-RateLimit-Remaining | Loops remaining in the current period          |
      | X-RateLimit-Reset     | UTC epoch seconds when the limit resets        |

  Scenario: Free tier enforces 1 concurrent loop
    Given I am on the Free tier
    And I have 1 loop currently running
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Second concurrent loop"
      }
      """
    Then the response status should be 429
    And the response body "error" should be "Too Many Requests"
    And the "message" field should mention "concurrent"

  Scenario: Pro tier allows up to 3 concurrent loops
    Given I am on the Pro tier
    And I have 2 loops currently running
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Third concurrent loop"
      }
      """
    Then the response status should be 201

  Scenario: Pro tier rejects 4th concurrent loop
    Given I am on the Pro tier
    And I have 3 loops currently running
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Fourth concurrent loop"
      }
      """
    Then the response status should be 429
    And the "message" field should mention "concurrent"

  Scenario: Team tier allows up to 10 concurrent loops
    Given I am on the Team tier
    And I have 9 loops currently running
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Tenth concurrent loop"
      }
      """
    Then the response status should be 201

  # ---------------------------------------------------------------------------
  # GET /v1/loops - List loops
  # ---------------------------------------------------------------------------

  Scenario: List loops returns paginated results
    Given I have created 15 loops
    When I GET /v1/loops?limit=10&offset=0
    Then the response status should be 200
    And the response body should contain:
      | field  | type    | description                        |
      | loops  | array   | List of loop summary objects       |
      | total  | integer | Total number of loops for the user |
      | limit  | integer | Requested page size                |
      | offset | integer | Requested offset                   |
    And the "loops" array should have 10 items
    And the "total" field should be 15

  Scenario: List loops with status filter
    Given I have 3 completed loops and 2 running loops
    When I GET /v1/loops?status=completed
    Then the response status should be 200
    And the "loops" array should have 3 items
    And every loop in the response should have status "completed"

  Scenario: List loops with project filter
    Given I have 2 loops for project "github.com/user/project-a"
    And I have 3 loops for project "github.com/user/project-b"
    When I GET /v1/loops?project=github.com/user/project-a
    Then the response status should be 200
    And the "loops" array should have 2 items

  Scenario: List loops returns empty array when none exist
    Given I have created no loops
    When I GET /v1/loops
    Then the response status should be 200
    And the "loops" array should have 0 items
    And the "total" field should be 0

  Scenario: List loops default pagination
    When I GET /v1/loops
    Then the response should use default limit of 20 and offset of 0

  Scenario: Loop summary includes essential fields
    Given I have at least 1 loop
    When I GET /v1/loops
    Then each loop summary should contain:
      | field         | type   | description                       |
      | loop_id       | string | Unique loop identifier            |
      | prompt        | string | Original prompt (possibly truncated) |
      | status        | string | One of: running, completed, failed, cancelled |
      | backend       | string | Backend used for the loop         |
      | created_at    | string | ISO 8601 timestamp                |
      | iterations    | integer| Number of iterations completed    |
      | cost          | number | Accumulated LLM cost in USD       |

  Scenario: List loops only shows the authenticated user's loops
    Given user "alice" has 3 loops
    And user "bob" has 5 loops
    When "alice" requests GET /v1/loops
    Then the "loops" array should have 3 items

  # ---------------------------------------------------------------------------
  # GET /v1/loops/:id - Get loop details
  # ---------------------------------------------------------------------------

  Scenario: Get details of a completed loop
    Given a loop "loop-abc123" exists and has completed successfully
    When I GET /v1/loops/loop-abc123
    Then the response status should be 200
    And the response body should contain:
      | field          | type    | description                           |
      | loop_id        | string  | "loop-abc123"                         |
      | prompt         | string  | The original prompt                   |
      | status         | string  | "completed"                           |
      | backend        | string  | Backend used                          |
      | repo           | string  | Git repo URL if provided              |
      | max_iterations | integer | Configured iteration limit            |
      | max_cost       | number  | Configured cost cap in USD            |
      | iterations     | integer | Actual iterations executed            |
      | cost           | number  | Actual LLM cost in USD                |
      | files_changed  | array   | List of files created or modified     |
      | created_at     | string  | ISO 8601 creation timestamp           |
      | completed_at   | string  | ISO 8601 completion timestamp         |
      | proof          | object  | Proof artifact (see proof scenario)   |

  Scenario: Get details of a running loop
    Given a loop "loop-running1" is currently running
    When I GET /v1/loops/loop-running1
    Then the response status should be 200
    And the "status" field should be "running"
    And the "completed_at" field should be null
    And the "iterations" field should reflect the current iteration count

  Scenario: Get details of a nonexistent loop returns 404
    When I GET /v1/loops/loop-does-not-exist
    Then the response status should be 404
    And the response body "error" should be "Not Found"

  Scenario: Get details of another user's loop returns 403
    Given user "alice" owns loop "loop-alice1"
    And I am authenticated as user "bob"
    When I GET /v1/loops/loop-alice1
    Then the response status should be 403
    And the response body "error" should be "Forbidden"

  Scenario: Loop details include proof artifact when completed
    Given a loop "loop-proven1" has completed successfully
    When I GET /v1/loops/loop-proven1
    Then the "proof" object should contain:
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

  Scenario: Loop details include null proof for failed loops
    Given a loop "loop-failed1" has failed
    When I GET /v1/loops/loop-failed1
    Then the "proof" field should be null

  # ---------------------------------------------------------------------------
  # GET /v1/loops/:id/stream - SSE event stream
  # ---------------------------------------------------------------------------

  Scenario: Stream a running loop via Server-Sent Events
    Given a loop "loop-stream1" is currently running
    When I connect to GET /v1/loops/loop-stream1/stream with Accept "text/event-stream"
    Then the response content-type should be "text/event-stream"
    And the connection should remain open
    And I should receive SSE events as the loop progresses

  Scenario: SSE events include all orchestration event types
    Given a loop "loop-stream2" runs through a full cycle
    When I stream GET /v1/loops/loop-stream2/stream
    Then I should receive events of these types:
      | event type      | description                              |
      | iteration_start | Fired at the beginning of each iteration |
      | hat_change      | Fired when the active hat changes        |
      | tool_call       | Fired when the agent invokes a tool      |
      | iteration_end   | Fired at the end of each iteration       |
      | loop_complete   | Fired when the loop finishes successfully|
    And each event should have an "id" field (monotonic sequence number)
    And each event should have a "data" field containing valid JSON

  Scenario: SSE loop_failed event on error
    Given a loop "loop-fail-stream" encounters a fatal error
    When I stream GET /v1/loops/loop-fail-stream/stream
    Then I should receive a "loop_failed" event
    And the event data should contain an "error" field with the failure reason

  Scenario: SSE iteration_start event contains iteration metadata
    Given a loop "loop-iter1" is running
    When I receive an "iteration_start" event from the stream
    Then the event data should contain:
      | field     | type    | description                 |
      | iteration | integer | Current iteration number    |
      | hat       | string  | Name of the active hat      |
      | timestamp | string  | ISO 8601 timestamp          |

  Scenario: SSE hat_change event contains hat transition info
    When I receive a "hat_change" event from the stream
    Then the event data should contain:
      | field    | type   | description              |
      | from_hat | string | Previous hat name        |
      | to_hat   | string | New hat name             |
      | reason   | string | Why the hat changed      |

  Scenario: SSE tool_call event contains tool invocation info
    When I receive a "tool_call" event from the stream
    Then the event data should contain:
      | field     | type   | description                |
      | tool_name | string | Name of the tool invoked   |
      | status    | string | "started" or "completed"   |

  Scenario: SSE stream supports Last-Event-ID for resumption
    Given a loop "loop-resume1" is running
    And I previously received events up to id "42"
    When I reconnect to GET /v1/loops/loop-resume1/stream with header Last-Event-ID "42"
    Then I should receive events starting from id "43"
    And I should NOT receive events with id <= 42

  Scenario: SSE stream of a completed loop replays all events then closes
    Given a loop "loop-done1" has already completed
    When I connect to GET /v1/loops/loop-done1/stream
    Then I should receive the full event history
    And the last event should be "loop_complete" or "loop_failed"
    And the connection should close after the final event

  Scenario: SSE stream is compatible with EventSource clients
    Given a loop "loop-es1" is running
    When I connect to GET /v1/loops/loop-es1/stream with Accept "text/event-stream"
    Then the response should NOT include "X-Accel-Buffering" or it should be "no"
    And the response should include "Cache-Control: no-cache"
    And the response should include "Connection: keep-alive"
    And each event should use the "data:" field prefix per the SSE specification
    And each event should be terminated by a blank line

  Scenario: SSE stream of nonexistent loop returns 404
    When I connect to GET /v1/loops/loop-nope/stream
    Then the response status should be 404

  Scenario: SSE stream of another user's loop returns 403
    Given user "alice" owns loop "loop-alice-stream"
    And I am authenticated as user "bob"
    When I connect to GET /v1/loops/loop-alice-stream/stream
    Then the response status should be 403

  # ---------------------------------------------------------------------------
  # POST /v1/loops/:id/cancel - Cancel a running loop
  # ---------------------------------------------------------------------------

  Scenario: Cancel a running loop
    Given a loop "loop-cancel1" is currently running
    When I POST /v1/loops/loop-cancel1/cancel
    Then the response status should be 200
    And the response body "status" should be "cancelled"
    And the response body should contain "iterations" and "cost" fields

  Scenario: Cancel an already completed loop returns 409
    Given a loop "loop-done2" has already completed
    When I POST /v1/loops/loop-done2/cancel
    Then the response status should be 409
    And the response body "error" should be "Conflict"
    And the "message" field should mention "already completed"

  Scenario: Cancel an already cancelled loop returns 409
    Given a loop "loop-cancelled1" was previously cancelled
    When I POST /v1/loops/loop-cancelled1/cancel
    Then the response status should be 409
    And the "message" field should mention "already cancelled"

  Scenario: Cancel a nonexistent loop returns 404
    When I POST /v1/loops/loop-nope/cancel
    Then the response status should be 404

  Scenario: Cancel another user's loop returns 403
    Given user "alice" owns loop "loop-alice-cancel"
    And I am authenticated as user "bob"
    When I POST /v1/loops/loop-alice-cancel/cancel
    Then the response status should be 403

  # ---------------------------------------------------------------------------
  # DELETE /v1/loops/:id - Delete loop and artifacts
  # ---------------------------------------------------------------------------

  Scenario: Delete a completed loop
    Given a loop "loop-del1" has completed
    When I DELETE /v1/loops/loop-del1
    Then the response status should be 204
    And GET /v1/loops/loop-del1 should return 404

  Scenario: Delete a failed loop
    Given a loop "loop-del2" has failed
    When I DELETE /v1/loops/loop-del2
    Then the response status should be 204

  Scenario: Delete a running loop returns 409
    Given a loop "loop-del3" is currently running
    When I DELETE /v1/loops/loop-del3
    Then the response status should be 409
    And the response body "error" should be "Conflict"
    And the "message" field should mention "cancel" before deleting

  Scenario: Delete a nonexistent loop returns 404
    When I DELETE /v1/loops/loop-nope
    Then the response status should be 404

  Scenario: Delete another user's loop returns 403
    Given user "alice" owns loop "loop-alice-del"
    And I am authenticated as user "bob"
    When I DELETE /v1/loops/loop-alice-del
    Then the response status should be 403

  Scenario: Deleting a loop removes its proof artifact
    Given a loop "loop-del-proof" has completed with a proof artifact
    When I DELETE /v1/loops/loop-del-proof
    Then the response status should be 204
    And the proof artifact for "loop-del-proof" should no longer exist

  # ---------------------------------------------------------------------------
  # Cost tracking and budget enforcement
  # ---------------------------------------------------------------------------

  Scenario: Loop tracks actual LLM cost
    Given a loop "loop-cost1" has completed
    When I GET /v1/loops/loop-cost1
    Then the "cost" field should be a number >= 0
    And the "cost" field should reflect accumulated LLM API spend in USD

  Scenario: Loop is cancelled when cost exceeds max_cost
    Given I create a loop with max_cost 2.00
    And the loop's LLM cost reaches $2.00 during iteration 3
    Then the loop should be cancelled with reason "BUDGET_EXCEEDED"
    And the final status should be "cancelled"
    And the "cost" field should be within 10% of 2.00

  Scenario: Loop without max_cost has no budget enforcement
    Given I create a loop without specifying max_cost
    When the loop completes after accumulating $15.00 in LLM costs
    Then the loop should complete normally
    And the "cost" field should be within 10% of 15.00

  # ---------------------------------------------------------------------------
  # Webhook delivery
  # ---------------------------------------------------------------------------

  Scenario: Webhook is delivered on loop completion
    Given I create a loop with webhook_url "https://example.com/hooks/hats"
    And the loop completes successfully
    Then a POST request should be sent to "https://example.com/hooks/hats"
    And the webhook payload should contain:
      | field      | type   | description                          |
      | loop_id    | string | The completed loop's ID              |
      | status     | string | "completed"                          |
      | iterations | integer| Number of iterations executed         |
      | cost       | number | Total LLM cost in USD                |
      | proof      | object | Proof artifact if available          |

  Scenario: Webhook is delivered on loop failure
    Given I create a loop with webhook_url "https://example.com/hooks/hats"
    And the loop fails
    Then a POST request should be sent to "https://example.com/hooks/hats"
    And the webhook payload "status" should be "failed"
    And the webhook payload should contain an "error" field

  Scenario: Webhook delivery retries on failure
    Given I create a loop with webhook_url "https://example.com/hooks/hats"
    And the webhook endpoint returns 500 on the first attempt
    Then the webhook should be retried up to 3 times with exponential backoff
    And if all retries fail, the loop status should still be "completed"

  Scenario: Webhook includes HMAC signature for verification
    Given I create a loop with webhook_url and a webhook_secret
    When the webhook is delivered
    Then the request should include an "X-Hats-Signature" header
    And the signature should be HMAC-SHA256 of the payload using the webhook_secret

  # ---------------------------------------------------------------------------
  # Git integration
  # ---------------------------------------------------------------------------

  Scenario: Loop clones a public repo
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Fix the README typo",
        "repo": "https://github.com/user/public-repo.git"
      }
      """
    Then the response status should be 201
    And the loop should clone the repo before starting

  Scenario: Loop pushes results as a pull request
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Add input validation",
        "repo": "https://github.com/user/project.git",
        "push_as_pr": true,
        "pr_branch": "hats/add-validation"
      }
      """
    Then the response status should be 201
    And on completion, a PR should be created on the repo
    And the PR branch should be "hats/add-validation"
    And the PR body should reference the loop ID and proof artifact

  Scenario: Loop fails gracefully when repo clone fails
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Do something",
        "repo": "https://github.com/user/nonexistent-repo.git"
      }
      """
    Then the response status should be 201
    And the loop should fail with error mentioning "clone"
    And the final status should be "failed"

  Scenario: Private repo requires credential setup
    Given I have not configured git credentials for "github.com/user/private-repo"
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Fix something",
        "repo": "https://github.com/user/private-repo.git"
      }
      """
    Then the loop should fail with error mentioning "authentication" or "credentials"

  # ---------------------------------------------------------------------------
  # CLI integration
  # ---------------------------------------------------------------------------

  Scenario: "hats cloud run" creates and streams a loop
    When I run "hats cloud run -p 'Add tests for auth module'"
    Then hats should POST to /v1/loops with the prompt
    And hats should connect to the SSE stream
    And loop progress should be printed to stdout in real time
    And the exit code should be 0 on successful completion

  Scenario: "hats cloud run" exits non-zero on loop failure
    Given the loop will fail due to an error
    When I run "hats cloud run -p 'Impossible task'"
    Then hats should stream progress until failure
    And the exit code should be non-zero

  Scenario: "hats cloud run" passes optional flags
    When I run "hats cloud run -p 'Refactor' --backend codex --max-iterations 25 --max-cost 5.00 --repo https://github.com/user/project.git"
    Then hats should POST to /v1/loops with:
      | field          | value                                    |
      | prompt         | Refactor                                 |
      | backend        | codex                                    |
      | max_iterations | 25                                       |
      | max_cost       | 5.00                                     |
      | repo           | https://github.com/user/project.git      |

  Scenario: "hats cloud status" shows loop details
    Given a loop "loop-status1" exists
    When I run "hats cloud status loop-status1"
    Then the output should display the loop's status, iterations, cost, and backend
    And the exit code should be 0

  Scenario: "hats cloud cancel" cancels a running loop
    Given a loop "loop-cli-cancel" is running
    When I run "hats cloud cancel loop-cli-cancel"
    Then the output should confirm the loop was cancelled
    And the exit code should be 0

  Scenario: "hats cloud cancel" reports error for completed loop
    Given a loop "loop-cli-done" has completed
    When I run "hats cloud cancel loop-cli-done"
    Then stderr should contain "already completed"
    And the exit code should be non-zero

  Scenario: "hats cloud ls" lists loops
    Given I have 3 loops
    When I run "hats cloud ls"
    Then the output should list all 3 loops with their ID, status, and prompt summary
    And the exit code should be 0

  Scenario: "hats cloud ls" supports status filter
    Given I have 2 running loops and 1 completed loop
    When I run "hats cloud ls --status running"
    Then the output should list only the 2 running loops

  Scenario: "hats cloud ls" supports JSON output
    When I run "hats cloud ls --format json"
    Then the output should be valid JSON
    And it should contain a "loops" array

  Scenario: "hats cloud run" handles authentication failure
    Given my token has expired
    When I run "hats cloud run -p 'Do something'"
    Then stderr should contain "Unauthorized" or "expired"
    And the output should suggest running "hats login"
    And the exit code should be non-zero

  # ---------------------------------------------------------------------------
  # Edge cases
  # ---------------------------------------------------------------------------

  Scenario: Loop with max_iterations of 1 runs exactly once
    When I POST /v1/loops with body:
      """
      {
        "prompt": "One-shot task",
        "max_iterations": 1
      }
      """
    Then the response status should be 201
    And the loop should complete after exactly 1 iteration

  Scenario: Concurrent requests to create loops are serialized per user
    Given I am on the Free tier with 1 concurrent loop allowed
    When I send 2 simultaneous POST /v1/loops requests
    Then exactly 1 should succeed with status 201
    And the other should fail with status 429

  Scenario: Deleting a cancelled loop succeeds
    Given a loop "loop-del-cancelled" was cancelled
    When I DELETE /v1/loops/loop-del-cancelled
    Then the response status should be 204

  Scenario: Loop created with inline hats_config overrides defaults
    When I POST /v1/loops with body:
      """
      {
        "prompt": "Custom config",
        "hats_config": "event_loop:\n  max_iterations: 5\nhats:\n  analyst:\n    backend: codex"
      }
      """
    Then the response status should be 201
    And the loop should use the inline hats_config for hat definitions

  Scenario: Very long prompt is accepted up to 100KB
    Given a prompt that is 50,000 characters long
    When I POST /v1/loops with the long prompt
    Then the response status should be 201

  Scenario: Prompt exceeding 100KB is rejected
    Given a prompt that is 150,000 characters long
    When I POST /v1/loops with the long prompt
    Then the response status should be 400
    And the "message" field should mention "prompt" and "size"

  Scenario: SSE stream handles client disconnect gracefully
    Given a loop "loop-disconnect" is running
    When I connect to the SSE stream and then disconnect
    Then the loop should continue running regardless
    And the loop should still be accessible via GET /v1/loops/loop-disconnect

  Scenario: Multiple SSE clients can stream the same loop
    Given a loop "loop-multi-stream" is running
    When 3 clients connect to GET /v1/loops/loop-multi-stream/stream
    Then all 3 clients should receive the same events
    And disconnecting one client should not affect the other 2
