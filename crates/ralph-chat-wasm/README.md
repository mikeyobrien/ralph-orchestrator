# Ralph v3 chat test surface

A single HTML page, compiled from Rust to WebAssembly, for watching a v3 run.
It is a test client of the icedrop event stream. It is not the Ralph web
dashboard, and it is not a product UI.

The page posts commands and renders the server-sent event stream:

- `POST /commands` with `{ "id", "command" }` — Send is `submit`, Steer is `submit` with `whenBusy: "steer"`, Cancel is `abort`
- `GET /events` — `text/event-stream`. Each `data` line is a `ServerMessage` (`kind: "event"` with a batch, or `kind: "result"`)

Those two routes are the contract in `docs/design/v3-on-icedrop.md` (`packages/protocol` on `icedrop-labs/pi-durable`). A `snapshot` replaces the transcript. There are no sequence numbers.

The daemon is not in this repo. `ralph-chat-mock` is a stand-in so the page can be exercised. It is a mock: no sqlite, no hats, no resume. The page says so when `agent.model` is `mock`.

## Build

No Node toolchain. Rust, the `wasm32-unknown-unknown` target, and [trunk](https://trunkrs.dev/).

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked

cd crates/ralph-chat-wasm
trunk build --release
```

Trunk 0.21 treats `NO_COLOR` as a boolean flag. If it is set to `1`, trunk exits with `invalid value '1' for '--no-color'`. Unset it for the build: `env -u NO_COLOR trunk build --release`.

Trunk writes `crates/ralph-chat-wasm/dist/`.

## Run the mock and the page

From the repo root, one process serves the API and the built page:

```bash
cargo run -p ralph-chat-mock -- --dist crates/ralph-chat-wasm/dist
```

Open <http://127.0.0.1:8788>. Send a line, watch the text deltas, then Cancel.

While iterating on the page, trunk can serve the wasm and the mock can serve only the API:

```bash
cargo run -p ralph-chat-mock -- --bind 127.0.0.1:8788
cd crates/ralph-chat-wasm && trunk serve --port 8080
```

Open <http://127.0.0.1:8080/?daemon=http%3A%2F%2F127.0.0.1%3A8788>.

Point `?daemon=` at a real icedrop daemon when one is available. The page will post the same JSON and render the same event batch. Commands other than `submit` and `abort` are answered `invalid` by the mock only; a real daemon accepts the rest of `packages/protocol`.

## What the mock does

| Action | Wire |
|---|---|
| Send | `{ "type": "submit", "text", "requestId" }` and a streamed assistant reply |
| Send while a run is going | queued as `followUp`, started after `run_end` unless the run was aborted |
| Steer while a run is going | `whenBusy: "steer"`, `inbox_update`, then folded in at the next text chunk |
| Steer or Send while idle | starts a run |
| `whenBusy: "reject"` while busy | `{ "ok": false, "error": { "code": "busy" } }` |
| Cancel | `{ "type": "abort" }`, then `run_end` with `status: "aborted"` |
| Same `requestId` again | the same ack, no second run |
| `requestId` starting with `wake:` or `depart:` | `invalid` — those ids are minted by the daemon |

`GET /events` sends a `snapshot` first (`agent.model` is `"mock"`), then live events. Comment keepalives go out every 15 seconds. A subscriber that falls behind the broadcast gets a fresh snapshot.

## Tests

```bash
cargo test -p ralph-chat-wasm -p ralph-chat-mock
```

The mock tests open the SSE stream, post a submit, read a `text_delta`, post `abort`, and require `run_end` / `aborted`.
