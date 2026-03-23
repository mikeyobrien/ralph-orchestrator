# Scope

Follow-up wave on the stream/auth family discovered in wave 1.

Inspected:

- `crates/ralph-api/src/runtime/dispatch.rs`
- `crates/ralph-api/src/stream_domain/mod.rs`
- `crates/ralph-api/src/transport.rs`

# Findings

## P1: `stream.unsubscribe` and `stream.ack` do not enforce subscription ownership

- Impacted files:
  - `crates/ralph-api/src/runtime/dispatch.rs`
  - `crates/ralph-api/src/stream_domain/mod.rs`
- Why it is a bug:
  - Subscriptions are created with an owning `principal`, and WebSocket attach validates that principal, but the RPC methods that mutate subscription state do not pass or verify the principal at all.
- Exact evidence:
  - Each subscription stores an owning principal: `crates/ralph-api/src/stream_domain/mod.rs:99-107`, `crates/ralph-api/src/stream_domain/mod.rs:155-164`.
  - WebSocket attach checks the stored principal before binding a socket: `crates/ralph-api/src/transport.rs:151-158`.
  - `dispatch_stream()` calls `unsubscribe(params)` and `ack(params)` without any principal argument or ownership check: `crates/ralph-api/src/runtime/dispatch.rs:320-339`.
  - `StreamDomain::unsubscribe()` and `StreamDomain::ack()` only look up by `subscription_id`; neither verifies `principal`: `crates/ralph-api/src/stream_domain/mod.rs:182-210`.
- Triggering scenario:
  - Client A subscribes and receives `subscriptionId`.
  - Client B, authenticated separately, learns or guesses that `subscriptionId`.
  - Client B sends `stream.unsubscribe` or `stream.ack` for A's subscription and can terminate it or advance its checkpoint.
- Likely impact:
  - Cross-client stream disruption and lost events; this is an authorization-boundary failure even though socket attach itself is protected.
- Recommended fix direction:
  - Thread `principal` through `stream.unsubscribe` and `stream.ack`, and reject mutations where the caller does not own the subscription.
  - Keep the WebSocket attach check, but do not rely on it as the only ownership gate.
- Confidence:
  - High.
- Whether current tests cover it:
  - No ownership test for `stream.unsubscribe`/`stream.ack` was visible in the inspected files.

# No-Finding Coverage Notes

- `crates/ralph-api/src/transport.rs`
  - Rechecked socket-attach principal validation.
  - The attach path itself appears sound in-scope.
- `crates/ralph-api/src/stream_domain/mod.rs`
  - Rechecked subscription storage and ownership metadata.
  - The missing enforcement is in mutation paths, not subscription creation.

# Remaining Blind Spots

- I did not validate exploitability against any external client in a live runtime.
- I did not inspect client code that might accidentally leak subscription IDs.

# Recommended Next Search

- Audit where subscription IDs are surfaced to the browser and whether logs or errors expose them unnecessarily.
