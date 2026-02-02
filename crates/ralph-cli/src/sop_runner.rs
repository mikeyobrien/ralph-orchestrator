//! SOP Runner - executes bundled SOPs in interactive backend sessions.
//!
//! This module provides functionality for the `ralph plan` and `ralph code-task` commands,
//! which are thin wrappers that bypass Ralph's event loop entirely. They:
//! 1. Resolve which backend to use (flag → config → auto-detect)
//! 2. Build a prompt with the SOP content wrapped in XML tags
//! 3. Spawn an interactive session with the backend

use ralph_adapters::{CliBackend, CustomBackendError, NoBackendError, detect_backend_default};
use ralph_core::RalphConfig;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

/// Bundled SOP content - embedded at compile time for self-contained binary.
///
/// Note: SOPs are copied into crates/ralph-cli/sops/ for crates.io packaging.
/// The source files live in .claude/skills/ but must be duplicated here because
/// `include_str!` paths outside the crate directory aren't included when publishing.
pub mod sops {
    /// PDD (Prompt-Driven Development) SOP for planning sessions.
    pub const PDD: &str = include_str!("../sops/pdd.md");

    /// Code Task Generator SOP for creating code task files.
    pub const CODE_TASK_GENERATOR: &str = include_str!("../sops/code-task-generator.md");
}

/// Which SOP to run.
#[derive(Debug, Clone, Copy)]
pub enum Sop {
    /// Prompt-Driven Development - transforms rough ideas into detailed designs.
    Pdd,
    /// Code Task Generator - creates structured code task files.
    CodeTaskGenerator,
}

impl Sop {
    /// Returns the bundled SOP content.
    pub fn content(self) -> &'static str {
        match self {
            Sop::Pdd => sops::PDD,
            Sop::CodeTaskGenerator => sops::CODE_TASK_GENERATOR,
        }
    }

    /// Returns a human-readable name for display.
    pub fn name(self) -> &'static str {
        match self {
            Sop::Pdd => "Prompt-Driven Development",
            Sop::CodeTaskGenerator => "Code Task Generator",
        }
    }
}

/// Configuration for running an SOP.
pub struct SopRunConfig {
    /// Which SOP to execute.
    pub sop: Sop,
    /// Optional user-provided input (idea for PDD, description for task generator).
    pub user_input: Option<String>,
    /// Explicit backend override (takes precedence over config and auto-detect).
    pub backend_override: Option<String>,
    /// Path to config file (for backend resolution fallback).
    pub config_path: Option<PathBuf>,
    /// Custom backend command and arguments (from CLI args).
    pub custom_args: Option<Vec<String>>,
}

/// Errors that can occur when running an SOP.
#[derive(Debug, Error)]
pub enum SopRunError {
    #[error("No supported backend found.\n\n{0}")]
    NoBackend(#[from] NoBackendError),

    #[error("Unknown backend: {0}\n\nValid backends: claude, kiro, gemini, codex, amp")]
    UnknownBackend(String),

    #[error("Failed to spawn backend: {0}")]
    SpawnError(#[from] std::io::Error),
}

impl From<CustomBackendError> for SopRunError {
    fn from(_: CustomBackendError) -> Self {
        SopRunError::UnknownBackend("custom".to_string())
    }
}

/// Runs an SOP in an interactive backend session.
///
/// This is the main entry point for `ralph plan` and `ralph code-task` commands.
/// It resolves the backend, builds the prompt, and spawns an interactive session.
pub fn run_sop(config: SopRunConfig) -> Result<(), SopRunError> {
    // 1. Resolve backend
    let backend_name = resolve_backend(
        config.backend_override.as_deref(),
        config.config_path.as_ref(),
    )?;

    // 2. Build the prompt
    let prompt = build_prompt(config.sop, config.user_input.as_deref());

    // 3. Get interactive backend configuration
    let cli_backend = if backend_name == "custom" {
        if let Some(args) = &config.custom_args {
            // Ad-hoc custom backend from CLI args
            if args.is_empty() {
                return Err(SopRunError::UnknownBackend(
                    "custom (no command specified in args)".to_string(),
                ));
            }
            let command = args[0].clone();
            let cli_args = args[1..].to_vec();

            CliBackend {
                command,
                args: cli_args,
                prompt_mode: ralph_adapters::PromptMode::Arg,
                prompt_flag: None, // Prompt appended as last arg by default
                output_format: ralph_adapters::OutputFormat::Text,
            }
        } else {
            // For custom backend from config, we need to load the configuration to get the command/args
            let config_path = config
                .config_path
                .as_deref()
                .unwrap_or_else(|| Path::new("ralph.yml"));

            if config_path.exists() {
                let ralph_config = RalphConfig::from_file(config_path).map_err(|e| {
                    SopRunError::SpawnError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    ))
                })?;
                CliBackend::custom(&ralph_config.cli)?
            } else {
                return Err(SopRunError::UnknownBackend(
                    "custom (configuration file not found and no CLI args provided)".to_string(),
                ));
            }
        }
    } else {
        CliBackend::for_interactive_prompt(&backend_name)?
    };

    // 4. Spawn the interactive session
    spawn_interactive(&cli_backend, &prompt)?;

    Ok(())
}

/// Resolves which backend to use.
///
/// Precedence (highest to lowest):
/// 1. CLI flag (`--backend`)
/// 2. Config file (`cli.backend` in ralph.yml)
/// 3. Auto-detect (first available from claude → kiro → gemini → codex → amp)
fn resolve_backend(
    flag_override: Option<&str>,
    config_path: Option<&PathBuf>,
) -> Result<String, SopRunError> {
    // 1. CLI flag takes precedence
    if let Some(backend) = flag_override {
        validate_backend_name(backend)?;
        return Ok(backend.to_string());
    }

    // 2. Check config file
    if let Some(path) = config_path
        && path.exists()
        && let Ok(config) = RalphConfig::from_file(path)
        && config.cli.backend != "auto"
    {
        return Ok(config.cli.backend);
    }

    // 3. Auto-detect
    detect_backend_default().map_err(SopRunError::NoBackend)
}

/// Validates a backend name.
fn validate_backend_name(name: &str) -> Result<(), SopRunError> {
    match name {
        "claude" | "kiro" | "gemini" | "codex" | "amp" | "copilot" | "opencode" | "custom" => {
            Ok(())
        }
        _ => Err(SopRunError::UnknownBackend(name.to_string())),
    }
}

/// Builds the combined SOP + user input prompt.
///
/// Format:
/// ```text
/// <sop>
/// {SOP content}
/// </sop>
/// <user-content>
/// {User's initial input if provided}
/// </user-content>
/// ```
fn build_prompt(sop: Sop, user_input: Option<&str>) -> String {
    let sop_content = sop.content();

    match user_input {
        Some(input) if !input.is_empty() => format!(
            "<sop>\n{}\n</sop>\n<user-content>\n{}\n</user-content>",
            sop_content, input
        ),
        _ => format!("<sop>\n{}\n</sop>", sop_content),
    }
}

/// Spawns an interactive backend session.
///
/// The session inherits stdin/stdout/stderr for full interactive capability.
fn spawn_interactive(backend: &CliBackend, prompt: &str) -> Result<(), SopRunError> {
    let (command, args, _stdin_input, _temp_file) = backend.build_command(prompt, true);

    let mut child = Command::new(&command)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    // Wait for the interactive session to complete
    child.wait()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::CwdGuard;

    #[test]
    fn test_sop_content_pdd() {
        let content = Sop::Pdd.content();
        // Should contain expected PDD content
        assert!(content.contains("Prompt-Driven Development"));
        assert!(content.contains("rough idea"));
    }

    #[test]
    fn test_sop_content_code_task_generator() {
        let content = Sop::CodeTaskGenerator.content();
        // Should contain expected code task generator content
        assert!(content.contains("Code Task Generator"));
        assert!(content.contains(".code-task.md"));
    }

    #[test]
    fn test_sop_name() {
        assert_eq!(Sop::Pdd.name(), "Prompt-Driven Development");
        assert_eq!(Sop::CodeTaskGenerator.name(), "Code Task Generator");
    }

    #[test]
    fn test_build_prompt_with_user_input() {
        let prompt = build_prompt(Sop::Pdd, Some("Build a REST API"));

        // Should have SOP wrapped in tags
        assert!(prompt.starts_with("<sop>\n"));
        assert!(prompt.contains("</sop>"));

        // Should have user input wrapped in tags
        assert!(prompt.contains("<user-content>\nBuild a REST API\n</user-content>"));
    }

    #[test]
    fn test_build_prompt_without_user_input() {
        let prompt = build_prompt(Sop::CodeTaskGenerator, None);

        // Should have SOP wrapped in tags
        assert!(prompt.starts_with("<sop>\n"));
        assert!(prompt.ends_with("</sop>"));

        // Should NOT have user-content tags
        assert!(!prompt.contains("<user-content>"));
    }

    #[test]
    fn test_build_prompt_with_empty_user_input() {
        let prompt = build_prompt(Sop::Pdd, Some(""));

        // Empty input should be treated like None
        assert!(!prompt.contains("<user-content>"));
    }

    #[test]
    fn test_validate_backend_name_valid() {
        assert!(validate_backend_name("claude").is_ok());
        assert!(validate_backend_name("kiro").is_ok());
        assert!(validate_backend_name("gemini").is_ok());
        assert!(validate_backend_name("codex").is_ok());
        assert!(validate_backend_name("amp").is_ok());
        assert!(validate_backend_name("copilot").is_ok());
        assert!(validate_backend_name("opencode").is_ok());
        assert!(validate_backend_name("custom").is_ok());
    }

    #[test]
    fn test_validate_backend_name_invalid() {
        let result = validate_backend_name("invalid_backend");
        assert!(result.is_err());

        if let Err(SopRunError::UnknownBackend(name)) = result {
            assert_eq!(name, "invalid_backend");
        } else {
            panic!("Expected UnknownBackend error");
        }
    }

    #[test]
    fn test_resolve_backend_from_flag() {
        let backend = resolve_backend(Some("claude"), None).expect("backend");
        assert_eq!(backend, "claude");
    }

    #[test]
    fn test_resolve_backend_invalid_flag() {
        let err = resolve_backend(Some("unknown"), None).expect_err("invalid backend");
        if let SopRunError::UnknownBackend(name) = err {
            assert_eq!(name, "unknown");
        } else {
            panic!("expected UnknownBackend");
        }
    }

    #[test]
    fn test_resolve_backend_from_config_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let config_path = temp_dir.path().join("ralph.yml");
        std::fs::write(&config_path, "cli:\n  backend: gemini\n").expect("write config");

        let backend = resolve_backend(None, Some(&config_path)).expect("backend");
        assert_eq!(backend, "gemini");
    }

    #[test]
    fn test_run_sop_custom_args_missing_errors() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let config = SopRunConfig {
            sop: Sop::Pdd,
            user_input: None,
            backend_override: Some("custom".to_string()),
            config_path: None,
            custom_args: None,
        };

        let err = run_sop(config).expect_err("expected error");
        if let SopRunError::UnknownBackend(msg) = err {
            assert!(msg.contains("configuration file not found"));
        } else {
            panic!("expected UnknownBackend");
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_run_sop_custom_args_executes() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let config = SopRunConfig {
            sop: Sop::Pdd,
            user_input: Some("Build a REST API".to_string()),
            backend_override: Some("custom".to_string()),
            config_path: None,
            custom_args: Some(vec![
                "sh".to_string(),
                "-c".to_string(),
                "exit 0".to_string(),
            ]),
        };

        run_sop(config).expect("run sop");
    }
}
