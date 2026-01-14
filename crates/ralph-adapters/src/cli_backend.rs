//! CLI backend definitions for different AI tools.

use ralph_core::CliConfig;
use std::fmt;
use std::io::Write;
use tempfile::NamedTempFile;

/// Error when creating a custom backend without a command.
#[derive(Debug, Clone)]
pub struct CustomBackendError;

impl fmt::Display for CustomBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "custom backend requires a command to be specified")
    }
}

impl std::error::Error for CustomBackendError {}

/// How to pass prompts to the CLI tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// Pass prompt as a command-line argument.
    Arg,
    /// Write prompt to stdin.
    Stdin,
}

/// A CLI backend configuration for executing prompts.
#[derive(Debug, Clone)]
pub struct CliBackend {
    /// The command to execute.
    pub command: String,
    /// Additional arguments before the prompt.
    pub args: Vec<String>,
    /// How to pass the prompt.
    pub prompt_mode: PromptMode,
    /// Argument flag for prompt (if prompt_mode is Arg).
    pub prompt_flag: Option<String>,
}

impl CliBackend {
    /// Creates a backend from configuration.
    ///
    /// # Errors
    /// Returns `CustomBackendError` if backend is "custom" but no command is specified.
    pub fn from_config(config: &CliConfig) -> Result<Self, CustomBackendError> {
        match config.backend.as_str() {
            "claude" => Ok(Self::claude()),
            "kiro" => Ok(Self::kiro()),
            "gemini" => Ok(Self::gemini()),
            "codex" => Ok(Self::codex()),
            "amp" => Ok(Self::amp()),
            "custom" => Self::custom(config),
            _ => Ok(Self::claude()), // Default to claude
        }
    }

    /// Creates the Claude backend.
    pub fn claude() -> Self {
        Self {
            command: "claude".to_string(),
            args: vec!["--dangerously-skip-permissions".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-p".to_string()),
        }
    }

    /// Creates the Kiro backend.
    ///
    /// Uses kiro-cli in headless mode with all tools trusted.
    pub fn kiro() -> Self {
        Self {
            command: "kiro-cli".to_string(),
            args: vec![
                "chat".to_string(),
                "--no-interactive".to_string(),
                "--trust-all-tools".to_string(),
            ],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
        }
    }

    /// Creates the Gemini backend.
    pub fn gemini() -> Self {
        Self {
            command: "gemini".to_string(),
            args: vec!["--yolo".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-p".to_string()),
        }
    }

    /// Creates the Codex backend.
    pub fn codex() -> Self {
        Self {
            command: "codex".to_string(),
            args: vec!["exec".to_string(), "--full-auto".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None, // Positional argument
        }
    }

    /// Creates the Amp backend.
    pub fn amp() -> Self {
        Self {
            command: "amp".to_string(),
            args: vec!["--dangerously-allow-all".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-x".to_string()),
        }
    }

    /// Creates a custom backend from configuration.
    ///
    /// # Errors
    /// Returns `CustomBackendError` if no command is specified.
    pub fn custom(config: &CliConfig) -> Result<Self, CustomBackendError> {
        let command = config.command.clone().ok_or(CustomBackendError)?;
        let prompt_mode = if config.prompt_mode == "stdin" {
            PromptMode::Stdin
        } else {
            PromptMode::Arg
        };

        Ok(Self {
            command,
            args: config.args.clone(),
            prompt_mode,
            prompt_flag: config.prompt_flag.clone(),
        })
    }

    /// Builds the full command with arguments for execution.
    ///
    /// # Arguments
    /// * `prompt` - The prompt text to pass to the agent
    /// * `interactive` - Whether to run in interactive mode (affects agent flags)
    pub fn build_command(&self, prompt: &str, interactive: bool) -> (String, Vec<String>, Option<String>, Option<NamedTempFile>) {
        let mut args = self.args.clone();

        // Filter args based on execution mode per interactive-mode.spec.md
        if interactive {
            args = self.filter_args_for_interactive(args);
        }

        // Handle large prompts for Claude (>7000 chars)
        let (stdin_input, temp_file) = match self.prompt_mode {
            PromptMode::Arg => {
                let (prompt_text, temp_file) = if self.command == "claude" && prompt.len() > 7000 {
                    // Write to temp file and instruct Claude to read it
                    match NamedTempFile::new() {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(prompt.as_bytes()) {
                                tracing::warn!("Failed to write prompt to temp file: {}", e);
                                (prompt.to_string(), None)
                            } else {
                                let path = file.path().display().to_string();
                                (format!("Please read and execute the task in {}", path), Some(file))
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create temp file: {}", e);
                            (prompt.to_string(), None)
                        }
                    }
                } else {
                    (prompt.to_string(), None)
                };

                if let Some(ref flag) = self.prompt_flag {
                    args.push(flag.clone());
                }
                args.push(prompt_text);
                (None, temp_file)
            }
            PromptMode::Stdin => (Some(prompt.to_string()), None),
        };

        (self.command.clone(), args, stdin_input, temp_file)
    }

    /// Filters args for interactive mode per spec table.
    fn filter_args_for_interactive(&self, args: Vec<String>) -> Vec<String> {
        match self.command.as_str() {
            "kiro-cli" => args.into_iter().filter(|a| a != "--no-interactive").collect(),
            "codex" => args.into_iter().filter(|a| a != "--full-auto").collect(),
            "amp" => args.into_iter().filter(|a| a != "--dangerously-allow-all").collect(),
            _ => args, // claude, gemini unchanged
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_backend() {
        let backend = CliBackend::claude();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "claude");
        assert_eq!(
            args,
            vec!["--dangerously-skip-permissions", "-p", "test prompt"]
        );
        assert!(stdin.is_none());
    }

    #[test]
    fn test_claude_large_prompt() {
        let backend = CliBackend::claude();
        let large_prompt = "x".repeat(7001);
        let (cmd, args, stdin, temp) = backend.build_command(&large_prompt, false);

        assert_eq!(cmd, "claude");
        assert_eq!(args[0], "--dangerously-skip-permissions");
        assert_eq!(args[1], "-p");
        assert!(args[2].starts_with("Please read and execute the task in"));
        assert!(stdin.is_none());
        assert!(temp.is_some());
    }

    #[test]
    fn test_claude_small_prompt() {
        let backend = CliBackend::claude();
        let small_prompt = "x".repeat(7000);
        let (cmd, args, stdin, temp) = backend.build_command(&small_prompt, false);

        assert_eq!(cmd, "claude");
        assert_eq!(args[2], small_prompt);
        assert!(stdin.is_none());
        assert!(temp.is_none());
    }

    #[test]
    fn test_non_claude_large_prompt() {
        let backend = CliBackend::kiro();
        let large_prompt = "x".repeat(7001);
        let (cmd, args, stdin, temp) = backend.build_command(&large_prompt, false);

        assert_eq!(cmd, "kiro-cli");
        assert_eq!(args[3], large_prompt);
        assert!(stdin.is_none());
        assert!(temp.is_none());
    }

    #[test]
    fn test_kiro_backend() {
        let backend = CliBackend::kiro();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "kiro-cli");
        assert_eq!(
            args,
            vec!["chat", "--no-interactive", "--trust-all-tools", "test prompt"]
        );
        assert!(stdin.is_none());
    }

    #[test]
    fn test_gemini_backend() {
        let backend = CliBackend::gemini();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "gemini");
        assert_eq!(args, vec!["--yolo", "-p", "test prompt"]);
        assert!(stdin.is_none());
    }

    #[test]
    fn test_codex_backend() {
        let backend = CliBackend::codex();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "codex");
        assert_eq!(args, vec!["exec", "--full-auto", "test prompt"]);
        assert!(stdin.is_none());
    }

    #[test]
    fn test_amp_backend() {
        let backend = CliBackend::amp();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "amp");
        assert_eq!(args, vec!["--dangerously-allow-all", "-x", "test prompt"]);
        assert!(stdin.is_none());
    }

    #[test]
    fn test_from_config() {
        let config = CliConfig {
            backend: "claude".to_string(),
            command: None,
            prompt_mode: "arg".to_string(),
            ..Default::default()
        };
        let backend = CliBackend::from_config(&config).unwrap();

        assert_eq!(backend.command, "claude");
        assert_eq!(backend.prompt_mode, PromptMode::Arg);
    }

    #[test]
    fn test_kiro_interactive_mode_omits_no_interactive_flag() {
        let backend = CliBackend::kiro();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", true);

        assert_eq!(cmd, "kiro-cli");
        assert_eq!(args, vec!["chat", "--trust-all-tools", "test prompt"]);
        assert!(stdin.is_none());
        assert!(!args.contains(&"--no-interactive".to_string()));
    }

    #[test]
    fn test_codex_interactive_mode_omits_full_auto() {
        let backend = CliBackend::codex();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", true);

        assert_eq!(cmd, "codex");
        assert_eq!(args, vec!["exec", "test prompt"]);
        assert!(stdin.is_none());
        assert!(!args.contains(&"--full-auto".to_string()));
    }

    #[test]
    fn test_amp_interactive_mode_no_flags() {
        let backend = CliBackend::amp();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", true);

        assert_eq!(cmd, "amp");
        assert_eq!(args, vec!["-x", "test prompt"]);
        assert!(stdin.is_none());
        assert!(!args.contains(&"--dangerously-allow-all".to_string()));
    }

    #[test]
    fn test_claude_interactive_mode_unchanged() {
        let backend = CliBackend::claude();
        let (cmd, args_auto, _, _) = backend.build_command("test prompt", false);
        let (_, args_interactive, _, _) = backend.build_command("test prompt", true);

        assert_eq!(cmd, "claude");
        assert_eq!(args_auto, args_interactive);
        assert!(args_auto.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn test_gemini_interactive_mode_unchanged() {
        let backend = CliBackend::gemini();
        let (cmd, args_auto, stdin_auto, _) = backend.build_command("test prompt", false);
        let (_, args_interactive, stdin_interactive, _) = backend.build_command("test prompt", true);

        assert_eq!(cmd, "gemini");
        assert_eq!(args_auto, args_interactive);
        assert_eq!(args_auto, vec!["--yolo", "-p", "test prompt"]);
        assert_eq!(stdin_auto, stdin_interactive);
        assert!(stdin_auto.is_none());
    }

    #[test]
    fn test_custom_backend_with_prompt_flag_short() {
        let config = CliConfig {
            backend: "custom".to_string(),
            command: Some("my-agent".to_string()),
            prompt_mode: "arg".to_string(),
            prompt_flag: Some("-p".to_string()),
            ..Default::default()
        };
        let backend = CliBackend::from_config(&config).unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "my-agent");
        assert_eq!(args, vec!["-p", "test prompt"]);
        assert!(stdin.is_none());
    }

    #[test]
    fn test_custom_backend_with_prompt_flag_long() {
        let config = CliConfig {
            backend: "custom".to_string(),
            command: Some("my-agent".to_string()),
            prompt_mode: "arg".to_string(),
            prompt_flag: Some("--prompt".to_string()),
            ..Default::default()
        };
        let backend = CliBackend::from_config(&config).unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "my-agent");
        assert_eq!(args, vec!["--prompt", "test prompt"]);
        assert!(stdin.is_none());
    }

    #[test]
    fn test_custom_backend_without_prompt_flag_positional() {
        let config = CliConfig {
            backend: "custom".to_string(),
            command: Some("my-agent".to_string()),
            prompt_mode: "arg".to_string(),
            prompt_flag: None,
            ..Default::default()
        };
        let backend = CliBackend::from_config(&config).unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "my-agent");
        assert_eq!(args, vec!["test prompt"]);
        assert!(stdin.is_none());
    }

    #[test]
    fn test_custom_backend_without_command_returns_error() {
        let config = CliConfig {
            backend: "custom".to_string(),
            command: None,
            prompt_mode: "arg".to_string(),
            ..Default::default()
        };
        let result = CliBackend::from_config(&config);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "custom backend requires a command to be specified"
        );
    }
}
