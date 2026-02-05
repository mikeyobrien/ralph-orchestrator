//! Mock mode support for cost-free E2E testing.
//!
//! This module provides a mock CLI adapter that replays recorded JSONL cassettes
//! instead of invoking real AI backends. This enables:
//! - Deterministic, reproducible tests
//! - Cost-free CI/CD pipeline execution
//! - Fast test execution (no API latency)
//!
//! # Architecture
//!
//! ```text
//! hats-e2e --mock
//!     │
//!     ├─ CassetteResolver: Finds cassette file for scenario+backend
//!     │
//!     └─ mock-cli subcommand: Replays cassette as fake CLI output
//!         │
//!         ├─ SessionPlayer: Reads JSONL and outputs terminal writes
//!         │
//!         └─ WhitelistExecutor: Runs approved local commands
//! ```
//!
//! # Cassette Naming Convention
//!
//! Cassettes are stored in `cassettes/e2e/` with the following naming:
//! - `<scenario-id>-<backend>.jsonl` (backend-specific)
//! - `<scenario-id>.jsonl` (fallback, backend-agnostic)
//!
//! # Example
//!
//! ```bash
//! # Run all E2E tests in mock mode
//! hats-e2e --mock
//!
//! # Run with accelerated replay (10x speed)
//! hats-e2e --mock --mock-speed 10.0
//!
//! # The mock-cli is invoked internally by hats as a custom backend
//! hats-e2e mock-cli --cassette cassettes/e2e/connect.jsonl
//! ```

use crate::Backend;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Default cassette directory relative to the E2E crate or workspace root.
pub const DEFAULT_CASSETTE_DIR: &str = "cassettes/e2e";

/// Errors that can occur during cassette resolution.
#[derive(Debug, Error)]
pub enum CassetteError {
    /// Cassette file not found for the given scenario and backend.
    #[error("cassette not found for scenario '{scenario}' backend '{backend}': tried {tried:?}")]
    NotFound {
        scenario: String,
        backend: String,
        tried: Vec<PathBuf>,
    },

    /// Cassette file exists but cannot be read.
    #[error("cassette file unreadable: {path}: {source}")]
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Cassette file contains invalid JSONL.
    #[error("cassette parse error in {path}: {message}")]
    ParseError { path: PathBuf, message: String },
}

/// Configuration for mock mode execution.
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Directory containing cassette files.
    pub cassette_dir: PathBuf,

    /// Replay speed multiplier (1.0 = real-time, 10.0 = 10x faster, 0.0 = instant).
    pub speed: f32,

    /// Commands allowed to be executed during mock replay.
    /// Format: comma-separated command prefixes (e.g., "hats task add,hats tools memory add").
    pub allow_commands: Option<String>,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            cassette_dir: PathBuf::from(DEFAULT_CASSETTE_DIR),
            speed: 0.0, // Instant by default for CI
            allow_commands: Some("hats task add,hats task close,hats tools memory add".into()),
        }
    }
}

impl MockConfig {
    /// Creates a new mock config with the given cassette directory.
    pub fn new(cassette_dir: impl Into<PathBuf>) -> Self {
        Self {
            cassette_dir: cassette_dir.into(),
            ..Default::default()
        }
    }

    /// Sets the replay speed.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.0);
        self
    }

    /// Sets the allowed commands.
    pub fn with_allow_commands(mut self, commands: impl Into<String>) -> Self {
        self.allow_commands = Some(commands.into());
        self
    }

    /// Disables command execution during replay.
    pub fn without_commands(mut self) -> Self {
        self.allow_commands = None;
        self
    }

    /// Resolves the cassette directory to an absolute path.
    ///
    /// If the cassette_dir is relative, resolves it relative to the workspace root.
    /// If no workspace root is found, returns the path as-is.
    pub fn resolve_cassette_dir(&self) -> PathBuf {
        if self.cassette_dir.is_absolute() {
            return self.cassette_dir.clone();
        }

        // Try to resolve relative to workspace root
        if let Some(root) = crate::executor::find_workspace_root() {
            root.join(&self.cassette_dir)
        } else {
            self.cassette_dir.clone()
        }
    }

    /// Sets an explicit workspace root for cassette directory resolution.
    ///
    /// This is primarily used for testing to override workspace detection.
    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        if self.cassette_dir.is_relative() {
            self.cassette_dir = root.join(&self.cassette_dir);
        }
        self
    }
}

/// Resolves cassette files for scenarios.
///
/// Cassettes are looked up in order:
/// 1. `<cassette_dir>/<scenario>-<backend>.jsonl` (backend-specific)
/// 2. `<cassette_dir>/<scenario>.jsonl` (fallback)
///
/// If neither exists, returns an error.
#[derive(Debug, Clone)]
pub struct CassetteResolver {
    /// Base directory for cassette files.
    cassette_dir: PathBuf,
}

impl CassetteResolver {
    /// Creates a new resolver with the given cassette directory.
    pub fn new(cassette_dir: impl Into<PathBuf>) -> Self {
        Self {
            cassette_dir: cassette_dir.into(),
        }
    }

    /// Resolves the cassette path for a scenario and backend.
    ///
    /// Returns the path to the cassette file, or an error if not found.
    ///
    /// # Resolution Order
    ///
    /// 1. `<scenario>-<backend>.jsonl` (e.g., `connect-claude.jsonl`)
    /// 2. `<scenario>.jsonl` (e.g., `connect.jsonl`)
    pub fn resolve(&self, scenario: &str, backend: Backend) -> Result<PathBuf, CassetteError> {
        let mut tried = Vec::new();

        // Try backend-specific cassette first
        let backend_specific =
            self.cassette_dir
                .join(format!("{}-{}.jsonl", scenario, backend.as_config_str()));
        tried.push(backend_specific.clone());

        if backend_specific.exists() {
            return Ok(backend_specific);
        }

        // Fall back to generic cassette
        let generic = self.cassette_dir.join(format!("{}.jsonl", scenario));
        tried.push(generic.clone());

        if generic.exists() {
            return Ok(generic);
        }

        Err(CassetteError::NotFound {
            scenario: scenario.to_string(),
            backend: backend.as_config_str().to_string(),
            tried,
        })
    }

    /// Returns all candidate paths that would be tried for a scenario.
    ///
    /// Useful for debugging and dry-run output.
    pub fn candidates(&self, scenario: &str, backend: Backend) -> Vec<PathBuf> {
        vec![
            self.cassette_dir
                .join(format!("{}-{}.jsonl", scenario, backend.as_config_str())),
            self.cassette_dir.join(format!("{}.jsonl", scenario)),
        ]
    }

    /// Returns the cassette directory.
    pub fn cassette_dir(&self) -> &Path {
        &self.cassette_dir
    }
}

/// Builds the command-line arguments for invoking the mock CLI.
///
/// This is used when configuring `hats.yml` to use `mock-cli` as a custom backend.
pub fn build_mock_cli_args(cassette_path: &Path, config: &MockConfig) -> Vec<String> {
    let mut args = vec![
        "mock-cli".to_string(),
        "--cassette".to_string(),
        cassette_path.to_string_lossy().to_string(),
    ];

    if config.speed > 0.0 {
        args.push("--speed".to_string());
        args.push(config.speed.to_string());
    }

    if let Some(allow) = &config.allow_commands {
        args.push("--allow".to_string());
        args.push(allow.clone());
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_cassette(dir: &Path, name: &str) {
        let cassette_path = dir.join(name);
        fs::write(
            &cassette_path,
            r#"{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"UE9ORw==","stdout":true,"offset_ms":0}}"#,
        )
        .unwrap();
    }

    #[test]
    fn test_resolver_finds_backend_specific() {
        let temp = TempDir::new().unwrap();
        let cassette_dir = temp.path().join("cassettes/e2e");
        fs::create_dir_all(&cassette_dir).unwrap();

        create_test_cassette(&cassette_dir, "connect-claude.jsonl");
        create_test_cassette(&cassette_dir, "connect.jsonl");

        let resolver = CassetteResolver::new(&cassette_dir);
        let path = resolver.resolve("connect", Backend::Claude).unwrap();

        assert!(path.ends_with("connect-claude.jsonl"));
    }

    #[test]
    fn test_resolver_falls_back_to_generic() {
        let temp = TempDir::new().unwrap();
        let cassette_dir = temp.path().join("cassettes/e2e");
        fs::create_dir_all(&cassette_dir).unwrap();

        create_test_cassette(&cassette_dir, "connect.jsonl");

        let resolver = CassetteResolver::new(&cassette_dir);
        let path = resolver.resolve("connect", Backend::Kiro).unwrap();

        assert!(path.ends_with("connect.jsonl"));
    }

    #[test]
    fn test_resolver_returns_error_when_missing() {
        let temp = TempDir::new().unwrap();
        let cassette_dir = temp.path().join("cassettes/e2e");
        fs::create_dir_all(&cassette_dir).unwrap();

        let resolver = CassetteResolver::new(&cassette_dir);
        let result = resolver.resolve("nonexistent", Backend::Claude);

        assert!(matches!(result, Err(CassetteError::NotFound { .. })));

        if let Err(CassetteError::NotFound { tried, .. }) = result {
            assert_eq!(tried.len(), 2);
        }
    }

    #[test]
    fn test_resolver_candidates() {
        let resolver = CassetteResolver::new("/cassettes/e2e");
        let candidates = resolver.candidates("connect", Backend::Claude);

        assert_eq!(candidates.len(), 2);
        assert!(
            candidates[0]
                .to_string_lossy()
                .contains("connect-claude.jsonl")
        );
        assert!(candidates[1].to_string_lossy().contains("connect.jsonl"));
    }

    #[test]
    fn test_mock_config_defaults() {
        let config = MockConfig::default();

        assert_eq!(config.cassette_dir, PathBuf::from(DEFAULT_CASSETTE_DIR));
        assert!((config.speed - 0.0).abs() < f32::EPSILON);
        assert!(config.allow_commands.is_some());
    }

    #[test]
    fn test_mock_config_builder() {
        let config = MockConfig::new("/custom/cassettes")
            .with_speed(10.0)
            .with_allow_commands("hats task add");

        assert_eq!(config.cassette_dir, PathBuf::from("/custom/cassettes"));
        assert!((config.speed - 10.0).abs() < f32::EPSILON);
        assert_eq!(config.allow_commands, Some("hats task add".into()));
    }

    #[test]
    fn test_mock_config_negative_speed_clamped() {
        let config = MockConfig::default().with_speed(-5.0);
        assert!((config.speed - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_build_mock_cli_args() {
        let config = MockConfig::default().with_speed(10.0);
        let cassette = PathBuf::from("/path/to/cassette.jsonl");

        let args = build_mock_cli_args(&cassette, &config);

        assert!(args.contains(&"mock-cli".to_string()));
        assert!(args.contains(&"--cassette".to_string()));
        assert!(args.contains(&"/path/to/cassette.jsonl".to_string()));
        assert!(args.contains(&"--speed".to_string()));
        assert!(args.contains(&"10".to_string()));
        assert!(args.contains(&"--allow".to_string()));
    }

    #[test]
    fn test_build_mock_cli_args_instant() {
        let config = MockConfig::default(); // speed = 0.0
        let cassette = PathBuf::from("/path/to/cassette.jsonl");

        let args = build_mock_cli_args(&cassette, &config);

        // Should not include --speed when speed is 0 (instant)
        assert!(!args.contains(&"--speed".to_string()));
    }

    #[test]
    fn test_build_mock_cli_args_no_commands() {
        let config = MockConfig::default().without_commands();
        let cassette = PathBuf::from("/path/to/cassette.jsonl");

        let args = build_mock_cli_args(&cassette, &config);

        assert!(!args.contains(&"--allow".to_string()));
    }

    /// Test that MockConfig::resolve_cassette_dir returns an absolute path
    /// even when initialized with a relative path.
    ///
    /// This tests the fix for the bug where cassette resolution fails when
    /// running from a directory other than the workspace root.
    #[test]
    fn test_mock_config_resolve_cassette_dir_returns_absolute_path() {
        // Create a mock config with the default relative path
        let config = MockConfig::default();

        // The resolved cassette directory should be absolute
        let resolved = config.resolve_cassette_dir();
        assert!(
            resolved.is_absolute(),
            "resolve_cassette_dir() should return an absolute path, got: {}",
            resolved.display()
        );
    }

    /// Test that CassetteResolver works correctly with workspace-relative paths
    /// when the cassette directory is resolved relative to workspace root.
    #[test]
    fn test_resolver_with_workspace_relative_path() {
        let temp = TempDir::new().unwrap();

        // Simulate a workspace structure
        let workspace_root = temp.path();
        let cassette_dir = workspace_root.join("cassettes/e2e");
        fs::create_dir_all(&cassette_dir).unwrap();
        create_test_cassette(&cassette_dir, "connect.jsonl");

        // Create a Cargo.toml with [workspace] to make this a workspace root
        fs::write(
            workspace_root.join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        )
        .unwrap();

        // Create a MockConfig and resolve the cassette directory
        let config = MockConfig::default().with_workspace_root(workspace_root);
        let resolved_dir = config.resolve_cassette_dir();

        // The resolved directory should be the absolute path to cassettes/e2e
        assert_eq!(resolved_dir, cassette_dir);

        // Now verify the resolver can find cassettes using this resolved path
        let resolver = CassetteResolver::new(&resolved_dir);
        let result = resolver.resolve("connect", Backend::Claude);
        assert!(
            result.is_ok(),
            "Should find cassette when using resolved absolute path"
        );
    }
}
