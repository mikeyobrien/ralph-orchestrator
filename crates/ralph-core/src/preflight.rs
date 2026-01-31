//! Preflight checks for validating environment and configuration before running.

use crate::{RalphConfig, git_ops};
use crate::config::ConfigWarning;
use async_trait::async_trait;
use serde::Serialize;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Status of a preflight check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

/// Result of a single preflight check.
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub label: String,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl CheckResult {
    pub fn pass(name: &str, label: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            label: label.into(),
            status: CheckStatus::Pass,
            message: None,
        }
    }

    pub fn warn(name: &str, label: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            label: label.into(),
            status: CheckStatus::Warn,
            message: Some(message.into()),
        }
    }

    pub fn fail(name: &str, label: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            label: label.into(),
            status: CheckStatus::Fail,
            message: Some(message.into()),
        }
    }
}

/// A single preflight check.
#[async_trait]
pub trait PreflightCheck: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self, config: &RalphConfig) -> CheckResult;
}

/// Aggregated preflight report.
#[derive(Debug, Clone, Serialize)]
pub struct PreflightReport {
    pub passed: bool,
    pub warnings: usize,
    pub failures: usize,
    pub checks: Vec<CheckResult>,
}

impl PreflightReport {
    fn from_results(checks: Vec<CheckResult>) -> Self {
        let warnings = checks
            .iter()
            .filter(|check| check.status == CheckStatus::Warn)
            .count();
        let failures = checks
            .iter()
            .filter(|check| check.status == CheckStatus::Fail)
            .count();
        let passed = failures == 0;

        Self {
            passed,
            warnings,
            failures,
            checks,
        }
    }
}

/// Runs a set of preflight checks.
pub struct PreflightRunner {
    checks: Vec<Box<dyn PreflightCheck>>,
}

impl PreflightRunner {
    pub fn default_checks() -> Self {
        Self {
            checks: vec![
                Box::new(ConfigValidCheck),
                Box::new(BackendAvailableCheck),
                Box::new(TelegramTokenCheck),
                Box::new(GitCleanCheck),
                Box::new(PathsExistCheck),
                Box::new(ToolsInPathCheck::default()),
            ],
        }
    }

    pub fn check_names(&self) -> Vec<&str> {
        self.checks.iter().map(|check| check.name()).collect()
    }

    pub async fn run_all(&self, config: &RalphConfig) -> PreflightReport {
        Self::run_checks(self.checks.iter(), config).await
    }

    pub async fn run_selected(
        &self,
        config: &RalphConfig,
        names: &[String],
    ) -> PreflightReport {
        let requested: Vec<String> = names.iter().map(|name| name.to_lowercase()).collect();
        let checks = self
            .checks
            .iter()
            .filter(|check| requested.contains(&check.name().to_lowercase()));

        Self::run_checks(checks, config).await
    }

    async fn run_checks<'a, I>(checks: I, config: &RalphConfig) -> PreflightReport
    where
        I: IntoIterator<Item = &'a Box<dyn PreflightCheck>>,
    {
        let mut results = Vec::new();
        for check in checks {
            results.push(check.run(config).await);
        }

        PreflightReport::from_results(results)
    }
}

struct ConfigValidCheck;

#[async_trait]
impl PreflightCheck for ConfigValidCheck {
    fn name(&self) -> &'static str {
        "config"
    }

    async fn run(&self, config: &RalphConfig) -> CheckResult {
        match config.validate() {
            Ok(warnings) if warnings.is_empty() => {
                CheckResult::pass(self.name(), "Configuration valid")
            }
            Ok(warnings) => {
                let warning_count = warnings.len();
                let details = format_config_warnings(&warnings);
                CheckResult::warn(
                    self.name(),
                    format!("Configuration valid ({warning_count} warning(s))"),
                    details,
                )
            }
            Err(err) => CheckResult::fail(
                self.name(),
                "Configuration invalid",
                format!("{err}"),
            ),
        }
    }
}

struct BackendAvailableCheck;

#[async_trait]
impl PreflightCheck for BackendAvailableCheck {
    fn name(&self) -> &'static str {
        "backend"
    }

    async fn run(&self, config: &RalphConfig) -> CheckResult {
        let backend = config.cli.backend.trim();
        if backend.eq_ignore_ascii_case("auto") {
            return check_auto_backend(self.name(), config);
        }

        check_named_backend(self.name(), config, backend)
    }
}

struct TelegramTokenCheck;

#[async_trait]
impl PreflightCheck for TelegramTokenCheck {
    fn name(&self) -> &'static str {
        "telegram"
    }

    async fn run(&self, config: &RalphConfig) -> CheckResult {
        if !config.robot.enabled {
            return CheckResult::pass(self.name(), "RObot disabled (skipping)");
        }

        let Some(token) = config.robot.resolve_bot_token() else {
            return CheckResult::fail(
                self.name(),
                "Telegram token missing",
                "Set RALPH_TELEGRAM_BOT_TOKEN or configure RObot.telegram.bot_token",
            );
        };

        match telegram_get_me(&token).await {
            Ok(info) => CheckResult::pass(
                self.name(),
                format!("Bot token valid (@{})", info.username),
            ),
            Err(err) => CheckResult::fail(
                self.name(),
                "Telegram token invalid",
                format!("{err}"),
            ),
        }
    }
}

struct GitCleanCheck;

#[async_trait]
impl PreflightCheck for GitCleanCheck {
    fn name(&self) -> &'static str {
        "git"
    }

    async fn run(&self, config: &RalphConfig) -> CheckResult {
        let root = &config.core.workspace_root;

        let branch = match git_ops::get_current_branch(root) {
            Ok(branch) => branch,
            Err(err) => {
                return CheckResult::fail(
                    self.name(),
                    "Git repository unavailable",
                    format!("{err}"),
                )
            }
        };

        match git_ops::is_working_tree_clean(root) {
            Ok(true) => CheckResult::pass(
                self.name(),
                format!("Working tree clean ({branch})"),
            ),
            Ok(false) => CheckResult::warn(
                self.name(),
                "Working tree has uncommitted changes",
                "Commit or stash changes before running for clean diffs",
            ),
            Err(err) => CheckResult::fail(
                self.name(),
                "Unable to read git status",
                format!("{err}"),
            ),
        }
    }
}

struct PathsExistCheck;

#[async_trait]
impl PreflightCheck for PathsExistCheck {
    fn name(&self) -> &'static str {
        "paths"
    }

    async fn run(&self, config: &RalphConfig) -> CheckResult {
        let mut created = Vec::new();

        let scratchpad_path = config.core.resolve_path(&config.core.scratchpad);
        if let Some(parent) = scratchpad_path.parent()
            && let Err(err) = ensure_directory(parent, &mut created)
        {
            return CheckResult::fail(
                self.name(),
                "Scratchpad path unavailable",
                format!("{}", err),
            );
        }

        let specs_path = config.core.resolve_path(&config.core.specs_dir);
        if let Err(err) = ensure_directory(&specs_path, &mut created) {
            return CheckResult::fail(
                self.name(),
                "Specs directory unavailable",
                format!("{}", err),
            );
        }

        if created.is_empty() {
            CheckResult::pass(self.name(), "Workspace paths accessible")
        } else {
            CheckResult::warn(
                self.name(),
                "Workspace paths created",
                format!("Created: {}", created.join(", ")),
            )
        }
    }
}

#[derive(Debug, Clone)]
struct ToolsInPathCheck {
    required: Vec<String>,
}

impl ToolsInPathCheck {
    fn new(required: Vec<String>) -> Self {
        Self { required }
    }
}

impl Default for ToolsInPathCheck {
    fn default() -> Self {
        Self::new(vec!["git".to_string()])
    }
}

#[async_trait]
impl PreflightCheck for ToolsInPathCheck {
    fn name(&self) -> &'static str {
        "tools"
    }

    async fn run(&self, _config: &RalphConfig) -> CheckResult {
        let missing: Vec<String> = self
            .required
            .iter()
            .filter(|tool| find_executable(tool).is_none())
            .cloned()
            .collect();

        if missing.is_empty() {
            CheckResult::pass(
                self.name(),
                format!("Required tools available ({})", self.required.join(", ")),
            )
        } else {
            CheckResult::fail(
                self.name(),
                "Missing required tools",
                format!("Missing: {}", missing.join(", ")),
            )
        }
    }
}

#[derive(Debug)]
struct TelegramBotInfo {
    username: String,
}

async fn telegram_get_me(token: &str) -> anyhow::Result<TelegramBotInfo> {
    let url = format!("https://api.telegram.org/bot{}/getMe", token);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|err| anyhow::anyhow!("Network error calling Telegram API: {err}"))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to parse Telegram API response: {err}"))?;

    if !status.is_success() || body.get("ok") != Some(&serde_json::Value::Bool(true)) {
        let description = body
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("Telegram API error: {description}");
    }

    let result = body
        .get("result")
        .ok_or_else(|| anyhow::anyhow!("Missing 'result' in Telegram response"))?;
    let username = result
        .get("username")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown_bot")
        .to_string();

    Ok(TelegramBotInfo { username })
}

fn check_auto_backend(name: &str, config: &RalphConfig) -> CheckResult {
    let priority = config.get_agent_priority();
    if priority.is_empty() {
        return CheckResult::fail(
            name,
            "Auto backend selection unavailable",
            "No backend priority list configured",
        );
    }

    let mut checked = Vec::new();

    for backend in priority {
        if !config.adapter_settings(backend).enabled {
            continue;
        }

        let Some(command) = backend_command(backend, None) else {
            continue;
        };
        checked.push(format!("{backend} ({command})"));
        if command_supports_version(backend) {
            if command_available(&command) {
                return CheckResult::pass(
                    name,
                    format!("Auto backend available ({backend})"),
                );
            }
        } else if find_executable(&command).is_some() {
            return CheckResult::pass(
                name,
                format!("Auto backend available ({backend})"),
            );
        }
    }

    if checked.is_empty() {
        return CheckResult::fail(
            name,
            "Auto backend selection unavailable",
            "All configured adapters are disabled",
        );
    }

    CheckResult::fail(
        name,
        "No available backend found",
        format!("Checked: {}", checked.join(", ")),
    )
}

fn check_named_backend(name: &str, config: &RalphConfig, backend: &str) -> CheckResult {
    let command_override = config.cli.command.as_deref();
    let Some(command) = backend_command(backend, command_override) else {
        return CheckResult::fail(
            name,
            "Backend command missing",
            "Set cli.command for custom backend",
        );
    };

    if backend.eq_ignore_ascii_case("custom") {
        if find_executable(&command).is_some() {
            return CheckResult::pass(
                name,
                format!("Custom backend available ({})", command),
            );
        }

        return CheckResult::fail(
            name,
            "Custom backend not found",
            format!("Command not found: {}", command),
        );
    }

    if command_available(&command) {
        CheckResult::pass(name, format!("Backend CLI available ({})", command))
    } else {
        CheckResult::fail(
            name,
            "Backend CLI not available",
            format!("Command not found or not executable: {}", command),
        )
    }
}

fn backend_command(backend: &str, override_cmd: Option<&str>) -> Option<String> {
    if let Some(command) = override_cmd {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return None;
        }
        return trimmed
            .split_whitespace()
            .next()
            .map(|value| value.to_string());
    }

    match backend {
        "kiro" => Some("kiro-cli".to_string()),
        _ => Some(backend.to_string()),
    }
}

fn command_supports_version(backend: &str) -> bool {
    !backend.eq_ignore_ascii_case("custom")
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn ensure_directory(path: &Path, created: &mut Vec<String>) -> anyhow::Result<()> {
    if path.exists() {
        if path.is_dir() {
            return Ok(());
        }
        anyhow::bail!("Path exists but is not a directory: {}", path.display());
    }

    std::fs::create_dir_all(path)?;
    created.push(path.display().to_string());
    Ok(())
}

fn find_executable(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return if path.is_file() {
            Some(path.to_path_buf())
        } else {
            None
        };
    }

    let path_var = env::var_os("PATH")?;
    let extensions = executable_extensions();

    for dir in env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = if ext.is_empty() {
                dir.join(command)
            } else {
                dir.join(format!("{}{}", command, ext.to_string_lossy()))
            };

            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn executable_extensions() -> Vec<OsString> {
    if cfg!(windows) {
        let exts = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        exts.split(';')
            .filter(|ext| !ext.trim().is_empty())
            .map(|ext| OsString::from(ext.trim().to_string()))
            .collect()
    } else {
        vec![OsString::new()]
    }
}

fn format_config_warnings(warnings: &[ConfigWarning]) -> String {
    warnings
        .iter()
        .map(|warning| warning.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn report_counts_statuses() {
        let checks = vec![
            CheckResult::pass("a", "ok"),
            CheckResult::warn("b", "warn", "needs attention"),
            CheckResult::fail("c", "fail", "broken"),
        ];

        let report = PreflightReport::from_results(checks);

        assert_eq!(report.warnings, 1);
        assert_eq!(report.failures, 1);
        assert!(!report.passed);
    }

    #[tokio::test]
    async fn config_check_emits_warning_details() {
        let mut config = RalphConfig::default();
        config.archive_prompts = true;

        let check = ConfigValidCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Warn);
        let message = result.message.expect("expected warning message");
        assert!(message.contains("archive_prompts"));
    }

    #[tokio::test]
    async fn tools_check_reports_missing_tools() {
        let check = ToolsInPathCheck::new(vec!["definitely-not-a-tool".to_string()]);
        let config = RalphConfig::default();

        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.unwrap_or_default().contains("Missing"));
    }

    #[tokio::test]
    async fn paths_check_creates_missing_dirs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();

        let mut config = RalphConfig::default();
        config.core.workspace_root = root.clone();
        config.core.scratchpad = "nested/scratchpad.md".to_string();
        config.core.specs_dir = "nested/specs".to_string();

        let check = PathsExistCheck;
        let result = check.run(&config).await;

        assert!(root.join("nested").exists());
        assert!(root.join("nested/specs").exists());
        assert_eq!(result.status, CheckStatus::Warn);
    }

    #[tokio::test]
    async fn telegram_check_skips_when_disabled() {
        let config = RalphConfig::default();
        let check = TelegramTokenCheck;

        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.label.contains("skipping"));
    }
}
