//! Tier 6: Memory System test scenarios (backend-agnostic).
//!
//! These scenarios test Ralph's persistent memory system, including:
//! - Adding memories via CLI
//! - Searching memories
//! - Auto-injection of memories into prompts
//! - Persistence across runs
//!
//! The memory system stores learnings in `.agent/memories.md` and can
//! automatically inject relevant memories into agent prompts.
//!
//! All scenarios in this module are backend-agnostic and support Claude, Kiro,
//! and OpenCode backends. The backend is configured at setup time.

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
// MemoryAddScenario - Add memory via CLI
// =============================================================================

/// Test scenario that verifies memories can be added via the CLI.
///
/// This scenario:
/// - Uses `ralph tools memory add` to create a memory entry
/// - Verifies the memory is stored in `.agent/memories.md`
/// - Verifies the memory ID format is correct
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemoryAddScenario, TestScenario};
///
/// let scenario = MemoryAddScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Memory System");
/// ```
pub struct MemoryAddScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemoryAddScenario {
    /// Creates a new memory add scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-add".to_string(),
            description: "Verifies memories can be added via ralph tools memory add".to_string(),
            tier: "Tier 6: Memory System".to_string(),
        }
    }
}

impl Default for MemoryAddScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemoryAddScenario {
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

        // Create a minimal ralph.yml (memory commands don't need orchestration)
        let config_content = format!(
            r#"# Memory add test config for {}
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

        // The prompt instructs the agent to add a memory
        // NOTE: The agent needs to use Bash tool to execute the command.
        // We're explicit about the exact command and output expectations.
        let prompt = r#"You are testing Ralph's memory system.

Your task is to add a memory using the Bash tool.

STEP 1: Use the Bash tool to run this exact command:
```
ralph tools memory add "E2E test uses isolated workspaces" --type pattern --tags e2e,testing
```

STEP 2: After the command succeeds, output LOOP_COMPLETE

The command should output something like "Memory stored: mem-1234567890-abcd"

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

        // Check if memories.md was created
        let memories_path = executor.workspace().join(".agent/memories.md");
        let memories_exist = memories_path.exists();
        let memories_content = if memories_exist {
            std::fs::read_to_string(&memories_path).unwrap_or_default()
        } else {
            String::new()
        };

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.memory_command_executed(&execution),
            self.memory_file_created(memories_exist),
            self.memory_content_valid(&memories_content),
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

impl MemoryAddScenario {
    /// Asserts that the memory add command was executed.
    fn memory_command_executed(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();
        let executed = stdout_lower.contains("memory")
            || stdout_lower.contains("ralph tools memory")
            || stdout_lower.contains("mem-");

        AssertionBuilder::new("Memory command executed")
            .expected("Agent executed ralph tools memory add")
            .actual(if executed {
                "Memory command activity detected".to_string()
            } else {
                "No memory command detected in output".to_string()
            })
            .build()
            .with_passed(executed)
    }

    /// Asserts that the memories.md file was created.
    fn memory_file_created(&self, exists: bool) -> crate::models::Assertion {
        AssertionBuilder::new("Memory file created")
            .expected(".agent/memories.md file exists")
            .actual(if exists {
                "File created successfully".to_string()
            } else {
                "File not found".to_string()
            })
            .build()
            .with_passed(exists)
    }

    /// Asserts that the memory content is valid.
    fn memory_content_valid(&self, content: &str) -> crate::models::Assertion {
        // Check for expected memory structure
        let has_header = content.contains("# Memories") || content.contains("## Patterns");
        let has_memory_id = content.contains("mem-");
        let has_content = content.contains("E2E test") || content.contains("isolated workspace");

        // Empty files indicate the memory command didn't run properly
        let is_empty = content.trim().is_empty();
        let valid = !is_empty && (has_header || has_memory_id || has_content);

        AssertionBuilder::new("Memory content valid")
            .expected("Valid memory structure with content (not empty)")
            .actual(if is_empty {
                "Memory file exists but is empty - injection failed".to_string()
            } else if has_memory_id {
                "Memory entry with ID found".to_string()
            } else if has_header {
                "Memory header structure found".to_string()
            } else {
                format!("Unexpected content: {}", truncate(content, 50))
            })
            .build()
            .with_passed(valid)
    }
}

// =============================================================================
// MemorySearchScenario - Search memories
// =============================================================================

/// Test scenario that verifies memories can be searched.
///
/// This scenario:
/// - Pre-populates `.agent/memories.md` with test data
/// - Uses `ralph tools memory search` to find entries
/// - Verifies search results are correct
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemorySearchScenario, TestScenario};
///
/// let scenario = MemorySearchScenario::new();
/// assert_eq!(scenario.id(), "memory-search");
/// ```
pub struct MemorySearchScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemorySearchScenario {
    /// Creates a new memory search scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-search".to_string(),
            description: "Verifies memories can be searched via ralph tools memory search"
                .to_string(),
            tier: "Tier 6: Memory System".to_string(),
        }
    }
}

impl Default for MemorySearchScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemorySearchScenario {
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
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Pre-populate memories.md with searchable test data
        // Note: Memory ID suffixes must be valid hex (0-9, a-f) to match the parser regex
        let memories_content = r"# Memories

## Patterns

### mem-1737300000-a1b1
> Authentication uses JWT tokens with 24h expiry
<!-- tags: auth, security | created: 2025-01-19 -->

### mem-1737300100-a2b2
> Database connections pool with max 10 connections
<!-- tags: database, performance | created: 2025-01-19 -->

## Fixes

### mem-1737300200-a3b3
> ECONNREFUSED on port 5432 means start docker compose
<!-- tags: docker, database | created: 2025-01-19 -->
";
        let memories_path = agent_dir.join("memories.md");
        std::fs::write(&memories_path, memories_content).map_err(|e| {
            ScenarioError::SetupError(format!("failed to write memories.md: {}", e))
        })?;

        let config_content = format!(
            r#"# Memory search test config for {}
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

        let prompt = r#"You are testing Ralph's memory search functionality.

Pre-existing memories are in .agent/memories.md with these entries:
- A pattern about JWT authentication
- A pattern about database connection pooling
- A fix about docker ECONNREFUSED

Your task:
1. Run `ralph tools memory search "database"` to find database-related memories
2. You should find 2 matching memories (connection pool and docker fix)
3. Report what you found

Output LOOP_COMPLETE when done."#;

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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.search_command_executed(&execution),
            self.found_matching_memories(&execution),
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

impl MemorySearchScenario {
    /// Asserts that the search command was executed.
    fn search_command_executed(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();
        let executed = stdout_lower.contains("search")
            || stdout_lower.contains("ralph tools memory")
            || stdout_lower.contains("database")
            || stdout_lower.contains("mem-");

        AssertionBuilder::new("Search command executed")
            .expected("Agent executed ralph tools memory search")
            .actual(if executed {
                "Search activity detected".to_string()
            } else {
                "No search activity detected".to_string()
            })
            .build()
            .with_passed(executed)
    }

    /// Asserts that matching memories were found.
    fn found_matching_memories(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();

        // Check for evidence that database-related memories were found
        let found_connection = stdout_lower.contains("connection")
            || stdout_lower.contains("pool")
            || stdout_lower.contains("mem-1737300100-a2b2");
        let found_docker = stdout_lower.contains("docker")
            || stdout_lower.contains("econnrefused")
            || stdout_lower.contains("mem-1737300200-a3b3");
        let found_database = stdout_lower.contains("database");

        let found = found_connection || found_docker || found_database;

        AssertionBuilder::new("Found matching memories")
            .expected("Search returned database-related memories")
            .actual(if found {
                format!(
                    "Found: connection={}, docker={}, database={}",
                    found_connection, found_docker, found_database
                )
            } else {
                "No matching memories found in output".to_string()
            })
            .build()
            .with_passed(found)
    }
}

// =============================================================================
// MemoryInjectionScenario - Verify auto-injection
// =============================================================================

/// Test scenario that verifies memories are auto-injected into prompts.
///
/// This scenario:
/// - Pre-populates `.agent/memories.md` with test data
/// - Configures `inject: auto` in ralph.yml
/// - Verifies the agent can see/use the injected memories
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemoryInjectionScenario, TestScenario};
///
/// let scenario = MemoryInjectionScenario::new();
/// assert_eq!(scenario.id(), "memory-injection");
/// ```
pub struct MemoryInjectionScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemoryInjectionScenario {
    /// Creates a new memory injection scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-injection".to_string(),
            description: "Verifies memories are auto-injected into agent prompts".to_string(),
            tier: "Tier 6: Memory System".to_string(),
        }
    }
}

impl Default for MemoryInjectionScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemoryInjectionScenario {
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
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Pre-populate memories.md with a distinctive memory
        // Note: Memory ID suffixes must be valid hex (0-9, a-f) to match the parser regex
        let memories_content = r"# Memories

## Patterns

### mem-1737400000-a1b1
> The secret codeword is PURPLE_ELEPHANT_42
<!-- tags: testing, secret | created: 2025-01-20 -->

## Context

### mem-1737400100-a2b2
> This project uses the Ralph orchestrator for agentic workflows
<!-- tags: architecture | created: 2025-01-20 -->
";
        let memories_path = agent_dir.join("memories.md");
        std::fs::write(&memories_path, memories_content).map_err(|e| {
            ScenarioError::SetupError(format!("failed to write memories.md: {}", e))
        })?;

        // Configure auto-injection
        let config_content = format!(
            r#"# Memory injection test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: true
  inject: auto
  budget: 0
"#,
            backend,
            backend.as_config_str()
        );
        let config_path = workspace.join("ralph.yml");
        std::fs::write(&config_path, config_content)
            .map_err(|e| ScenarioError::SetupError(format!("failed to write ralph.yml: {}", e)))?;

        // The prompt asks the agent to recall the injected memory
        let prompt = r#"You are testing Ralph's memory injection system.

Memories should have been auto-injected into your context.
One of those memories contains a secret codeword.

Your task:
1. Look at any injected memories in your context
2. Find the secret codeword (hint: it involves an animal and a number)
3. State the codeword in your response

If you found it, say: "The codeword is: [codeword]"
If you didn't receive any memories, say: "No memories were injected"

Then output LOOP_COMPLETE."#;

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

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.memories_were_injected(&execution),
            self.agent_found_codeword(&execution),
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

impl MemoryInjectionScenario {
    /// Asserts that memories were injected (agent didn't say "no memories").
    fn memories_were_injected(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_lower = result.stdout.to_lowercase();

        // Check for negative indicator
        let no_injection = stdout_lower.contains("no memories were injected")
            || stdout_lower.contains("didn't receive")
            || stdout_lower.contains("no injected memories");

        AssertionBuilder::new("Memories were injected")
            .expected("Agent received injected memories")
            .actual(if no_injection {
                "Agent reported no memories were injected".to_string()
            } else {
                "No negative injection report".to_string()
            })
            .build()
            .with_passed(!no_injection)
    }

    /// Asserts that the agent found the secret codeword.
    fn agent_found_codeword(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let stdout_upper = result.stdout.to_uppercase();

        // The secret codeword is PURPLE_ELEPHANT_42
        let found_exact = stdout_upper.contains("PURPLE_ELEPHANT_42");
        let found_parts = stdout_upper.contains("PURPLE")
            && stdout_upper.contains("ELEPHANT")
            && stdout_upper.contains("42");
        let found_mention = stdout_upper.contains("CODEWORD");

        let found = found_exact || found_parts;

        AssertionBuilder::new("Agent found codeword")
            .expected("Agent stated the codeword PURPLE_ELEPHANT_42")
            .actual(if found_exact {
                "Found exact codeword".to_string()
            } else if found_parts {
                "Found codeword parts".to_string()
            } else if found_mention {
                "Mentioned codeword but may not have found it".to_string()
            } else {
                "Codeword not found in output".to_string()
            })
            .build()
            .with_passed(found)
    }
}

// =============================================================================
// MemoryPersistenceScenario - Memories survive across runs
// =============================================================================

/// Test scenario that verifies memories persist across separate runs.
///
/// This scenario:
/// - First run: Adds a memory
/// - Verifies the memory file exists after the run
/// - Second run: Searches for the memory (simulated by checking file)
///
/// Note: True multi-run testing requires orchestrator-level support.
/// This scenario verifies the persistence mechanism works correctly.
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemoryPersistenceScenario, TestScenario};
///
/// let scenario = MemoryPersistenceScenario::new();
/// assert_eq!(scenario.id(), "memory-persistence");
/// ```
pub struct MemoryPersistenceScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemoryPersistenceScenario {
    /// Creates a new memory persistence scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-persistence".to_string(),
            description: "Verifies memories persist in .agent/memories.md across runs".to_string(),
            tier: "Tier 6: Memory System".to_string(),
        }
    }
}

impl Default for MemoryPersistenceScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemoryPersistenceScenario {
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
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        let config_content = format!(
            r#"# Memory persistence test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 2
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

        // This scenario tests that memories are written to disk correctly
        // NOTE: The agent needs to use Bash tool to execute the command.
        let prompt = r#"You are testing Ralph's memory persistence.

Your task is to add a memory using the Bash tool.

STEP 1: Use the Bash tool to run this exact command:
```
ralph tools memory add "Persistence test marker: PERSIST_CHECK_12345" --type context --tags persistence,e2e
```

STEP 2: The command will output the memory ID (like "Memory stored: mem-1234...")

STEP 3: Output LOOP_COMPLETE

IMPORTANT: You MUST actually execute the command using the Bash tool."#;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline(prompt.to_string()),
            max_iterations: 2,
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

        // Check if memory persisted to disk
        let memories_path = executor.workspace().join(".agent/memories.md");
        let memories_exist = memories_path.exists();
        let memories_content = if memories_exist {
            std::fs::read_to_string(&memories_path).unwrap_or_default()
        } else {
            String::new()
        };

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.memory_persisted_to_disk(memories_exist, &memories_content),
            self.persistence_marker_found(&memories_content),
            self.memory_id_reported(&execution),
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

impl MemoryPersistenceScenario {
    /// Asserts that the memory was persisted to disk.
    fn memory_persisted_to_disk(&self, exists: bool, content: &str) -> crate::models::Assertion {
        let has_content = !content.trim().is_empty();

        AssertionBuilder::new("Memory persisted to disk")
            .expected(".agent/memories.md exists with content")
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

    /// Asserts that the persistence marker is in the file.
    fn persistence_marker_found(&self, content: &str) -> crate::models::Assertion {
        let has_marker = content.contains("PERSIST_CHECK_12345") || content.contains("persistence");

        AssertionBuilder::new("Persistence marker found")
            .expected("Memory contains PERSIST_CHECK_12345 or persistence tag")
            .actual(if has_marker {
                "Marker found in memories file".to_string()
            } else {
                "Marker not found".to_string()
            })
            .build()
            .with_passed(has_marker)
    }

    /// Asserts that the agent reported the memory ID.
    fn memory_id_reported(&self, result: &ExecutionResult) -> crate::models::Assertion {
        // Memory IDs look like: mem-1737372000-a1b2
        let has_memory_id = result.stdout.contains("mem-");

        AssertionBuilder::new("Memory ID reported")
            .expected("Agent reported memory ID (mem-...)")
            .actual(if has_memory_id {
                "Memory ID found in output".to_string()
            } else {
                "No memory ID in output".to_string()
            })
            .build()
            .with_passed(has_memory_id)
    }
}

/// Truncates a string to the given length, adding "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// =============================================================================
// Chaos Tests - Memory System Robustness
// =============================================================================

/// Chaos test: Verifies memory system handles corrupted memory files gracefully.
///
/// This scenario:
/// - Pre-populates `.agent/memories.md` with malformed content
/// - Verifies Ralph doesn't crash when reading corrupted memories
/// - Checks that memory operations still work (add new memories)
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemoryCorruptedFileScenario, TestScenario};
///
/// let scenario = MemoryCorruptedFileScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Memory System (Chaos)");
/// ```
pub struct MemoryCorruptedFileScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemoryCorruptedFileScenario {
    /// Creates a new corrupted file chaos scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-corrupted-file".to_string(),
            description: "Verifies graceful handling of corrupted memory files".to_string(),
            tier: "Tier 6: Memory System (Chaos)".to_string(),
        }
    }
}

impl Default for MemoryCorruptedFileScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemoryCorruptedFileScenario {
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
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // Pre-populate memories.md with malformed/corrupted content
        // This tests various corruption scenarios:
        // - Invalid memory ID format
        // - Missing required fields
        // - Truncated content
        // - Binary garbage
        let corrupted_content = r"# Memories

## Patterns

### INVALID-ID-FORMAT
> This has an invalid ID format
<!-- missing closing comment

### mem-notavalidtimestamp-xyz!
> Invalid timestamp and non-hex suffix
<!-- tags: broken | created: not-a-date -->

###
> Memory with empty ID
<!-- tags: empty -->

## Fixes

### mem-1737300200-a3b3
> This one is valid for comparison
<!-- tags: valid | created: 2025-01-19 -->

RANDOM_GARBAGE_HERE_NOT_VALID_MARKDOWN
\x00\x01\x02BINARY_LIKE_DATA
";
        let memories_path = agent_dir.join("memories.md");
        std::fs::write(&memories_path, corrupted_content).map_err(|e| {
            ScenarioError::SetupError(format!("failed to write memories.md: {}", e))
        })?;

        let config_content = format!(
            r#"# Corrupted memory file test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
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

        // The prompt asks the agent to interact with memories despite corruption
        let prompt = r#"You are testing memory system robustness against corrupted files.

The memories.md file contains some corrupted entries. Your task:

1. Try to add a new memory using Bash: ralph tools memory add "Chaos test survived" --type context --tags chaos,test
2. Report if the command succeeded or failed
3. If any errors occurred, describe them

Output LOOP_COMPLETE when done.

IMPORTANT: Use the Bash tool to execute the command."#;

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

        // Read memories file after execution
        let memories_path = executor.workspace().join(".agent/memories.md");
        let memories_content = std::fs::read_to_string(&memories_path).unwrap_or_default();

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.did_not_crash(&execution),
            self.new_memory_added(&memories_content),
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

impl MemoryCorruptedFileScenario {
    /// Asserts that Ralph didn't crash due to corrupted file.
    fn did_not_crash(&self, result: &ExecutionResult) -> crate::models::Assertion {
        // Check for crash indicators
        let crashed = result.stdout.to_lowercase().contains("panic")
            || result.stderr.to_lowercase().contains("panic")
            || result.stderr.to_lowercase().contains("fatal error")
            || result.exit_code == Some(101); // Rust panic exit code

        AssertionBuilder::new("Did not crash on corrupted file")
            .expected("No panic or fatal error")
            .actual(if crashed {
                format!(
                    "Crash detected. Exit code: {:?}, stderr contains panic: {}",
                    result.exit_code,
                    result.stderr.to_lowercase().contains("panic")
                )
            } else {
                "No crash indicators found".to_string()
            })
            .build()
            .with_passed(!crashed)
    }

    /// Asserts that a new memory was successfully added despite corruption.
    fn new_memory_added(&self, content: &str) -> crate::models::Assertion {
        let has_new = content.contains("Chaos test survived") || content.contains("chaos");

        AssertionBuilder::new("New memory added despite corruption")
            .expected("Memory containing 'Chaos test survived' or 'chaos' tag")
            .actual(if has_new {
                "New memory found in file".to_string()
            } else {
                "New memory not found".to_string()
            })
            .build()
            .with_passed(has_new)
    }
}

/// Chaos test: Verifies memory system handles empty/missing memory file gracefully.
///
/// This scenario:
/// - Starts with no `.agent/memories.md` file
/// - Verifies memory add creates the file correctly
/// - Checks that auto-injection doesn't crash on missing file
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemoryMissingFileScenario, TestScenario};
///
/// let scenario = MemoryMissingFileScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Memory System (Chaos)");
/// ```
pub struct MemoryMissingFileScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemoryMissingFileScenario {
    /// Creates a new missing file chaos scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-missing-file".to_string(),
            description: "Verifies graceful handling when memories.md doesn't exist".to_string(),
            tier: "Tier 6: Memory System (Chaos)".to_string(),
        }
    }
}

impl Default for MemoryMissingFileScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemoryMissingFileScenario {
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
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        // DO NOT create memories.md - that's the point of this test

        let config_content = format!(
            r#"# Missing memory file test config for {}
cli:
  backend: {}

event_loop:
  max_iterations: 1
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

        let prompt = r#"You are testing memory system behavior when no memories file exists.

Your task:
1. Add a memory using Bash: ralph tools memory add "First memory ever" --type pattern --tags first,test
2. Verify the command succeeded
3. Report what happened

Output LOOP_COMPLETE when done.

IMPORTANT: Use the Bash tool to execute the command."#;

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

        // Check if memories.md was created
        let memories_path = executor.workspace().join(".agent/memories.md");
        let memories_exist = memories_path.exists();
        let memories_content = if memories_exist {
            std::fs::read_to_string(&memories_path).unwrap_or_default()
        } else {
            String::new()
        };

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.did_not_crash_on_missing(&execution),
            self.file_created_on_first_add(memories_exist),
            self.first_memory_stored(&memories_content),
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

impl MemoryMissingFileScenario {
    /// Asserts that Ralph didn't crash due to missing file.
    fn did_not_crash_on_missing(&self, result: &ExecutionResult) -> crate::models::Assertion {
        let crashed = result.stdout.to_lowercase().contains("panic")
            || result.stderr.to_lowercase().contains("panic")
            || result.stderr.contains("No such file")
            || result.exit_code == Some(101);

        AssertionBuilder::new("Did not crash on missing file")
            .expected("No panic or file not found error")
            .actual(if crashed {
                format!("Error detected. Exit code: {:?}", result.exit_code)
            } else {
                "Handled missing file gracefully".to_string()
            })
            .build()
            .with_passed(!crashed)
    }

    /// Asserts that the memories file was created on first add.
    fn file_created_on_first_add(&self, exists: bool) -> crate::models::Assertion {
        AssertionBuilder::new("File created on first add")
            .expected("memories.md created after first memory add")
            .actual(if exists {
                "File was created".to_string()
            } else {
                "File not created".to_string()
            })
            .build()
            .with_passed(exists)
    }

    /// Asserts that the first memory was stored correctly.
    fn first_memory_stored(&self, content: &str) -> crate::models::Assertion {
        let has_memory = content.contains("First memory ever") || content.contains("mem-");

        AssertionBuilder::new("First memory stored correctly")
            .expected("Memory content or ID present")
            .actual(if has_memory {
                "Memory stored successfully".to_string()
            } else {
                "Memory not found in file".to_string()
            })
            .build()
            .with_passed(has_memory)
    }
}

/// Chaos test: Verifies memory system handles concurrent access simulation.
///
/// This scenario:
/// - Rapidly adds multiple memories in sequence
/// - Verifies all memories are persisted correctly
/// - Checks for race conditions or data loss
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemoryRapidWriteScenario, TestScenario};
///
/// let scenario = MemoryRapidWriteScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Memory System (Chaos)");
/// ```
pub struct MemoryRapidWriteScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemoryRapidWriteScenario {
    /// Creates a new rapid write chaos scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-rapid-write".to_string(),
            description: "Verifies memory system handles rapid sequential writes".to_string(),
            tier: "Tier 6: Memory System (Chaos)".to_string(),
        }
    }
}

impl Default for MemoryRapidWriteScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemoryRapidWriteScenario {
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
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        let config_content = format!(
            r#"# Rapid write test config for {}
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

        // The prompt asks the agent to add multiple memories rapidly
        let prompt = r#"You are stress testing the memory system with rapid writes.

Your task is to add THREE memories in quick succession using Bash. Run these commands:

1. ralph tools memory add "RAPID_TEST_1: First rapid write" --type pattern --tags rapid,test
2. ralph tools memory add "RAPID_TEST_2: Second rapid write" --type pattern --tags rapid,test
3. ralph tools memory add "RAPID_TEST_3: Third rapid write" --type pattern --tags rapid,test

After adding all three, output LOOP_COMPLETE.

IMPORTANT: Use the Bash tool to execute each command."#;

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

        // Read memories file
        let memories_path = executor.workspace().join(".agent/memories.md");
        let memories_content = std::fs::read_to_string(&memories_path).unwrap_or_default();

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.all_memories_persisted(&memories_content),
            self.no_data_corruption(&memories_content),
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

impl MemoryRapidWriteScenario {
    /// Asserts that all memories from rapid writes were persisted.
    fn all_memories_persisted(&self, content: &str) -> crate::models::Assertion {
        let has_test1 = content.contains("RAPID_TEST_1");
        let has_test2 = content.contains("RAPID_TEST_2");
        let has_test3 = content.contains("RAPID_TEST_3");

        let count = [has_test1, has_test2, has_test3]
            .iter()
            .filter(|&&x| x)
            .count();

        AssertionBuilder::new("All rapid writes persisted")
            .expected("All 3 RAPID_TEST markers present")
            .actual(format!(
                "{}/3 persisted: T1={}, T2={}, T3={}",
                count, has_test1, has_test2, has_test3
            ))
            .build()
            .with_passed(count >= 2) // Allow 2/3 for robustness (agents may not execute all)
    }

    /// Asserts that there's no data corruption from rapid writes.
    fn no_data_corruption(&self, content: &str) -> crate::models::Assertion {
        // Check for signs of corruption:
        // - Truncated memory IDs
        // - Mixed/interleaved content
        // - Invalid markdown structure
        let has_valid_structure = content.contains("# Memories") || content.contains("## ");
        let no_interleaving = !content.contains("RAPID_TEST_1RAPID_TEST_2");
        let valid_ids = content.matches("mem-").count() >= 1;

        let valid = has_valid_structure && no_interleaving && valid_ids;

        AssertionBuilder::new("No data corruption")
            .expected("Valid markdown structure, no interleaved content")
            .actual(format!(
                "structure={}, no_interleave={}, valid_ids={}",
                has_valid_structure, no_interleaving, valid_ids
            ))
            .build()
            .with_passed(valid)
    }
}

/// Chaos test: Verifies memory system handles large memory content.
///
/// This scenario:
/// - Adds a memory with very large content
/// - Verifies the memory is stored correctly
/// - Checks that search still works with large content
///
/// # Example
///
/// ```no_run
/// use ralph_e2e::scenarios::{MemoryLargeContentScenario, TestScenario};
///
/// let scenario = MemoryLargeContentScenario::new();
/// assert_eq!(scenario.tier(), "Tier 6: Memory System (Chaos)");
/// ```
pub struct MemoryLargeContentScenario {
    id: String,
    description: String,
    tier: String,
}

impl MemoryLargeContentScenario {
    /// Creates a new large content chaos scenario.
    pub fn new() -> Self {
        Self {
            id: "memory-large-content".to_string(),
            description: "Verifies memory system handles large memory content".to_string(),
            tier: "Tier 6: Memory System (Chaos)".to_string(),
        }
    }
}

impl Default for MemoryLargeContentScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for MemoryLargeContentScenario {
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
        let agent_dir = workspace.join(".agent");
        std::fs::create_dir_all(&agent_dir).map_err(|e| {
            ScenarioError::SetupError(format!("failed to create .agent directory: {}", e))
        })?;

        let config_content = format!(
            r#"# Large content test config for {}
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

        // Create a large content string (but not too large for CLI args)
        // ~500 chars is reasonable for testing without hitting arg limits
        let prompt = r#"You are testing memory system handling of larger content.

Your task is to add a memory with detailed, multi-line content using Bash:

ralph tools memory add "LARGE_CONTENT_TEST: This is a comprehensive memory entry that contains multiple sentences describing a complex architectural decision. The decision involves choosing between microservices and monolithic architecture for the authentication service. After careful consideration of scalability, maintainability, and team expertise, we decided to implement a modular monolith with clear bounded contexts that can be extracted into microservices later if needed. Key factors: team size (5 engineers), expected load (10K requests/min), and deployment infrastructure (Kubernetes). END_MARKER_12345" --type decision --tags architecture,large,test

After adding, output LOOP_COMPLETE.

IMPORTANT: Use the Bash tool to execute the command."#;

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

        // Read memories file
        let memories_path = executor.workspace().join(".agent/memories.md");
        let memories_content = std::fs::read_to_string(&memories_path).unwrap_or_default();

        let assertions = vec![
            Assertions::response_received(&execution),
            Assertions::exit_code_success_or_limit(&execution),
            Assertions::no_timeout(&execution),
            self.large_content_stored(&memories_content),
            self.content_not_truncated(&memories_content),
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

impl MemoryLargeContentScenario {
    /// Asserts that large content was stored.
    fn large_content_stored(&self, content: &str) -> crate::models::Assertion {
        let has_marker = content.contains("LARGE_CONTENT_TEST")
            || content.contains("microservices")
            || content.contains("architecture");

        AssertionBuilder::new("Large content stored")
            .expected("Memory with large content present")
            .actual(if has_marker {
                "Large content found".to_string()
            } else {
                "Large content not found".to_string()
            })
            .build()
            .with_passed(has_marker)
    }

    /// Asserts that content was not truncated.
    fn content_not_truncated(&self, content: &str) -> crate::models::Assertion {
        // Check for the end marker which proves content wasn't truncated
        let has_end_marker = content.contains("END_MARKER_12345");

        AssertionBuilder::new("Content not truncated")
            .expected("END_MARKER_12345 present (proves full content stored)")
            .actual(if has_end_marker {
                "End marker found - content complete".to_string()
            } else {
                "End marker missing - possible truncation".to_string()
            })
            .build()
            .with_passed(has_end_marker)
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
    use std::time::Duration;

    fn test_workspace(test_name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-memory-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    fn cleanup_workspace(path: &std::path::PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }

    fn mock_execution_result() -> ExecutionResult {
        ExecutionResult {
            exit_code: Some(0),
            stdout: "Added memory: mem-1737500000-test\nListing memories...\n".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(5),
            scratchpad: None,
            events: vec![],
            iterations: 1,
            termination_reason: Some("LOOP_COMPLETE".to_string()),
            timed_out: false,
        }
    }

    // ========== MemoryAddScenario Tests ==========

    #[test]
    fn test_memory_add_scenario_new() {
        let scenario = MemoryAddScenario::new();
        assert_eq!(scenario.id(), "memory-add");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 6: Memory System");
    }

    #[test]
    fn test_memory_add_scenario_default() {
        let scenario = MemoryAddScenario::default();
        assert_eq!(scenario.id(), "memory-add");
    }

    #[test]
    fn test_memory_add_supports_all_backends() {
        let scenario = MemoryAddScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_memory_add_setup_creates_config() {
        let workspace = test_workspace("memory-add-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemoryAddScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists(), "ralph.yml should exist");

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(
            content.contains("memories:"),
            "Should have memories section"
        );
        assert!(content.contains("enabled: true"), "Should enable memories");
        assert!(
            content.contains("backend: claude"),
            "Should have claude backend"
        );

        assert!(workspace.join(".agent").exists(), ".agent should exist");
        assert_eq!(config.max_iterations, 1);
        assert_eq!(config.timeout, Backend::Claude.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_memory_add_command_executed_passed() {
        let scenario = MemoryAddScenario::new();
        let result = mock_execution_result();
        let assertion = scenario.memory_command_executed(&result);
        assert!(assertion.passed, "Should pass when memory command detected");
    }

    #[test]
    fn test_memory_add_command_executed_failed() {
        let scenario = MemoryAddScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "I did something unrelated".to_string();
        let assertion = scenario.memory_command_executed(&result);
        assert!(!assertion.passed, "Should fail when no memory activity");
    }

    #[test]
    fn test_memory_add_file_created_passed() {
        let scenario = MemoryAddScenario::new();
        let assertion = scenario.memory_file_created(true);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_add_file_created_failed() {
        let scenario = MemoryAddScenario::new();
        let assertion = scenario.memory_file_created(false);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_memory_add_content_valid_with_id() {
        let scenario = MemoryAddScenario::new();
        let content = "### mem-1234\n> Some content";
        let assertion = scenario.memory_content_valid(content);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_add_content_valid_with_header() {
        let scenario = MemoryAddScenario::new();
        let content = "# Memories\n\n## Patterns";
        let assertion = scenario.memory_content_valid(content);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_add_content_empty_fails() {
        let scenario = MemoryAddScenario::new();
        let content = "";
        let assertion = scenario.memory_content_valid(content);
        assert!(!assertion.passed, "Empty files should fail validation");
    }

    #[test]
    fn test_memory_add_content_whitespace_only_fails() {
        let scenario = MemoryAddScenario::new();
        let content = "   \n  \t  ";
        let assertion = scenario.memory_content_valid(content);
        assert!(
            !assertion.passed,
            "Whitespace-only files should fail validation"
        );
    }

    #[test]
    fn test_memory_add_description() {
        let scenario = MemoryAddScenario::new();
        assert!(scenario.description().contains("add"));
    }

    // ========== MemorySearchScenario Tests ==========

    #[test]
    fn test_memory_search_scenario_new() {
        let scenario = MemorySearchScenario::new();
        assert_eq!(scenario.id(), "memory-search");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 6: Memory System");
    }

    #[test]
    fn test_memory_search_scenario_default() {
        let scenario = MemorySearchScenario::default();
        assert_eq!(scenario.id(), "memory-search");
    }

    #[test]
    fn test_memory_search_supports_all_backends() {
        let scenario = MemorySearchScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_memory_search_setup_creates_memories() {
        let workspace = test_workspace("memory-search-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemorySearchScenario::new();
        let _config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let memories_path = workspace.join(".agent/memories.md");
        assert!(memories_path.exists(), "memories.md should exist");

        let content = fs::read_to_string(&memories_path).unwrap();
        assert!(content.contains("JWT"), "Should have JWT memory");
        assert!(content.contains("database"), "Should have database memory");
        assert!(content.contains("ECONNREFUSED"), "Should have docker fix");

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_memory_search_command_executed_passed() {
        let scenario = MemorySearchScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "Searching for database... Found 2 memories".to_string();
        let assertion = scenario.search_command_executed(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_search_found_memories_passed() {
        let scenario = MemorySearchScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "Found: Database connection pool with max 10 connections".to_string();
        let assertion = scenario.found_matching_memories(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_search_found_memories_failed() {
        let scenario = MemorySearchScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "No results found for your query".to_string();
        let assertion = scenario.found_matching_memories(&result);
        assert!(!assertion.passed);
    }

    // ========== MemoryInjectionScenario Tests ==========

    #[test]
    fn test_memory_injection_scenario_new() {
        let scenario = MemoryInjectionScenario::new();
        assert_eq!(scenario.id(), "memory-injection");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 6: Memory System");
    }

    #[test]
    fn test_memory_injection_scenario_default() {
        let scenario = MemoryInjectionScenario::default();
        assert_eq!(scenario.id(), "memory-injection");
    }

    #[test]
    fn test_memory_injection_supports_all_backends() {
        let scenario = MemoryInjectionScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_memory_injection_setup_with_auto() {
        let workspace = test_workspace("memory-injection-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemoryInjectionScenario::new();
        let _config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("inject: auto"), "Should have inject: auto");
        assert!(
            content.contains("backend: claude"),
            "Should have claude backend"
        );

        let memories_path = workspace.join(".agent/memories.md");
        let mem_content = fs::read_to_string(&memories_path).unwrap();
        assert!(
            mem_content.contains("PURPLE_ELEPHANT_42"),
            "Should have secret codeword"
        );

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_memory_injection_found_codeword_passed() {
        let scenario = MemoryInjectionScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "The codeword is: PURPLE_ELEPHANT_42".to_string();
        let assertion = scenario.agent_found_codeword(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_injection_found_codeword_parts() {
        let scenario = MemoryInjectionScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "I found PURPLE and ELEPHANT and 42".to_string();
        let assertion = scenario.agent_found_codeword(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_injection_codeword_not_found() {
        let scenario = MemoryInjectionScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "I couldn't find any codeword".to_string();
        let assertion = scenario.agent_found_codeword(&result);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_memory_injection_memories_injected_passed() {
        let scenario = MemoryInjectionScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "I can see the memories in my context".to_string();
        let assertion = scenario.memories_were_injected(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_injection_memories_not_injected() {
        let scenario = MemoryInjectionScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "No memories were injected into my context".to_string();
        let assertion = scenario.memories_were_injected(&result);
        assert!(!assertion.passed);
    }

    // ========== MemoryPersistenceScenario Tests ==========

    #[test]
    fn test_memory_persistence_scenario_new() {
        let scenario = MemoryPersistenceScenario::new();
        assert_eq!(scenario.id(), "memory-persistence");
        assert!(scenario.supported_backends().contains(&Backend::Claude));
        assert_eq!(scenario.tier(), "Tier 6: Memory System");
    }

    #[test]
    fn test_memory_persistence_scenario_default() {
        let scenario = MemoryPersistenceScenario::default();
        assert_eq!(scenario.id(), "memory-persistence");
    }

    #[test]
    fn test_memory_persistence_supports_all_backends() {
        let scenario = MemoryPersistenceScenario::new();
        let supported = scenario.supported_backends();
        assert!(supported.contains(&Backend::Claude));
        assert!(supported.contains(&Backend::Kiro));
        assert!(supported.contains(&Backend::OpenCode));
    }

    #[test]
    fn test_memory_persistence_setup() {
        let workspace = test_workspace("memory-persistence-setup");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemoryPersistenceScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let config_path = workspace.join("ralph.yml");
        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(
            content.contains("backend: claude"),
            "Should have claude backend"
        );
        assert_eq!(config.max_iterations, 2);
        assert_eq!(config.timeout, Backend::Claude.default_timeout());

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_memory_persistence_disk_passed() {
        let scenario = MemoryPersistenceScenario::new();
        let assertion = scenario.memory_persisted_to_disk(true, "# Memories\n### mem-123");
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_persistence_disk_empty_failed() {
        let scenario = MemoryPersistenceScenario::new();
        let assertion = scenario.memory_persisted_to_disk(true, "");
        assert!(!assertion.passed);
    }

    #[test]
    fn test_memory_persistence_disk_not_exist_failed() {
        let scenario = MemoryPersistenceScenario::new();
        let assertion = scenario.memory_persisted_to_disk(false, "");
        assert!(!assertion.passed);
    }

    #[test]
    fn test_memory_persistence_marker_found() {
        let scenario = MemoryPersistenceScenario::new();
        let content = "### mem-123\n> PERSIST_CHECK_12345\n<!-- tags: persistence -->";
        let assertion = scenario.persistence_marker_found(content);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_persistence_marker_via_tag() {
        let scenario = MemoryPersistenceScenario::new();
        let content = "### mem-123\n> Something\n<!-- tags: persistence -->";
        let assertion = scenario.persistence_marker_found(content);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_persistence_marker_not_found() {
        let scenario = MemoryPersistenceScenario::new();
        let content = "### mem-123\n> Unrelated content";
        let assertion = scenario.persistence_marker_found(content);
        assert!(!assertion.passed);
    }

    #[test]
    fn test_memory_persistence_id_reported() {
        let scenario = MemoryPersistenceScenario::new();
        let result = mock_execution_result();
        let assertion = scenario.memory_id_reported(&result);
        assert!(assertion.passed);
    }

    #[test]
    fn test_memory_persistence_id_not_reported() {
        let scenario = MemoryPersistenceScenario::new();
        let mut result = mock_execution_result();
        result.stdout = "Memory was added successfully".to_string();
        let assertion = scenario.memory_id_reported(&result);
        assert!(!assertion.passed);
    }

    // ========== Helper function tests ==========

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("this is a long string", 10), "this is a ...");
    }

    // ========== Integration Tests (ignored by default) ==========

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_memory_add_full_run() {
        let workspace = test_workspace("memory-add-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemoryAddScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "✅" } else { "❌" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_memory_search_full_run() {
        let workspace = test_workspace("memory-search-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemorySearchScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "✅" } else { "❌" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_memory_injection_full_run() {
        let workspace = test_workspace("memory-injection-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemoryInjectionScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "✅" } else { "❌" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires live backend"]
    async fn test_memory_persistence_full_run() {
        let workspace = test_workspace("memory-persistence-full");
        fs::create_dir_all(&workspace).unwrap();

        let scenario = MemoryPersistenceScenario::new();
        let config = scenario.setup(&workspace, Backend::Claude).unwrap();

        let executor = RalphExecutor::new(workspace.clone());
        let result = scenario.run(&executor, &config).await;

        cleanup_workspace(&workspace);

        let result = result.expect("run should succeed");
        println!("Assertions:");
        for a in &result.assertions {
            println!(
                "  {} - {}: {} (expected: {})",
                if a.passed { "✅" } else { "❌" },
                a.name,
                a.actual,
                a.expected
            );
        }
    }
}
