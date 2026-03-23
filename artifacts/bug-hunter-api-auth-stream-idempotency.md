# Scope

Scoped lane `api-auth-stream-idempotency`, limited to:

- `crates/ralph-api/src/auth.rs`
- `crates/ralph-api/src/idempotency.rs`
- `crates/ralph-api/src/runtime.rs`
- `crates/ralph-api/src/transport.rs`
- `crates/ralph-api/src/stream_domain/mod.rs`

# Findings

## P1: Idempotency check/store is non-atomic, so concurrent duplicate mutations can both execute

- Impacted files:
  - `crates/ralph-api/src/runtime.rs`
  - `crates/ralph-api/src/idempotency.rs`
- Why it is a bug:
  - `execute_request()` performs `check(...)`, then dispatches the mutation, then stores the result. The in-memory idempotency store has no in-flight reservation state, so overlapping same-key mutations can both see `New` and both run the side effect.
- Exact evidence:
  - Runtime check before dispatch: `crates/ralph-api/src/runtime.rs:296-337`.
  - Result stored only after dispatch completes: `crates/ralph-api/src/runtime.rs:344-360`.
  - Store implementation only tracks completed entries; no pending or claimed state exists: `crates/ralph-api/src/idempotency.rs:57-88`.
- Triggering scenario:
  - A client retries a mutating RPC after a timeout while the first attempt is still executing.
  - Both requests use the same method, same params, and same `meta.idempotencyKey`.
  - Both perform the mutation before either stores the cached response.
- Likely impact:
  - Duplicate side effects for supposedly idempotent task, loop, planning, or config mutations.
- Recommended fix direction:
  - Replace split `check`/`store` with an atomic claim/in-progress API, or serialize same-key mutations and let later callers wait for the first response.
- Confidence:
  - High.
- Whether current tests cover it:
  - No concurrent same-key mutation regression was visible in the inspected files.

## P1: WebSocket replay/live handoff can drop events published during reconnect bootstrap

- Impacted files:
  - `crates/ralph-api/src/transport.rs`
  - `crates/ralph-api/src/stream_domain/mod.rs`
- Why it is a bug:
  - The server computes and sends replay first, then subscribes to live traffic. Events published in the gap are in neither stream.
- Exact evidence:
  - `stream_connection()` calls `replay_for_subscription(...)`, sends replay, and only then creates `live_receiver()`: `crates/ralph-api/src/transport.rs:161-183`.
  - Live delivery uses `broadcast::subscribe()` and only sees future sends: `crates/ralph-api/src/stream_domain/mod.rs:224-226`.
  - `publish()` immediately appends to history and broadcasts live, so gap events are real: `crates/ralph-api/src/stream_domain/mod.rs:358-383`.
- Triggering scenario:
  - A client reconnects with a backlog.
  - While replay is being sent, a new stream event is published.
  - That event is not in the captured replay batch and also not in the later-created live receiver.
- Likely impact:
  - Lost status/log/config events during exactly the reconnect flows that depend on reliable replay.
- Recommended fix direction:
  - Subscribe to live traffic before replay and de-duplicate by cursor/sequence, or make replay snapshot plus live starting point atomic.
- Confidence:
  - High.
- Whether current tests cover it:
  - No reconnect-under-concurrent-publish test was visible in the inspected files.

# No-Finding Coverage Notes

- `crates/ralph-api/src/auth.rs`
  - Checked header/body token extraction and config selection.
  - No direct auth bypass was confirmed in-scope.
- `crates/ralph-api/src/transport.rs`
  - Checked WebSocket attach authentication.
  - The attach path does enforce principal matching before binding a socket to an existing subscription: `crates/ralph-api/src/transport.rs:151-158`.
- `crates/ralph-api/src/stream_domain/mod.rs`
  - Checked stored principal tracking, replay generation, ack handling, and history buffering.
  - No stronger defect found in the inspected file beyond replay gap risk.

# Remaining Blind Spots

- This lane did not inspect the RPC stream dispatch path outside the requested files.
- This lane did not validate ownership enforcement for `stream.ack` / `stream.unsubscribe`.

# Recommended Next Search

- Inspect the stream RPC dispatch path next. `StreamDomain::ack` and `StreamDomain::unsubscribe` do not take a principal, so if dispatch also omits ownership checks, that becomes the next likely auth-boundary defect.
