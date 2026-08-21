//! Lossless Ralph-to-Autoloop backend configuration mapping.

use anyhow::{Context, Result, bail};
use ralph_core::RalphConfig;

use crate::{CliBackend, PromptMode};

/// Process-level overrides for an [`crate::AutoloopRunner`].
#[derive(Debug, PartialEq, Eq)]
pub struct AutoloopBackendMapping {
    /// Autoloop's `-b` selector for native session backends. Command backends
    /// use structured `--set` overrides because Autoloop treats some executable
    /// names (for example `kiro-cli`) as native backend aliases.
    pub selector: Option<String>,
    /// Ordered Autoloop `--set key=value` overrides.
    pub set_overrides: Vec<(String, String)>,
}

/// Translate Ralph's resolved backend into Autoloop's backend vocabulary.
///
/// Returns an error rather than silently dropping Ralph configuration that
/// Autoloop cannot represent.
pub fn map_ralph_backend(config: &RalphConfig) -> Result<AutoloopBackendMapping> {
    let backend_name = config.cli.backend.as_str();
    if backend_name == "auto" || backend_name.is_empty() {
        bail!(
            "ralph backend `{backend_name}` was not resolved before starting Autoloop; select a backend explicitly or fix backend auto-detection"
        );
    }
    if !matches!(
        backend_name,
        "claude"
            | "kiro"
            | "kiro-acp"
            | "gemini"
            | "codex"
            | "forge"
            | "amp"
            | "copilot"
            | "opencode"
            | "pi"
            | "roo"
            | "custom"
    ) {
        bail!("ralph backend `{backend_name}` has no Autoloop mapping");
    }

    let has_native_overrides = config.cli.command.is_some()
        || !config.cli.args.is_empty()
        || config.cli.prompt_mode != "arg"
        || config.cli.prompt_flag.is_some();
    let native_selector = match backend_name {
        "claude" => Some("claude-sdk"),
        "kiro-acp" => Some("kiro"),
        "pi" => Some("pi"),
        _ => None,
    };
    if let Some(selector) = native_selector {
        if has_native_overrides {
            bail!(
                "ralph backend `{backend_name}` maps to Autoloop's native backend `{selector}`, but cli.command/args/prompt_mode/prompt_flag overrides cannot be preserved by that native backend"
            );
        }
        return Ok(AutoloopBackendMapping {
            selector: Some(selector.to_string()),
            set_overrides: vec![(
                "backend.timeout_ms".to_string(),
                timeout_ms(config, backend_name)?.to_string(),
            )],
        });
    }

    if backend_name == "roo" {
        bail!(
            "ralph backend `roo` uses per-iteration --prompt-file handling that Autoloop's command backend cannot preserve"
        );
    }

    let backend = CliBackend::from_config(&config.cli)
        .with_context(|| format!("resolving ralph backend `{backend_name}`"))?;
    if let Some(flag) = backend.prompt_flag.as_deref() {
        bail!(
            "ralph backend `{backend_name}` requires prompt flag `{flag}`, but Autoloop's command backend has no prompt flag setting"
        );
    }
    if let Some(arg) = backend.args.iter().find(|arg| arg.contains(',')) {
        bail!(
            "ralph backend `{backend_name}` argument `{arg}` contains a comma and cannot be represented losslessly in Autoloop's backend.args override"
        );
    }
    if backend.args.is_empty() && config.core.autoloop_preset.is_some() {
        bail!(
            "ralph backend `{backend_name}` has no command arguments, but Autoloop cannot clear backend.args from an explicit preset via --set; remove the preset backend arguments or use a generated Ralph preset"
        );
    }
    let prompt_mode = match backend.prompt_mode {
        PromptMode::Arg => "arg",
        PromptMode::Stdin => "stdin",
        PromptMode::NoPrompt => {
            bail!(
                "ralph backend `{backend_name}` does not accept a prompt, which Autoloop's command backend cannot preserve"
            )
        }
    };

    let mut set_overrides = vec![
        ("backend.kind".to_string(), "command".to_string()),
        ("backend.command".to_string(), backend.command),
    ];
    if !backend.args.is_empty() {
        set_overrides.push(("backend.args".to_string(), backend.args.join(",")));
    }
    set_overrides.extend([
        ("backend.prompt_mode".to_string(), prompt_mode.to_string()),
        (
            "backend.timeout_ms".to_string(),
            timeout_ms(config, backend_name)?.to_string(),
        ),
    ]);

    Ok(AutoloopBackendMapping {
        selector: None,
        set_overrides,
    })
}

fn timeout_ms(config: &RalphConfig, backend_name: &str) -> Result<u64> {
    config
        .adapter_settings(backend_name)
        .timeout
        .checked_mul(1000)
        .context("ralph backend timeout is too large to convert to milliseconds")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(yaml: &str) -> RalphConfig {
        RalphConfig::parse_yaml(yaml).expect("valid test config")
    }

    #[test]
    fn maps_native_ralph_backends_to_autoloop_selectors() {
        for (ralph, autoloop) in [("claude", "claude-sdk"), ("kiro-acp", "kiro"), ("pi", "pi")] {
            let mapping =
                map_ralph_backend(&config(&format!("cli:\n  backend: {ralph}\n"))).unwrap();
            assert_eq!(mapping.selector.as_deref(), Some(autoloop));
            assert_eq!(
                mapping.set_overrides,
                vec![("backend.timeout_ms".to_string(), "300000".to_string())]
            );
        }
    }

    #[test]
    fn maps_command_backend_with_ralph_defaults_and_timeout() {
        let mapping = map_ralph_backend(&config(
            "cli:\n  backend: codex\nadapters:\n  codex:\n    timeout: 17\n",
        ))
        .unwrap();

        assert_eq!(mapping.selector, None);
        assert_eq!(
            mapping.set_overrides,
            vec![
                ("backend.kind".to_string(), "command".to_string()),
                ("backend.command".to_string(), "codex".to_string()),
                ("backend.args".to_string(), "exec,--yolo".to_string()),
                ("backend.prompt_mode".to_string(), "arg".to_string()),
                ("backend.timeout_ms".to_string(), "17000".to_string()),
            ]
        );
    }

    #[test]
    fn maps_other_positional_command_backends() {
        for (name, command, args) in [
            (
                "kiro",
                "kiro-cli",
                "chat,--no-interactive,--trust-all-tools",
            ),
            ("opencode", "opencode", "run"),
        ] {
            let mapping =
                map_ralph_backend(&config(&format!("cli:\n  backend: {name}\n"))).unwrap();
            assert_eq!(mapping.selector, None);
            assert!(
                mapping
                    .set_overrides
                    .contains(&("backend.command".to_string(), command.to_string()))
            );
            assert!(
                mapping
                    .set_overrides
                    .contains(&("backend.args".to_string(), args.to_string()))
            );
        }
    }

    #[test]
    fn maps_representable_custom_backend_configuration() {
        let mapping = map_ralph_backend(&config(
            "cli:\n  backend: custom\n  command: /opt/agent\n  args: [run, --yes]\n  prompt_mode: stdin\n",
        ))
        .unwrap();

        assert_eq!(mapping.selector, None);
        assert!(
            mapping
                .set_overrides
                .contains(&("backend.command".to_string(), "/opt/agent".to_string()))
        );
        assert!(
            mapping
                .set_overrides
                .contains(&("backend.args".to_string(), "run,--yes".to_string()))
        );
        assert!(
            mapping
                .set_overrides
                .contains(&("backend.prompt_mode".to_string(), "stdin".to_string()))
        );
    }

    #[test]
    fn rejects_backend_semantics_autoloop_cannot_preserve() {
        for (yaml, expected) in [
            ("cli:\n  backend: gemini\n", "prompt flag"),
            ("cli:\n  backend: forge\n", "prompt flag"),
            ("cli:\n  backend: amp\n", "prompt flag"),
            ("cli:\n  backend: copilot\n", "prompt flag"),
            ("cli:\n  backend: roo\n", "prompt-file"),
            (
                "cli:\n  backend: claude\n  args: [--model, opus]\n",
                "native backend",
            ),
            (
                "cli:\n  backend: custom\n  command: agent\n  prompt_flag: -p\n",
                "prompt flag",
            ),
            (
                "cli:\n  backend: custom\n  command: agent\n  args: ['a,b']\n",
                "comma",
            ),
            ("cli:\n  backend: auto\n", "not resolved"),
            (
                "core:\n  autoloop_preset: preset\ncli:\n  backend: custom\n  command: agent\n",
                "cannot clear backend.args",
            ),
        ] {
            let err = map_ralph_backend(&config(yaml)).unwrap_err().to_string();
            assert!(
                err.contains(expected),
                "expected {err:?} to contain {expected:?}"
            );
        }
    }
}
