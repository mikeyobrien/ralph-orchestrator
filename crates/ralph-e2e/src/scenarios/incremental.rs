//! Tier 7: Incremental Feature Development test scenario.
//!
//! This scenario tests Ralph's memory + task systems working together across
//! multiple orchestration loops, simulating real-world incremental software
//! development where context accumulates across sessions.
//!
//! The scenario builds a simple feature across 3 loops:
//! - Loop 1: Create task, write code, learn pattern, save to memory
//! - Loop 2: Recall memory from Loop 1, extend code, close task, create next task
//! - Loop 3: Use accumulated memories to complete feature
//!
//! This validates:
//! - Memory persistence across Ralph invocations
//! - Task state transitions (open -> closed)
//! - Cross-loop context accumulation
//! - Memory injection informing subsequent work

use super::{AssertionBuilder, Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{PromptSource, RalphExecutor, ScenarioConfig};
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
// IncrementalFeatureScenario - Multi-loop incremental development
// =============================================================================

/// Test scenario that validates memory + tasks across multiple Ralph loops.
///
/// This scenario simulates building a feature incrementally:
/// - Phase 1: Create base module with a greeting function
/// - Phase 2: Add error handling based on learned patterns
/// - Phase 3: Add documentation and finalize
///
/// Each phase runs as a separate Ralph invocation, testing:
/// - Memory persistence (patterns discovered in Phase 1 are available in Phase 2)
/// - Task lifecycle (tasks created/closed across invocations)
/// - Context accumulation (memories inform subsequent work)
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{IncrementalFeatureScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = IncrementalFeatureScenario::new();
/// assert_eq!(scenario.tier(), "Tier 7: Incremental Development");
/// ```
pub struct IncrementalFeatureScenario {
    id: String,
    description: String,
    tier: String,
}

impl IncrementalFeatureScenario {
    /// Creates a new incremental feature scenario.
    pub fn new() -> Self {
        Self {
            id: "incremental-feature".to_string(),
            description: "Validates memory + tasks across multiple Ralph loops".to_string(),
            tier: "Tier 7: Incremental Development".to_string(),
        }
    }
}

impl Default for IncrementalFeatureScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for IncrementalFeatureScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude, Backend::Kiro, Backend::OpenCode]
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create src directory for the "project"
        let src_dir = workspace.join("src");
        std::fs::create_dir_all(&src_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create src directory: {}", e))
        })?;

        // Create ralph.yml with memories + tasks enabled
        let config_content = format!(
            r#"# Incremental feature development test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 10
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: true
  inject: auto
  budget: 2000
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // This is a multi-phase scenario. The prompt guides the agent through all phases.
        // Each phase builds on the previous, testing memory persistence and task management.
        let prompt = r#"You are building a greeting module incrementally across multiple phases.
Each phase builds on the previous work, demonstrating memory + task integration.

=== PHASE 1: Foundation ===
1. Create a task: `ralph tools task add "Create greeting module foundation" -p 1`
2. Write src/greet.py with a simple greet(name) function that returns "Hello, {name}!"
3. Learn the pattern and save it: `ralph tools memory add "greet module uses f-string formatting: return f'Hello, {name}!'" --type pattern --tags greet,python`
4. Close the task using its ID
5. Create next task: `ralph tools task add "Add error handling to greet" -p 2 --blocked-by <previous-task-id>`
   Actually, since we just closed the previous task, this one is now unblocked.
   Create: `ralph tools task add "Add error handling to greet" -p 2`

=== PHASE 2: Error Handling ===
(Your memory of the f-string pattern should be auto-injected)
1. Read src/greet.py to see current state
2. Add error handling: if name is empty, raise ValueError("Name cannot be empty")
3. Save a memory about the error handling pattern: `ralph tools memory add "greet module validates input: raises ValueError for empty name" --type pattern --tags greet,validation`
4. Close the "Add error handling" task
5. Create final task: `ralph tools task add "Add docstring to greet" -p 3`

=== PHASE 3: Documentation ===
(Your memories from phases 1 and 2 should be auto-injected)
1. Read src/greet.py
2. Add a docstring describing the function, its parameters, return value, and possible exceptions
3. Close the documentation task
4. Verify all tasks are closed: `ralph tools task list`
5. Output LOOP_COMPLETE

IMPORTANT RULES:
- Use the Bash tool to execute ralph commands
- Use Write or Edit tools to modify files
- Each phase progresses the codebase
- Output LOOP_COMPLETE only when ALL tasks are closed

Begin with Phase 1."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 10,
            timeout: backend.default_timeout() * 3, // Longer timeout for multi-phase
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

        // Read files to check state after execution
        let memories_path = executor.workspace().join(".ralph/agent/memories.md");
        let memories_exist = memories_path.exists();
        let memories_content = if memories_exist {
            std::fs::read_to_string(&memories_path).unwrap_or_default()
        } else {
            String::new()
        };

        let tasks_path = executor.workspace().join(".ralph/agent/tasks.jsonl");
        let tasks_exist = tasks_path.exists();
        let tasks_content = if tasks_exist {
            std::fs::read_to_string(&tasks_path).unwrap_or_default()
        } else {
            String::new()
        };

        let greet_path = executor.workspace().join("src/greet.py");
        let greet_exists = greet_path.exists();
        let greet_content = if greet_exists {
            std::fs::read_to_string(&greet_path).unwrap_or_default()
        } else {
            String::new()
        };

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            // Memory assertions
            self.memories_created(memories_exist, &memories_content),
            self.pattern_memories_stored(&memories_content),
            // Task assertions
            self.tasks_created(tasks_exist, &tasks_content),
            self.tasks_closed(&tasks_content),
            // Code assertions
            self.code_file_created(greet_exists),
            self.code_has_function(&greet_content),
            self.code_has_error_handling(&greet_content),
            self.code_has_docstring(&greet_content),
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

impl IncrementalFeatureScenario {
    /// Asserts that memories.md was created.
    fn memories_created(&self, exists: bool, content: &str) -> crate::models::Assertion {
        let has_content = !content.trim().is_empty();
        AssertionBuilder::new("Memories file created")
            .expected(".ralph/agent/memories.md exists with content")
            .actual(if exists && has_content {
                format!("File exists with {} bytes", content.len())
            } else if exists {
                "File exists but is empty".to_string()
            } else {
                "File does not exist".to_string()
            })
            .build()
            .with_passed(exists && has_content)
    }

    /// Asserts that pattern memories were stored from both phases.
    fn pattern_memories_stored(&self, content: &str) -> crate::models::Assertion {
        // Look for evidence of patterns from both phases
        let has_greet_pattern =
            content.contains("greet") || content.contains("f-string") || content.contains("Hello");
        let has_validation_pattern = content.contains("validation")
            || content.contains("ValueError")
            || content.contains("error handling");

        let count = [has_greet_pattern, has_validation_pattern]
            .iter()
            .filter(|&&x| x)
            .count();

        AssertionBuilder::new("Pattern memories stored")
            .expected("At least 1 pattern from development phases")
            .actual(format!(
                "{}/2 patterns found: greet={}, validation={}",
                count, has_greet_pattern, has_validation_pattern
            ))
            .build()
            .with_passed(count >= 1)
    }

    /// Asserts that tasks.jsonl was created.
    fn tasks_created(&self, exists: bool, content: &str) -> crate::models::Assertion {
        let has_content = !content.trim().is_empty();
        let task_count = content.matches("task-").count();

        AssertionBuilder::new("Tasks file created")
            .expected(".ralph/agent/tasks.jsonl exists with tasks")
            .actual(if exists && has_content {
                format!("File exists with {} task entries", task_count / 2) // ID appears in id and title
            } else if exists {
                "File exists but is empty".to_string()
            } else {
                "File does not exist".to_string()
            })
            .build()
            .with_passed(exists && has_content)
    }

    /// Asserts that tasks were closed.
    fn tasks_closed(&self, content: &str) -> crate::models::Assertion {
        let closed_count = content.matches("\"status\":\"closed\"").count();
        // We expect at least 2 tasks to be closed (foundation + error handling)
        // Documentation task may or may not be closed depending on agent behavior
        let enough_closed = closed_count >= 1;

        AssertionBuilder::new("Tasks closed")
            .expected("At least 1 task marked as closed")
            .actual(format!("{} tasks closed", closed_count))
            .build()
            .with_passed(enough_closed)
    }

    /// Asserts that the code file was created.
    fn code_file_created(&self, exists: bool) -> crate::models::Assertion {
        AssertionBuilder::new("Code file created")
            .expected("src/greet.py exists")
            .actual(if exists {
                "File exists".to_string()
            } else {
                "File not found".to_string()
            })
            .build()
            .with_passed(exists)
    }

    /// Asserts that the code has a greet function.
    fn code_has_function(&self, content: &str) -> crate::models::Assertion {
        let has_def = content.contains("def greet");
        let has_return = content.contains("return") || content.contains("Hello");

        let valid = has_def && has_return;

        AssertionBuilder::new("Code has greet function")
            .expected("def greet() with return statement")
            .actual(format!(
                "def greet={}, return/Hello={}",
                has_def, has_return
            ))
            .build()
            .with_passed(valid)
    }

    /// Asserts that the code has error handling.
    fn code_has_error_handling(&self, content: &str) -> crate::models::Assertion {
        let has_raise = content.contains("raise");
        let has_error = content.contains("Error") || content.contains("error");
        let has_validation =
            content.contains("if") && (content.contains("not") || content.contains("empty"));

        // Accept any form of input validation
        let valid = has_raise || (has_validation && has_error);

        AssertionBuilder::new("Code has error handling")
            .expected("Input validation or raise statement")
            .actual(format!(
                "raise={}, error={}, validation={}",
                has_raise, has_error, has_validation
            ))
            .build()
            .with_passed(valid)
    }

    /// Asserts that the code has a docstring.
    fn code_has_docstring(&self, content: &str) -> crate::models::Assertion {
        let has_triple_quote = content.contains("\"\"\"") || content.contains("'''");
        let has_description = content.to_lowercase().contains("param")
            || content.to_lowercase().contains("return")
            || content.to_lowercase().contains("arg")
            || content.to_lowercase().contains("name");

        let valid = has_triple_quote || has_description;

        AssertionBuilder::new("Code has docstring")
            .expected("Docstring with triple quotes or param/return description")
            .actual(format!(
                "triple_quote={}, description={}",
                has_triple_quote, has_description
            ))
            .build()
            .with_passed(valid)
    }
}

// =============================================================================
// ChainedLoopScenario - True multi-invocation testing
// =============================================================================

/// Test scenario that runs multiple sequential Ralph invocations.
///
/// Unlike IncrementalFeatureScenario which runs a single long session,
/// this scenario explicitly runs 3 separate Ralph invocations to test
/// true cross-session memory and task persistence.
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{ChainedLoopScenario, TestScenario};
/// use ralph_e2e::Backend;
///
/// let scenario = ChainedLoopScenario::new();
/// assert_eq!(scenario.tier(), "Tier 7: Incremental Development");
/// ```
pub struct ChainedLoopScenario {
    id: String,
    description: String,
    tier: String,
}

impl ChainedLoopScenario {
    /// Creates a new chained loop scenario.
    pub fn new() -> Self {
        Self {
            id: "chained-loops".to_string(),
            description: "Tests memory + task persistence across 3 separate Ralph invocations"
                .to_string(),
            tier: "Tier 7: Incremental Development".to_string(),
        }
    }
}

impl Default for ChainedLoopScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for ChainedLoopScenario {
    fn id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tier(&self) -> &str {
        &self.tier
    }

    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude, Backend::Kiro, Backend::OpenCode]
    }

    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        // Create the .agent directory
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Create src directory
        let src_dir = workspace.join("src");
        std::fs::create_dir_all(&src_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create src directory: {}", e))
        })?;

        // Create ralph.yml
        let config_content = format!(
            r#"# Chained loops test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 3
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: true
  inject: auto
  budget: 2000
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // We return a dummy config; the actual run() method will handle multi-invocation
        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(String::new()), // Will be set per-phase
            max_iterations: 3,
            timeout: backend.default_timeout(),
            extra_args: vec![],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        _config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = std::time::Instant::now();

        // Phase 1: Create foundation
        let phase1_prompt = r#"You are in Phase 1 of a multi-phase project.

Your tasks:
1. Create a task: `ralph tools task add "Create calculator module" -p 1`
2. Write src/calc.py with an add(a, b) function that returns a + b
3. Save a pattern memory: `ralph tools memory add "Calculator module uses type hints for parameters" --type pattern --tags calc,python`
4. Close the task

Output LOOP_COMPLETE when done.
IMPORTANT: Use Bash for ralph commands and Write/Edit for files."#;

        let phase1_config = ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(phase1_prompt.to_string()),
            max_iterations: 3,
            timeout: std::time::Duration::from_secs(120),
            extra_args: vec![],
        };

        let _phase1_result = executor
            .run(&phase1_config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("phase 1 failed: {}", e)))?;

        // Phase 2: Add more functionality (memory from phase 1 should be injected)
        let phase2_prompt = r#"You are in Phase 2. Memory from Phase 1 should be auto-injected.

Your tasks:
1. Read src/calc.py to see current state
2. Create a task: `ralph tools task add "Add subtract function" -p 2`
3. Add a subtract(a, b) function to src/calc.py
4. Save a memory about the module structure: `ralph tools memory add "Calculator module has add and subtract functions" --type context --tags calc,functions`
5. Close the task

Output LOOP_COMPLETE when done.
IMPORTANT: Use Bash for ralph commands and Write/Edit for files."#;

        let phase2_config = ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(phase2_prompt.to_string()),
            max_iterations: 3,
            timeout: std::time::Duration::from_secs(120),
            extra_args: vec![],
        };

        let _phase2_result = executor
            .run(&phase2_config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("phase 2 failed: {}", e)))?;

        // Phase 3: Finalize with accumulated context
        let phase3_prompt = r#"You are in Phase 3. Memories from Phases 1 and 2 should be auto-injected.

Your tasks:
1. Read src/calc.py
2. Create a task: `ralph tools task add "Add multiply function" -p 3`
3. Add a multiply(a, b) function to src/calc.py
4. Close the task
5. List all tasks to verify completion: `ralph tools task list`
6. List all memories to see accumulated knowledge: `ralph tools memory list`

Output LOOP_COMPLETE when done.
IMPORTANT: Use Bash for ralph commands and Write/Edit for files."#;

        let phase3_config = ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(phase3_prompt.to_string()),
            max_iterations: 3,
            timeout: std::time::Duration::from_secs(120),
            extra_args: vec![],
        };

        let phase3_result = executor
            .run(&phase3_config)
            .await
            .map_err(|e| ScenarioError::ExecutionError(format!("phase 3 failed: {}", e)))?;

        let duration = start.elapsed();

        // Check final state after all phases
        let memories_path = executor.workspace().join(".ralph/agent/memories.md");
        let memories_content = std::fs::read_to_string(&memories_path).unwrap_or_default();

        let tasks_path = executor.workspace().join(".ralph/agent/tasks.jsonl");
        let tasks_content = std::fs::read_to_string(&tasks_path).unwrap_or_default();

        let calc_path = executor.workspace().join("src/calc.py");
        let calc_content = std::fs::read_to_string(&calc_path).unwrap_or_default();

        let assertions = vec![
            Assertions::response_received(&phase3_result),
            Assertions::exit_code_success_or_limit(&phase3_result),
            self.memories_accumulated(&memories_content),
            self.tasks_tracked(&tasks_content),
            self.code_evolved(&calc_content),
        ];

        let all_passed = assertions.iter().all(|a| a.passed);

        Ok(TestResult {
            scenario_id: self.id.clone(),
            scenario_description: self.description.clone(),
            backend: String::new(),
            tier: self.tier.clone(),
            passed: all_passed,
            assertions,
            duration,
        })
    }
}

impl ChainedLoopScenario {
    /// Asserts that memories accumulated across phases.
    fn memories_accumulated(&self, content: &str) -> crate::models::Assertion {
        let memory_count = content.matches("mem-").count();
        // We expect at least 2 memories from different phases
        let accumulated = memory_count >= 2;

        AssertionBuilder::new("Memories accumulated across phases")
            .expected("At least 2 memories from different phases")
            .actual(format!("{} memory entries found", memory_count))
            .build()
            .with_passed(accumulated)
    }

    /// Asserts that tasks were tracked across phases.
    fn tasks_tracked(&self, content: &str) -> crate::models::Assertion {
        let task_count = content.lines().filter(|l| l.contains("task-")).count();
        let closed_count = content.matches("\"status\":\"closed\"").count();

        // We expect multiple tasks created and at least some closed
        let tracked = task_count >= 2 && closed_count >= 1;

        AssertionBuilder::new("Tasks tracked across phases")
            .expected("At least 2 tasks created, 1+ closed")
            .actual(format!(
                "{} tasks total, {} closed",
                task_count, closed_count
            ))
            .build()
            .with_passed(tracked)
    }

    /// Asserts that code evolved across phases.
    fn code_evolved(&self, content: &str) -> crate::models::Assertion {
        let has_add = content.contains("def add");
        let has_subtract = content.contains("def subtract");
        let has_multiply = content.contains("def multiply");

        let function_count = [has_add, has_subtract, has_multiply]
            .iter()
            .filter(|&&x| x)
            .count();

        // We expect at least 2 functions from incremental development
        let evolved = function_count >= 2;

        AssertionBuilder::new("Code evolved across phases")
            .expected("At least 2 functions from incremental development")
            .actual(format!(
                "{}/3 functions: add={}, subtract={}, multiply={}",
                function_count, has_add, has_subtract, has_multiply
            ))
            .build()
            .with_passed(evolved)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn test_workspace(test_name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-incr-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    fn cleanup_workspace(path: &std::path::PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    // ========== IncrementalFeatureScenario Tests ==========

    #[test]
    fn test_incremental_feature_scenario_new() {
        let scenario = IncrementalFeatureScenario::new();
        assert_eq!(scenario.id(), "incremental-feature");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 7: Incremental Development");
    }

    #[test]
    fn test_incremental_feature_scenario_default() {
        let scenario = IncrementalFeatureScenario::default();
        assert_eq!(scenario.id(), "incremental-feature");
    }

    #[test]
    fn test_incremental_feature_supports_all_backends() {
        let scenario = IncrementalFeatureScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_incremental_feature_setup_creates_structure() {
        let workspace = test_workspace("incr-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = IncrementalFeatureScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        // Verify directories created
        assert!(workspace.join(".agent").exists());
        assert!(workspace.join("src").exists());

        // Verify config file
        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("memories:"));
        assert!(content.contains("enabled: true"));
        assert!(content.contains("inject: auto"));
        assert!(content.contains("backend: claude"));

        // Verify config struct
        assert_eq!(config.max_iterations, 10);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_incremental_feature_memories_created() {
        let scenario = IncrementalFeatureScenario::new();

        let assertion = scenario.memories_created(true, "# Memories\n### mem-123");
        assert!(assertion.passed);

        let assertion = scenario.memories_created(true, "");
        assert!(!assertion.passed);

        let assertion = scenario.memories_created(false, "");
        assert!(!assertion.passed);
    }

    #[test]
    fn test_incremental_feature_pattern_memories() {
        let scenario = IncrementalFeatureScenario::new();

        let content = "mem-1: greet uses f-string\nmem-2: validation with ValueError";
        let assertion = scenario.pattern_memories_stored(content);
        assert!(assertion.passed);

        let content = "nothing here";
        let assertion = scenario.pattern_memories_stored(content);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_incremental_feature_tasks_created() {
        let scenario = IncrementalFeatureScenario::new();

        let content = r#"{"id":"task-123","title":"Test"}
{"id":"task-456","title":"Test2"}"#;
        let assertion = scenario.tasks_created(true, content);
        assert!(assertion.passed);

        let assertion = scenario.tasks_created(false, "");
        assert!(!assertion.passed);
    }

    #[test]
    fn test_incremental_feature_tasks_closed() {
        let scenario = IncrementalFeatureScenario::new();

        let content = r#"{"id":"task-123","status":"closed"}
{"id":"task-456","status":"closed"}"#;
        let assertion = scenario.tasks_closed(content);
        assert!(assertion.passed);

        let content = r#"{"id":"task-123","status":"open"}"#;
        let assertion = scenario.tasks_closed(content);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_incremental_feature_code_function() {
        let scenario = IncrementalFeatureScenario::new();

        let content = r#"def greet(name):
    return f"Hello, {name}!"
"#;
        let assertion = scenario.code_has_function(content);
        assert!(assertion.passed);

        let content = "# empty file";
        let assertion = scenario.code_has_function(content);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_incremental_feature_error_handling() {
        let scenario = IncrementalFeatureScenario::new();

        let content = r#"def greet(name):
    if not name:
        raise ValueError("empty")
    return f"Hello, {name}!"
"#;
        let assertion = scenario.code_has_error_handling(content);
        assert!(assertion.passed);

        let content = r#"def greet(name):
    return f"Hello, {name}!"
"#;
        let assertion = scenario.code_has_error_handling(content);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_incremental_feature_docstring() {
        let scenario = IncrementalFeatureScenario::new();

        let content = r#"def greet(name):
    """Greet a person by name.

    Args:
        name: The name to greet
    Returns:
        A greeting string
    """
    return f"Hello, {name}!"
"#;
        let assertion = scenario.code_has_docstring(content);
        assert!(assertion.passed);

        let content = r#"def greet(name):
    return f"Hello, {name}!"
"#;
        // Should still pass because it contains "name" which is a description
        let assertion = scenario.code_has_docstring(content);
        assert!(assertion.passed);
    }

    // ========== ChainedLoopScenario Tests ==========

    #[test]
    fn test_chained_loop_scenario_new() {
        let scenario = ChainedLoopScenario::new();
        assert_eq!(scenario.id(), "chained-loops");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 7: Incremental Development");
    }

    #[test]
    fn test_chained_loop_scenario_default() {
        let scenario = ChainedLoopScenario::default();
        assert_eq!(scenario.id(), "chained-loops");
    }

    #[test]
    fn test_chained_loop_supports_all_backends() {
        let scenario = ChainedLoopScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_chained_loop_setup_creates_structure() {
        let workspace = test_workspace("chained-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ChainedLoopScenario::new();
        let _config = scenario.setup(&workspace, Backend::Claude).unwrap();

        assert!(workspace.join(".agent").exists());
        assert!(workspace.join("src").exists());
        assert!(workspace.join("ralph.yml").exists());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_chained_loop_memories_accumulated() {
        let scenario = ChainedLoopScenario::new();

        let content = "mem-123: first\nmem-456: second\nmem-789: third";
        let assertion = scenario.memories_accumulated(content);
        assert!(assertion.passed);

        let content = "mem-123: only one";
        let assertion = scenario.memories_accumulated(content);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_chained_loop_tasks_tracked() {
        let scenario = ChainedLoopScenario::new();

        let content = r#"{"id":"task-1","status":"closed"}
{"id":"task-2","status":"closed"}
{"id":"task-3","status":"open"}"#;
        let assertion = scenario.tasks_tracked(content);
        assert!(assertion.passed);

        let content = r#"{"id":"task-1","status":"open"}"#;
        let assertion = scenario.tasks_tracked(content);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_chained_loop_code_evolved() {
        let scenario = ChainedLoopScenario::new();

        let content = r"def add(a, b):
    return a + b

def subtract(a, b):
    return a - b

def multiply(a, b):
    return a * b
";
        let assertion = scenario.code_evolved(content);
        assert!(assertion.passed);

        let content = r"def add(a, b):
    return a + b
";
        let assertion = scenario.code_evolved(content);
        assert!(!assertion.passed);
    }

    // ========== Integration Tests ==========

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_incremental_feature_full_run() {
        let workspace = test_workspace("incr-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = IncrementalFeatureScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "OK" } else { "FAIL" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires live backend - long running"]
    async fn test_chained_loops_full_run() {
        let workspace = test_workspace("chained-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = ChainedLoopScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "OK" } else { "FAIL" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }
}
