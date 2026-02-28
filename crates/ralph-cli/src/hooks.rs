//! CLI commands for the `ralph hooks` namespace.
//!
//! This command surface validates hook configuration and command wiring
//! without starting loop execution.

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::{ConfigSource, HatsSource};

/// Manage hook-related commands.
#[derive(Parser, Debug)]
pub struct HooksArgs {
    #[command(subcommand)]
    pub command: HooksCommands,
}

#[derive(Subcommand, Debug)]
pub enum HooksCommands {
    /// Validate hooks configuration and command wiring
    Validate(ValidateArgs),
}

/// Output format for `ralph hooks validate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HooksValidateFormat {
    Human,
    Json,
}

/// Arguments for `ralph hooks validate`.
#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Output format (human or json)
    #[arg(long, value_enum, default_value_t = HooksValidateFormat::Human)]
    pub format: HooksValidateFormat,
}

/// Execute a hooks command.
pub fn execute(
    _config_sources: &[ConfigSource],
    _hats_source: Option<&HatsSource>,
    args: HooksArgs,
    _use_colors: bool,
) -> Result<()> {
    match args.command {
        HooksCommands::Validate(_) => {
            anyhow::bail!("`ralph hooks validate` is not implemented yet");
        }
    }
}
