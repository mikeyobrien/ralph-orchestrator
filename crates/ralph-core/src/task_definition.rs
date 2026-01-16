//! Task definition types for benchmark harness.
//!
//! Defines the JSON schema for benchmark tasks, including setup, verification,
//! and metrics collection. Tasks run in isolated workspaces with their own
//! `.git` directories to avoid polluting the main repository.
//!
//! # Example
//!
//! ```
//! use ralph_core::task_definition::{TaskDefinition, TaskSuite, Verification};
//!
//! let task = TaskDefinition::builder("hello-world", "tasks/hello-world/PROMPT.md", "TASK_COMPLETE")
//!     .verification_command("python hello.py | grep -q 'Hello, World!'")
//!     .max_iterations(5)
//!     .expected_iterations(1)
//!     .complexity("simple")
//!     .build();
//!
//! assert_eq!(task.name, "hello-world");
//! assert!(task.verification.command.contains("Hello, World!"));
//! ```

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A suite of benchmark tasks loaded from a JSON file.
///
/// The suite contains multiple tasks that can be run sequentially during
/// batch benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSuite {
    /// List of task definitions.
    pub tasks: Vec<TaskDefinition>,

    /// Optional suite-level metadata.
    #[serde(default)]
    pub metadata: SuiteMetadata,
}

impl TaskSuite {
    /// Loads a task suite from a JSON file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TaskDefinitionError> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref)?;
        let suite: Self = serde_json::from_str(&content)?;
        suite.validate()?;
        Ok(suite)
    }

    /// Validates all tasks in the suite.
    pub fn validate(&self) -> Result<(), TaskDefinitionError> {
        if self.tasks.is_empty() {
            return Err(TaskDefinitionError::Validation(
                "Task suite must contain at least one task".to_string(),
            ));
        }

        for task in &self.tasks {
            task.validate()?;
        }

        // Check for duplicate names
        let mut names = std::collections::HashSet::new();
        for task in &self.tasks {
            if !names.insert(&task.name) {
                return Err(TaskDefinitionError::Validation(format!(
                    "Duplicate task name: '{}'",
                    task.name
                )));
            }
        }

        Ok(())
    }

    /// Returns tasks filtered by complexity level.
    pub fn filter_by_complexity(&self, complexity: &str) -> Vec<&TaskDefinition> {
        self.tasks
            .iter()
            .filter(|t| t.complexity == complexity)
            .collect()
    }

    /// Returns tasks filtered by tag.
    pub fn filter_by_tag(&self, tag: &str) -> Vec<&TaskDefinition> {
        self.tasks
            .iter()
            .filter(|t| t.tags.iter().any(|t| t == tag))
            .collect()
    }
}

/// Suite-level metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SuiteMetadata {
    /// Optional suite name.
    pub name: Option<String>,

    /// Optional description.
    pub description: Option<String>,

    /// Suite version.
    pub version: Option<String>,
}

/// A single benchmark task definition.
///
/// Tasks define what the agent should accomplish, how to verify success,
/// and optional setup requirements. Each task runs in an isolated workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    // ─────────────────────────────────────────────────────────────────────────
    // REQUIRED FIELDS
    // ─────────────────────────────────────────────────────────────────────────
    /// Unique task identifier (alphanumeric + hyphens).
    ///
    /// Used for recording filenames and result reporting.
    pub name: String,

    /// Path to the prompt markdown file.
    ///
    /// Relative to the task suite file or absolute path.
    pub prompt_file: String,

    /// String the agent outputs when task is complete.
    ///
    /// This is detected by the orchestration loop to terminate the task.
    pub completion_promise: String,

    /// Verification configuration for confirming task success.
    pub verification: Verification,

    // ─────────────────────────────────────────────────────────────────────────
    // OPTIONAL FIELDS
    // ─────────────────────────────────────────────────────────────────────────
    /// Human-readable description of the task.
    #[serde(default)]
    pub description: Option<String>,

    /// Task complexity level: "simple", "medium", or "complex".
    ///
    /// Used for filtering and baseline comparisons.
    #[serde(default = "default_complexity")]
    pub complexity: String,

    /// Maximum iterations before the task is considered failed.
    ///
    /// Safety limit to prevent runaway loops.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Expected number of iterations for baseline comparison.
    ///
    /// Used to calculate `iteration_delta` in results.
    #[serde(default)]
    pub expected_iterations: Option<u32>,

    /// Timeout in seconds for the entire task.
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,

    /// Setup configuration for the task workspace.
    #[serde(default)]
    pub setup: TaskSetup,

    /// Tags for filtering and categorization.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_complexity() -> String {
    "medium".to_string()
}

fn default_max_iterations() -> u32 {
    100
}

fn default_timeout_seconds() -> u64 {
    300 // 5 minutes
}

impl TaskDefinition {
    /// Creates a builder for constructing task definitions.
    pub fn builder(
        name: impl Into<String>,
        prompt_file: impl Into<String>,
        completion_promise: impl Into<String>,
    ) -> TaskDefinitionBuilder {
        TaskDefinitionBuilder::new(name, prompt_file, completion_promise)
    }

    /// Validates the task definition.
    pub fn validate(&self) -> Result<(), TaskDefinitionError> {
        // Validate name format (alphanumeric + hyphens)
        if self.name.is_empty() {
            return Err(TaskDefinitionError::MissingField("name".to_string()));
        }

        if !self
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(TaskDefinitionError::Validation(format!(
                "Task name '{}' contains invalid characters. Use alphanumeric, hyphens, or underscores only.",
                self.name
            )));
        }

        // Validate prompt_file is not empty
        if self.prompt_file.is_empty() {
            return Err(TaskDefinitionError::MissingField("prompt_file".to_string()));
        }

        // Validate completion_promise is not empty
        if self.completion_promise.is_empty() {
            return Err(TaskDefinitionError::MissingField(
                "completion_promise".to_string(),
            ));
        }

        // Validate verification command is not empty
        if self.verification.command.is_empty() {
            return Err(TaskDefinitionError::MissingField(
                "verification.command".to_string(),
            ));
        }

        // Validate complexity is valid
        if !["simple", "medium", "complex"].contains(&self.complexity.as_str()) {
            return Err(TaskDefinitionError::Validation(format!(
                "Invalid complexity '{}'. Must be one of: simple, medium, complex",
                self.complexity
            )));
        }

        Ok(())
    }

    /// Returns the iteration delta if expected_iterations is set.
    ///
    /// `delta = actual - expected` (positive means took more iterations)
    pub fn iteration_delta(&self, actual: u32) -> Option<i32> {
        self.expected_iterations
            .map(|expected| actual as i32 - expected as i32)
    }
}

/// Builder for constructing task definitions.
pub struct TaskDefinitionBuilder {
    name: String,
    prompt_file: String,
    completion_promise: String,
    verification: Verification,
    description: Option<String>,
    complexity: String,
    max_iterations: u32,
    expected_iterations: Option<u32>,
    timeout_seconds: u64,
    setup: TaskSetup,
    tags: Vec<String>,
}

impl TaskDefinitionBuilder {
    /// Creates a new builder with required fields.
    pub fn new(
        name: impl Into<String>,
        prompt_file: impl Into<String>,
        completion_promise: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            prompt_file: prompt_file.into(),
            completion_promise: completion_promise.into(),
            verification: Verification::default(),
            description: None,
            complexity: default_complexity(),
            max_iterations: default_max_iterations(),
            expected_iterations: None,
            timeout_seconds: default_timeout_seconds(),
            setup: TaskSetup::default(),
            tags: Vec::new(),
        }
    }

    /// Sets the verification command.
    pub fn verification_command(mut self, command: impl Into<String>) -> Self {
        self.verification.command = command.into();
        self
    }

    /// Sets the verification success exit code.
    pub fn verification_exit_code(mut self, code: i32) -> Self {
        self.verification.success_exit_code = code;
        self
    }

    /// Sets the full verification configuration.
    pub fn verification(mut self, verification: Verification) -> Self {
        self.verification = verification;
        self
    }

    /// Sets the task description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the complexity level.
    pub fn complexity(mut self, complexity: impl Into<String>) -> Self {
        self.complexity = complexity.into();
        self
    }

    /// Sets the maximum iterations.
    pub fn max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Sets the expected iterations for baseline comparison.
    pub fn expected_iterations(mut self, expected: u32) -> Self {
        self.expected_iterations = Some(expected);
        self
    }

    /// Sets the timeout in seconds.
    pub fn timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Sets the setup configuration.
    pub fn setup(mut self, setup: TaskSetup) -> Self {
        self.setup = setup;
        self
    }

    /// Sets the setup script.
    pub fn setup_script(mut self, script: impl Into<String>) -> Self {
        self.setup.script = Some(script.into());
        self
    }

    /// Sets the setup files.
    pub fn setup_files(mut self, files: Vec<String>) -> Self {
        self.setup.files = files;
        self
    }

    /// Adds tags.
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Adds a single tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Builds the task definition.
    pub fn build(self) -> TaskDefinition {
        TaskDefinition {
            name: self.name,
            prompt_file: self.prompt_file,
            completion_promise: self.completion_promise,
            verification: self.verification,
            description: self.description,
            complexity: self.complexity,
            max_iterations: self.max_iterations,
            expected_iterations: self.expected_iterations,
            timeout_seconds: self.timeout_seconds,
            setup: self.setup,
            tags: self.tags,
        }
    }
}

/// Verification configuration for a task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Verification {
    /// Bash command to verify task success.
    ///
    /// Runs in the task workspace after completion promise is detected.
    #[serde(default)]
    pub command: String,

    /// Exit code that indicates success (default: 0).
    #[serde(default)]
    pub success_exit_code: i32,
}

impl Verification {
    /// Creates a new verification with the given command.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            success_exit_code: 0,
        }
    }

    /// Creates a verification that expects a non-zero exit code.
    pub fn expect_failure(command: impl Into<String>, exit_code: i32) -> Self {
        Self {
            command: command.into(),
            success_exit_code: exit_code,
        }
    }
}

/// Setup configuration for task workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskSetup {
    /// Script to run before the task starts.
    ///
    /// Executed in the task workspace directory.
    #[serde(default)]
    pub script: Option<String>,

    /// Files to copy to the task workspace.
    ///
    /// Paths relative to the task suite file.
    #[serde(default)]
    pub files: Vec<String>,
}

impl TaskSetup {
    /// Returns true if there is any setup to perform.
    pub fn has_setup(&self) -> bool {
        self.script.is_some() || !self.files.is_empty()
    }
}

/// Errors that can occur when working with task definitions.
#[derive(Debug, thiserror::Error)]
pub enum TaskDefinitionError {
    /// IO error reading task file.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parse error.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Missing required field.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_definition_builder() {
        let task = TaskDefinition::builder("hello-world", "tasks/hello.md", "TASK_COMPLETE")
            .verification_command("python hello.py | grep -q 'Hello, World!'")
            .description("Create a hello world script")
            .complexity("simple")
            .max_iterations(5)
            .expected_iterations(1)
            .tag("python")
            .build();

        assert_eq!(task.name, "hello-world");
        assert_eq!(task.prompt_file, "tasks/hello.md");
        assert_eq!(task.completion_promise, "TASK_COMPLETE");
        assert!(task.verification.command.contains("Hello, World!"));
        assert_eq!(task.complexity, "simple");
        assert_eq!(task.max_iterations, 5);
        assert_eq!(task.expected_iterations, Some(1));
        assert!(task.tags.contains(&"python".to_string()));
    }

    #[test]
    fn test_task_definition_defaults() {
        let task = TaskDefinition::builder("test", "prompt.md", "DONE")
            .verification_command("echo ok")
            .build();

        assert_eq!(task.complexity, "medium");
        assert_eq!(task.max_iterations, 100);
        assert_eq!(task.timeout_seconds, 300);
        assert!(task.expected_iterations.is_none());
        assert!(task.tags.is_empty());
    }

    #[test]
    fn test_task_validation_valid() {
        let task = TaskDefinition::builder("valid-task", "prompt.md", "DONE")
            .verification_command("echo ok")
            .build();

        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_task_validation_invalid_name() {
        let task = TaskDefinition::builder("invalid task name!", "prompt.md", "DONE")
            .verification_command("echo ok")
            .build();

        let err = task.validate().unwrap_err();
        assert!(matches!(err, TaskDefinitionError::Validation(_)));
    }

    #[test]
    fn test_task_validation_empty_prompt() {
        let task = TaskDefinition::builder("test", "", "DONE")
            .verification_command("echo ok")
            .build();

        let err = task.validate().unwrap_err();
        assert!(matches!(err, TaskDefinitionError::MissingField(f) if f == "prompt_file"));
    }

    #[test]
    fn test_task_validation_empty_verification() {
        let task = TaskDefinition::builder("test", "prompt.md", "DONE").build();

        let err = task.validate().unwrap_err();
        assert!(matches!(err, TaskDefinitionError::MissingField(f) if f == "verification.command"));
    }

    #[test]
    fn test_task_validation_invalid_complexity() {
        let task = TaskDefinition::builder("test", "prompt.md", "DONE")
            .verification_command("echo ok")
            .complexity("invalid")
            .build();

        let err = task.validate().unwrap_err();
        assert!(matches!(err, TaskDefinitionError::Validation(_)));
    }

    #[test]
    fn test_iteration_delta() {
        let task = TaskDefinition::builder("test", "prompt.md", "DONE")
            .verification_command("echo ok")
            .expected_iterations(5)
            .build();

        // Took fewer iterations than expected
        assert_eq!(task.iteration_delta(3), Some(-2));

        // Took more iterations than expected
        assert_eq!(task.iteration_delta(7), Some(2));

        // Took exactly expected
        assert_eq!(task.iteration_delta(5), Some(0));
    }

    #[test]
    fn test_iteration_delta_no_expected() {
        let task = TaskDefinition::builder("test", "prompt.md", "DONE")
            .verification_command("echo ok")
            .build();

        assert!(task.iteration_delta(5).is_none());
    }

    #[test]
    fn test_task_suite_parse() {
        let json = r#"{
            "tasks": [
                {
                    "name": "hello-world",
                    "prompt_file": "tasks/hello/PROMPT.md",
                    "completion_promise": "TASK_COMPLETE",
                    "verification": {
                        "command": "python hello.py | grep -q 'Hello, World!'"
                    },
                    "complexity": "simple",
                    "max_iterations": 5,
                    "expected_iterations": 1
                },
                {
                    "name": "fizzbuzz-tdd",
                    "description": "Implement FizzBuzz with TDD",
                    "prompt_file": "tasks/fizzbuzz/PROMPT.md",
                    "completion_promise": "TESTS_PASSING",
                    "verification": {
                        "command": "pytest test_fizzbuzz.py -v"
                    },
                    "complexity": "medium",
                    "max_iterations": 15,
                    "expected_iterations": 5,
                    "setup": {
                        "files": ["test_fizzbuzz.py"]
                    },
                    "tags": ["python", "tdd"]
                }
            ],
            "metadata": {
                "name": "Ralph Benchmark Suite",
                "version": "1.0.0"
            }
        }"#;

        let suite: TaskSuite = serde_json::from_str(json).unwrap();
        assert_eq!(suite.tasks.len(), 2);

        let hello = &suite.tasks[0];
        assert_eq!(hello.name, "hello-world");
        assert_eq!(hello.complexity, "simple");
        assert_eq!(hello.max_iterations, 5);
        assert_eq!(hello.expected_iterations, Some(1));

        let fizzbuzz = &suite.tasks[1];
        assert_eq!(fizzbuzz.name, "fizzbuzz-tdd");
        assert!(fizzbuzz.description.is_some());
        assert_eq!(fizzbuzz.setup.files.len(), 1);
        assert!(fizzbuzz.tags.contains(&"tdd".to_string()));

        assert_eq!(
            suite.metadata.name,
            Some("Ralph Benchmark Suite".to_string())
        );
    }

    #[test]
    fn test_task_suite_validation_empty() {
        let suite = TaskSuite {
            tasks: vec![],
            metadata: SuiteMetadata::default(),
        };

        let err = suite.validate().unwrap_err();
        assert!(matches!(err, TaskDefinitionError::Validation(_)));
    }

    #[test]
    fn test_task_suite_validation_duplicates() {
        let task = TaskDefinition::builder("duplicate", "prompt.md", "DONE")
            .verification_command("echo ok")
            .build();

        let suite = TaskSuite {
            tasks: vec![task.clone(), task],
            metadata: SuiteMetadata::default(),
        };

        let err = suite.validate().unwrap_err();
        assert!(err.to_string().contains("Duplicate task name"));
    }

    #[test]
    fn test_filter_by_complexity() {
        let json = r#"{
            "tasks": [
                {"name": "t1", "prompt_file": "p.md", "completion_promise": "DONE", "verification": {"command": "echo ok"}, "complexity": "simple"},
                {"name": "t2", "prompt_file": "p.md", "completion_promise": "DONE", "verification": {"command": "echo ok"}, "complexity": "medium"},
                {"name": "t3", "prompt_file": "p.md", "completion_promise": "DONE", "verification": {"command": "echo ok"}, "complexity": "simple"}
            ]
        }"#;

        let suite: TaskSuite = serde_json::from_str(json).unwrap();
        let simple = suite.filter_by_complexity("simple");
        assert_eq!(simple.len(), 2);
        assert!(simple.iter().all(|t| t.complexity == "simple"));
    }

    #[test]
    fn test_filter_by_tag() {
        let json = r#"{
            "tasks": [
                {"name": "t1", "prompt_file": "p.md", "completion_promise": "DONE", "verification": {"command": "echo ok"}, "tags": ["python", "testing"]},
                {"name": "t2", "prompt_file": "p.md", "completion_promise": "DONE", "verification": {"command": "echo ok"}, "tags": ["rust"]},
                {"name": "t3", "prompt_file": "p.md", "completion_promise": "DONE", "verification": {"command": "echo ok"}, "tags": ["python"]}
            ]
        }"#;

        let suite: TaskSuite = serde_json::from_str(json).unwrap();
        let python = suite.filter_by_tag("python");
        assert_eq!(python.len(), 2);
    }

    #[test]
    fn test_setup_has_setup() {
        let empty = TaskSetup::default();
        assert!(!empty.has_setup());

        let with_script = TaskSetup {
            script: Some("setup.sh".to_string()),
            files: vec![],
        };
        assert!(with_script.has_setup());

        let with_files = TaskSetup {
            script: None,
            files: vec!["file.py".to_string()],
        };
        assert!(with_files.has_setup());
    }

    #[test]
    fn test_verification_new() {
        let v = Verification::new("pytest tests/");
        assert_eq!(v.command, "pytest tests/");
        assert_eq!(v.success_exit_code, 0);
    }

    #[test]
    fn test_verification_expect_failure() {
        let v = Verification::expect_failure("false", 1);
        assert_eq!(v.command, "false");
        assert_eq!(v.success_exit_code, 1);
    }
}
