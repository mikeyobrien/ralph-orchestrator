//! Browser test client for the Ralph v3 event stream.
//!
//! The page is `wasm-bindgen` and `web-sys`. It posts `POST /commands` and
//! renders `GET /events`. Wire types in this crate are the JSON the icedrop
//! daemon already speaks (`packages/protocol` in `icedrop-labs/pi-durable`).
//! There is no UI framework and no second protocol.

mod location;
mod reduce;
mod wire;

#[cfg(target_arch = "wasm32")]
mod page;

pub use location::query_param;
pub use reduce::{ChatMessage, View, apply_server_message};
pub use wire::{
    Command, CommandData, CommandEnvelope, CommandResult, EntrySeed, Failure, Incoming,
    MOCK_CONVERSATION_ID, MOCK_MODEL, QueueMode, Queued, ServerMessage, SteerMode,
    decode_server_message, event_batch, inbox_update_event, message_end_event, message_start_event,
    parse_incoming, run_end_event, run_start_event, snapshot_event, text_delta_event,
};
