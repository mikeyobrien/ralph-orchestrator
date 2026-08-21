# Autoloop E2E fixtures

These fixtures use the shared fake-autoloop JSONL schema documented in
[`crates/ralph-cli/tests/fixtures/autoloop/README.md`](../../../ralph-cli/tests/fixtures/autoloop/README.md).

`engine_completion.jsonl` emits a terminal `completion_event`, writes a matching
journal terminal record, and prints the canonical summary consumed by Ralph's
real autoloop engine adapter.
