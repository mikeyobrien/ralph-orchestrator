//! Data models for E2E test harness.
//!
//! This module defines the core data structures used throughout the test harness,
//! including test results, assertions, and report formats.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Report output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportFormat {
    /// Markdown format (agent-readable)
    #[default]
    Markdown,
    /// JSON format (machine-readable)
    Json,
    /// Both markdown and JSON
    Both,
}

/// Result of a single test scenario execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Unique identifier for the scenario
    pub scenario_id: String,
    /// Human-readable description
    pub scenario_description: String,
    /// Backend that was tested
    pub backend: String,
    /// Test tier (e.g., "Tier 1: Connectivity")
    pub tier: String,
    /// Whether the test passed
    pub passed: bool,
    /// Individual assertions checked
    pub assertions: Vec<Assertion>,
    /// How long the test took
    #[serde(with = "duration_serde")]
    pub duration: Duration,
}

/// A single assertion within a test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    /// Name of the assertion
    pub name: String,
    /// Whether the assertion passed
    pub passed: bool,
    /// Expected value/condition
    pub expected: String,
    /// Actual value/condition observed
    pub actual: String,
}

/// Serde helper for Duration serialization.
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs_f64().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}
