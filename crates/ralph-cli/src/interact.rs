//! Interact commands for human-in-the-loop communication.
//!
//! Provides non-blocking notification tools for agents:
//! - `ralph tools interact progress "message"` — Send a progress update via Telegram

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::bot;

#[derive(Parser, Debug)]
pub struct InteractArgs {
    #[command(subcommand)]
    pub command: InteractCommands,
}

#[derive(Subcommand, Debug)]
pub enum InteractCommands {
    /// Send a non-blocking progress update via Telegram
    Progress(ProgressArgs),
}

#[derive(Parser, Debug)]
pub struct ProgressArgs {
    /// The message to send
    pub message: String,
}

pub async fn execute(args: InteractArgs) -> Result<()> {
    match args.command {
        InteractCommands::Progress(progress_args) => send_progress(progress_args).await,
    }
}

async fn send_progress(args: ProgressArgs) -> Result<()> {
    let token = bot::resolve_token().context(
        "No bot token. Run `ralph bot onboard --telegram` or set RALPH_TELEGRAM_BOT_TOKEN",
    )?;
    let chat_id = bot::resolve_chat_id()
        .context("No chat_id found. Run `ralph bot onboard --telegram` to detect it")?;

    bot::telegram_send_message(&token, chat_id, &args.message).await?;

    println!("Sent.");
    Ok(())
}
