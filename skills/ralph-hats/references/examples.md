# Ralph Hats Examples

## Pipeline Example

Use this when the workflow is linear.

```yaml
event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"

hats:
  planner:
    name: "Planner"
    description: "Scopes the work"
    triggers: ["work.start"]
    publishes: ["plan.ready"]
    default_publishes: "plan.ready"
    instructions: |
      Break the task into a clear plan.

  builder:
    name: "Builder"
    description: "Implements the plan"
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    default_publishes: "build.done"
    instructions: |
      Implement the approved plan.

  reviewer:
    name: "Reviewer"
    description: "Verifies the result"
    triggers: ["build.done"]
    publishes: ["LOOP_COMPLETE"]
    default_publishes: "LOOP_COMPLETE"
    instructions: |
      Review the implementation and complete the run.
```

## Review Loop Example

Use this when the workflow needs iteration and rejection.

```yaml
event_loop:
  starting_event: "review.start"
  completion_promise: "REVIEW_COMPLETE"

events:
  review.section:
    description: "Primary review section is ready for deeper analysis"
  analysis.complete:
    description: "Deep analysis finished for the current review wave"

hats:
  reviewer:
    name: "Reviewer"
    description: "Performs the first review pass"
    triggers: ["review.start", "review.followup"]
    publishes: ["review.section"]
    default_publishes: "review.section"
    instructions: |
      Produce the next review section.

  analyzer:
    name: "Analyzer"
    description: "Deepens the highest-risk findings"
    triggers: ["review.section"]
    publishes: ["analysis.complete"]
    default_publishes: "analysis.complete"
    instructions: |
      Analyze the highest-risk review area.

  closer:
    name: "Closer"
    description: "Decides whether to continue or finish"
    triggers: ["analysis.complete"]
    publishes: ["review.followup", "REVIEW_COMPLETE"]
    instructions: |
      Decide whether another review wave is needed.
```
