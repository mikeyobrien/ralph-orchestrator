//! Tier 0: v3 engine smoke scenarios.

use super::{AssertionBuilder, Assertions, ScenarioError, TestScenario};
use crate::Backend;
use crate::executor::{PromptSource, RalphExecutor, ScenarioConfig};
use crate::models::{Assertion, TestResult};
use async_trait::async_trait;
use ralph_core::testing::fake_autoloop::build_fake_autoloop;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const AUTOLOOPS_TOML: &str = r#"
[event_loop]
max_iterations = 1
completion_event = "task.complete"
"#;

const TOPOLOGY_TOML: &str = r#"
name = "engine-completion-e2e"
completion = "task.complete"

[[role]]
id = "worker"
emits = ["task.complete"]
prompt_file = "roles/worker.md"

[handoff]
"loop.start" = ["worker"]
"#;

/// Drives `ralph run` through the production autoloop engine path with a
/// fixture-driven fake `autoloop` executable.
pub struct EngineCompletionScenario;

impl EngineCompletionScenario {
    pub fn new() -> Self {
        Self
    }

    fn summary_file_exists(&self, workspace: &Path) -> Assertion {
        let path = workspace.join(".ralph/agent/summary.md");
        let content = std::fs::read_to_string(&path);
        let passed = content
            .as_ref()
            .is_ok_and(|text| text.contains("**Status:** Completed successfully"));
        AssertionBuilder::new("Completion summary file")
            .expected(".ralph/agent/summary.md exists and records successful completion")
            .actual(match content {
                Ok(text) => format!("{}: {} bytes", path.display(), text.len()),
                Err(error) => format!("{}: {error}", path.display()),
            })
            .build()
            .with_passed(passed)
    }

    fn completion_history_exists(&self, workspace: &Path) -> Assertion {
        let path = workspace.join(".ralph/history.jsonl");
        let content = std::fs::read_to_string(&path);
        let found = content.as_ref().is_ok_and(|text| {
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .any(|line| {
                    serde_json::from_str::<Value>(line).is_ok_and(|record| {
                        record.pointer("/type/kind").and_then(Value::as_str)
                            == Some("loop_completed")
                            && record.pointer("/type/reason").and_then(Value::as_str)
                                == Some("completion_promise")
                    })
                })
        });
        AssertionBuilder::new("Loop history completion record")
            .expected("LoopCompleted reason=completion_promise in .ralph/history.jsonl")
            .actual(match content {
                Ok(_) if found => format!("matching record found in {}", path.display()),
                Ok(text) => format!("no matching record; history={text}"),
                Err(error) => format!("{}: {error}", path.display()),
            })
            .build()
            .with_passed(found)
    }
}

impl Default for EngineCompletionScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestScenario for EngineCompletionScenario {
    fn id(&self) -> &'static str {
        "engine-completion"
    }

    fn description(&self) -> &'static str {
        "Runs the real autoloop engine path to completion with fixture replay"
    }

    fn tier(&self) -> &'static str {
        "Tier 0: Engine (v3)"
    }

    fn supported_backends(&self) -> Vec<Backend> {
        vec![Backend::Claude]
    }

    fn setup(&self, workspace: &Path, _backend: Backend) -> Result<ScenarioConfig, ScenarioError> {
        let preset = workspace.join("preset");
        std::fs::create_dir_all(preset.join("roles"))?;
        std::fs::write(preset.join("autoloops.toml"), AUTOLOOPS_TOML)?;
        std::fs::write(preset.join("topology.toml"), TOPOLOGY_TOML)?;
        std::fs::write(
            preset.join("roles/worker.md"),
            "Complete the engine smoke test.",
        )?;
        std::fs::write(
            workspace.join("ralph.yml"),
            "core:\n  engine: autoloop\n  autoloop_preset: preset\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n",
        )?;
        std::fs::write(workspace.join("README.md"), "engine completion e2e\n")?;
        std::fs::create_dir_all(workspace.join("home"))?;

        run_git(workspace, &["init", "--quiet"])?;
        run_git(workspace, &["add", "README.md", "ralph.yml", "preset"])?;
        run_git(
            workspace,
            &[
                "-c",
                "user.name=Ralph E2E",
                "-c",
                "user.email=ralph-e2e@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "initial",
            ],
        )?;

        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/autoloop/engine_completion.jsonl");
        build_fake_autoloop(&workspace.join("fake-autoloop"), &fixture).map_err(|error| {
            ScenarioError::SetupError(format!("failed to build fake autoloop: {error}"))
        })?;

        Ok(ScenarioConfig {
            config_file: "ralph.yml".into(),
            prompt: PromptSource::Inline("Complete via the configured completion event.".into()),
            max_iterations: 1,
            timeout: Duration::from_secs(30),
            extra_args: vec!["--no-tui".into(), "--skip-preflight".into()],
        })
    }

    async fn run(
        &self,
        executor: &RalphExecutor,
        config: &ScenarioConfig,
    ) -> Result<TestResult, ScenarioError> {
        let start = Instant::now();
        let workspace = executor.workspace();
        let fake_bin = workspace.join("fake-autoloop/bin");
        let environment = vec![
            (
                "HOME".to_string(),
                workspace.join("home").to_string_lossy().into_owned(),
            ),
            (
                "USERPROFILE".to_string(),
                workspace.join("home").to_string_lossy().into_owned(),
            ),
            (
                "ARGV_OUT".to_string(),
                workspace
                    .join("fake-autoloop/argv.txt")
                    .to_string_lossy()
                    .into_owned(),
            ),
        ];
        let execution = executor
            .run_with_environment(config, &environment, &[fake_bin])
            .await
            .map_err(|error| {
                ScenarioError::ExecutionError(format!("ralph execution failed: {error}"))
            })?;

        let assertions = vec![
            Assertions::exit_code(&execution, 0),
            Assertions::output_contains(&execution, "autoloop engine: run_id="),
            self.summary_file_exists(workspace),
            self.completion_history_exists(workspace),
        ];
        let passed = assertions.iter().all(|assertion| assertion.passed);

        Ok(TestResult {
            scenario_id: self.id().to_string(),
            scenario_description: self.description().to_string(),
            backend: String::new(),
            tier: self.tier().to_string(),
            passed,
            assertions,
            duration: start.elapsed(),
        })
    }
}

fn run_git(workspace: &Path, args: &[&str]) -> Result<(), ScenarioError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ScenarioError::SetupError(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

trait AssertionExt {
    fn with_passed(self, passed: bool) -> Self;
}

impl AssertionExt for Assertion {
    fn with_passed(mut self, passed: bool) -> Self {
        self.passed = passed;
        self
    }
}
