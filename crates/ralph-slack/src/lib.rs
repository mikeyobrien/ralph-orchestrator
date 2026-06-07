//! Slack integration seams for Ralph human-in-the-loop surfaces.
//!
//! The crate owns Slack Web API calls, loop/thread state, inbound thread routing,
//! and the loop-local [`SlackService`] implementation of `RobotService`.

pub mod api;
pub mod daemon;
pub mod error;
pub mod handler;
pub mod renderer;
pub mod service;
pub mod socket_mode;
pub mod state;

pub use api::SlackApi;
pub use daemon::{CommandLoopSpawner, SlackApiNotifier, SlackDaemon, SlackDaemonConfig};
pub use error::{SlackError, SlackResult};
pub use handler::{HandlerAction, SlackMessageEvent, handle_message};
pub use renderer::{SlackBlocks, SlackRenderedMessage};
pub use service::SlackService;
pub use state::{
    PendingSlackQuestion, SlackState, SlackStateManager, SlackThreadBinding, SlackThreadStatus,
};
