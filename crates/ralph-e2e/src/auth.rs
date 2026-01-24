//! Authentication and backend availability checking.
//!
//! This module provides functionality to detect which AI backends are available
//! on the system and whether they are properly authenticated.
//!
//! # Example
//!
//! ```no_run
//! use ralph_e2e::auth::AuthChecker;
//!
//! #[tokio::main]
//! async fn main() {
//!     let checker = AuthChecker::new();
//!     let backends = checker.check_all().await;
//!
//!     for info in &backends {
//!         println!("{}: {}", info.backend, if info.is_authenticated { "✅" } else { "❌" });
//!     }
//! }
//! ```

use crate::backend::Backend;
use std::process::Stdio;
use tokio::process::Command;

/// Information about a backend's availability and authentication status.
#[derive(Debug, Clone)]
pub struct BackendInfo {
    /// The backend this info is about.
    pub backend: Backend,
    /// Whether the CLI is installed and available on PATH.
    pub is_available: bool,
    /// Whether the backend is authenticated (can make API calls).
    pub is_authenticated: bool,
    /// The CLI version string, if available.
    pub version: Option<String>,
    /// Error message if checking failed.
    pub error: Option<String>,
}

impl BackendInfo {
    /// Creates a new BackendInfo indicating the backend is not available.
    pub fn unavailable(backend: Backend, error: Option<String>) -> Self {
        Self {
            backend,
            is_available: false,
            is_authenticated: false,
            version: None,
            error,
        }
    }

    /// Creates a new BackendInfo indicating the backend is available but not authenticated.
    pub fn available_not_authenticated(backend: Backend, version: Option<String>) -> Self {
        Self {
            backend,
            is_available: true,
            is_authenticated: false,
            version,
            error: None,
        }
    }

    /// Creates a new BackendInfo indicating the backend is available and authenticated.
    pub fn authenticated(backend: Backend, version: Option<String>) -> Self {
        Self {
            backend,
            is_available: true,
            is_authenticated: true,
            version,
            error: None,
        }
    }

    /// Returns a human-readable status string.
    pub fn status_string(&self) -> String {
        if !self.is_available {
            "Not installed".to_string()
        } else if !self.is_authenticated {
            match &self.version {
                Some(v) => format!("{v} - Not authenticated"),
                None => "Not authenticated".to_string(),
            }
        } else {
            match &self.version {
                Some(v) => format!("{v} - Authenticated"),
                None => "Authenticated".to_string(),
            }
        }
    }
}

/// Checks backend availability and authentication status.
#[derive(Debug, Default)]
pub struct AuthChecker {
    /// Optional timeout for auth checks (in seconds). Defaults to 10.
    pub timeout_secs: u64,
}

impl AuthChecker {
    /// Creates a new AuthChecker with default settings.
    pub fn new() -> Self {
        Self { timeout_secs: 10 }
    }

    /// Creates a new AuthChecker with a custom timeout.
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    /// Checks all backends and returns their status.
    pub async fn check_all(&self) -> Vec<BackendInfo> {
        let mut results = Vec::with_capacity(Backend::all().len());
        for backend in Backend::all() {
            results.push(self.check(*backend).await);
        }
        results
    }

    /// Checks a single backend's availability and authentication.
    pub async fn check(&self, backend: Backend) -> BackendInfo {
        // First check if the CLI is available
        if !Self::is_available(backend).await {
            return BackendInfo::unavailable(backend, Some("CLI not found on PATH".to_string()));
        }

        // Get the version
        let version = Self::get_version(backend).await;

        // Check authentication
        if Self::is_authenticated(backend).await {
            BackendInfo::authenticated(backend, version)
        } else {
            BackendInfo::available_not_authenticated(backend, version)
        }
    }

    /// Checks if the CLI command is available on PATH.
    pub async fn is_available(backend: Backend) -> bool {
        // Use `which` on Unix systems to find the command
        let result = Command::new("which")
            .arg(backend.command())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        matches!(result, Ok(status) if status.success())
    }

    /// Gets the CLI version string.
    pub async fn get_version(backend: Backend) -> Option<String> {
        let output = Command::new(backend.command())
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse version from output - usually first line
            let version_line = stdout.lines().next()?;
            // Extract version, handling formats like "claude 1.0.5" or "claude-cli 1.0.5"
            Some(version_line.trim().to_string())
        } else {
            None
        }
    }

    /// Checks if the backend is authenticated by running a minimal command.
    ///
    /// Different backends have different ways to check authentication:
    /// - Claude: `claude --version` with API key set returns successfully
    /// - Kiro: `kiro-cli --version` similarly
    /// - OpenCode: `opencode --version`
    /// - Cursor: `agent --version` similarly
    ///
    /// For now, we use a simple heuristic: if the CLI is available and
    /// can report its version, we assume it's configured. A more robust
    /// check would involve a minimal API call, but that costs money.
    pub async fn is_authenticated(backend: Backend) -> bool {
        // For Claude, we can check if ANTHROPIC_API_KEY is set or if
        // the CLI has been configured with `claude config`
        //
        // A simple heuristic: run `<cmd> --help` and see if it mentions
        // authentication errors. This is backend-specific.
        //
        // For MVP, we'll assume if the version command works, auth is configured.
        // This can be enhanced later with backend-specific checks.
        match backend {
            Backend::Claude => Self::check_claude_auth().await,
            Backend::Kiro => Self::check_kiro_auth().await,
            Backend::OpenCode => Self::check_opencode_auth().await,
            Backend::Cursor => Self::check_cursor_auth().await,
        }
    }

    /// Claude-specific authentication check.
    async fn check_claude_auth() -> bool {
        // Claude CLI stores auth in ~/.config/claude/ or uses ANTHROPIC_API_KEY
        // We can try running `claude doctor` or check if config exists
        // For now, check if version command succeeds (indicates basic setup)
        let output = Command::new(Backend::Claude.command())
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        matches!(output, Ok(o) if o.status.success())
    }

    /// Kiro-specific authentication check.
    async fn check_kiro_auth() -> bool {
        // Similar to Claude
        let output = Command::new(Backend::Kiro.command())
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        matches!(output, Ok(o) if o.status.success())
    }

    /// OpenCode-specific authentication check.
    async fn check_opencode_auth() -> bool {
        // Similar to Claude
        let output = Command::new(Backend::OpenCode.command())
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        matches!(output, Ok(o) if o.status.success())
    }

    /// Cursor-specific authentication check.
    async fn check_cursor_auth() -> bool {
        // Cursor CLI uses `agent` command
        // Check if version command succeeds (indicates basic setup)
        let output = Command::new(Backend::Cursor.command())
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        matches!(output, Ok(o) if o.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_info_unavailable() {
        let info = BackendInfo::unavailable(Backend::Claude, Some("not found".to_string()));
        assert!(!info.is_available);
        assert!(!info.is_authenticated);
        assert!(info.version.is_none());
        assert_eq!(info.error, Some("not found".to_string()));
    }

    #[test]
    fn test_backend_info_available_not_authenticated() {
        let info =
            BackendInfo::available_not_authenticated(Backend::Kiro, Some("1.0.0".to_string()));
        assert!(info.is_available);
        assert!(!info.is_authenticated);
        assert_eq!(info.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_backend_info_authenticated() {
        let info = BackendInfo::authenticated(Backend::Claude, Some("claude 1.0.5".to_string()));
        assert!(info.is_available);
        assert!(info.is_authenticated);
        assert_eq!(info.version, Some("claude 1.0.5".to_string()));
    }

    #[test]
    fn test_backend_info_status_string_not_installed() {
        let info = BackendInfo::unavailable(Backend::OpenCode, None);
        assert_eq!(info.status_string(), "Not installed");
    }

    #[test]
    fn test_backend_info_status_string_not_authenticated() {
        let info =
            BackendInfo::available_not_authenticated(Backend::Kiro, Some("kiro 0.3.2".to_string()));
        assert_eq!(info.status_string(), "kiro 0.3.2 - Not authenticated");
    }

    #[test]
    fn test_backend_info_status_string_authenticated() {
        let info = BackendInfo::authenticated(Backend::Claude, Some("claude 1.0.5".to_string()));
        assert_eq!(info.status_string(), "claude 1.0.5 - Authenticated");
    }

    #[test]
    fn test_auth_checker_new() {
        let checker = AuthChecker::new();
        assert_eq!(checker.timeout_secs, 10);
    }

    #[test]
    fn test_auth_checker_with_timeout() {
        let checker = AuthChecker::with_timeout(30);
        assert_eq!(checker.timeout_secs, 30);
    }

    #[tokio::test]
    async fn test_is_available_for_nonexistent_command() {
        // Create a fake backend for testing
        // Since we can't easily create a fake Backend, we'll test with a real one
        // that might not be installed
        let is_available = AuthChecker::is_available(Backend::OpenCode).await;
        // This test will pass regardless of whether opencode is installed
        // The important thing is it doesn't panic
        let _ = is_available;
    }

    #[tokio::test]
    async fn test_check_all_returns_all_backends() {
        let checker = AuthChecker::new();
        let results = checker.check_all().await;
        assert_eq!(results.len(), 4);
        assert!(results.iter().any(|r| r.backend == Backend::Claude));
        assert!(results.iter().any(|r| r.backend == Backend::Kiro));
        assert!(results.iter().any(|r| r.backend == Backend::OpenCode));
        assert!(results.iter().any(|r| r.backend == Backend::Cursor));
    }

    #[tokio::test]
    async fn test_check_backend_detection() {
        let checker = AuthChecker::new();
        // Check Claude (most likely to be installed in development)
        let info = checker.check(Backend::Claude).await;
        // If available, should have a version
        if info.is_available {
            assert!(
                info.version.is_some(),
                "Available backend should have version"
            );
        } else {
            // If not available, should have error message
            assert!(
                info.error.is_some(),
                "Unavailable backend should have error"
            );
        }
    }
}
