//! Meta-Ralph analysis for E2E test results.
//!
//! This module uses Ralph itself to analyze test results and generate rich diagnostics.
//! This dogfoods Ralph and creates a self-improving feedback loop.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │  Raw Results    │────▶│  Build Prompt   │────▶│  Run Ralph      │
//! └─────────────────┘     └─────────────────┘     └─────────────────┘
//!                                                         │
//!                                                         ▼
//! ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │  Final Results  │◀────│  Merge Analysis │◀────│  Parse Event    │
//! └─────────────────┘     └─────────────────┘     └─────────────────┘
//! ```
//!
//! # Example
//!
//! ```no_run
//! use ralph_e2e::{MetaRalphAnalyzer, TestResult};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() {
//!     let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
//!
//!     // Analyze test results
//!     let raw_results: Vec<TestResult> = vec![]; // Your test results
//!     let analyzed = analyzer.analyze(&raw_results).await.unwrap();
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

use crate::executor::{PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::TestResult;

/// Errors that can occur during analysis.
#[derive(Debug, Error)]
pub enum AnalyzerError {
    /// Failed to create workspace for analysis.
    #[error("failed to create analysis workspace: {0}")]
    WorkspaceError(#[from] std::io::Error),

    /// Failed to execute Ralph for analysis.
    #[error("failed to execute analyzer: {0}")]
    ExecutionError(#[from] crate::executor::ExecutorError),

    /// Failed to parse analysis from Ralph output.
    #[error("failed to parse analysis: {0}")]
    ParseError(String),

    /// Analysis timed out.
    #[error("analysis timed out")]
    Timeout,

    /// No analysis event found in output.
    #[error("no analyze.complete event found in output")]
    NoAnalysisEvent,
}

/// Configuration for the analyzer.
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Timeout for the analysis run.
    pub timeout: Duration,

    /// Maximum iterations for analysis.
    pub max_iterations: u32,

    /// Backend to use for analysis (defaults to claude).
    pub backend: String,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            max_iterations: 1,
            backend: "claude".to_string(),
        }
    }
}

/// Quality score for passed tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum QualityScore {
    /// Fast, efficient, exactly as expected.
    Optimal,
    /// Passed cleanly, minor room for improvement.
    #[default]
    Good,
    /// Passed but with warnings or inefficiencies.
    Acceptable,
    /// Passed but needs attention (slow, wasteful, etc.).
    Suboptimal,
}

/// Failure type classification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FailureType {
    /// CLI tool returned error.
    BackendError,
    /// Agent didn't follow instructions.
    PromptIneffective,
    /// Expected event not emitted.
    EventMissing,
    /// Event emitted but wrong format.
    EventMalformed,
    /// Took too long.
    TimeoutExceeded,
    /// Loop ended early.
    UnexpectedTermination,
    /// Output didn't match expected.
    AssertionMismatch,
    /// Bad ralph.yml.
    ConfigurationError,
    /// Credentials issue.
    AuthenticationError,
    /// Unknown failure.
    #[default]
    Unknown,
}

/// AI-friendly diagnosis for failures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Diagnosis {
    /// Classification of the failure.
    #[serde(default)]
    pub failure_type: FailureType,

    /// Hypothesis about the root cause.
    #[serde(default)]
    pub root_cause_hypothesis: String,

    /// Evidence supporting the hypothesis.
    #[serde(default)]
    pub evidence: Vec<String>,

    /// Other tests with similar failures.
    #[serde(default)]
    pub similar_failures: Vec<String>,

    /// Suggested next steps for investigation.
    #[serde(default)]
    pub suggested_investigations: Vec<String>,

    /// Potential fixes with confidence scores.
    #[serde(default)]
    pub potential_fixes: Vec<PotentialFix>,
}

/// A potential fix suggestion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PotentialFix {
    /// Description of the fix.
    #[serde(default)]
    pub description: String,

    /// Confidence score (0.0 - 1.0).
    #[serde(default)]
    pub confidence: f32,

    /// File that may need modification.
    #[serde(default)]
    pub file_to_modify: Option<String>,

    /// Suggested code change.
    #[serde(default)]
    pub suggested_change: Option<String>,
}

/// AI-friendly analysis for passed tests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PassedAnalysis {
    /// Overall quality assessment.
    #[serde(default)]
    pub quality_score: QualityScore,

    /// Performance metrics.
    #[serde(default)]
    pub metrics: TestMetrics,

    /// Warnings about potential issues.
    #[serde(default)]
    pub warnings: Vec<Warning>,

    /// Optimization opportunities.
    #[serde(default)]
    pub optimizations: Vec<Optimization>,
}

/// Performance metrics for a test.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestMetrics {
    /// Execution duration in seconds.
    #[serde(default)]
    pub duration_seconds: f64,

    /// Number of iterations used.
    #[serde(default)]
    pub iterations_used: u32,

    /// Expected number of iterations.
    #[serde(default)]
    pub iterations_expected: u32,

    /// Estimated token usage.
    #[serde(default)]
    pub tokens_estimated: Option<u64>,

    /// Number of events emitted.
    #[serde(default)]
    pub events_emitted: u32,

    /// Number of tool calls.
    #[serde(default)]
    pub tool_calls: u32,

    /// Number of internal retries.
    #[serde(default)]
    pub retries_needed: u32,
}

/// Warning about a test.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Warning {
    /// Warning category.
    #[serde(default)]
    pub category: WarningCategory,

    /// Warning message.
    #[serde(default)]
    pub message: String,

    /// Evidence for the warning.
    #[serde(default)]
    pub evidence: String,
}

/// Categories of warnings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum WarningCategory {
    /// Took longer than expected.
    SlowExecution,
    /// Used more iterations than needed.
    ExcessiveIterations,
    /// Agent needed multiple attempts.
    PromptStruggle,
    /// Output had anomalies but test passed.
    UnexpectedOutput,
    /// Used deprecated feature.
    DeprecationUsed,
    /// High token/API usage.
    ResourceIntensive,
    /// Other warning.
    #[default]
    Other,
}

/// Optimization suggestion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Optimization {
    /// Category of optimization.
    #[serde(default)]
    pub category: String,

    /// Description of the optimization.
    #[serde(default)]
    pub description: String,

    /// Potential improvement (e.g., "Could reduce iterations from 5 to 2").
    #[serde(default)]
    pub potential_improvement: String,

    /// Suggested change to implement the optimization.
    #[serde(default)]
    pub suggested_change: Option<String>,
}

/// Full analysis response from meta-Ralph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisResponse {
    /// Analysis for failed tests.
    pub failed_analyses: Vec<FailedAnalysis>,

    /// Analysis for passed tests.
    pub passed_analyses: Vec<PassedTestAnalysis>,

    /// Patterns detected across tests.
    pub patterns: Vec<Pattern>,

    /// Overall recommendations.
    pub recommendations: Vec<Recommendation>,
}

/// Analysis for a single failed test.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FailedAnalysis {
    /// Scenario ID.
    pub scenario_id: String,

    /// Diagnosis details.
    #[serde(flatten)]
    pub diagnosis: Diagnosis,
}

/// Analysis for a single passed test.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PassedTestAnalysis {
    /// Scenario ID.
    pub scenario_id: String,

    /// Analysis details.
    #[serde(flatten)]
    pub analysis: PassedAnalysis,
}

/// A pattern detected across multiple tests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pattern {
    /// Description of the pattern.
    pub description: String,

    /// Tests affected by this pattern.
    pub affected_tests: Vec<String>,

    /// Suggested fix for the pattern.
    pub suggested_fix: Option<String>,
}

/// A recommendation from the analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Recommendation {
    /// Severity level.
    pub severity: Severity,

    /// Category of recommendation.
    pub category: String,

    /// Short title.
    pub title: String,

    /// Detailed description.
    pub description: String,

    /// Affected tests.
    #[serde(default)]
    pub affected_tests: Vec<String>,
}

/// Severity levels for recommendations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Severity {
    /// Must fix immediately.
    Critical,
    /// Should fix soon.
    Warning,
    /// Informational.
    #[default]
    Info,
}

/// Analyzed test result with diagnosis or optimization analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedResult {
    /// Original test result.
    pub result: TestResult,

    /// Diagnosis for failed tests.
    pub diagnosis: Option<Diagnosis>,

    /// Analysis for passed tests.
    pub analysis: Option<PassedAnalysis>,
}

/// Meta-Ralph analyzer for E2E test results.
///
/// Uses Ralph itself to analyze test results and generate rich diagnostics.
pub struct MetaRalphAnalyzer {
    /// Base workspace for analysis.
    workspace: PathBuf,

    /// Analyzer configuration.
    config: AnalyzerConfig,
}

impl MetaRalphAnalyzer {
    /// Creates a new analyzer with the given workspace.
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            config: AnalyzerConfig::default(),
        }
    }

    /// Creates a new analyzer with custom configuration.
    pub fn with_config(workspace: PathBuf, config: AnalyzerConfig) -> Self {
        Self { workspace, config }
    }

    /// Returns the workspace path.
    pub fn workspace(&self) -> &PathBuf {
        &self.workspace
    }

    /// Returns the analyzer configuration.
    pub fn config(&self) -> &AnalyzerConfig {
        &self.config
    }

    /// Builds the analysis prompt from raw test results.
    pub fn build_analysis_prompt(&self, results: &[TestResult]) -> String {
        let mut prompt = String::new();

        // Header
        prompt.push_str("# E2E Test Analysis Request\n\n");
        prompt.push_str("Analyze these test results and provide structured feedback.\n\n");

        // Summary
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        prompt.push_str("## Test Run Summary\n");
        prompt.push_str(&format!("- Total: {} tests\n", total));
        prompt.push_str(&format!("- Passed: {}\n", passed));
        prompt.push_str(&format!("- Failed: {}\n\n", failed));

        // Failed tests section
        let failed_results: Vec<_> = results.iter().filter(|r| !r.passed).collect();
        if !failed_results.is_empty() {
            prompt.push_str("## Failed Tests\n\n");
            for result in &failed_results {
                prompt.push_str(&format!("### {} (FAILED)\n\n", result.scenario_id));
                prompt.push_str(&format!(
                    "**Description:** {}\n",
                    result.scenario_description
                ));
                prompt.push_str(&format!("**Backend:** {}\n", result.backend));
                prompt.push_str(&format!("**Tier:** {}\n", result.tier));
                prompt.push_str(&format!(
                    "**Duration:** {:.1}s\n\n",
                    result.duration.as_secs_f64()
                ));

                prompt.push_str("**Assertions:**\n");
                prompt.push_str("| Name | Expected | Actual | Passed |\n");
                prompt.push_str("|------|----------|--------|--------|\n");
                for assertion in &result.assertions {
                    let status = if assertion.passed { "✅" } else { "❌" };
                    prompt.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        assertion.name, assertion.expected, assertion.actual, status
                    ));
                }
                prompt.push_str("\n---\n\n");
            }
        }

        // Passed tests section
        let passed_results: Vec<_> = results.iter().filter(|r| r.passed).collect();
        if !passed_results.is_empty() {
            prompt.push_str("## Passed Tests (Analyze for Optimization)\n\n");
            for result in &passed_results {
                prompt.push_str(&format!("### {} (PASSED)\n\n", result.scenario_id));
                prompt.push_str(&format!(
                    "**Duration:** {:.1}s\n",
                    result.duration.as_secs_f64()
                ));
                prompt.push_str(&format!("**Tier:** {}\n\n", result.tier));
                prompt.push_str("---\n\n");
            }
        }

        // Instructions
        prompt.push_str("## Your Task\n\n");
        prompt.push_str("Provide analysis as JSON inside an event:\n\n");
        prompt.push_str("```json\n");
        prompt.push_str(r#"{
  "failed_analyses": [
    {
      "scenario_id": "...",
      "failure_type": "PromptIneffective|BackendError|EventMissing|...",
      "root_cause_hypothesis": "...",
      "evidence": ["...", "..."],
      "suggested_investigations": ["...", "..."],
      "potential_fixes": [
        {"description": "...", "confidence": 0.8, "file_to_modify": "...", "suggested_change": "..."}
      ]
    }
  ],
  "passed_analyses": [
    {
      "scenario_id": "...",
      "quality_score": "Optimal|Good|Acceptable|Suboptimal",
      "warnings": [{"category": "...", "message": "...", "evidence": "..."}],
      "optimizations": [{"description": "...", "potential_improvement": "...", "suggested_change": "..."}]
    }
  ],
  "patterns": [
    {"description": "Multiple tests show X", "affected_tests": ["...", "..."], "suggested_fix": "..."}
  ],
  "recommendations": [
    {"severity": "Critical|Warning|Info", "category": "...", "title": "...", "description": "..."}
  ]
}
"#);
        prompt.push_str("```\n\n");
        prompt.push_str("Emit your analysis:\n");
        prompt.push_str("<event topic=\"analyze.complete\">{...your JSON...}</event>\n\n");
        prompt.push_str("Then output: ANALYSIS_COMPLETE\n");

        prompt
    }

    /// Generates the embedded analyzer config YAML.
    pub fn generate_analyzer_config(&self) -> String {
        format!(
            r#"cli:
  backend: {backend}

event_loop:
  max_iterations: {max_iter}
  completion_promise: "ANALYSIS_COMPLETE"

hats:
  analyzer:
    name: "E2E Analyzer"
    triggers: ["analyze.request"]
    publishes: ["analyze.complete"]
    instructions: |
      You are the E2E Test Analyzer. Your job is to analyze test results and provide:

      ## For FAILED tests:
      1. Failure type classification (BackendError, PromptIneffective, EventMissing, etc.)
      2. Root cause hypothesis with evidence
      3. Suggested investigations
      4. Potential fixes with confidence scores (0.0-1.0)

      ## For PASSED tests:
      1. Quality score: Optimal, Good, Acceptable, or Suboptimal
      2. Warnings (slow, excessive iterations, agent struggled)
      3. Optimization opportunities

      ## For ALL tests:
      - Look for patterns across multiple tests
      - Identify systemic issues vs one-off problems
      - Prioritize recommendations by impact

      Output your analysis as structured JSON inside an event:
      <event topic="analyze.complete">{{...analysis JSON...}}</event>

      Then output: ANALYSIS_COMPLETE
"#,
            backend = self.config.backend,
            max_iter = self.config.max_iterations,
        )
    }

    /// Parses the analysis response from Ralph output.
    pub fn parse_analysis_event(&self, output: &str) -> Result<AnalysisResponse, AnalyzerError> {
        // Look for the analyze.complete event
        let event_regex =
            regex::Regex::new(r#"<event\s+topic="analyze\.complete">([\s\S]*?)</event>"#)
                .map_err(|e| AnalyzerError::ParseError(e.to_string()))?;

        let captures = event_regex
            .captures(output)
            .ok_or(AnalyzerError::NoAnalysisEvent)?;

        let json_str = captures
            .get(1)
            .ok_or(AnalyzerError::NoAnalysisEvent)?
            .as_str()
            .trim();

        // Parse the JSON
        serde_json::from_str(json_str)
            .map_err(|e| AnalyzerError::ParseError(format!("JSON parse error: {}", e)))
    }

    /// Merges raw results with analysis.
    pub fn merge_results(
        &self,
        results: &[TestResult],
        analysis: &AnalysisResponse,
    ) -> Vec<AnalyzedResult> {
        results
            .iter()
            .map(|result| {
                let diagnosis = analysis
                    .failed_analyses
                    .iter()
                    .find(|a| a.scenario_id == result.scenario_id)
                    .map(|a| a.diagnosis.clone());

                let passed_analysis = analysis
                    .passed_analyses
                    .iter()
                    .find(|a| a.scenario_id == result.scenario_id)
                    .map(|a| a.analysis.clone());

                AnalyzedResult {
                    result: result.clone(),
                    diagnosis,
                    analysis: passed_analysis,
                }
            })
            .collect()
    }

    /// Runs the full analysis on test results.
    ///
    /// This creates a workspace, runs Ralph with the analyzer hat,
    /// parses the output, and returns analyzed results.
    pub async fn analyze(
        &self,
        results: &[TestResult],
    ) -> Result<Vec<AnalyzedResult>, AnalyzerError> {
        // If no results, return empty
        if results.is_empty() {
            return Ok(vec![]);
        }

        // Create analysis workspace
        let analysis_workspace = self.workspace.join("_analysis");
        std::fs::create_dir_all(&analysis_workspace)?;
        std::fs::create_dir_all(analysis_workspace.join(".agent"))?;

        // Write the analyzer config
        let config_path = analysis_workspace.join("ralph-analyzer.yml");
        std::fs::write(&config_path, self.generate_analyzer_config())?;

        // Build and write the analysis prompt
        let prompt = self.build_analysis_prompt(results);
        let prompt_path = analysis_workspace.join("analysis-prompt.md");
        std::fs::write(&prompt_path, &prompt)?;

        // Create executor and config
        let executor = RalphExecutor::new(analysis_workspace.clone());
        let scenario_config = ScenarioConfig {
            config_file: PathBuf::from("ralph-analyzer.yml"),
            prompt: PromptSource::File(PathBuf::from("analysis-prompt.md")),
            max_iterations: self.config.max_iterations,
            timeout: self.config.timeout,
            extra_args: vec![],
        };

        // Run Ralph
        let exec_result = executor.run(&scenario_config).await?;

        // Check for timeout
        if exec_result.timed_out {
            return Err(AnalyzerError::Timeout);
        }

        // Parse the analysis from output
        let analysis = self.parse_analysis_event(&exec_result.stdout)?;

        // Merge and return
        Ok(self.merge_results(results, &analysis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Assertion;
    use std::time::Duration;

    fn mock_failed_result() -> TestResult {
        TestResult {
            scenario_id: "hat-instructions".to_string(),
            scenario_description: "Verify hat-specific instructions are followed".to_string(),
            backend: "claude".to_string(),
            tier: "Tier 5: Hat Collections".to_string(),
            passed: false,
            assertions: vec![
                Assertion {
                    name: "Agent mentions Builder role".to_string(),
                    passed: false,
                    expected: "Contains 'I am the Builder'".to_string(),
                    actual: "No mention of Builder role".to_string(),
                },
                Assertion {
                    name: "build.task event emitted".to_string(),
                    passed: true,
                    expected: "Event present".to_string(),
                    actual: "Event found".to_string(),
                },
            ],
            duration: Duration::from_secs_f64(45.2),
        }
    }

    fn mock_passed_result() -> TestResult {
        TestResult {
            scenario_id: "claude-connect".to_string(),
            scenario_description: "Basic connectivity test for Claude".to_string(),
            backend: "claude".to_string(),
            tier: "Tier 1: Connectivity".to_string(),
            passed: true,
            assertions: vec![
                Assertion {
                    name: "Response received".to_string(),
                    passed: true,
                    expected: "Non-empty stdout".to_string(),
                    actual: "stdout has content".to_string(),
                },
                Assertion {
                    name: "Exit code is 0".to_string(),
                    passed: true,
                    expected: "0".to_string(),
                    actual: "0".to_string(),
                },
            ],
            duration: Duration::from_secs_f64(12.3),
        }
    }

    #[test]
    fn test_analyzer_new() {
        let workspace = PathBuf::from(".e2e-tests");
        let analyzer = MetaRalphAnalyzer::new(workspace.clone());
        assert_eq!(analyzer.workspace(), &workspace);
        assert_eq!(analyzer.config().timeout, Duration::from_secs(120));
        assert_eq!(analyzer.config().max_iterations, 1);
        assert_eq!(analyzer.config().backend, "claude");
    }

    #[test]
    fn test_analyzer_with_config() {
        let workspace = PathBuf::from(".e2e-tests");
        let config = AnalyzerConfig {
            timeout: Duration::from_secs(60),
            max_iterations: 2,
            backend: "kiro".to_string(),
        };
        let analyzer = MetaRalphAnalyzer::with_config(workspace.clone(), config);
        assert_eq!(analyzer.config().timeout, Duration::from_secs(60));
        assert_eq!(analyzer.config().max_iterations, 2);
        assert_eq!(analyzer.config().backend, "kiro");
    }

    #[test]
    fn test_build_analysis_prompt_empty() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let prompt = analyzer.build_analysis_prompt(&[]);
        assert!(prompt.contains("# E2E Test Analysis Request"));
        assert!(prompt.contains("Total: 0 tests"));
        assert!(prompt.contains("Passed: 0"));
        assert!(prompt.contains("Failed: 0"));
    }

    #[test]
    fn test_build_analysis_prompt_with_results() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let results = vec![mock_failed_result(), mock_passed_result()];
        let prompt = analyzer.build_analysis_prompt(&results);

        assert!(prompt.contains("Total: 2 tests"));
        assert!(prompt.contains("Passed: 1"));
        assert!(prompt.contains("Failed: 1"));
        assert!(prompt.contains("hat-instructions (FAILED)"));
        assert!(prompt.contains("claude-connect (PASSED)"));
        assert!(prompt.contains("Tier 5: Hat Collections"));
        assert!(prompt.contains("analyze.complete"));
    }

    #[test]
    fn test_build_analysis_prompt_includes_assertions() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let results = vec![mock_failed_result()];
        let prompt = analyzer.build_analysis_prompt(&results);

        assert!(prompt.contains("Agent mentions Builder role"));
        assert!(prompt.contains("Contains 'I am the Builder'"));
        assert!(prompt.contains("No mention of Builder role"));
    }

    #[test]
    fn test_generate_analyzer_config() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let config = analyzer.generate_analyzer_config();

        assert!(config.contains("backend: claude"));
        assert!(config.contains("max_iterations: 1"));
        assert!(config.contains("completion_promise: \"ANALYSIS_COMPLETE\""));
        assert!(config.contains("E2E Analyzer"));
        assert!(config.contains("analyze.request"));
        assert!(config.contains("analyze.complete"));
    }

    #[test]
    fn test_generate_analyzer_config_custom() {
        let config = AnalyzerConfig {
            timeout: Duration::from_secs(60),
            max_iterations: 3,
            backend: "kiro".to_string(),
        };
        let analyzer = MetaRalphAnalyzer::with_config(PathBuf::from(".e2e-tests"), config);
        let yaml = analyzer.generate_analyzer_config();

        assert!(yaml.contains("backend: kiro"));
        assert!(yaml.contains("max_iterations: 3"));
    }

    #[test]
    fn test_parse_analysis_event_success() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let output = r#"
Some output here...

<event topic="analyze.complete">
{
  "failed_analyses": [
    {
      "scenario_id": "hat-instructions",
      "failure_type": "PromptIneffective",
      "root_cause_hypothesis": "Hat instructions not injected",
      "evidence": ["No Builder mention in output"],
      "suggested_investigations": ["Check prompt building"],
      "potential_fixes": [
        {"description": "Add IMPORTANT prefix", "confidence": 0.8}
      ]
    }
  ],
  "passed_analyses": [
    {
      "scenario_id": "claude-connect",
      "quality_score": "Optimal",
      "warnings": [],
      "optimizations": []
    }
  ],
  "patterns": [],
  "recommendations": [
    {"severity": "Critical", "category": "prompt", "title": "Fix hat instructions", "description": "Review prompt building"}
  ]
}
</event>

ANALYSIS_COMPLETE
"#;

        let analysis = analyzer.parse_analysis_event(output).unwrap();
        assert_eq!(analysis.failed_analyses.len(), 1);
        assert_eq!(analysis.passed_analyses.len(), 1);
        assert_eq!(analysis.failed_analyses[0].scenario_id, "hat-instructions");
        assert_eq!(
            analysis.failed_analyses[0].diagnosis.failure_type,
            FailureType::PromptIneffective
        );
        assert_eq!(
            analysis.passed_analyses[0].analysis.quality_score,
            QualityScore::Optimal
        );
    }

    #[test]
    fn test_parse_analysis_event_no_event() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let output = "No event here";

        let result = analyzer.parse_analysis_event(output);
        assert!(matches!(result, Err(AnalyzerError::NoAnalysisEvent)));
    }

    #[test]
    fn test_parse_analysis_event_invalid_json() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let output = r#"<event topic="analyze.complete">not valid json</event>"#;

        let result = analyzer.parse_analysis_event(output);
        assert!(matches!(result, Err(AnalyzerError::ParseError(_))));
    }

    #[test]
    fn test_merge_results_empty() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let results: Vec<TestResult> = vec![];
        let analysis = AnalysisResponse::default();

        let merged = analyzer.merge_results(&results, &analysis);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_merge_results_with_diagnosis() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let results = vec![mock_failed_result()];
        let analysis = AnalysisResponse {
            failed_analyses: vec![FailedAnalysis {
                scenario_id: "hat-instructions".to_string(),
                diagnosis: Diagnosis {
                    failure_type: FailureType::PromptIneffective,
                    root_cause_hypothesis: "Instructions not followed".to_string(),
                    evidence: vec!["No Builder in output".to_string()],
                    ..Default::default()
                },
            }],
            ..Default::default()
        };

        let merged = analyzer.merge_results(&results, &analysis);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].diagnosis.is_some());
        assert!(merged[0].analysis.is_none());
        assert_eq!(
            merged[0].diagnosis.as_ref().unwrap().failure_type,
            FailureType::PromptIneffective
        );
    }

    #[test]
    fn test_merge_results_with_passed_analysis() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let results = vec![mock_passed_result()];
        let analysis = AnalysisResponse {
            passed_analyses: vec![PassedTestAnalysis {
                scenario_id: "claude-connect".to_string(),
                analysis: PassedAnalysis {
                    quality_score: QualityScore::Optimal,
                    ..Default::default()
                },
            }],
            ..Default::default()
        };

        let merged = analyzer.merge_results(&results, &analysis);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].diagnosis.is_none());
        assert!(merged[0].analysis.is_some());
        assert_eq!(
            merged[0].analysis.as_ref().unwrap().quality_score,
            QualityScore::Optimal
        );
    }

    #[test]
    fn test_merge_results_no_matching_analysis() {
        let analyzer = MetaRalphAnalyzer::new(PathBuf::from(".e2e-tests"));
        let results = vec![mock_failed_result()];
        let analysis = AnalysisResponse::default(); // No analyses

        let merged = analyzer.merge_results(&results, &analysis);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].diagnosis.is_none());
        assert!(merged[0].analysis.is_none());
    }

    #[test]
    fn test_quality_score_serialization() {
        let optimal = QualityScore::Optimal;
        let json = serde_json::to_string(&optimal).unwrap();
        assert_eq!(json, "\"Optimal\"");

        let parsed: QualityScore = serde_json::from_str("\"Good\"").unwrap();
        assert_eq!(parsed, QualityScore::Good);
    }

    #[test]
    fn test_failure_type_serialization() {
        let failure = FailureType::PromptIneffective;
        let json = serde_json::to_string(&failure).unwrap();
        assert_eq!(json, "\"PromptIneffective\"");

        let parsed: FailureType = serde_json::from_str("\"BackendError\"").unwrap();
        assert_eq!(parsed, FailureType::BackendError);
    }

    #[test]
    fn test_severity_serialization() {
        let critical = Severity::Critical;
        let json = serde_json::to_string(&critical).unwrap();
        assert_eq!(json, "\"Critical\"");

        let parsed: Severity = serde_json::from_str("\"Warning\"").unwrap();
        assert_eq!(parsed, Severity::Warning);
    }

    #[test]
    fn test_warning_category_default() {
        let warning = Warning::default();
        assert_eq!(warning.category, WarningCategory::Other);
    }

    #[test]
    fn test_analyzed_result_serialization() {
        let result = AnalyzedResult {
            result: mock_passed_result(),
            diagnosis: None,
            analysis: Some(PassedAnalysis {
                quality_score: QualityScore::Optimal,
                ..Default::default()
            }),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("claude-connect"));
        assert!(json.contains("Optimal"));
    }

    #[test]
    fn test_potential_fix_serialization() {
        let fix = PotentialFix {
            description: "Add IMPORTANT prefix".to_string(),
            confidence: 0.85,
            file_to_modify: Some("src/hatless_ralph.rs".to_string()),
            suggested_change: Some("Wrap instructions".to_string()),
        };

        let json = serde_json::to_string(&fix).unwrap();
        assert!(json.contains("Add IMPORTANT prefix"));
        assert!(json.contains("0.85"));
        assert!(json.contains("src/hatless_ralph.rs"));
    }

    #[test]
    fn test_recommendation_with_affected_tests() {
        let rec = Recommendation {
            severity: Severity::Critical,
            category: "prompt".to_string(),
            title: "Fix hat instructions".to_string(),
            description: "Hat instructions are not being followed".to_string(),
            affected_tests: vec!["hat-instructions".to_string(), "hat-single".to_string()],
        };

        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"severity\":\"Critical\""));
        assert!(json.contains("hat-instructions"));
        assert!(json.contains("hat-single"));
    }

    // Integration test - requires ralph binary
    #[tokio::test]
    #[ignore = "requires ralph binary"]
    async fn test_analyzer_integration() {
        use std::env;

        let workspace =
            env::temp_dir().join(format!("ralph-e2e-analyzer-test-{}", std::process::id()));
        std::fs::create_dir_all(&workspace).unwrap();

        let analyzer = MetaRalphAnalyzer::new(workspace.clone());
        let results = vec![mock_failed_result(), mock_passed_result()];

        let analyzed = analyzer.analyze(&results).await;

        // Clean up
        std::fs::remove_dir_all(&workspace).ok();

        // Verify
        let analyzed = analyzed.expect("analysis should complete");
        assert_eq!(analyzed.len(), 2);
    }
}
