//! Preflight checks for validating environment and configuration before running.

use crate::config::ConfigWarning;
use crate::{RalphConfig, git_ops};
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
                Box::new(SpecCompletenessCheck),
            ],
        }
    }

    pub fn check_names(&self) -> Vec<&str> {
        self.checks.iter().map(|check| check.name()).collect()
    }

    pub async fn run_all(&self, config: &RalphConfig) -> PreflightReport {
        Self::run_checks(self.checks.iter(), config).await
    }

    pub async fn run_selected(&self, config: &RalphConfig, names: &[String]) -> PreflightReport {
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
            Err(err) => CheckResult::fail(self.name(), "Configuration invalid", format!("{err}")),
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
            Ok(info) => {
                CheckResult::pass(self.name(), format!("Bot token valid (@{})", info.username))
            }
            Err(err) => CheckResult::fail(self.name(), "Telegram token invalid", format!("{err}")),
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
        if !is_git_workspace(root) {
            return CheckResult::pass(self.name(), "Not a git repository (skipping)");
        }

        let branch = match git_ops::get_current_branch(root) {
            Ok(branch) => branch,
            Err(err) => {
                return CheckResult::fail(
                    self.name(),
                    "Git repository unavailable",
                    format!("{err}"),
                );
            }
        };

        match git_ops::is_working_tree_clean(root) {
            Ok(true) => CheckResult::pass(self.name(), format!("Working tree clean ({branch})")),
            Ok(false) => CheckResult::warn(
                self.name(),
                "Working tree has uncommitted changes",
                "Commit or stash changes before running for clean diffs",
            ),
            Err(err) => {
                CheckResult::fail(self.name(), "Unable to read git status", format!("{err}"))
            }
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
    optional: Vec<String>,
}

impl ToolsInPathCheck {
    #[cfg(test)]
    fn new(required: Vec<String>) -> Self {
        Self {
            required,
            optional: Vec::new(),
        }
    }

    fn new_with_optional(required: Vec<String>, optional: Vec<String>) -> Self {
        Self { required, optional }
    }
}

impl Default for ToolsInPathCheck {
    fn default() -> Self {
        Self::new_with_optional(vec!["git".to_string()], Vec::new())
    }
}

#[async_trait]
impl PreflightCheck for ToolsInPathCheck {
    fn name(&self) -> &'static str {
        "tools"
    }

    async fn run(&self, config: &RalphConfig) -> CheckResult {
        if !is_git_workspace(&config.core.workspace_root) {
            return CheckResult::pass(self.name(), "Not a git repository (skipping)");
        }

        let missing_required: Vec<String> = self
            .required
            .iter()
            .filter(|tool| find_executable(tool).is_none())
            .cloned()
            .collect();

        let missing_optional: Vec<String> = self
            .optional
            .iter()
            .filter(|tool| find_executable(tool).is_none())
            .cloned()
            .collect();

        if missing_required.is_empty() && missing_optional.is_empty() {
            let mut tools = self.required.clone();
            tools.extend(self.optional.clone());
            CheckResult::pass(
                self.name(),
                format!("Required tools available ({})", tools.join(", ")),
            )
        } else if missing_required.is_empty() {
            CheckResult::warn(
                self.name(),
                "Missing optional tools",
                format!("Missing: {}", missing_optional.join(", ")),
            )
        } else {
            let mut detail = format!("required: {}", missing_required.join(", "));
            if !missing_optional.is_empty() {
                detail.push_str(&format!("; optional: {}", missing_optional.join(", ")));
            }
            CheckResult::fail(
                self.name(),
                "Missing required tools",
                format!("Missing {}", detail),
            )
        }
    }
}

struct SpecCompletenessCheck;

#[async_trait]
impl PreflightCheck for SpecCompletenessCheck {
    fn name(&self) -> &'static str {
        "specs"
    }

    async fn run(&self, config: &RalphConfig) -> CheckResult {
        let specs_dir = config.core.resolve_path(&config.core.specs_dir);

        if !specs_dir.exists() {
            return CheckResult::pass(self.name(), "No specs directory (skipping)");
        }

        let spec_files = match collect_spec_files(&specs_dir) {
            Ok(files) => files,
            Err(err) => {
                return CheckResult::fail(
                    self.name(),
                    "Unable to read specs directory",
                    format!("{err}"),
                );
            }
        };

        if spec_files.is_empty() {
            return CheckResult::pass(self.name(), "No spec files found (skipping)");
        }

        let mut incomplete: Vec<String> = Vec::new();

        for path in &spec_files {
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(err) => {
                    incomplete.push(format!(
                        "{}: unreadable ({})",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        err
                    ));
                    continue;
                }
            };

            if let Some(reason) = check_spec_completeness(path, &content) {
                incomplete.push(reason);
            }
        }

        if incomplete.is_empty() {
            CheckResult::pass(
                self.name(),
                format!(
                    "{} spec(s) valid with acceptance criteria",
                    spec_files.len()
                ),
            )
        } else {
            let total = spec_files.len();
            CheckResult::warn(
                self.name(),
                format!(
                    "{} of {} spec(s) missing acceptance criteria",
                    incomplete.len(),
                    total
                ),
                format!(
                    "Specs should include Given/When/Then acceptance criteria.\n{}",
                    incomplete.join("\n")
                ),
            )
        }
    }
}

/// Recursively collect all `.spec.md` files under a directory.
fn collect_spec_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_spec_files_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_spec_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_spec_files_recursive(&path, files)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".spec.md"))
        {
            files.push(path);
        }
    }
    Ok(())
}

/// Check whether a spec file has the required sections for Level 5 completeness.
///
/// Returns `None` if the spec is complete, or `Some(reason)` if incomplete.
fn check_spec_completeness(path: &Path, content: &str) -> Option<String> {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Skip specs that are already marked as implemented — they passed review
    let content_lower = content.to_lowercase();
    if content_lower.contains("status: implemented") {
        return None;
    }

    let has_acceptance = has_acceptance_criteria(content);

    if !has_acceptance {
        return Some(format!(
            "{filename}: missing acceptance criteria (Given/When/Then)"
        ));
    }

    None
}

/// Detect whether content contains Given/When/Then acceptance criteria.
///
/// Matches common spec patterns:
/// - `**Given**` / `**When**` / `**Then**` (bold markdown)
/// - `Given ` / `When ` / `Then ` at line start (plain text)
/// - `- Given ` / `- When ` / `- Then ` (list items)
fn has_acceptance_criteria(content: &str) -> bool {
    let mut has_given = false;
    let mut has_when = false;
    let mut has_then = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        // Bold markdown: **Given**, **When**, **Then**
        // Plain text at line start: Given, When, Then
        // List items: - Given, - When, - Then
        if lower.starts_with("**given**")
            || lower.starts_with("given ")
            || lower.starts_with("- given ")
            || lower.starts_with("- **given**")
        {
            has_given = true;
        }
        if lower.starts_with("**when**")
            || lower.starts_with("when ")
            || lower.starts_with("- when ")
            || lower.starts_with("- **when**")
        {
            has_when = true;
        }
        if lower.starts_with("**then**")
            || lower.starts_with("then ")
            || lower.starts_with("- then ")
            || lower.starts_with("- **then**")
        {
            has_then = true;
        }

        if has_given && has_when && has_then {
            return true;
        }
    }

    // Require at least Given+Then (When is sometimes implicit)
    has_given && has_then
}

/// A single acceptance criterion extracted from a spec file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcceptanceCriterion {
    /// The precondition (Given clause).
    pub given: String,
    /// The action or trigger (When clause). Optional because some specs omit it.
    pub when: Option<String>,
    /// The expected outcome (Then clause).
    pub then: String,
}

/// Extract structured Given/When/Then acceptance criteria from spec content.
///
/// Parses the same patterns recognized by [`has_acceptance_criteria`] but returns
/// structured triples instead of a boolean. Each contiguous Given[/When]/Then
/// group produces one [`AcceptanceCriterion`].
pub fn extract_acceptance_criteria(content: &str) -> Vec<AcceptanceCriterion> {
    let mut criteria = Vec::new();
    let mut current_given: Option<String> = None;
    let mut current_when: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        if let Some(text) = match_clause(&lower, trimmed, "given") {
            // Flush any previous incomplete criterion before starting a new Given
            if let Some(given) = current_given.take() {
                // Previous Given without Then — skip incomplete criterion
                let _ = given;
            }
            current_given = Some(text);
            current_when = None;
        } else if let Some(text) = match_clause(&lower, trimmed, "when") {
            current_when = Some(text);
        } else if let Some(text) = match_clause(&lower, trimmed, "then") {
            if let Some(given) = current_given.take() {
                criteria.push(AcceptanceCriterion {
                    given,
                    when: current_when.take(),
                    then: text,
                });
            }
            // Reset for next criterion
            current_when = None;
        }
    }

    criteria
}

/// Match a Given/When/Then clause line and extract the text after the keyword.
///
/// Handles bold (`**Given**`), plain (`Given `), list (`- Given `), and bold-list
/// (`- **Given**`) formats. Returns the text portion after the keyword, or `None`
/// if the line doesn't match.
fn match_clause(lower: &str, original: &str, keyword: &str) -> Option<String> {
    let bold = format!("**{keyword}**");
    let plain = format!("{keyword} ");
    let list_plain = format!("- {keyword} ");
    let list_bold = format!("- **{keyword}**");

    // Determine the offset where the actual text starts
    let text_start = if lower.starts_with(&bold) {
        Some(bold.len())
    } else if lower.starts_with(&list_bold) {
        Some(list_bold.len())
    } else if lower.starts_with(&list_plain) {
        Some(list_plain.len())
    } else if lower.starts_with(&plain) {
        Some(plain.len())
    } else {
        None
    };

    text_start.map(|offset| original[offset..].trim().to_string())
}

/// Extract acceptance criteria from a spec file at the given path.
///
/// Reads the file, skips `status: implemented` specs, and returns structured
/// criteria. Returns an empty vec if the file is unreadable or already implemented.
pub fn extract_criteria_from_file(path: &Path) -> Vec<AcceptanceCriterion> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Skip implemented specs
    if content.to_lowercase().contains("status: implemented") {
        return Vec::new();
    }

    extract_acceptance_criteria(&content)
}

/// Extract acceptance criteria from all spec files in a directory.
///
/// Returns a vec of `(filename, criteria)` pairs. Only includes specs that
/// have at least one criterion and are not marked as implemented.
pub fn extract_all_criteria(
    specs_dir: &Path,
) -> std::io::Result<Vec<(String, Vec<AcceptanceCriterion>)>> {
    let files = collect_spec_files(specs_dir)?;
    let mut results = Vec::new();

    for path in files {
        let criteria = extract_criteria_from_file(&path);
        if !criteria.is_empty() {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            results.push((filename, criteria));
        }
    }

    Ok(results)
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
                return CheckResult::pass(name, format!("Auto backend available ({backend})"));
            }
        } else if find_executable(&command).is_some() {
            return CheckResult::pass(name, format!("Auto backend available ({backend})"));
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
            return CheckResult::pass(name, format!("Custom backend available ({})", command));
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

fn is_git_workspace(path: &Path) -> bool {
    let git_dir = path.join(".git");
    git_dir.is_dir() || git_dir.is_file()
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
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join(".git")).expect("create .git");
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        let check = ToolsInPathCheck::new(vec!["definitely-not-a-tool".to_string()]);

        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.unwrap_or_default().contains("Missing"));
    }

    #[tokio::test]
    async fn tools_check_warns_on_missing_optional_tools() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join(".git")).expect("create .git");
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        let check = ToolsInPathCheck::new_with_optional(
            Vec::new(),
            vec!["definitely-not-a-tool".to_string()],
        );

        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Warn);
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

    #[tokio::test]
    async fn git_check_skips_outside_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();

        let check = GitCleanCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.label.contains("skipping"));
    }

    #[tokio::test]
    async fn tools_check_skips_outside_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();

        let check = ToolsInPathCheck::new(vec!["definitely-not-a-tool".to_string()]);
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.label.contains("skipping"));
    }

    #[tokio::test]
    async fn specs_check_skips_when_no_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        config.core.specs_dir = "nonexistent/specs".to_string();

        let check = SpecCompletenessCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.label.contains("skipping"));
    }

    #[tokio::test]
    async fn specs_check_skips_when_empty_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("specs")).expect("create specs dir");
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        config.core.specs_dir = "specs".to_string();

        let check = SpecCompletenessCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.label.contains("skipping"));
    }

    #[tokio::test]
    async fn specs_check_passes_with_complete_spec() {
        let temp = tempfile::tempdir().expect("tempdir");
        let specs_dir = temp.path().join("specs");
        std::fs::create_dir_all(&specs_dir).expect("create specs dir");
        std::fs::write(
            specs_dir.join("feature.spec.md"),
            r"---
status: draft
---

# Feature Spec

## Goal

Add a new feature.

## Acceptance Criteria

**Given** the system is running
**When** the user triggers the feature
**Then** the expected output is produced
",
        )
        .expect("write spec");

        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        config.core.specs_dir = "specs".to_string();

        let check = SpecCompletenessCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.label.contains("1 spec(s) valid"));
    }

    #[tokio::test]
    async fn specs_check_warns_on_missing_acceptance_criteria() {
        let temp = tempfile::tempdir().expect("tempdir");
        let specs_dir = temp.path().join("specs");
        std::fs::create_dir_all(&specs_dir).expect("create specs dir");
        std::fs::write(
            specs_dir.join("incomplete.spec.md"),
            r"---
status: draft
---

# Incomplete Spec

## Goal

Do something.

## Requirements

1. Some requirement
",
        )
        .expect("write spec");

        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        config.core.specs_dir = "specs".to_string();

        let check = SpecCompletenessCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Warn);
        assert!(result.label.contains("missing acceptance criteria"));
        let message = result.message.expect("expected message");
        assert!(message.contains("incomplete.spec.md"));
    }

    #[tokio::test]
    async fn specs_check_skips_implemented_specs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let specs_dir = temp.path().join("specs");
        std::fs::create_dir_all(&specs_dir).expect("create specs dir");
        // This spec lacks acceptance criteria but is already implemented
        std::fs::write(
            specs_dir.join("done.spec.md"),
            r"---
status: implemented
---

# Done Spec

## Goal

Already done.
",
        )
        .expect("write spec");

        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        config.core.specs_dir = "specs".to_string();

        let check = SpecCompletenessCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[tokio::test]
    async fn specs_check_finds_specs_in_subdirectories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let specs_dir = temp.path().join("specs");
        let sub_dir = specs_dir.join("adapters");
        std::fs::create_dir_all(&sub_dir).expect("create subdirectory");
        std::fs::write(
            sub_dir.join("adapter.spec.md"),
            r"---
status: draft
---

# Adapter Spec

## Acceptance Criteria

- **Given** an adapter is configured
- **When** a request is sent
- **Then** the adapter responds correctly
",
        )
        .expect("write spec");

        let mut config = RalphConfig::default();
        config.core.workspace_root = temp.path().to_path_buf();
        config.core.specs_dir = "specs".to_string();

        let check = SpecCompletenessCheck;
        let result = check.run(&config).await;

        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.label.contains("1 spec(s) valid"));
    }

    #[test]
    fn has_acceptance_criteria_detects_bold_format() {
        let content = r"
## Acceptance Criteria

**Given** the system is ready
**When** the user clicks
**Then** the result appears
";
        assert!(has_acceptance_criteria(content));
    }

    #[test]
    fn has_acceptance_criteria_detects_list_format() {
        let content = r"
## Acceptance Criteria

- Given the system is ready
- When the user clicks
- Then the result appears
";
        assert!(has_acceptance_criteria(content));
    }

    #[test]
    fn has_acceptance_criteria_detects_bold_list_format() {
        let content = r"
## Acceptance Criteria

- **Given** the system is ready
- **When** the user clicks
- **Then** the result appears
";
        assert!(has_acceptance_criteria(content));
    }

    #[test]
    fn has_acceptance_criteria_requires_given_and_then() {
        // Only has Given, missing When and Then
        let content = "**Given** something\n";
        assert!(!has_acceptance_criteria(content));

        // Has Given and Then (When implicit) — should pass
        let content = "**Given** something\n**Then** result\n";
        assert!(has_acceptance_criteria(content));
    }

    #[test]
    fn has_acceptance_criteria_rejects_content_without_criteria() {
        let content = r"
# Some Spec

## Goal

Build something.

## Requirements

1. It should work.
";
        assert!(!has_acceptance_criteria(content));
    }

    // --- extract_acceptance_criteria tests ---

    #[test]
    fn extract_criteria_bold_format() {
        let content = r#"
## Acceptance Criteria

**Given** `backend: "amp"` in config
**When** Ralph executes an iteration
**Then** both flags are included
"#;
        let criteria = extract_acceptance_criteria(content);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].given, "`backend: \"amp\"` in config");
        assert_eq!(
            criteria[0].when.as_deref(),
            Some("Ralph executes an iteration")
        );
        assert_eq!(criteria[0].then, "both flags are included");
    }

    #[test]
    fn extract_criteria_multiple_triples() {
        let content = r"
**Given** system A is running
**When** user clicks button
**Then** dialog appears

**Given** dialog is open
**When** user confirms
**Then** action completes
";
        let criteria = extract_acceptance_criteria(content);
        assert_eq!(criteria.len(), 2);
        assert_eq!(criteria[0].given, "system A is running");
        assert_eq!(criteria[1].given, "dialog is open");
        assert_eq!(criteria[1].then, "action completes");
    }

    #[test]
    fn extract_criteria_list_format() {
        let content = r"
## Acceptance Criteria

- **Given** an adapter is configured
- **When** a request is sent
- **Then** the adapter responds correctly
";
        let criteria = extract_acceptance_criteria(content);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].given, "an adapter is configured");
        assert_eq!(criteria[0].when.as_deref(), Some("a request is sent"));
        assert_eq!(criteria[0].then, "the adapter responds correctly");
    }

    #[test]
    fn extract_criteria_plain_text_format() {
        let content = r"
Given the server is started
When a GET request is sent
Then a 200 response is returned
";
        let criteria = extract_acceptance_criteria(content);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].given, "the server is started");
        assert_eq!(criteria[0].when.as_deref(), Some("a GET request is sent"));
        assert_eq!(criteria[0].then, "a 200 response is returned");
    }

    #[test]
    fn extract_criteria_given_then_without_when() {
        let content = r"
**Given** the config is empty
**Then** defaults are used
";
        let criteria = extract_acceptance_criteria(content);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].given, "the config is empty");
        assert!(criteria[0].when.is_none());
        assert_eq!(criteria[0].then, "defaults are used");
    }

    #[test]
    fn extract_criteria_empty_content() {
        let criteria = extract_acceptance_criteria("");
        assert!(criteria.is_empty());
    }

    #[test]
    fn extract_criteria_no_criteria() {
        let content = r"
# Spec

## Goal

Build something.
";
        let criteria = extract_acceptance_criteria(content);
        assert!(criteria.is_empty());
    }

    #[test]
    fn extract_criteria_incomplete_given_without_then_is_dropped() {
        let content = r"
**Given** orphan precondition

Some other text here.
";
        let criteria = extract_acceptance_criteria(content);
        assert!(criteria.is_empty());
    }

    #[test]
    fn extract_criteria_from_file_skips_implemented() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("done.spec.md");
        std::fs::write(
            &path,
            r"---
status: implemented
---

**Given** something
**When** something happens
**Then** result
",
        )
        .expect("write");

        let criteria = extract_criteria_from_file(&path);
        assert!(criteria.is_empty());
    }

    #[test]
    fn extract_criteria_from_file_returns_criteria() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("feature.spec.md");
        std::fs::write(
            &path,
            r"---
status: draft
---

# Feature

**Given** the system is ready
**When** user acts
**Then** feature works
",
        )
        .expect("write");

        let criteria = extract_criteria_from_file(&path);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].given, "the system is ready");
    }

    #[test]
    fn extract_all_criteria_collects_from_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let specs_dir = temp.path().join("specs");
        std::fs::create_dir_all(&specs_dir).expect("create dir");

        std::fs::write(
            specs_dir.join("a.spec.md"),
            "**Given** A\n**When** B\n**Then** C\n",
        )
        .expect("write a");

        std::fs::write(specs_dir.join("b.spec.md"), "**Given** X\n**Then** Y\n").expect("write b");

        // Implemented spec should be excluded
        std::fs::write(
            specs_dir.join("c.spec.md"),
            "---\nstatus: implemented\n---\n**Given** skip\n**Then** skip\n",
        )
        .expect("write c");

        let results = extract_all_criteria(&specs_dir).expect("extract");
        assert_eq!(results.len(), 2);

        let filenames: Vec<&str> = results.iter().map(|(f, _)| f.as_str()).collect();
        assert!(filenames.contains(&"a.spec.md"));
        assert!(filenames.contains(&"b.spec.md"));
    }

    #[test]
    fn match_clause_extracts_text() {
        assert_eq!(
            match_clause("**given** the system", "**Given** the system", "given"),
            Some("the system".to_string())
        );
        assert_eq!(
            match_clause("- **when** user clicks", "- **When** user clicks", "when"),
            Some("user clicks".to_string())
        );
        assert_eq!(
            match_clause("then result", "Then result", "then"),
            Some("result".to_string())
        );
        assert_eq!(
            match_clause("- given something", "- Given something", "given"),
            Some("something".to_string())
        );
        assert_eq!(
            match_clause("no match here", "No match here", "given"),
            None
        );
    }
}
