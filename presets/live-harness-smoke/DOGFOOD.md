# Sanitized live dogfood evidence

This record intentionally contains no credentials, account metadata, prompts, or
provider output. The retained runtime workspace and raw streams are not committed.

- Observation mode: `pure-watch`
- Result: PASS
- Ralph/autoloop starts: 6
- Ralph/autoloop finishes: 6
- Passed post-iteration probe/handoff gates: 6
- Literal native-journal `smoke.complete` topics: 1
- Ordered evidence lines: 6
- Result parser exit status: 0
- Elapsed time: 115.7 seconds
- Estimated provider cost: USD 0.22

The sanitized ordered evidence was:

```text
HARNESS_SMOKE:claude
HARNESS_SMOKE:codex
HARNESS_SMOKE:opencode
HARNESS_SMOKE:pi
HARNESS_SMOKE:hermes
HARNESS_SMOKE:kiro
```

The native artifacts established one exact fixed-probe sentinel/result and one
passing lifecycle gate for each role, followed by the exact response and handoff.
Provider-native streams do not expose a uniform, complete inventory of every
read-only tool call. Therefore this record does **not** claim that exact absence
of unrelated read-only calls was independently observable across all six
providers; it claims only the exact invocation/result evidence that the runtime
artifacts and gates can verify.
