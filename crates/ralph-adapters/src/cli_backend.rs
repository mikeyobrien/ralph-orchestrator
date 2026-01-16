//! CLI backend definitions for different AI tools.

use ralph_core::{CliConfig, HatBackend};
use std::fmt;
use std::io::Write;
use tempfile::NamedTempFile;

/// Output format supported by a CLI backend.
///
/// This allows adapters to declare whether they emit structured JSON
/// for real-time streaming or plain text output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain text output (default for most adapters)
    #[default]
    Text,
    /// Newline-delimited JSON stream (Claude with --output-format stream-json)
    StreamJson,
}

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
    /// Output format emitted by this backend.
    pub output_format: OutputFormat,
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
    ///
    /// Uses `-p` flag for headless/print mode execution. This runs Claude
    /// in non-interactive mode where it executes the prompt and exits.
    /// For interactive mode, stdin is used instead (handled in build_command).
    ///
    /// Emits `--output-format stream-json` for NDJSON streaming output.
    /// Note: `--verbose` is required when using `--output-format stream-json` with `-p`.
    pub fn claude() -> Self {
        Self {
            command: "claude".to_string(),
            args: vec![
                "--dangerously-skip-permissions".to_string(),
                "--verbose".to_string(),
                "--output-format".to_string(),
                "stream-json".to_string(),
            ],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-p".to_string()),
            output_format: OutputFormat::StreamJson,
        }
    }

    /// Creates the Claude TUI backend for interactive mode.
    ///
    /// Runs Claude in full interactive mode (no -p flag), allowing
    /// Claude's native TUI to render. The prompt is passed as a
    /// positional argument.
    ///
    /// Unlike the standard `claude()` backend:
    /// - No `-p` flag (enters interactive TUI mode)
    /// - No `--output-format stream-json` (raw terminal output)
    /// - Prompt is a positional argument, not a flag value
    pub fn claude_tui() -> Self {
        Self {
            command: "claude".to_string(),
            args: vec!["--dangerously-skip-permissions".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,                 // No -p flag - prompt is positional
            output_format: OutputFormat::Text, // Not stream-json
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
            output_format: OutputFormat::Text,
        }
    }

    /// Creates the Kiro backend with a specific agent.
    ///
    /// Uses kiro-cli with --agent flag to select a specific agent.
    pub fn kiro_with_agent(agent: String) -> Self {
        Self {
            command: "kiro-cli".to_string(),
            args: vec![
                "chat".to_string(),
                "--no-interactive".to_string(),
                "--trust-all-tools".to_string(),
                "--agent".to_string(),
                agent,
            ],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        }
    }

    /// Creates a backend from a named backend string.
    ///
    /// # Errors
    /// Returns error if the backend name is invalid.
    pub fn from_name(name: &str) -> Result<Self, CustomBackendError> {
        match name {
            "claude" => Ok(Self::claude()),
            "kiro" => Ok(Self::kiro()),
            "gemini" => Ok(Self::gemini()),
            "codex" => Ok(Self::codex()),
            "amp" => Ok(Self::amp()),
            _ => Err(CustomBackendError),
        }
    }

    /// Creates a backend from a HatBackend configuration.
    ///
    /// # Errors
    /// Returns error if the backend configuration is invalid.
    pub fn from_hat_backend(hat_backend: &HatBackend) -> Result<Self, CustomBackendError> {
        match hat_backend {
            HatBackend::Named(name) => Self::from_name(name),
            HatBackend::KiroAgent { agent, .. } => Ok(Self::kiro_with_agent(agent.clone())),
            HatBackend::Custom { command, args } => Ok(Self {
                command: command.clone(),
                args: args.clone(),
                prompt_mode: PromptMode::Arg,
                prompt_flag: None,
                output_format: OutputFormat::Text,
            }),
        }
    }

    /// Creates the Gemini backend.
    pub fn gemini() -> Self {
        Self {
            command: "gemini".to_string(),
            args: vec!["--yolo".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-p".to_string()),
            output_format: OutputFormat::Text,
        }
    }

    /// Creates the Codex backend.
    pub fn codex() -> Self {
        Self {
            command: "codex".to_string(),
            args: vec!["exec".to_string(), "--full-auto".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None, // Positional argument
            output_format: OutputFormat::Text,
        }
    }

    /// Creates the Amp backend.
    pub fn amp() -> Self {
        Self {
            command: "amp".to_string(),
            args: vec!["--dangerously-allow-all".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-x".to_string()),
            output_format: OutputFormat::Text,
        }
    }

    /// Creates a backend configured for interactive mode with initial prompt.
    ///
    /// This factory method returns the correct backend configuration for running
    /// an interactive session with an initial prompt. The key differences from
    /// headless mode are:
    ///
    /// | Backend | Interactive + Prompt |
    /// |---------|---------------------|
    /// | Claude  | positional arg (no `-p` flag) |
    /// | Kiro    | removes `--no-interactive` |
    /// | Gemini  | uses `-i` instead of `-p` |
    /// | Codex   | no `exec` subcommand |
    /// | Amp     | removes `--dangerously-allow-all` |
    ///
    /// # Errors
    /// Returns `CustomBackendError` if the backend name is not recognized.
    pub fn for_interactive_prompt(backend_name: &str) -> Result<Self, CustomBackendError> {
        match backend_name {
            "claude" => Ok(Self::claude_tui()),
            "kiro" => Ok(Self::kiro_interactive()),
            "gemini" => Ok(Self::gemini_interactive()),
            "codex" => Ok(Self::codex_interactive()),
            "amp" => Ok(Self::amp_interactive()),
            _ => Err(CustomBackendError),
        }
    }

    /// Kiro in interactive mode (removes --no-interactive).
    ///
    /// Unlike headless `kiro()`, this allows the user to interact with
    /// Kiro's TUI while still passing an initial prompt.
    pub fn kiro_interactive() -> Self {
        Self {
            command: "kiro-cli".to_string(),
            args: vec!["chat".to_string(), "--trust-all-tools".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        }
    }

    /// Gemini in interactive mode with initial prompt (uses -i, not -p).
    ///
    /// **Critical quirk**: Gemini requires `-i` flag for interactive+prompt mode.
    /// Using `-p` would make it run headless and exit after one response.
    pub fn gemini_interactive() -> Self {
        Self {
            command: "gemini".to_string(),
            args: vec!["--yolo".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-i".to_string()), // NOT -p!
            output_format: OutputFormat::Text,
        }
    }

    /// Codex in interactive TUI mode (no exec subcommand).
    ///
    /// Unlike headless `codex()`, this runs without `exec` and `--full-auto`
    /// flags, allowing interactive TUI mode.
    pub fn codex_interactive() -> Self {
        Self {
            command: "codex".to_string(),
            args: vec![], // No exec, no --full-auto
            prompt_mode: PromptMode::Arg,
            prompt_flag: None, // Positional argument
            output_format: OutputFormat::Text,
        }
    }

    /// Amp in interactive mode (removes --dangerously-allow-all).
    ///
    /// Unlike headless `amp()`, this runs without the auto-approve flag,
    /// requiring user confirmation for tool usage.
    pub fn amp_interactive() -> Self {
        Self {
            command: "amp".to_string(),
            args: vec![],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-x".to_string()),
            output_format: OutputFormat::Text,
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
            output_format: OutputFormat::Text,
        })
    }

    /// Builds the full command with arguments for execution.
    ///
    /// # Arguments
    /// * `prompt` - The prompt text to pass to the agent
    /// * `interactive` - Whether to run in interactive mode (affects agent flags)
    pub fn build_command(
        &self,
        prompt: &str,
        interactive: bool,
    ) -> (String, Vec<String>, Option<String>, Option<NamedTempFile>) {
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
                                (
                                    format!("Please read and execute the task in {}", path),
                                    Some(file),
                                )
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

        // Log the full command being built
        tracing::debug!(
            command = %self.command,
            args_count = args.len(),
            prompt_len = prompt.len(),
            interactive = interactive,
            uses_stdin = stdin_input.is_some(),
            uses_temp_file = temp_file.is_some(),
            "Built CLI command"
        );
        // Log full prompt at trace level for debugging
        tracing::trace!(prompt = %prompt, "Full prompt content");

        (self.command.clone(), args, stdin_input, temp_file)
    }

    /// Filters args for interactive mode per spec table.
    fn filter_args_for_interactive(&self, args: Vec<String>) -> Vec<String> {
        match self.command.as_str() {
            "kiro-cli" => args
                .into_iter()
                .filter(|a| a != "--no-interactive")
                .collect(),
            "codex" => args.into_iter().filter(|a| a != "--full-auto").collect(),
            "amp" => args
                .into_iter()
                .filter(|a| a != "--dangerously-allow-all")
                .collect(),
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
            vec![
                "--dangerously-skip-permissions",
                "--verbose",
                "--output-format",
                "stream-json",
                "-p",
                "test prompt"
            ]
        );
        assert!(stdin.is_none()); // Uses -p flag, not stdin
        assert_eq!(backend.output_format, OutputFormat::StreamJson);
    }

    #[test]
    fn test_claude_tui_backend() {
        let backend = CliBackend::claude_tui();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "claude");
        // Should have --dangerously-skip-permissions and prompt as positional arg
        // No -p flag, no --output-format, no --verbose
        assert_eq!(args, vec!["--dangerously-skip-permissions", "test prompt"]);
        assert!(stdin.is_none()); // Uses positional arg, not stdin
        assert_eq!(backend.output_format, OutputFormat::Text);
        assert_eq!(backend.prompt_flag, None);
    }

    #[test]
    fn test_claude_large_prompt_uses_temp_file() {
        // With -p mode, large prompts (>7000 chars) use temp file to avoid CLI issues
        let backend = CliBackend::claude();
        let large_prompt = "x".repeat(7001);
        let (cmd, args, _stdin, temp) = backend.build_command(&large_prompt, false);

        assert_eq!(cmd, "claude");
        // Should have temp file for large prompts
        assert!(temp.is_some());
        // Args should contain instruction to read from temp file
        assert!(args.iter().any(|a| a.contains("Please read and execute")));
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
            vec![
                "chat",
                "--no-interactive",
                "--trust-all-tools",
                "test prompt"
            ]
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
        // Claude backend uses -p arg mode for headless execution
        let config = CliConfig {
            backend: "claude".to_string(),
            command: None,
            prompt_mode: "arg".to_string(),
            ..Default::default()
        };
        let backend = CliBackend::from_config(&config).unwrap();

        assert_eq!(backend.command, "claude");
        assert_eq!(backend.prompt_mode, PromptMode::Arg);
        assert_eq!(backend.prompt_flag, Some("-p".to_string()));
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
        let (cmd, args_auto, stdin_auto, _) = backend.build_command("test prompt", false);
        let (_, args_interactive, stdin_interactive, _) =
            backend.build_command("test prompt", true);

        assert_eq!(cmd, "claude");
        assert_eq!(args_auto, args_interactive);
        assert_eq!(
            args_auto,
            vec![
                "--dangerously-skip-permissions",
                "--verbose",
                "--output-format",
                "stream-json",
                "-p",
                "test prompt"
            ]
        );
        // -p mode is used for both auto and interactive
        assert!(stdin_auto.is_none());
        assert!(stdin_interactive.is_none());
    }

    #[test]
    fn test_gemini_interactive_mode_unchanged() {
        let backend = CliBackend::gemini();
        let (cmd, args_auto, stdin_auto, _) = backend.build_command("test prompt", false);
        let (_, args_interactive, stdin_interactive, _) =
            backend.build_command("test prompt", true);

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

    #[test]
    fn test_kiro_with_agent() {
        let backend = CliBackend::kiro_with_agent("my-agent".to_string());
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "kiro-cli");
        assert_eq!(
            args,
            vec![
                "chat",
                "--no-interactive",
                "--trust-all-tools",
                "--agent",
                "my-agent",
                "test prompt"
            ]
        );
        assert!(stdin.is_none());
    }

    #[test]
    fn test_from_name_claude() {
        let backend = CliBackend::from_name("claude").unwrap();
        assert_eq!(backend.command, "claude");
        assert_eq!(backend.prompt_flag, Some("-p".to_string()));
    }

    #[test]
    fn test_from_name_kiro() {
        let backend = CliBackend::from_name("kiro").unwrap();
        assert_eq!(backend.command, "kiro-cli");
    }

    #[test]
    fn test_from_name_gemini() {
        let backend = CliBackend::from_name("gemini").unwrap();
        assert_eq!(backend.command, "gemini");
    }

    #[test]
    fn test_from_name_codex() {
        let backend = CliBackend::from_name("codex").unwrap();
        assert_eq!(backend.command, "codex");
    }

    #[test]
    fn test_from_name_amp() {
        let backend = CliBackend::from_name("amp").unwrap();
        assert_eq!(backend.command, "amp");
    }

    #[test]
    fn test_from_name_invalid() {
        let result = CliBackend::from_name("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_hat_backend_named() {
        let hat_backend = HatBackend::Named("claude".to_string());
        let backend = CliBackend::from_hat_backend(&hat_backend).unwrap();
        assert_eq!(backend.command, "claude");
    }

    #[test]
    fn test_from_hat_backend_kiro_agent() {
        let hat_backend = HatBackend::KiroAgent {
            backend_type: "kiro".to_string(),
            agent: "my-agent".to_string(),
        };
        let backend = CliBackend::from_hat_backend(&hat_backend).unwrap();
        let (cmd, args, _, _) = backend.build_command("test", false);
        assert_eq!(cmd, "kiro-cli");
        assert!(args.contains(&"--agent".to_string()));
        assert!(args.contains(&"my-agent".to_string()));
    }

    #[test]
    fn test_from_hat_backend_custom() {
        let hat_backend = HatBackend::Custom {
            command: "my-cli".to_string(),
            args: vec!["--flag".to_string()],
        };
        let backend = CliBackend::from_hat_backend(&hat_backend).unwrap();
        assert_eq!(backend.command, "my-cli");
        assert_eq!(backend.args, vec!["--flag"]);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Tests for interactive prompt backends
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_for_interactive_prompt_claude() {
        let backend = CliBackend::for_interactive_prompt("claude").unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "claude");
        // Should use positional arg (no -p flag)
        assert_eq!(args, vec!["--dangerously-skip-permissions", "test prompt"]);
        assert!(stdin.is_none());
        assert_eq!(backend.prompt_flag, None);
    }

    #[test]
    fn test_for_interactive_prompt_kiro() {
        let backend = CliBackend::for_interactive_prompt("kiro").unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "kiro-cli");
        // Should NOT have --no-interactive
        assert_eq!(args, vec!["chat", "--trust-all-tools", "test prompt"]);
        assert!(!args.contains(&"--no-interactive".to_string()));
        assert!(stdin.is_none());
    }

    #[test]
    fn test_for_interactive_prompt_gemini() {
        let backend = CliBackend::for_interactive_prompt("gemini").unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "gemini");
        // Critical: should use -i flag, NOT -p
        assert_eq!(args, vec!["--yolo", "-i", "test prompt"]);
        assert_eq!(backend.prompt_flag, Some("-i".to_string()));
        assert!(stdin.is_none());
    }

    #[test]
    fn test_for_interactive_prompt_codex() {
        let backend = CliBackend::for_interactive_prompt("codex").unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "codex");
        // Should NOT have exec or --full-auto
        assert_eq!(args, vec!["test prompt"]);
        assert!(!args.contains(&"exec".to_string()));
        assert!(!args.contains(&"--full-auto".to_string()));
        assert!(stdin.is_none());
    }

    #[test]
    fn test_for_interactive_prompt_amp() {
        let backend = CliBackend::for_interactive_prompt("amp").unwrap();
        let (cmd, args, stdin, _temp) = backend.build_command("test prompt", false);

        assert_eq!(cmd, "amp");
        // Should NOT have --dangerously-allow-all
        assert_eq!(args, vec!["-x", "test prompt"]);
        assert!(!args.contains(&"--dangerously-allow-all".to_string()));
        assert!(stdin.is_none());
    }

    #[test]
    fn test_for_interactive_prompt_invalid() {
        let result = CliBackend::for_interactive_prompt("invalid_backend");
        assert!(result.is_err());
    }
}
