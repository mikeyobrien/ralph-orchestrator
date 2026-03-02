//! Shared backend metadata for CLI validation and user-facing error messages.

/// Supported LLM backend identifiers in ralph-cli.
pub const VALID_BACKENDS: &[&str] = &[
    "claude", "kiro", "kiro-acp", "gemini", "codex", "amp", "copilot", "opencode", "pi", "custom",
];

/// Human-readable list for CLI messages and docs.
pub const VALID_BACKENDS_LABEL: &str =
    "claude, kiro, kiro-acp, gemini, codex, amp, copilot, opencode, pi, custom";

/// Returns `true` if the backend identifier is known.
pub fn is_known_backend(name: &str) -> bool {
    VALID_BACKENDS.contains(&name)
}

/// Formats the canonical unknown-backend error with all supported backends.
pub fn unknown_backend_message(name: &str) -> String {
    format!(
        "Unknown backend: {}\n\nValid backends: {}",
        name, VALID_BACKENDS_LABEL
    )
}
