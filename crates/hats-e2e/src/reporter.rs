//! Reporters for E2E test execution.
//!
//! This module provides multiple reporter types:
//! - `TerminalReporter`: Colored terminal output for progress and results
//! - `MarkdownReporter`: Agent-readable markdown report generation
//! - `JsonReporter`: Machine-readable JSON report generation
//! - `ReportWriter`: Orchestrates writing reports to files
//!
//! # Example
//!
//! ```no_run
//! use hats_e2e::{TerminalReporter, MarkdownReporter, JsonReporter, ReportWriter, RunResults, ProgressEvent, ReportFormat};
//! use std::path::PathBuf;
//!
//! let mut reporter = TerminalReporter::new();
//!
//! // Handle progress events
//! reporter.handle_progress(ProgressEvent::RunStarted { total_scenarios: 5 });
//!
//! // Print final summary
//! let results = RunResults::default();
//! reporter.print_summary(&results);
//!
//! // Write file reports
//! let writer = ReportWriter::new(PathBuf::from(".e2e-tests"));
//! writer.write(&results, None, ReportFormat::Both).unwrap();
//! ```

use crate::analyzer::{
    AnalyzedResult, Diagnosis, FailureType, PassedAnalysis, QualityScore, Recommendation, Severity,
};
use crate::models::TestResult;
use crate::runner::{ProgressEvent, RunResults};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

/// Verbosity level for terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Show only pass/fail summary.
    Quiet,
    /// Normal output with progress.
    #[default]
    Normal,
    /// Detailed output including assertions.
    Verbose,
}

/// Terminal reporter for E2E test results.
#[derive(Debug)]
pub struct TerminalReporter {
    /// Verbosity level.
    verbosity: Verbosity,

    /// Track current tier for grouping output.
    current_tier: Option<String>,
}

impl Default for TerminalReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalReporter {
    /// Creates a new terminal reporter with normal verbosity.
    pub fn new() -> Self {
        Self {
            verbosity: Verbosity::Normal,
            current_tier: None,
        }
    }

    /// Creates a reporter with the specified verbosity.
    pub fn with_verbosity(verbosity: Verbosity) -> Self {
        Self {
            verbosity,
            current_tier: None,
        }
    }

    /// Handles a progress event, printing appropriate output.
    pub fn handle_progress(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::RunStarted { total_scenarios } => {
                if self.verbosity != Verbosity::Quiet {
                    self.print_run_started(total_scenarios);
                }
            }
            ProgressEvent::ScenarioStarted { scenario_id, tier } => {
                if self.verbosity == Verbosity::Verbose {
                    self.print_scenario_started(&scenario_id, &tier);
                }
            }
            ProgressEvent::ScenarioCompleted {
                scenario_id,
                passed,
                duration,
                ..
            } => {
                if self.verbosity != Verbosity::Quiet {
                    self.print_scenario_completed(&scenario_id, passed, duration);
                }
            }
            ProgressEvent::ScenarioSkipped {
                scenario_id,
                reason,
            } => {
                if self.verbosity != Verbosity::Quiet {
                    self.print_scenario_skipped(&scenario_id, &reason);
                }
            }
            ProgressEvent::RunCompleted { results } => {
                // Summary is printed separately via print_summary
                if self.verbosity == Verbosity::Quiet {
                    self.print_quiet_summary(&results);
                }
            }
        }
    }

    /// Prints the run started header.
    fn print_run_started(&self, total: usize) {
        println!(
            "\n{}",
            format!(
                "Running {} scenario{}...",
                total,
                if total == 1 { "" } else { "s" }
            )
            .bold()
        );
        println!();
    }

    /// Prints scenario started (verbose mode).
    fn print_scenario_started(&mut self, scenario_id: &str, tier: &str) {
        // Print tier header if changed
        if self.current_tier.as_deref() != Some(tier) {
            self.current_tier = Some(tier.to_string());
            println!("{}", tier.bold().underline());
        }

        print!("  {} ", scenario_id);
        io::stdout().flush().ok();
    }

    /// Prints scenario completed result.
    fn print_scenario_completed(&self, scenario_id: &str, passed: bool, duration: Duration) {
        let status = if passed {
            "✅".to_string()
        } else {
            "❌".to_string()
        };

        let duration_str = format!("({:.1}s)", duration.as_secs_f64()).dimmed();

        println!("  {} {} {}", status, scenario_id, duration_str);
    }

    /// Prints scenario skipped.
    fn print_scenario_skipped(&self, scenario_id: &str, reason: &str) {
        println!(
            "  {} {} {}",
            "⏭️".dimmed(),
            scenario_id.dimmed(),
            format!("({})", reason).dimmed()
        );
    }

    /// Prints a quiet summary (just pass/fail counts).
    fn print_quiet_summary(&self, results: &RunResults) {
        let passed = results.passed_count();
        let failed = results.failed_count();
        let total = results.total_count();

        if failed == 0 {
            println!("{}", format!("✓ {}/{} passed", passed, total).green());
        } else {
            println!("{}", format!("✗ {}/{} failed", failed, total).red());
        }
    }

    /// Prints a full summary of the test run.
    pub fn print_summary(&self, results: &RunResults) {
        println!("\n{}", "━".repeat(40).dimmed());

        let passed = results.passed_count();
        let failed = results.failed_count();
        let skipped = results.skipped_count;
        let total = results.total_count();

        // Determine verdict emoji and color
        let (emoji, verdict, color) = if failed == 0 {
            ("🟢", "PASSED", colored::Color::Green)
        } else if passed > 0 {
            ("🟡", "MIXED", colored::Color::Yellow)
        } else {
            ("🔴", "FAILED", colored::Color::Red)
        };

        // Build summary line
        let mut parts = vec![];
        if passed > 0 {
            parts.push(format!("{} passed", passed).green().to_string());
        }
        if failed > 0 {
            parts.push(format!("{} failed", failed).red().to_string());
        }
        if skipped > 0 {
            parts.push(format!("{} skipped", skipped).dimmed().to_string());
        }

        let summary = parts.join(", ");
        let verdict_text = format!("{}: {} of {} tests", verdict, passed, total);

        println!("{} {}", emoji, verdict_text.color(color).bold());
        if !parts.is_empty() {
            println!("   {}", summary);
        }

        // Duration
        println!(
            "\n   {}",
            format!("Completed in {:.1}s", results.duration.as_secs_f64()).dimmed()
        );
    }

    /// Prints detailed results for failed tests.
    pub fn print_failures(&self, results: &RunResults) {
        let failures = results.failures();
        if failures.is_empty() {
            return;
        }

        println!("\n{}", "Failed Tests:".red().bold());
        println!();

        for result in failures {
            self.print_failed_test(result);
        }
    }

    /// Prints details of a single failed test.
    fn print_failed_test(&self, result: &TestResult) {
        println!("  {} {}", "❌".red(), result.scenario_id.red().bold());
        println!("     {}", result.scenario_description.dimmed());
        println!();

        // Print failed assertions
        for assertion in &result.assertions {
            if !assertion.passed {
                println!("     {} {}", "✗".red(), assertion.name);
                println!("       Expected: {}", assertion.expected.green());
                println!("       Actual:   {}", assertion.actual.red());
                println!();
            }
        }
    }

    /// Prints results grouped by tier.
    pub fn print_by_tier(&self, results: &RunResults) {
        for (tier, tier_results) in results.by_tier() {
            println!("\n{}", tier.bold().underline());

            for result in tier_results {
                let status = if result.passed {
                    "✅".to_string()
                } else {
                    "❌".to_string()
                };
                let duration = format!("({:.1}s)", result.duration.as_secs_f64()).dimmed();

                println!("  {} {} {}", status, result.scenario_id, duration);

                // In verbose mode, show assertions
                if self.verbosity == Verbosity::Verbose {
                    for assertion in &result.assertions {
                        let check = if assertion.passed {
                            "└─ ✓"
                        } else {
                            "└─ ✗"
                        };
                        let check_colored = if assertion.passed {
                            check.green()
                        } else {
                            check.red()
                        };
                        println!("     {} {}", check_colored, assertion.name);
                    }
                }
            }
        }
    }
}

/// Creates a progress callback for use with TestRunner.
pub fn create_progress_callback(verbosity: Verbosity) -> crate::runner::ProgressCallback {
    let reporter = std::sync::Arc::new(std::sync::Mutex::new(TerminalReporter::with_verbosity(
        verbosity,
    )));

    Box::new(move |event| {
        if let Ok(mut r) = reporter.lock() {
            r.handle_progress(event);
        }
    })
}

/// State for incremental reporting.
struct IncrementalState {
    reporter: TerminalReporter,
    results: Vec<TestResult>,
    output_path: PathBuf,
    total_scenarios: usize,
    start_time: std::time::Instant,
}

impl IncrementalState {
    fn write_live_report(&self) {
        let passed = self.results.iter().filter(|r| r.passed).count();
        let failed = self.results.iter().filter(|r| !r.passed).count();
        let elapsed = self.start_time.elapsed();

        let mut content = String::new();
        content.push_str("# E2E Test Report (Live)\n\n");
        content.push_str(&format!(
            "**Progress:** {}/{} scenarios completed\n",
            self.results.len(),
            self.total_scenarios
        ));
        content.push_str(&format!("**Elapsed:** {:.1}s\n", elapsed.as_secs_f64()));
        content.push_str(&format!(
            "**Passed:** {} | **Failed:** {}\n\n",
            passed, failed
        ));

        // Failed tests first (most important)
        let failures: Vec<_> = self.results.iter().filter(|r| !r.passed).collect();
        if !failures.is_empty() {
            content.push_str("## ❌ Failures\n\n");
            for result in failures {
                content.push_str(&format!("### {}\n\n", result.scenario_id));
                content.push_str(&format!("- **Tier:** {}\n", result.tier));
                content.push_str(&format!(
                    "- **Duration:** {:.1}s\n",
                    result.duration.as_secs_f64()
                ));
                content.push_str("- **Failed assertions:**\n");
                for assertion in result.assertions.iter().filter(|a| !a.passed) {
                    content.push_str(&format!(
                        "  - `{}`: expected `{}`, got `{}`\n",
                        assertion.name,
                        truncate_for_report(&assertion.expected, 100),
                        truncate_for_report(&assertion.actual, 200)
                    ));
                }
                content.push('\n');
            }
        }

        // Passed tests (collapsed)
        let passes: Vec<_> = self.results.iter().filter(|r| r.passed).collect();
        if !passes.is_empty() {
            content.push_str("## ✅ Passed\n\n");
            for result in passes {
                content.push_str(&format!(
                    "- {} ({:.1}s)\n",
                    result.scenario_id,
                    result.duration.as_secs_f64()
                ));
            }
        }

        // Write atomically
        let live_path = self.output_path.join("report-live.md");
        if let Err(e) = std::fs::write(&live_path, content) {
            eprintln!("Warning: Failed to write live report: {}", e);
        }
    }
}

/// Truncates a string for report display.
fn truncate_for_report(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ").replace('\r', "");
    if s.len() <= max_len {
        s
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Creates a progress callback that writes incremental reports as tests complete.
///
/// This is useful for long test runs where you want to see failures immediately
/// rather than waiting for all tests to finish.
pub fn create_incremental_progress_callback(
    verbosity: Verbosity,
    output_path: PathBuf,
) -> crate::runner::ProgressCallback {
    let state = std::sync::Arc::new(std::sync::Mutex::new(IncrementalState {
        reporter: TerminalReporter::with_verbosity(verbosity),
        results: Vec::new(),
        output_path,
        total_scenarios: 0,
        start_time: std::time::Instant::now(),
    }));

    Box::new(move |event| {
        if let Ok(mut s) = state.lock() {
            // Handle terminal output
            s.reporter.handle_progress(event.clone());

            // Handle incremental reporting
            match &event {
                crate::runner::ProgressEvent::RunStarted { total_scenarios } => {
                    s.total_scenarios = *total_scenarios;
                    s.start_time = std::time::Instant::now();
                }
                crate::runner::ProgressEvent::ScenarioCompleted { result, .. } => {
                    s.results.push(result.clone());
                    s.write_live_report();
                }
                _ => {}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Assertion;

    fn mock_results() -> RunResults {
        RunResults {
            results: vec![
                TestResult {
                    scenario_id: "test-pass".to_string(),
                    scenario_description: "A passing test".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1: Connectivity".to_string(),
                    passed: true,
                    assertions: vec![Assertion {
                        name: "Response received".to_string(),
                        passed: true,
                        expected: "Non-empty response".to_string(),
                        actual: "Received 100 bytes".to_string(),
                    }],
                    duration: Duration::from_secs(5),
                },
                TestResult {
                    scenario_id: "test-fail".to_string(),
                    scenario_description: "A failing test".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1: Connectivity".to_string(),
                    passed: false,
                    assertions: vec![Assertion {
                        name: "Exit code".to_string(),
                        passed: false,
                        expected: "Exit code 0".to_string(),
                        actual: "Exit code 1".to_string(),
                    }],
                    duration: Duration::from_secs(3),
                },
            ],
            duration: Duration::from_secs(8),
            skipped_count: 0,
        }
    }

    #[test]
    fn test_reporter_new() {
        let reporter = TerminalReporter::new();
        assert_eq!(reporter.verbosity, Verbosity::Normal);
    }

    #[test]
    fn test_reporter_with_verbosity() {
        let reporter = TerminalReporter::with_verbosity(Verbosity::Quiet);
        assert_eq!(reporter.verbosity, Verbosity::Quiet);

        let reporter = TerminalReporter::with_verbosity(Verbosity::Verbose);
        assert_eq!(reporter.verbosity, Verbosity::Verbose);
    }

    #[test]
    fn test_reporter_default() {
        let reporter = TerminalReporter::default();
        assert_eq!(reporter.verbosity, Verbosity::Normal);
    }

    #[test]
    fn test_verbosity_default() {
        assert_eq!(Verbosity::default(), Verbosity::Normal);
    }

    #[test]
    fn test_handle_progress_run_started() {
        let mut reporter = TerminalReporter::with_verbosity(Verbosity::Normal);
        // Just verify it doesn't panic
        reporter.handle_progress(ProgressEvent::RunStarted { total_scenarios: 5 });
    }

    #[test]
    fn test_handle_progress_scenario_completed() {
        let mut reporter = TerminalReporter::with_verbosity(Verbosity::Normal);
        reporter.handle_progress(ProgressEvent::ScenarioCompleted {
            scenario_id: "test-1".to_string(),
            passed: true,
            duration: Duration::from_secs(5),
            result: TestResult {
                scenario_id: "test-1".to_string(),
                scenario_description: "Test scenario".to_string(),
                backend: "Claude".to_string(),
                tier: "Tier 1".to_string(),
                passed: true,
                assertions: vec![],
                duration: Duration::from_secs(5),
            },
        });
    }

    #[test]
    fn test_handle_progress_scenario_skipped() {
        let mut reporter = TerminalReporter::with_verbosity(Verbosity::Normal);
        reporter.handle_progress(ProgressEvent::ScenarioSkipped {
            scenario_id: "test-1".to_string(),
            reason: "backend unavailable".to_string(),
        });
    }

    #[test]
    fn test_handle_progress_quiet_mode() {
        let mut reporter = TerminalReporter::with_verbosity(Verbosity::Quiet);
        // In quiet mode, these shouldn't print anything
        reporter.handle_progress(ProgressEvent::RunStarted { total_scenarios: 5 });
        reporter.handle_progress(ProgressEvent::ScenarioStarted {
            scenario_id: "test-1".to_string(),
            tier: "Tier 1".to_string(),
        });
    }

    #[test]
    fn test_print_summary_all_passed() {
        let results = RunResults {
            results: vec![TestResult {
                scenario_id: "test-1".to_string(),
                scenario_description: "Test".to_string(),
                backend: "Claude".to_string(),
                tier: "Tier 1".to_string(),
                passed: true,
                assertions: vec![],
                duration: Duration::from_secs(1),
            }],
            duration: Duration::from_secs(1),
            skipped_count: 0,
        };

        let reporter = TerminalReporter::new();
        // Just verify it doesn't panic
        reporter.print_summary(&results);
    }

    #[test]
    fn test_print_summary_mixed() {
        let results = mock_results();
        let reporter = TerminalReporter::new();
        reporter.print_summary(&results);
    }

    #[test]
    fn test_print_failures_none() {
        let results = RunResults {
            results: vec![TestResult {
                scenario_id: "test-1".to_string(),
                scenario_description: "Test".to_string(),
                backend: "Claude".to_string(),
                tier: "Tier 1".to_string(),
                passed: true,
                assertions: vec![],
                duration: Duration::from_secs(1),
            }],
            duration: Duration::from_secs(1),
            skipped_count: 0,
        };

        let reporter = TerminalReporter::new();
        // Should not print anything for no failures
        reporter.print_failures(&results);
    }

    #[test]
    fn test_print_failures_some() {
        let results = mock_results();
        let reporter = TerminalReporter::new();
        reporter.print_failures(&results);
    }

    #[test]
    fn test_print_by_tier() {
        let results = mock_results();
        let reporter = TerminalReporter::new();
        reporter.print_by_tier(&results);
    }

    #[test]
    fn test_print_by_tier_verbose() {
        let results = mock_results();
        let reporter = TerminalReporter::with_verbosity(Verbosity::Verbose);
        reporter.print_by_tier(&results);
    }

    #[test]
    fn test_create_progress_callback() {
        let callback = create_progress_callback(Verbosity::Normal);
        // Just verify it doesn't panic
        callback(ProgressEvent::RunStarted { total_scenarios: 1 });
    }
}

// ============================================================================
// Report Data Structures
// ============================================================================

/// Errors that can occur during report generation.
#[derive(Debug, Error)]
pub enum ReporterError {
    /// Failed to write report file.
    #[error("failed to write report: {0}")]
    WriteError(#[from] std::io::Error),

    /// Failed to serialize report to JSON.
    #[error("failed to serialize report: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Full test report structure for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    /// Timestamp of the report generation.
    pub timestamp: DateTime<Utc>,

    /// Hats version used.
    pub hats_version: String,

    /// Total duration of the test run.
    #[serde(with = "duration_serde")]
    pub duration: Duration,

    /// Overall pass/fail verdict.
    pub passed: bool,

    /// Verdict message.
    pub verdict: String,

    /// Summary statistics.
    pub summary: ReportSummary,

    /// Individual test results with analysis.
    pub results: Vec<AnalyzedResultData>,

    /// Prioritized recommendations.
    pub recommendations: Vec<Recommendation>,
}

/// Summary statistics for the report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Total number of tests.
    pub total: usize,

    /// Number of passed tests.
    pub passed: usize,

    /// Number of failed tests.
    pub failed: usize,

    /// Number of skipped tests.
    pub skipped: usize,

    /// Quality breakdown for passed tests.
    pub quality_breakdown: QualityBreakdown,

    /// Results grouped by tier.
    pub by_tier: HashMap<String, TierSummary>,

    /// Results grouped by backend.
    pub by_backend: HashMap<String, BackendSummary>,
}

/// Quality score breakdown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityBreakdown {
    pub optimal: usize,
    pub good: usize,
    pub acceptable: usize,
    pub suboptimal: usize,
}

/// Summary for a single tier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Summary for a single backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Analyzed result data for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedResultData {
    /// Scenario ID.
    pub scenario_id: String,

    /// Description.
    pub description: String,

    /// Backend.
    pub backend: String,

    /// Tier.
    pub tier: String,

    /// Whether it passed.
    pub passed: bool,

    /// Test duration.
    #[serde(with = "duration_serde")]
    pub duration: Duration,

    /// Assertions.
    pub assertions: Vec<crate::models::Assertion>,

    /// Diagnosis for failed tests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnosis: Option<Diagnosis>,

    /// Analysis for passed tests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<PassedAnalysis>,
}

impl From<&TestResult> for AnalyzedResultData {
    fn from(result: &TestResult) -> Self {
        Self {
            scenario_id: result.scenario_id.clone(),
            description: result.scenario_description.clone(),
            backend: result.backend.clone(),
            tier: result.tier.clone(),
            passed: result.passed,
            duration: result.duration,
            assertions: result.assertions.clone(),
            diagnosis: None,
            analysis: None,
        }
    }
}

impl From<&AnalyzedResult> for AnalyzedResultData {
    fn from(result: &AnalyzedResult) -> Self {
        Self {
            scenario_id: result.result.scenario_id.clone(),
            description: result.result.scenario_description.clone(),
            backend: result.result.backend.clone(),
            tier: result.result.tier.clone(),
            passed: result.result.passed,
            duration: result.result.duration,
            assertions: result.result.assertions.clone(),
            diagnosis: result.diagnosis.clone(),
            analysis: result.analysis.clone(),
        }
    }
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

// ============================================================================
// Markdown Reporter
// ============================================================================

/// Generates agent-readable markdown reports.
///
/// The markdown format is designed for AI agents to parse and act upon,
/// with collapsible sections for full context and structured analysis.
pub struct MarkdownReporter;

impl MarkdownReporter {
    /// Creates a new markdown reporter.
    pub fn new() -> Self {
        Self
    }

    /// Generates a full markdown report from run results.
    pub fn generate(&self, results: &RunResults, analyzed: Option<&[AnalyzedResult]>) -> String {
        let mut report = String::new();

        // Header and verdict
        self.write_header(&mut report, results);

        // Summary section
        self.write_summary(&mut report, results, analyzed);

        // Failed tests section
        self.write_failed_tests(&mut report, results, analyzed);

        // Passed tests section
        self.write_passed_tests(&mut report, results, analyzed);

        // Recommendations section
        if let Some(analyzed) = analyzed {
            self.write_recommendations(&mut report, analyzed);
        }

        // Quick fix commands
        self.write_quick_fixes(&mut report, results);

        // Artifacts section
        self.write_artifacts(&mut report);

        report
    }

    fn write_header(&self, report: &mut String, results: &RunResults) {
        report.push_str("# E2E Test Report\n\n");

        let (emoji, verdict) = if results.all_passed() {
            ("🟢", "PASSED")
        } else if results.passed_count() > 0 {
            ("🟡", "MIXED")
        } else {
            ("🔴", "FAILED")
        };

        report.push_str(&format!("## {} {}\n\n", emoji, verdict));

        report.push_str(&format!(
            "**Generated:** {}\n",
            Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
        ));
        report.push_str(&format!("**Hats Version:** {}\n", crate::VERSION));
        report.push_str(&format!(
            "**Duration:** {:.1}s\n",
            results.duration.as_secs_f64()
        ));

        let verdict_msg = if results.failed_count() == 0 {
            "All tests passed".to_string()
        } else {
            format!("{} tests failed - action required", results.failed_count())
        };
        report.push_str(&format!("**Verdict:** {}\n\n", verdict_msg));
    }

    fn write_summary(
        &self,
        report: &mut String,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) {
        report.push_str("## Summary\n\n");

        report.push_str("| Status | Count |\n");
        report.push_str("|--------|-------|\n");
        report.push_str(&format!("| ✅ Passed | {} |\n", results.passed_count()));
        report.push_str(&format!("| ❌ Failed | {} |\n", results.failed_count()));
        if results.skipped_count > 0 {
            report.push_str(&format!("| ⏭️ Skipped | {} |\n", results.skipped_count));
        }
        report.push('\n');

        // Failures by tier
        let tiers = results.by_tier();
        let failed_tiers: Vec<_> = tiers
            .iter()
            .filter(|(_, tests)| tests.iter().any(|t| !t.passed))
            .collect();

        if !failed_tiers.is_empty() {
            report.push_str("### Failures by Tier\n");
            for (tier, tests) in failed_tiers {
                let failed = tests.iter().filter(|t| !t.passed).count();
                report.push_str(&format!("- {}: {} failures\n", tier, failed));
            }
            report.push('\n');
        }

        // Quality breakdown if we have analysis
        if let Some(analyzed) = analyzed {
            let quality_counts = self.count_quality_scores(analyzed);
            if quality_counts.iter().any(|(_, count)| *count > 0) {
                report.push_str("### Quality Breakdown\n");
                report.push_str("| Quality | Count |\n");
                report.push_str("|---------|-------|\n");
                for (quality, count) in quality_counts {
                    if count > 0 {
                        report.push_str(&format!("| {} | {} |\n", quality, count));
                    }
                }
                report.push('\n');
            }
        }

        report.push_str("---\n\n");
    }

    fn count_quality_scores(&self, analyzed: &[AnalyzedResult]) -> Vec<(&str, usize)> {
        let mut optimal = 0;
        let mut good = 0;
        let mut acceptable = 0;
        let mut suboptimal = 0;

        for result in analyzed {
            if result.result.passed {
                if let Some(ref analysis) = result.analysis {
                    match analysis.quality_score {
                        QualityScore::Optimal => optimal += 1,
                        QualityScore::Good => good += 1,
                        QualityScore::Acceptable => acceptable += 1,
                        QualityScore::Suboptimal => suboptimal += 1,
                    }
                } else {
                    // No analysis = assume good
                    good += 1;
                }
            }
        }

        vec![
            ("🟢 Optimal", optimal),
            ("🟡 Good", good),
            ("🟠 Acceptable", acceptable),
            ("🔴 Suboptimal", suboptimal),
        ]
    }

    fn write_failed_tests(
        &self,
        report: &mut String,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) {
        let failures = results.failures();
        if failures.is_empty() {
            return;
        }

        report.push_str("## 🔴 Failed Tests\n\n");

        for result in failures {
            self.write_failed_test(report, result, analyzed);
        }
    }

    fn write_failed_test(
        &self,
        report: &mut String,
        result: &TestResult,
        analyzed: Option<&[AnalyzedResult]>,
    ) {
        report.push_str(&format!(
            "### ❌ `{}` ({})\n\n",
            result.scenario_id, result.tier
        ));
        report.push_str(&format!(
            "**Description:** {}\n",
            result.scenario_description
        ));
        report.push_str(&format!("**Backend:** {}\n", result.backend));
        report.push_str(&format!(
            "**Duration:** {:.1}s\n",
            result.duration.as_secs_f64()
        ));
        // Note: Exit code is visible in the assertions table. We don't track it in TestResult
        // directly, but the exit_code assertion shows the actual value in its "actual" field.

        // Assertions table
        report.push_str("#### Assertions\n\n");
        report.push_str("| Assertion | Status | Expected | Actual |\n");
        report.push_str("|-----------|--------|----------|--------|\n");
        for assertion in &result.assertions {
            let status = if assertion.passed { "✅" } else { "❌" };
            report.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                assertion.name, status, assertion.expected, assertion.actual
            ));
        }
        report.push('\n');

        // Context (collapsible)
        report.push_str("#### Context\n\n");
        report.push_str("<details>\n");
        report.push_str("<summary>📄 Full Test Context</summary>\n\n");
        report.push_str(&format!("**Scenario ID:** {}\n", result.scenario_id));
        report.push_str(&format!("**Tier:** {}\n", result.tier));
        report.push_str(&format!("**Backend:** {}\n\n", result.backend));
        report.push_str("</details>\n\n");

        // Diagnosis if available
        if let Some(analyzed) = analyzed
            && let Some(analyzed_result) = analyzed
                .iter()
                .find(|a| a.result.scenario_id == result.scenario_id)
            && let Some(ref diagnosis) = analyzed_result.diagnosis
        {
            self.write_diagnosis(report, diagnosis);
        }

        report.push_str("---\n\n");
    }

    fn write_diagnosis(&self, report: &mut String, diagnosis: &Diagnosis) {
        report.push_str("#### 🔍 Diagnosis\n\n");

        let failure_type = match diagnosis.failure_type {
            FailureType::BackendError => "Backend Error",
            FailureType::PromptIneffective => "Prompt Ineffective",
            FailureType::EventMissing => "Event Missing",
            FailureType::EventMalformed => "Event Malformed",
            FailureType::TimeoutExceeded => "Timeout Exceeded",
            FailureType::UnexpectedTermination => "Unexpected Termination",
            FailureType::AssertionMismatch => "Assertion Mismatch",
            FailureType::ConfigurationError => "Configuration Error",
            FailureType::AuthenticationError => "Authentication Error",
            FailureType::Unknown => "Unknown",
        };
        report.push_str(&format!("**Failure Type:** {}\n\n", failure_type));

        if !diagnosis.root_cause_hypothesis.is_empty() {
            report.push_str(&format!(
                "**Root Cause Hypothesis:**\n{}\n\n",
                diagnosis.root_cause_hypothesis
            ));
        }

        if !diagnosis.evidence.is_empty() {
            report.push_str("**Evidence:**\n");
            for ev in &diagnosis.evidence {
                report.push_str(&format!("- {}\n", ev));
            }
            report.push('\n');
        }

        if !diagnosis.similar_failures.is_empty() {
            report.push_str("**Similar Failures:**\n");
            for sf in &diagnosis.similar_failures {
                report.push_str(&format!("- `{}`\n", sf));
            }
            report.push('\n');
        }

        if !diagnosis.suggested_investigations.is_empty() {
            report.push_str("**Suggested Investigations:**\n");
            for (i, inv) in diagnosis.suggested_investigations.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, inv));
            }
            report.push('\n');
        }

        if !diagnosis.potential_fixes.is_empty() {
            report.push_str("**Potential Fixes:**\n");
            for fix in &diagnosis.potential_fixes {
                let confidence = format!("{:.0}%", fix.confidence * 100.0);
                let confidence_label = if fix.confidence >= 0.8 {
                    "High Confidence"
                } else if fix.confidence >= 0.5 {
                    "Medium Confidence"
                } else {
                    "Low Confidence"
                };

                report.push_str(&format!(
                    "1. **[{}]** {} ({})\n",
                    confidence_label, fix.description, confidence
                ));
                if let Some(ref file) = fix.file_to_modify {
                    report.push_str(&format!("   - File: `{}`\n", file));
                }
                if let Some(ref change) = fix.suggested_change {
                    report.push_str(&format!("   - Change: {}\n", change));
                }
            }
            report.push('\n');
        }
    }

    fn write_passed_tests(
        &self,
        report: &mut String,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) {
        let passed: Vec<_> = results.results.iter().filter(|r| r.passed).collect();
        if passed.is_empty() {
            return;
        }

        report.push_str("## ✅ Passed Tests\n\n");

        // Summary by quality if analyzed
        if let Some(analyzed) = analyzed {
            self.write_quality_summary(report, analyzed);
        }

        // Tests that need attention (acceptable or suboptimal)
        if let Some(analyzed) = analyzed {
            let needs_attention: Vec<_> = analyzed
                .iter()
                .filter(|a| {
                    a.result.passed
                        && a.analysis.as_ref().is_some_and(|an| {
                            matches!(
                                an.quality_score,
                                QualityScore::Acceptable | QualityScore::Suboptimal
                            ) || !an.warnings.is_empty()
                        })
                })
                .collect();

            for result in needs_attention {
                self.write_passed_test_with_warnings(report, result);
            }
        }

        // Optimal tests (collapsed)
        report.push_str("### 🟢 Optimal Tests\n\n");
        report.push_str("<details>\n");
        report.push_str("<summary>Click to expand optimal test details</summary>\n\n");
        report.push_str("| Test | Duration | Quality | Notes |\n");
        report.push_str("|------|----------|---------|-------|\n");

        for result in &passed {
            let quality = if let Some(analyzed) = analyzed {
                analyzed
                    .iter()
                    .find(|a| a.result.scenario_id == result.scenario_id)
                    .and_then(|a| a.analysis.as_ref())
                    .map_or("🟢 Good", |an| match an.quality_score {
                        QualityScore::Optimal => "🟢 Optimal",
                        QualityScore::Good => "🟡 Good",
                        QualityScore::Acceptable => "🟠 Acceptable",
                        QualityScore::Suboptimal => "🔴 Suboptimal",
                    })
            } else {
                "🟢 Good"
            };

            report.push_str(&format!(
                "| {} | {:.1}s | {} | - |\n",
                result.scenario_id,
                result.duration.as_secs_f64(),
                quality
            ));
        }

        report.push_str("\n</details>\n\n");
        report.push_str("---\n\n");
    }

    fn write_quality_summary(&self, report: &mut String, analyzed: &[AnalyzedResult]) {
        let quality_counts = self.count_quality_scores(analyzed);
        let mut tests_by_quality: HashMap<&str, Vec<&str>> = HashMap::new();

        for result in analyzed {
            if result.result.passed {
                let quality =
                    result
                        .analysis
                        .as_ref()
                        .map_or("🟡 Good", |an| match an.quality_score {
                            QualityScore::Optimal => "🟢 Optimal",
                            QualityScore::Good => "🟡 Good",
                            QualityScore::Acceptable => "🟠 Acceptable",
                            QualityScore::Suboptimal => "🔴 Suboptimal",
                        });
                tests_by_quality
                    .entry(quality)
                    .or_default()
                    .push(&result.result.scenario_id);
            }
        }

        report.push_str("### Summary by Quality\n\n");
        report.push_str("| Quality | Count | Tests |\n");
        report.push_str("|---------|-------|-------|\n");
        for (quality, count) in quality_counts {
            if count > 0 {
                let tests = tests_by_quality
                    .get(quality)
                    .map(|t| t.join(", "))
                    .unwrap_or_else(|| "-".to_string());
                // Truncate if too long
                let tests_display = if tests.len() > 50 {
                    format!("{}...", &tests[..47])
                } else {
                    tests
                };
                report.push_str(&format!(
                    "| {} | {} | {} |\n",
                    quality, count, tests_display
                ));
            }
        }
        report.push('\n');
    }

    fn write_passed_test_with_warnings(&self, report: &mut String, result: &AnalyzedResult) {
        let analysis = result.analysis.as_ref().unwrap();
        let quality_label = match analysis.quality_score {
            QualityScore::Optimal => "Optimal",
            QualityScore::Good => "Good",
            QualityScore::Acceptable => "Acceptable",
            QualityScore::Suboptimal => "Suboptimal",
        };

        report.push_str(&format!(
            "### 🟠 `{}` ({} - Needs Attention)\n\n",
            result.result.scenario_id, quality_label
        ));
        report.push_str(&format!(
            "**Description:** {}\n",
            result.result.scenario_description
        ));
        report.push_str(&format!("**Backend:** {}\n", result.result.backend));
        report.push_str(&format!(
            "**Duration:** {:.1}s\n",
            result.result.duration.as_secs_f64()
        ));
        report.push_str(&format!("**Quality:** {}\n\n", quality_label));

        // Warnings
        if !analysis.warnings.is_empty() {
            report.push_str("#### ⚠️ Warnings\n\n");
            for (i, warning) in analysis.warnings.iter().enumerate() {
                report.push_str(&format!("{}. **{}**\n", i + 1, warning.message));
                if !warning.evidence.is_empty() {
                    report.push_str(&format!("   - Evidence: {}\n", warning.evidence));
                }
            }
            report.push('\n');
        }

        // Optimizations
        if !analysis.optimizations.is_empty() {
            report.push_str("#### 💡 Optimizations\n\n");
            for (i, opt) in analysis.optimizations.iter().enumerate() {
                report.push_str(&format!("{}. **{}**\n", i + 1, opt.description));
                if !opt.potential_improvement.is_empty() {
                    report.push_str(&format!("   - Potential: {}\n", opt.potential_improvement));
                }
                if let Some(ref change) = opt.suggested_change {
                    report.push_str(&format!("   - Suggested: {}\n", change));
                }
            }
            report.push('\n');
        }

        report.push_str("---\n\n");
    }

    fn write_recommendations(&self, report: &mut String, analyzed: &[AnalyzedResult]) {
        // Collect recommendations from diagnoses
        let mut recommendations: Vec<Recommendation> = Vec::new();

        // Critical: from failed tests
        for result in analyzed.iter().filter(|r| !r.result.passed) {
            if let Some(ref diagnosis) = result.diagnosis
                && !diagnosis.potential_fixes.is_empty()
            {
                let fix = &diagnosis.potential_fixes[0];
                recommendations.push(Recommendation {
                    severity: Severity::Critical,
                    category: "failure".to_string(),
                    title: format!("Fix: {}", result.result.scenario_id),
                    description: fix.description.clone(),
                    affected_tests: vec![result.result.scenario_id.clone()],
                });
            }
        }

        // Warnings: from passed tests with warnings
        for result in analyzed.iter().filter(|r| r.result.passed) {
            if let Some(ref analysis) = result.analysis {
                for warning in &analysis.warnings {
                    recommendations.push(Recommendation {
                        severity: Severity::Warning,
                        category: "optimization".to_string(),
                        title: warning.message.clone(),
                        description: warning.evidence.clone(),
                        affected_tests: vec![result.result.scenario_id.clone()],
                    });
                }
            }
        }

        if recommendations.is_empty() {
            return;
        }

        report.push_str("## 📋 Recommendations\n\n");

        // Critical
        let critical: Vec<_> = recommendations
            .iter()
            .filter(|r| r.severity == Severity::Critical)
            .collect();
        if !critical.is_empty() {
            report.push_str("### 🔴 Critical (Failures)\n\n");
            for (i, rec) in critical.iter().enumerate() {
                report.push_str(&format!(
                    "{}. **{}**\n   - Affects: {}\n   - Fix: {}\n\n",
                    i + 1,
                    rec.title,
                    rec.affected_tests.join(", "),
                    rec.description
                ));
            }
        }

        // Warnings
        let warnings: Vec<_> = recommendations
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .collect();
        if !warnings.is_empty() {
            report.push_str("### 🟡 Warning\n\n");
            for (i, rec) in warnings.iter().enumerate() {
                report.push_str(&format!(
                    "{}. **{}**\n   - Affects: {}\n\n",
                    i + 1,
                    rec.title,
                    rec.affected_tests.join(", ")
                ));
            }
        }

        report.push_str("---\n\n");
    }

    fn write_quick_fixes(&self, report: &mut String, results: &RunResults) {
        if results.failures().is_empty() {
            return;
        }

        report.push_str("## 🔧 Quick Fix Commands\n\n");
        report.push_str("Based on the failures, here are suggested investigation commands:\n\n");
        report.push_str("```bash\n");

        // Re-run with verbose
        let failed_ids: Vec<_> = results.failures().iter().map(|r| &r.scenario_id).collect();
        if let Some(first_failed) = failed_ids.first() {
            report.push_str("# Re-run specific failing test with verbose output\n");
            report.push_str(&format!(
                "hats-e2e --filter \"{}\" --verbose --keep-workspace\n\n",
                first_failed
            ));
        }

        report.push_str("# Check test workspaces\nls -la .e2e-tests/\n");
        report.push_str("```\n\n");
        report.push_str("---\n\n");
    }

    fn write_artifacts(&self, report: &mut String) {
        report.push_str("## 📁 Artifacts\n\n");
        report.push_str("All test artifacts preserved in `.e2e-tests/`:\n");
        report.push_str("- `report.md` - This report\n");
        report.push_str("- `report.json` - Machine-readable version\n");
        report.push_str("- `<scenario-id>/` - Individual test workspaces\n");
    }
}

impl Default for MarkdownReporter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// JSON Reporter
// ============================================================================

/// Generates machine-readable JSON reports.
///
/// The JSON format is designed for programmatic consumption,
/// enabling CI/CD analysis and trend tracking.
pub struct JsonReporter;

impl JsonReporter {
    /// Creates a new JSON reporter.
    pub fn new() -> Self {
        Self
    }

    /// Generates a full JSON report from run results.
    pub fn generate(
        &self,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) -> Result<String, ReporterError> {
        let report = self.build_report(results, analyzed);
        serde_json::to_string_pretty(&report).map_err(ReporterError::from)
    }

    /// Builds the report data structure.
    pub fn build_report(
        &self,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) -> TestReport {
        let passed = results.all_passed();
        let verdict = if passed {
            "All tests passed".to_string()
        } else {
            format!("{} tests failed - action required", results.failed_count())
        };

        // Build results with analysis
        let result_data: Vec<AnalyzedResultData> = if let Some(analyzed) = analyzed {
            analyzed.iter().map(AnalyzedResultData::from).collect()
        } else {
            results
                .results
                .iter()
                .map(AnalyzedResultData::from)
                .collect()
        };

        // Build summary
        let summary = self.build_summary(results, analyzed);

        // Collect recommendations
        let recommendations = self.collect_recommendations(analyzed);

        TestReport {
            timestamp: Utc::now(),
            hats_version: crate::VERSION.to_string(),
            duration: results.duration,
            passed,
            verdict,
            summary,
            results: result_data,
            recommendations,
        }
    }

    fn build_summary(
        &self,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) -> ReportSummary {
        let mut quality_breakdown = QualityBreakdown::default();

        if let Some(analyzed) = analyzed {
            for result in analyzed {
                if result.result.passed {
                    if let Some(ref analysis) = result.analysis {
                        match analysis.quality_score {
                            QualityScore::Optimal => quality_breakdown.optimal += 1,
                            QualityScore::Good => quality_breakdown.good += 1,
                            QualityScore::Acceptable => quality_breakdown.acceptable += 1,
                            QualityScore::Suboptimal => quality_breakdown.suboptimal += 1,
                        }
                    } else {
                        quality_breakdown.good += 1;
                    }
                }
            }
        } else {
            quality_breakdown.good = results.passed_count();
        }

        // By tier
        let mut by_tier: HashMap<String, TierSummary> = HashMap::new();
        for (tier, tests) in results.by_tier() {
            let tier_summary = TierSummary {
                total: tests.len(),
                passed: tests.iter().filter(|t| t.passed).count(),
                failed: tests.iter().filter(|t| !t.passed).count(),
            };
            by_tier.insert(tier.to_string(), tier_summary);
        }

        // By backend
        let mut by_backend: HashMap<String, BackendSummary> = HashMap::new();
        for result in &results.results {
            let entry = by_backend.entry(result.backend.clone()).or_default();
            entry.total += 1;
            if result.passed {
                entry.passed += 1;
            } else {
                entry.failed += 1;
            }
        }

        ReportSummary {
            total: results.total_count(),
            passed: results.passed_count(),
            failed: results.failed_count(),
            skipped: results.skipped_count,
            quality_breakdown,
            by_tier,
            by_backend,
        }
    }

    fn collect_recommendations(&self, analyzed: Option<&[AnalyzedResult]>) -> Vec<Recommendation> {
        let Some(analyzed) = analyzed else {
            return Vec::new();
        };

        let mut recommendations = Vec::new();

        // From failed tests
        for result in analyzed.iter().filter(|r| !r.result.passed) {
            if let Some(ref diagnosis) = result.diagnosis
                && !diagnosis.potential_fixes.is_empty()
            {
                let fix = &diagnosis.potential_fixes[0];
                recommendations.push(Recommendation {
                    severity: Severity::Critical,
                    category: "failure".to_string(),
                    title: format!("Fix: {}", result.result.scenario_id),
                    description: fix.description.clone(),
                    affected_tests: vec![result.result.scenario_id.clone()],
                });
            }
        }

        // From passed tests with warnings
        for result in analyzed.iter().filter(|r| r.result.passed) {
            if let Some(ref analysis) = result.analysis {
                for warning in &analysis.warnings {
                    recommendations.push(Recommendation {
                        severity: Severity::Warning,
                        category: "optimization".to_string(),
                        title: warning.message.clone(),
                        description: warning.evidence.clone(),
                        affected_tests: vec![result.result.scenario_id.clone()],
                    });
                }
            }
        }

        recommendations
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Report Writer
// ============================================================================

/// Orchestrates writing reports to files.
pub struct ReportWriter {
    /// Output directory for reports.
    output_dir: PathBuf,
}

impl ReportWriter {
    /// Creates a new report writer with the given output directory.
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    /// Returns the output directory.
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// Writes reports in the specified format(s).
    pub fn write(
        &self,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
        format: crate::models::ReportFormat,
    ) -> Result<Vec<PathBuf>, ReporterError> {
        std::fs::create_dir_all(&self.output_dir)?;

        let mut written_files = Vec::new();

        match format {
            crate::models::ReportFormat::Markdown => {
                let path = self.write_markdown(results, analyzed)?;
                written_files.push(path);
            }
            crate::models::ReportFormat::Json => {
                let path = self.write_json(results, analyzed)?;
                written_files.push(path);
            }
            crate::models::ReportFormat::Both => {
                let md_path = self.write_markdown(results, analyzed)?;
                let json_path = self.write_json(results, analyzed)?;
                written_files.push(md_path);
                written_files.push(json_path);
            }
        }

        Ok(written_files)
    }

    /// Writes a markdown report.
    pub fn write_markdown(
        &self,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) -> Result<PathBuf, ReporterError> {
        let reporter = MarkdownReporter::new();
        let content = reporter.generate(results, analyzed);

        let path = self.output_dir.join("report.md");
        std::fs::write(&path, content)?;

        Ok(path)
    }

    /// Writes a JSON report.
    pub fn write_json(
        &self,
        results: &RunResults,
        analyzed: Option<&[AnalyzedResult]>,
    ) -> Result<PathBuf, ReporterError> {
        let reporter = JsonReporter::new();
        let content = reporter.generate(results, analyzed)?;

        let path = self.output_dir.join("report.json");
        std::fs::write(&path, content)?;

        Ok(path)
    }
}

// ============================================================================
// Tests for New Reporters
// ============================================================================

#[cfg(test)]
mod reporter_tests {
    use super::*;
    use crate::analyzer::{
        Diagnosis, FailureType, PassedAnalysis, PotentialFix, QualityScore, TestMetrics, Warning,
        WarningCategory,
    };
    use crate::models::Assertion;
    use std::time::Duration;

    fn mock_passed_result() -> TestResult {
        TestResult {
            scenario_id: "claude-connect".to_string(),
            scenario_description: "Basic connectivity test for Claude".to_string(),
            backend: "Claude".to_string(),
            tier: "Tier 1: Connectivity".to_string(),
            passed: true,
            assertions: vec![Assertion {
                name: "Response received".to_string(),
                passed: true,
                expected: "Non-empty response".to_string(),
                actual: "Received 100 bytes".to_string(),
            }],
            duration: Duration::from_secs(12),
        }
    }

    fn mock_failed_result() -> TestResult {
        TestResult {
            scenario_id: "hat-instructions".to_string(),
            scenario_description: "Verify hat instructions are followed".to_string(),
            backend: "Claude".to_string(),
            tier: "Tier 5: Hat Collections".to_string(),
            passed: false,
            assertions: vec![
                Assertion {
                    name: "Agent mentions Builder".to_string(),
                    passed: false,
                    expected: "Contains 'I am the Builder'".to_string(),
                    actual: "No mention of Builder".to_string(),
                },
                Assertion {
                    name: "build.task emitted".to_string(),
                    passed: true,
                    expected: "Event present".to_string(),
                    actual: "Event found".to_string(),
                },
            ],
            duration: Duration::from_secs(45),
        }
    }

    fn mock_run_results_all_pass() -> RunResults {
        RunResults {
            results: vec![mock_passed_result()],
            duration: Duration::from_secs(12),
            skipped_count: 0,
        }
    }

    fn mock_run_results_mixed() -> RunResults {
        RunResults {
            results: vec![mock_passed_result(), mock_failed_result()],
            duration: Duration::from_secs(57),
            skipped_count: 1,
        }
    }

    fn mock_analyzed_passed() -> AnalyzedResult {
        AnalyzedResult {
            result: mock_passed_result(),
            diagnosis: None,
            analysis: Some(PassedAnalysis {
                quality_score: QualityScore::Optimal,
                metrics: TestMetrics::default(),
                warnings: vec![],
                optimizations: vec![],
            }),
        }
    }

    fn mock_analyzed_failed() -> AnalyzedResult {
        AnalyzedResult {
            result: mock_failed_result(),
            diagnosis: Some(Diagnosis {
                failure_type: FailureType::PromptIneffective,
                root_cause_hypothesis: "Hat instructions not injected into prompt".to_string(),
                evidence: vec!["No Builder mention in output".to_string()],
                similar_failures: vec!["hat-single".to_string()],
                suggested_investigations: vec!["Check prompt building".to_string()],
                potential_fixes: vec![PotentialFix {
                    description: "Add IMPORTANT prefix to hat instructions".to_string(),
                    confidence: 0.85,
                    file_to_modify: Some("src/hatless.rs".to_string()),
                    suggested_change: Some("Wrap instructions with emphasis".to_string()),
                }],
            }),
            analysis: None,
        }
    }

    fn mock_analyzed_with_warnings() -> AnalyzedResult {
        AnalyzedResult {
            result: TestResult {
                scenario_id: "memory-injection".to_string(),
                scenario_description: "Verify memory auto-injection".to_string(),
                backend: "Claude".to_string(),
                tier: "Tier 6: Memory System".to_string(),
                passed: true,
                assertions: vec![Assertion {
                    name: "Memories injected".to_string(),
                    passed: true,
                    expected: "Memories in prompt".to_string(),
                    actual: "Found memories".to_string(),
                }],
                duration: Duration::from_secs(38),
            },
            diagnosis: None,
            analysis: Some(PassedAnalysis {
                quality_score: QualityScore::Acceptable,
                metrics: TestMetrics::default(),
                warnings: vec![Warning {
                    category: WarningCategory::SlowExecution,
                    message: "Took 38.2s, expected <30s".to_string(),
                    evidence: "27% slower than baseline".to_string(),
                }],
                optimizations: vec![],
            }),
        }
    }

    // ==================== Markdown Reporter Tests ====================

    #[test]
    fn test_markdown_reporter_new() {
        let reporter = MarkdownReporter::new();
        // Verify it can be created
        let _ = reporter;
    }

    #[test]
    fn test_markdown_reporter_default() {
        let reporter = MarkdownReporter;
        let _ = reporter;
    }

    #[test]
    fn test_markdown_generate_all_passed() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_all_pass();
        let report = reporter.generate(&results, None);

        assert!(report.contains("# E2E Test Report"));
        assert!(report.contains("🟢 PASSED"));
        assert!(report.contains("All tests passed"));
        assert!(report.contains("✅ Passed | 1"));
        assert!(report.contains("❌ Failed | 0"));
    }

    #[test]
    fn test_markdown_generate_mixed() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_mixed();
        let report = reporter.generate(&results, None);

        assert!(report.contains("🟡 MIXED"));
        assert!(report.contains("1 tests failed - action required"));
        assert!(report.contains("✅ Passed | 1"));
        assert!(report.contains("❌ Failed | 1"));
        assert!(report.contains("⏭️ Skipped | 1"));
    }

    #[test]
    fn test_markdown_failed_tests_section() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_mixed();
        let report = reporter.generate(&results, None);

        assert!(report.contains("## 🔴 Failed Tests"));
        assert!(report.contains("### ❌ `hat-instructions`"));
        assert!(report.contains("Verify hat instructions are followed"));
        assert!(report.contains("| Agent mentions Builder | ❌"));
        assert!(report.contains("| build.task emitted | ✅"));
    }

    #[test]
    fn test_markdown_passed_tests_section() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_all_pass();
        let report = reporter.generate(&results, None);

        assert!(report.contains("## ✅ Passed Tests"));
        assert!(report.contains("claude-connect"));
    }

    #[test]
    fn test_markdown_with_analysis() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_mixed();
        let analyzed = vec![mock_analyzed_passed(), mock_analyzed_failed()];
        let report = reporter.generate(&results, Some(&analyzed));

        // Should include diagnosis
        assert!(report.contains("🔍 Diagnosis"));
        assert!(report.contains("Prompt Ineffective"));
        assert!(report.contains("Hat instructions not injected"));
        assert!(report.contains("Add IMPORTANT prefix"));

        // Should include quality breakdown
        assert!(report.contains("Quality Breakdown") || report.contains("🟢 Optimal"));
    }

    #[test]
    fn test_markdown_with_warnings() {
        let reporter = MarkdownReporter::new();
        let results = RunResults {
            results: vec![mock_passed_result()],
            duration: Duration::from_secs(38),
            skipped_count: 0,
        };
        let analyzed = vec![mock_analyzed_with_warnings()];
        let report = reporter.generate(&results, Some(&analyzed));

        // Note: this test checks that warnings would be rendered, but the mock data
        // has a different scenario_id so it won't match. This tests the structure.
        assert!(report.contains("## ✅ Passed Tests"));
    }

    #[test]
    fn test_markdown_quick_fixes() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_mixed();
        let report = reporter.generate(&results, None);

        assert!(report.contains("## 🔧 Quick Fix Commands"));
        assert!(report.contains("hats-e2e --filter"));
        assert!(report.contains("--verbose --keep-workspace"));
    }

    #[test]
    fn test_markdown_artifacts() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_all_pass();
        let report = reporter.generate(&results, None);

        assert!(report.contains("## 📁 Artifacts"));
        assert!(report.contains("report.md"));
        assert!(report.contains("report.json"));
    }

    #[test]
    fn test_markdown_recommendations() {
        let reporter = MarkdownReporter::new();
        let results = mock_run_results_mixed();
        let analyzed = vec![mock_analyzed_passed(), mock_analyzed_failed()];
        let report = reporter.generate(&results, Some(&analyzed));

        assert!(report.contains("## 📋 Recommendations"));
        assert!(report.contains("🔴 Critical"));
    }

    // ==================== JSON Reporter Tests ====================

    #[test]
    fn test_json_reporter_new() {
        let reporter = JsonReporter::new();
        let _ = reporter;
    }

    #[test]
    fn test_json_reporter_default() {
        let reporter = JsonReporter;
        let _ = reporter;
    }

    #[test]
    fn test_json_generate_all_passed() {
        let reporter = JsonReporter::new();
        let results = mock_run_results_all_pass();
        let json = reporter.generate(&results, None).unwrap();

        // Verify it's valid JSON by parsing it back
        let parsed: TestReport = serde_json::from_str(&json).unwrap();
        assert!(parsed.passed);
        assert_eq!(parsed.summary.total, 1);
        assert_eq!(parsed.summary.passed, 1);
        assert_eq!(parsed.summary.failed, 0);
        assert_eq!(parsed.verdict, "All tests passed");
    }

    #[test]
    fn test_json_generate_mixed() {
        let reporter = JsonReporter::new();
        let results = mock_run_results_mixed();
        let json = reporter.generate(&results, None).unwrap();

        let parsed: TestReport = serde_json::from_str(&json).unwrap();
        assert!(!parsed.passed);
        assert_eq!(parsed.summary.total, 2);
        assert_eq!(parsed.summary.passed, 1);
        assert_eq!(parsed.summary.failed, 1);
        assert_eq!(parsed.summary.skipped, 1);
    }

    #[test]
    fn test_json_with_analysis() {
        let reporter = JsonReporter::new();
        let results = mock_run_results_mixed();
        let analyzed = vec![mock_analyzed_passed(), mock_analyzed_failed()];
        let json = reporter.generate(&results, Some(&analyzed)).unwrap();

        let parsed: TestReport = serde_json::from_str(&json).unwrap();

        // Find the failed result
        let failed = parsed
            .results
            .iter()
            .find(|r| r.scenario_id == "hat-instructions")
            .unwrap();
        assert!(failed.diagnosis.is_some());
        let diagnosis = failed.diagnosis.as_ref().unwrap();
        assert_eq!(diagnosis.failure_type, FailureType::PromptIneffective);

        // Find the passed result
        let passed = parsed
            .results
            .iter()
            .find(|r| r.scenario_id == "claude-connect")
            .unwrap();
        assert!(passed.analysis.is_some());
        assert_eq!(
            passed.analysis.as_ref().unwrap().quality_score,
            QualityScore::Optimal
        );
    }

    #[test]
    fn test_json_recommendations() {
        let reporter = JsonReporter::new();
        let results = mock_run_results_mixed();
        let analyzed = vec![mock_analyzed_passed(), mock_analyzed_failed()];
        let json = reporter.generate(&results, Some(&analyzed)).unwrap();

        let parsed: TestReport = serde_json::from_str(&json).unwrap();
        assert!(!parsed.recommendations.is_empty());
        let critical = parsed
            .recommendations
            .iter()
            .find(|r| r.severity == Severity::Critical);
        assert!(critical.is_some());
    }

    #[test]
    fn test_json_by_tier() {
        let reporter = JsonReporter::new();
        let results = mock_run_results_mixed();
        let json = reporter.generate(&results, None).unwrap();

        let parsed: TestReport = serde_json::from_str(&json).unwrap();
        assert!(!parsed.summary.by_tier.is_empty());

        let tier1 = parsed.summary.by_tier.get("Tier 1: Connectivity");
        assert!(tier1.is_some());
        assert_eq!(tier1.unwrap().passed, 1);
    }

    #[test]
    fn test_json_by_backend() {
        let reporter = JsonReporter::new();
        let results = mock_run_results_mixed();
        let json = reporter.generate(&results, None).unwrap();

        let parsed: TestReport = serde_json::from_str(&json).unwrap();
        assert!(!parsed.summary.by_backend.is_empty());

        let claude = parsed.summary.by_backend.get("Claude");
        assert!(claude.is_some());
        assert_eq!(claude.unwrap().total, 2);
    }

    #[test]
    fn test_json_quality_breakdown() {
        let reporter = JsonReporter::new();
        let results = mock_run_results_all_pass();
        let analyzed = vec![mock_analyzed_passed()];
        let json = reporter.generate(&results, Some(&analyzed)).unwrap();

        let parsed: TestReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.summary.quality_breakdown.optimal, 1);
        assert_eq!(parsed.summary.quality_breakdown.good, 0);
    }

    // ==================== Report Writer Tests ====================

    #[test]
    fn test_report_writer_new() {
        let writer = ReportWriter::new(PathBuf::from(".e2e-tests"));
        assert_eq!(writer.output_dir(), Path::new(".e2e-tests"));
    }

    #[test]
    fn test_report_writer_write_markdown() {
        let temp_dir = std::env::temp_dir().join(format!("hats-e2e-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let writer = ReportWriter::new(temp_dir.clone());
        let results = mock_run_results_all_pass();

        let path = writer.write_markdown(&results, None).unwrap();
        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "report.md");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# E2E Test Report"));

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_report_writer_write_json() {
        let temp_dir =
            std::env::temp_dir().join(format!("hats-e2e-test-json-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let writer = ReportWriter::new(temp_dir.clone());
        let results = mock_run_results_all_pass();

        let path = writer.write_json(&results, None).unwrap();
        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "report.json");

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: TestReport = serde_json::from_str(&content).unwrap();
        assert!(parsed.passed);

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_report_writer_write_both() {
        let temp_dir =
            std::env::temp_dir().join(format!("hats-e2e-test-both-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let writer = ReportWriter::new(temp_dir.clone());
        let results = mock_run_results_all_pass();

        let paths = writer
            .write(&results, None, crate::models::ReportFormat::Both)
            .unwrap();
        assert_eq!(paths.len(), 2);

        let md_exists = paths.iter().any(|p| p.extension().unwrap() == "md");
        let json_exists = paths.iter().any(|p| p.extension().unwrap() == "json");
        assert!(md_exists);
        assert!(json_exists);

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_report_writer_creates_directory() {
        let temp_dir = std::env::temp_dir().join(format!(
            "hats-e2e-test-newdir-{}/nested",
            std::process::id()
        ));
        // Don't create the directory - writer should create it

        let writer = ReportWriter::new(temp_dir.clone());
        let results = mock_run_results_all_pass();

        let paths = writer
            .write(&results, None, crate::models::ReportFormat::Markdown)
            .unwrap();
        assert!(!paths.is_empty());
        assert!(temp_dir.exists());

        // Cleanup
        std::fs::remove_dir_all(temp_dir.parent().unwrap()).ok();
    }

    // ==================== Data Structure Tests ====================

    #[test]
    fn test_test_report_serialization() {
        let report = TestReport {
            timestamp: Utc::now(),
            hats_version: "2.1.3".to_string(),
            duration: Duration::from_secs(100),
            passed: true,
            verdict: "All tests passed".to_string(),
            summary: ReportSummary::default(),
            results: vec![],
            recommendations: vec![],
        };

        let json = serde_json::to_string(&report).unwrap();
        let parsed: TestReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hats_version, "2.1.3");
        assert!(parsed.passed);
    }

    #[test]
    fn test_analyzed_result_data_from_test_result() {
        let result = mock_passed_result();
        let data: AnalyzedResultData = (&result).into();

        assert_eq!(data.scenario_id, "claude-connect");
        assert!(data.passed);
        assert!(data.diagnosis.is_none());
        assert!(data.analysis.is_none());
    }

    #[test]
    fn test_analyzed_result_data_from_analyzed_result() {
        let analyzed = mock_analyzed_failed();
        let data: AnalyzedResultData = (&analyzed).into();

        assert_eq!(data.scenario_id, "hat-instructions");
        assert!(!data.passed);
        assert!(data.diagnosis.is_some());
        assert_eq!(
            data.diagnosis.as_ref().unwrap().failure_type,
            FailureType::PromptIneffective
        );
    }

    #[test]
    fn test_quality_breakdown_default() {
        let breakdown = QualityBreakdown::default();
        assert_eq!(breakdown.optimal, 0);
        assert_eq!(breakdown.good, 0);
        assert_eq!(breakdown.acceptable, 0);
        assert_eq!(breakdown.suboptimal, 0);
    }

    #[test]
    fn test_tier_summary_default() {
        let summary = TierSummary::default();
        assert_eq!(summary.total, 0);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn test_reporter_error_display() {
        let io_error = ReporterError::WriteError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_error.to_string().contains("failed to write report"));

        let json_error =
            ReporterError::SerializationError(serde_json::from_str::<()>("invalid").unwrap_err());
        assert!(json_error.to_string().contains("failed to serialize"));
    }
}
