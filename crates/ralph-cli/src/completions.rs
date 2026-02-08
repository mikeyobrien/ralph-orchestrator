//! Shell completion generation for Ralph CLI.
//!
//! Provides the `ralph completions` subcommand to generate shell completion
//! scripts for bash, zsh, fish, and PowerShell.

use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::{Shell, generate};
use std::io;

/// Arguments for the completions subcommand.
#[derive(Parser, Debug)]
pub struct CompletionsArgs {
    /// The shell to generate completions for
    #[arg(value_enum)]
    pub shell: ShellArg,
}

/// Shell options for completion generation.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShellArg {
    /// Bash shell completions
    Bash,
    /// Zsh shell completions
    Zsh,
    /// Fish shell completions
    Fish,
    /// PowerShell completions
    PowerShell,
}

impl From<ShellArg> for Shell {
    fn from(arg: ShellArg) -> Self {
        match arg {
            ShellArg::Bash => Shell::Bash,
            ShellArg::Zsh => Shell::Zsh,
            ShellArg::Fish => Shell::Fish,
            ShellArg::PowerShell => Shell::PowerShell,
        }
    }
}

/// Generate shell completions for the given shell.
///
/// This function uses clap_complete to generate a completion script
/// for the specified shell, printing it to stdout.
pub fn generate_completions(args: &CompletionsArgs) {
    let shell: Shell = args.shell.into();
    let mut cmd = crate::Cli::command();
    generate(shell, &mut cmd, "ralph", &mut io::stdout());
}
