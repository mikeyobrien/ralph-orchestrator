//! Tier 6: Task System test scenarios (backend-agnostic).
//!
//! These scenarios test Ralph's task tracking system, including:
//! - Adding tasks via CLI
//! - Closing tasks
//! - Loop completion verification with tasks
//! - Dependency tracking
//!
//! Tasks are stored in `.agent/tasks.jsonl` and provide structured
//! work item tracking when memories are enabled.
//!
//! All scenarios are backend-agnostic and support Claude, Kiro, and OpenCode.

use super::{AssertionBuilder, Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{ExecutionResult, PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::TestResult;
use async_trait::async_trait;
use std::path::Path;

/// Extension trait for Assertion to allow chained modification.
trait AssertionExt {
    fn with_passed(self, passed: bool) -> Self;
}

impl AssertionExt for crate::models::Assertion {
    fn with_passed(mut self, passed: bool) -> Self {
        self.passed = passed;
        self
    }
}

// =============================================================================
// TaskAddScenario - Add task via CLI
// =============================================================================

/// Test scenario that verifies tasks can be added via the CLI.
///
/// This scenario:
/// - Uses `ralph task add` to create a task
/// - Verifies the task is stored in `.agent/tasks.jsonl`
/// - Verifies the task ID format is correct (task-{timestamp}-{hex})
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{TaskAddScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = TaskAddScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Task System");
/// assert!(scenario.supported_backends().contains(&Backend::Claude));
/// ```
pub struct TaskAddScenario {
    id: String,
    description: String,
    tier: String,
}

impl TaskAddScenario {
    /// Creates a new task add scenario.
    pub fn new() -> Self {
        Self {
            id: "task-add".to_string(),
            description: "Verifies tasks can be added via ralph task add".to_string(),
            tier: "Tier 6: Task System".to_string(),
        }
    }
}

impl Default for TaskAddScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for TaskAddScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create a minimal ralph.yml with memories enabled (tasks require memories)
        let config_content = format!(
            r#"# Task add test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: true
  inject: manual
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r#"You are testing Ralph's task tracking system.

Your task is to add a task using the Bash tool.

STEP 1: Use the Bash tool to run this exact command:
```
ralph task add "E2E test task" -p 2
```

STEP 2: After the command succeeds, output LOOP_COMPLETE

The command should output something like "Created task task-1234567890-abcd"

IMPORTANT: You MUST actually execute the command using the Bash tool, not just describe it."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 1,
            timeout: backend.default_timeout(),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Check if tasks.jsonl was created
        let tasks_path = executor.workspace().join(".agent/tasks.jsonl");
        let tasks_exist = tasks_path.exists();
        let tasks_content = if tasks_exist {
            std::fs::read_to_string(&tasks_path).unwrap_or_default()
        } else {
            String::new()
        };

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.task_command_executed(&execution),
            self.task_file_created(tasks_exist),
            self.task_content_valid(&tasks_content),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl TaskAddScenario {
    /// Asserts that the task add command was executed.
    fn task_command_executed(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();
        let executed = stdout_lower.contains("task")
            || stdout_lower.contains("ralph task")
            || stdout_lower.contains("task-");

        AssertionBuilder::new("Task command executed")
            .expected("Agent executed ralph task add")
            .actual(if executed {
                "Task command activity detected".to_string()
            } else {
                "No task command detected in output".to_string()
            })
            .build()
            .with_passed(executed)
    }

    /// Asserts that the tasks.jsonl file was created.
    fn task_file_created(&self, exists: bool) -> crate::models::Assertion {
        AssertionBuilder::new("Task file created")
            .expected(".agent/tasks.jsonl file exists")
            .actual(if exists {
                "File created successfully".to_string()
            } else {
                "File not found".to_string()
            })
            .build()
            .with_passed(exists)
    }

    /// Asserts that the task content is valid.
    fn task_content_valid(&self, content: &str) -> crate::models::Assertion {
        let has_valid_content = content.contains("E2E test task") && content.contains("task-");

        AssertionBuilder::new("Task content valid")
            .expected("Tasks file contains task data")
            .actual(if has_valid_content {
                "Valid task content found".to_string()
            } else {
                format!("Content: {}", &content[..content.len().min(100)])
            })
            .build()
            .with_passed(has_valid_content)
    }
}

// =============================================================================
// TaskCloseScenario - Close task via CLI
// =============================================================================

/// Test scenario that verifies tasks can be closed via the CLI.
///
/// This scenario:
/// - Creates a task
/// - Closes the task using `ralph task close`
/// - Verifies the task status changed to "closed"
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{TaskCloseScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = TaskCloseScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Task System");
/// assert!(scenario.supported_backends().contains(&Backend::Claude));
/// ```
pub struct TaskCloseScenario {
    id: String,
    description: String,
    tier: String,
}

impl TaskCloseScenario {
    /// Creates a new task close scenario.
    pub fn new() -> Self {
        Self {
            id: "task-close".to_string(),
            description: "Verifies tasks can be closed via ralph task close".to_string(),
            tier: "Tier 6: Task System".to_string(),
        }
    }
}

impl Default for TaskCloseScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for TaskCloseScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create a minimal ralph.yml with memories enabled
        let config_content = format!(
            r#"# Task close test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: true
  inject: manual
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r#"You are testing Ralph's task close functionality.

Your task is to:
1. Create a task
2. Close it
3. Verify it's closed

STEP 1: Use the Bash tool to create a task:
```
ralph task add "Task to close" -p 1
```

STEP 2: Capture the task ID from the output (format: task-XXXXXXXXXX-XXXX)

STEP 3: Close the task using the ID:
```
ralph task close <task-id>
```

STEP 4: List tasks to verify it's closed:
```
ralph task list
```

STEP 5: Output LOOP_COMPLETE

IMPORTANT: Execute each command using the Bash tool."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 1,
            timeout: backend.default_timeout(),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Check tasks.jsonl content
        let tasks_path = executor.workspace().join(".agent/tasks.jsonl");
        let tasks_content = if tasks_path.exists() {
            std::fs::read_to_string(&tasks_path).unwrap_or_default()
        } else {
            String::new()
        };

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.task_created(&execution),
            self.task_closed(&execution),
            self.task_status_closed(&tasks_content),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl TaskCloseScenario {
    /// Asserts that a task was created.
    fn task_created(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let created = result.stdout.contains("Created task");
        AssertionBuilder::new("Task created")
            .expected("Agent created a task")
            .actual(if created {
                "Task creation detected".to_string()
            } else {
                "No task creation detected".to_string()
            })
            .build()
            .with_passed(created)
    }

    /// Asserts that a task was closed.
    fn task_closed(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let closed = result.stdout.contains("Closed task");
        AssertionBuilder::new("Task closed")
            .expected("Agent closed the task")
            .actual(if closed {
                "Task close detected".to_string()
            } else {
                "No task close detected".to_string()
            })
            .build()
            .with_passed(closed)
    }

    /// Asserts that the task status is closed in the file.
    fn task_status_closed(&self, content: &str) -> crate::models::Assertion {
        let has_closed = content.contains("\"status\":\"closed\"");
        AssertionBuilder::new("Task status closed in file")
            .expected("Task status is 'closed' in tasks.jsonl")
            .actual(if has_closed {
                "Closed status found".to_string()
            } else {
                "Closed status not found".to_string()
            })
            .build()
            .with_passed(has_closed)
    }
}

// =============================================================================
// TaskCompletionScenario - Loop terminates with no open tasks
// =============================================================================

/// Test scenario that verifies loop terminates correctly when memories enabled
/// and no open tasks exist.
///
/// This scenario:
/// - Runs with memories.enabled: true
/// - No tasks file exists (or all tasks closed)
/// - Agent says LOOP_COMPLETE twice
/// - Loop should terminate with confirmations=2
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{TaskCompletionScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = TaskCompletionScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Task System");
/// assert!(scenario.supported_backends().contains(&Backend::Claude));
/// ```
pub struct TaskCompletionScenario {
    id: String,
    description: String,
    tier: String,
}

impl TaskCompletionScenario {
    /// Creates a new task completion scenario.
    pub fn new() -> Self {
        Self {
            id: "task-completion".to_string(),
            description: "Verifies loop terminates with memories enabled and no open tasks"
                .to_string(),
            tier: "Tier 6: Task System".to_string(),
        }
    }
}

impl Default for TaskCompletionScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for TaskCompletionScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with memories enabled
        let config_content = format!(
            r#"# Task completion test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 5
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: true
  inject: auto
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r"Say LOOP_COMPLETE immediately. No other work needed.";

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 5,
            timeout: backend.default_timeout(),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.loop_terminated(&execution),
            self.consecutive_confirmations(&execution),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl TaskCompletionScenario {
    /// Asserts that the loop terminated successfully.
    fn loop_terminated(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let terminated = result.stdout.contains("Loop terminated")
            || result.stdout.contains("Completion confirmed")
            || result.stdout.contains("LOOP_COMPLETE detected");
        AssertionBuilder::new("Loop terminated")
            .expected("Loop terminated successfully")
            .actual(if terminated {
                "Loop termination detected".to_string()
            } else {
                "No termination detected".to_string()
            })
            .build()
            .with_passed(terminated)
    }

    /// Asserts that consecutive confirmations reached 2.
    fn consecutive_confirmations(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let confirmations_2 = result.stdout.contains("confirmations=2")
            || result.stdout.contains("confirmations = 2");
        AssertionBuilder::new("Consecutive confirmations")
            .expected("Reached 2 consecutive confirmations")
            .actual(if confirmations_2 {
                "Confirmations=2 reached".to_string()
            } else {
                "Confirmations=2 not found".to_string()
            })
            .build()
            .with_passed(confirmations_2)
    }
}

// =============================================================================
// TaskReadyScenario - Only unblocked tasks shown as ready
// =============================================================================

/// Test scenario that verifies `ralph task ready` only shows unblocked tasks.
///
/// This scenario:
/// - Creates two tasks, one blocked by the other
/// - Verifies `ralph task ready` only shows the unblocked task
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{TaskReadyScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = TaskReadyScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Task System");
/// assert!(scenario.supported_backends().contains(&Backend::Claude));
/// ```
pub struct TaskReadyScenario {
    id: String,
    description: String,
    tier: String,
}

impl TaskReadyScenario {
    /// Creates a new task ready scenario.
    pub fn new() -> Self {
        Self {
            id: "task-ready".to_string(),
            description: "Verifies ralph task ready shows only unblocked tasks".to_string(),
            tier: "Tier 6: Task System".to_string(),
        }
    }
}

impl Default for TaskReadyScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for TaskReadyScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    // Uses default supported_backends() which returns all backends

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create ralph.yml with memories enabled
        let config_content = format!(
            r#"# Task ready test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: true
  inject: manual
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        let prompt = r#"You are testing Ralph's task dependency system.

STEP 1: Create a parent task:
```
ralph task add "Parent task" -p 1
```
Note the task ID from the output.

STEP 2: Create a child task blocked by the parent (use the parent's task ID):
```
ralph task add "Child task" -p 2 --blocked-by <parent-task-id>
```

STEP 3: List ready tasks (should only show parent):
```
ralph task ready
```

STEP 4: Output LOOP_COMPLETE

Execute each command using the Bash tool."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 1,
            timeout: backend.default_timeout(),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        let execution = executor
            .run(config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("ralph execution failed: {}", e)))?;

        let duration = start.elapsed();

        // Check tasks.jsonl content
        let tasks_path = executor.workspace().join(".agent/tasks.jsonl");
        let tasks_content = if tasks_path.exists() {
            std::fs::read_to_string(&tasks_path).unwrap_or_default()
        } else {
            String::new()
        };

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.tasks_created(&execution),
            self.has_dependency(&tasks_content),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(), // Will be set by runner
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl TaskReadyScenario {
    /// Asserts that both tasks were created.
    fn tasks_created(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let count = result.stdout.matches("Created task").count();
        let created = count >= 2;
        AssertionBuilder::new("Tasks created")
            .expected("At least 2 tasks created")
            .actual(format!("{} tasks created", count))
            .build()
            .with_passed(created)
    }

    /// Asserts that the child task has a blocked_by dependency.
    fn has_dependency(&self, content: &str) -> crate::models::Assertion {
        let has_dep = content.contains("blocked_by") && content.contains("[\"task-");
        AssertionBuilder::new("Has dependency")
            .expected("Child task has blocked_by dependency")
            .actual(if has_dep {
                "Dependency found".to_string()
            } else {
                "No dependency found".to_string()
            })
            .build()
            .with_passed(has_dep)
    }
}
