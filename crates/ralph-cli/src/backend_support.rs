//! Shared backend metadata for CLI validation, resolution, and user-facing error messages.

use ralph_adapters::detect_backend_default;
use ralph_core::RalphConfig;
use std::path::Path;

/// Supported LLM backend identifiers in ralph-cli.
pub const VALID_BACKENDS: &[&str] = &[
    "claude", "kiro", "kiro-acp", "gemini", "codex", "forge", "amp", "copilot", "opencode", "pi",
    "custom",
];

/// Human-readable list derived from [`VALID_BACKENDS`] so the two can never drift.
pub fn valid_backends_label() -> String {
    VALID_BACKENDS.join(", ")
}

/// Returns `true` if the backend identifier is known.
pub fn is_known_backend(name: &str) -> bool {
    VALID_BACKENDS.contains(&name)
}

/// Formats the canonical unknown-backend error with all supported backends.
pub fn unknown_backend_message(name: &str) -> String {
    format!(
        "Unknown backend: {}\n\nValid backends: {}",
        name,
        valid_backends_label()
    )
}

/// Validates a backend name, returning the error message on failure.
pub fn validate_backend_name(name: &str) -> Result<(), String> {
    if is_known_backend(name) {
        Ok(())
    } else {
        Err(unknown_backend_message(name))
    }
}

/// Resolves which backend to use.
///
/// Precedence (highest to lowest):
/// 1. CLI flag (`--backend`)
/// 2. In-memory config (`cli.backend` unless "auto")
/// 3. Config file on disk (loaded from `config_path`, if provided)
/// 4. Auto-detect (first available from claude → kiro → gemini → codex → amp)
pub fn resolve_backend(
    flag_override: Option<&str>,
    config: Option<&RalphConfig>,
    config_path: Option<&Path>,
) -> Result<String, String> {
    if let Some(backend) = flag_override {
        validate_backend_name(backend)?;
        return Ok(backend.to_string());
    }

    if let Some(config) = config
        && config.cli.backend != "auto"
    {
        return Ok(config.cli.backend.clone());
    }

    if let Some(path) = config_path
        && path.exists()
        && let Ok(config) = RalphConfig::from_file(path)
        && config.cli.backend != "auto"
    {
        return Ok(config.cli.backend);
    }

    detect_backend_default().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_is_known_backend() {
        assert!(is_known_backend("forge"));
        assert!(VALID_BACKENDS.contains(&"forge"));
    }

    #[test]
    fn unknown_backend_message_lists_forge() {
        let message = unknown_backend_message("bogus");
        assert!(message.contains("forge"));
    }

    #[test]
    fn validate_known_backend_ok() {
        assert!(validate_backend_name("claude").is_ok());
        assert!(validate_backend_name("kiro-acp").is_ok());
    }

    #[test]
    fn validate_unknown_backend_err() {
        let err = validate_backend_name("bogus").unwrap_err();
        assert!(err.contains("Unknown backend: bogus"));
        assert!(err.contains("Valid backends"));
    }

    #[test]
    fn resolve_backend_flag_override() {
        let result = resolve_backend(Some("kiro"), None, None);
        assert_eq!(result.unwrap(), "kiro");
    }

    #[test]
    fn resolve_backend_invalid_flag() {
        let err = resolve_backend(Some("unknown"), None, None).unwrap_err();
        assert!(err.contains("Unknown backend: unknown"));
    }

    #[test]
    fn resolve_backend_from_config() {
        let config = RalphConfig::parse_yaml("cli:\n  backend: gemini\n").expect("parse");
        let result = resolve_backend(None, Some(&config), None);
        assert_eq!(result.unwrap(), "gemini");
    }

    #[test]
    fn resolve_backend_auto_skips_to_detect() {
        let config = RalphConfig::default();
        // auto-detect may fail (no backends installed in CI), but it shouldn't
        // return the literal "auto" string.
        let result = resolve_backend(None, Some(&config), None);
        assert!(result.as_deref() != Ok("auto"));
    }
}
