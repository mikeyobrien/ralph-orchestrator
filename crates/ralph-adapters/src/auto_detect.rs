//! Auto-detection logic for agent backends.
//!
//! When config specifies `agent: auto`, this module handles detecting
//! which backends are available in the system PATH.

use std::process::Command;
use std::sync::OnceLock;
use tracing::debug;

pub use ralph_core::backend::default_priority;

/// Cached detection result for session duration.
static DETECTED_BACKEND: OnceLock<Option<String>> = OnceLock::new();

/// Error returned when no backends are available.
#[derive(Debug, Clone)]
pub struct NoBackendError {
    /// Backends that were checked.
    pub checked: Vec<String>,
}

impl std::fmt::Display for NoBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "No supported AI backend found in PATH.")?;
        writeln!(f)?;
        writeln!(f, "Checked backends: {}", self.checked.join(", "))?;
        writeln!(f)?;
        writeln!(
            f,
            "Fix: install a backend CLI or run `ralph doctor` to validate your setup."
        )?;
        writeln!(f, "See: docs/reference/troubleshooting.md#agent-not-found")?;
        writeln!(f)?;
        writeln!(f, "Install one of the following:")?;
        // Render install guidance from the core catalog so the list (and the Pi
        // URL) never drifts. `kiro` and `kiro-acp` share the `kiro-cli` binary,
        // so list each command once.
        let mut seen_commands: Vec<&str> = Vec::new();
        for metadata in ralph_core::backend::iter() {
            if seen_commands.contains(&metadata.command) {
                continue;
            }
            seen_commands.push(metadata.command);
            writeln!(
                f,
                "  • {} CLI: {}",
                metadata.display_name, metadata.install_url
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for NoBackendError {}

/// Checks if a backend is available by running its version command.
///
/// Each backend is detected by running `<command> --version` and checking
/// for exit code 0. The command is resolved from the core catalog (e.g. the
/// `kiro`/`kiro-acp` backends use the `kiro-cli` binary); an unrecognized name
/// falls back to probing itself.
pub fn is_backend_available(backend: &str) -> bool {
    let command = ralph_core::backend::command_for(backend).unwrap_or(backend);
    let result = Command::new(command).arg("--version").output();

    match result {
        Ok(output) => {
            let available = output.status.success();
            debug!(
                backend = backend,
                command = command,
                available = available,
                "Backend availability check"
            );
            available
        }
        Err(_) => {
            debug!(
                backend = backend,
                command = command,
                available = false,
                "Backend not found in PATH"
            );
            false
        }
    }
}

/// Detects the first available backend from a priority list.
///
/// # Arguments
/// * `priority` - List of backend names to check in order
/// * `adapter_enabled` - Function that returns whether an adapter is enabled in config
///
/// # Returns
/// * `Ok(backend_name)` - First available backend
/// * `Err(NoBackendError)` - No backends available
pub fn detect_backend<F>(priority: &[&str], adapter_enabled: F) -> Result<String, NoBackendError>
where
    F: Fn(&str) -> bool,
{
    debug!(priority = ?priority, "Starting backend auto-detection");

    // Check cache first
    if let Some(cached) = DETECTED_BACKEND.get()
        && let Some(backend) = cached
    {
        debug!(backend = %backend, "Using cached backend detection result");
        return Ok(backend.clone());
    }

    let mut checked = Vec::new();

    for &backend in priority {
        // Skip if adapter is disabled in config
        if !adapter_enabled(backend) {
            debug!(backend = backend, "Skipping disabled adapter");
            continue;
        }

        checked.push(backend.to_string());

        if is_backend_available(backend) {
            debug!(backend = backend, "Backend detected and selected");
            // Cache the result (ignore if already set)
            let _ = DETECTED_BACKEND.set(Some(backend.to_string()));
            return Ok(backend.to_string());
        }
    }

    debug!(checked = ?checked, "No backends available");
    // Cache the failure too
    let _ = DETECTED_BACKEND.set(None);

    Err(NoBackendError { checked })
}

/// Detects a backend using default priority and all adapters enabled.
pub fn detect_backend_default() -> Result<String, NoBackendError> {
    detect_backend(&default_priority(), |_| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_backend_available_echo() {
        // 'echo' command should always be available
        let result = Command::new("echo").arg("--version").output();
        // Just verify the command runs without panic
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_backend_available_nonexistent() {
        // Nonexistent command should return false
        assert!(!is_backend_available(
            "definitely_not_a_real_command_xyz123"
        ));
    }

    #[test]
    fn test_detect_backend_with_disabled_adapters() {
        // All adapters disabled should fail
        let result = detect_backend(&["claude", "gemini"], |_| false);
        // Should return error since all are disabled (empty checked list)
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.checked.is_empty());
        }
    }

    #[test]
    fn test_no_backend_error_display() {
        let err = NoBackendError {
            checked: vec!["claude".to_string(), "gemini".to_string()],
        };
        let msg = format!("{}", err);
        assert!(msg.contains("No supported AI backend found"));
        assert!(msg.contains("claude, gemini"));
        assert!(msg.contains("ralph doctor"));
        assert!(msg.contains("docs/reference/troubleshooting.md#agent-not-found"));
        assert!(msg.contains("Pi CLI"));
    }

    #[test]
    fn no_backend_error_install_urls_come_from_catalog() {
        // The no-backend install list must be derived from the core catalog so it
        // never drifts. The Pi URL in particular was stale
        // (github.com/anthropics/pi-coding-agent); the catalog corrected it to the
        // npm registry.
        let err = NoBackendError {
            checked: vec!["claude".to_string()],
        };
        let msg = format!("{}", err);
        assert!(
            msg.contains("npmjs.com"),
            "Pi install URL should be the npm registry: {msg}"
        );

        assert!(
            !msg.contains("anthropics/pi-coding-agent"),
            "stale Pi URL must be gone: {msg}"
        );
        // Every catalog backend is listed by display name AND install URL, mirroring
        // the Display's command-dedup: kiro and kiro-acp share kiro-cli, so only the
        // first (kiro) is rendered — kiro-acp's "Kiro ACP" name is intentionally absent.
        let mut seen_commands: Vec<&str> = Vec::new();
        for metadata in ralph_core::backend::iter() {
            if seen_commands.contains(&metadata.command) {
                continue;
            }
            seen_commands.push(metadata.command);
            assert!(
                msg.contains(metadata.display_name),
                "missing display name for {}: {msg}",
                metadata.id
            );
            assert!(
                msg.contains(metadata.install_url),
                "missing install URL for {}: {msg}",
                metadata.id
            );
        }
        // Dedup-by-command: kiro and kiro-acp share the kiro-cli binary / kiro.dev
        // URL, so the install list must render that line exactly once.
        let kiro_dev_count = msg.matches("https://kiro.dev").count();
        assert_eq!(
            kiro_dev_count, 1,
            "kiro.dev should appear exactly once (got {kiro_dev_count}): {msg}"
        );
    }

    #[test]
    fn default_priority_preserves_pi_roo_omp_tail_placement() {
        // Structural invariant of the catalog-derived detection order: the tail
        // is stable at pi, roo, omp (omp appended last after roo). A reorder
        // that broke tail placement would fail here.
        let priority = default_priority();
        assert!(priority.contains(&"pi"), "priority should include 'pi'");
        assert!(priority.contains(&"roo"), "priority should include 'roo'");
        assert!(priority.contains(&"omp"), "priority should include 'omp'");
        let len = priority.len();
        assert_eq!(
            &priority[len - 3..],
            &["pi", "roo", "omp"],
            "tail order must be pi, roo, omp"
        );
    }

    #[test]
    fn test_detect_backend_default_priority_order() {
        // Test that default priority order is respected when no backends are available
        // Use non-existent backends to ensure they all fail
        let fake_priority = &[
            "fake_claude",
            "fake_kiro",
            "fake_gemini",
            "fake_codex",
            "fake_amp",
        ];
        let result = detect_backend(fake_priority, |_| true);

        // Should fail since no backends are actually available, but check the order
        assert!(result.is_err());
        if let Err(e) = result {
            // Should check backends in the specified priority order
            assert_eq!(
                e.checked,
                vec![
                    "fake_claude",
                    "fake_kiro",
                    "fake_gemini",
                    "fake_codex",
                    "fake_amp"
                ]
            );
        }
    }

    #[test]
    fn test_detect_backend_custom_priority_order() {
        // Test that custom priority order is honored
        let custom_priority = &["fake_gemini", "fake_claude", "fake_amp"];
        let result = detect_backend(custom_priority, |_| true);

        // Should fail since no backends are actually available, but check the order
        assert!(result.is_err());
        if let Err(e) = result {
            // Should check backends in custom priority order
            assert_eq!(e.checked, vec!["fake_gemini", "fake_claude", "fake_amp"]);
        }
    }

    #[test]
    fn test_detect_backend_skips_disabled_adapters() {
        // Test that disabled adapters are skipped even if in priority list
        let priority = &["fake_claude", "fake_gemini", "fake_kiro", "fake_codex"];
        let result = detect_backend(priority, |backend| {
            // Only enable fake_gemini and fake_codex
            matches!(backend, "fake_gemini" | "fake_codex")
        });

        // Should fail since no backends are actually available, but check only enabled ones were checked
        assert!(result.is_err());
        if let Err(e) = result {
            // Should only check enabled backends (fake_gemini, fake_codex), skipping disabled ones (fake_claude, fake_kiro)
            assert_eq!(e.checked, vec!["fake_gemini", "fake_codex"]);
        }
    }

    #[test]
    fn test_detect_backend_respects_priority_with_mixed_enabled() {
        // Test priority ordering with some adapters disabled
        let priority = &[
            "fake_claude",
            "fake_kiro",
            "fake_gemini",
            "fake_codex",
            "fake_amp",
        ];
        let result = detect_backend(priority, |backend| {
            // Disable fake_kiro and fake_codex
            !matches!(backend, "fake_kiro" | "fake_codex")
        });

        // Should fail since no backends are actually available, but check the filtered order
        assert!(result.is_err());
        if let Err(e) = result {
            // Should check in priority order but skip disabled ones
            assert_eq!(e.checked, vec!["fake_claude", "fake_gemini", "fake_amp"]);
        }
    }

    #[test]
    fn test_detect_backend_empty_priority_list() {
        // Test behavior with empty priority list
        let result = detect_backend(&[], |_| true);

        // Should fail with empty checked list
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.checked.is_empty());
        }
    }

    #[test]
    fn test_detect_backend_all_disabled() {
        // Test that all disabled adapters results in empty checked list
        let priority = &["claude", "gemini", "kiro"];
        let result = detect_backend(priority, |_| false);

        // Should fail with empty checked list since all are disabled
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.checked.is_empty());
        }
    }

    #[test]
    fn test_detect_backend_finds_first_available() {
        // Test that the first available backend in priority order is selected
        // Mix available and unavailable backends to test priority
        let priority = &[
            "fake_nonexistent1",
            "fake_nonexistent2",
            "echo",
            "fake_nonexistent3",
        ];
        let result = detect_backend(priority, |_| true);

        // Should succeed and return "echo" (first available in the priority list)
        assert!(result.is_ok());
        if let Ok(backend) = result {
            assert_eq!(backend, "echo");
        }
    }

    #[test]
    fn test_detect_backend_skips_to_next_available() {
        // Test that detection continues through priority list until it finds an available backend
        let priority = &["fake_nonexistent1", "fake_nonexistent2", "echo"];
        let result = detect_backend(priority, |backend| {
            // Disable the first fake backend, enable the rest
            backend != "fake_nonexistent1"
        });

        // Should succeed and return "echo" (first enabled and available)
        assert!(result.is_ok());
        if let Ok(backend) = result {
            assert_eq!(backend, "echo");
        }
    }
}
