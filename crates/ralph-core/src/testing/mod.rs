//! Testing utilities for deterministic E2E tests.

pub mod mock_backend;
#[cfg(feature = "recording")]
pub mod replay_backend;
pub mod scenario;
#[cfg(feature = "recording")]
pub mod smoke_runner;

pub use mock_backend::{ExecutionRecord, MockBackend};
#[cfg(feature = "recording")]
pub use replay_backend::{ReplayBackend, ReplayTimingMode};
pub use scenario::{ExecutionTrace, Scenario, ScenarioRunner};
#[cfg(feature = "recording")]
pub use smoke_runner::{
    SmokeRunner, SmokeTestConfig, SmokeTestError, SmokeTestResult, TerminationReason, list_fixtures,
};

use std::path::Path;
use std::process::Command;

/// Initialise a throwaway git repo with one commit for use in tests.
///
/// Optionally writes a `.gitignore` with the given entries before the initial
/// commit (useful when the test creates `.ralph/` artifacts that would dirty
/// the working tree).
pub fn init_test_repo(dir: &Path, gitignore: &[&str]) {
    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(dir)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@test.local"])
        .current_dir(dir)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .unwrap();

    std::fs::write(dir.join("README.md"), "# Test").unwrap();

    let mut add_args = vec!["add", "README.md"];
    if !gitignore.is_empty() {
        let content = gitignore.join("\n") + "\n";
        std::fs::write(dir.join(".gitignore"), content).unwrap();
        add_args.push(".gitignore");
    }

    Command::new("git")
        .args(&add_args)
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()
        .unwrap();
}
